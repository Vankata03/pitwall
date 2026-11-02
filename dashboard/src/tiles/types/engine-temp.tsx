import { DEFAULT_UNITS, temperature } from '../../device/units';
import { useSlow } from '../../runtime/react';
import { effectiveReadoutState } from '../../runtime/readout';
import { ReadoutState } from '../../runtime/wire';
import { defineTileModule, type TileProps } from '../contract';
import { boundCar, mapValue, SlowTile } from '../slow';

/** The gauge's scale in °C. Decoration only: the number in its centre is the readout. */
const SCALE_MIN_C = 40;
const SCALE_MAX_C = 140;
/** Smallest tile, in CSS pixels, that draws the gauge rather than the number alone. */
const GAUGE_MIN = { width: 150, height: 130 } as const;

const START = 135; // degrees, clockwise from 3 o'clock
const SWEEP = 270;
const CX = 100;
const CY = 96;
const R = 74;

const polar = (degrees: number, radius: number) => {
  const a = (degrees * Math.PI) / 180;
  return [CX + Math.cos(a) * radius, CY + Math.sin(a) * radius] as const;
};
const ARC = (() => {
  const [x0, y0] = polar(START, R);
  const [x1, y1] = polar(START + SWEEP, R);
  return `M${x0.toFixed(2)} ${y0.toFixed(2)}A${R} ${R} 0 1 1 ${x1.toFixed(2)} ${y1.toFixed(2)}`;
})();
const TICKS = Array.from({ length: 21 }, (_, i) => {
  const major = i % 5 === 0;
  const [x0, y0] = polar(START + (SWEEP * i) / 20, R + 9);
  const [x1, y1] = polar(START + (SWEEP * i) / 20, R + (major ? 16 : 12));
  return { major, d: `M${x0.toFixed(2)} ${y0.toFixed(2)}L${x1.toFixed(2)} ${y1.toFixed(2)}` };
});

/** A 270° arc filled to the engine temperature on a fixed scale; it never implies a limit. */
function Gauge({ celsius, stale, unit }: { celsius: number | null; stale: boolean; unit: 'c' | 'f' }) {
  const share = celsius === null ? 0 : Math.min(1, Math.max(0, (celsius - SCALE_MIN_C) / (SCALE_MAX_C - SCALE_MIN_C)));
  const label = (c: number) => String(Math.round(temperature(c, { ...DEFAULT_UNITS, temperature: unit }).value));
  // The scale's ends sit just under the arc's open ends.
  const [lx, ly] = polar(START, R);
  const [rx, ry] = polar(START + SWEEP, R);
  return (
    <svg className="pw-et-gauge" viewBox="0 0 200 180" aria-hidden="true" data-testid="engine-gauge">
      <defs>
        <linearGradient id="pw-et-fill" x1="0" x2="1" y1="0" y2="0">
          <stop offset="0" stopColor="var(--pw-temp-cold)" />
          <stop offset="0.45" stopColor="var(--pw-energy)" />
          <stop offset="0.7" stopColor="var(--pw-sector-slower)" />
          <stop offset="1" stopColor="var(--pw-temp-hot)" />
        </linearGradient>
      </defs>
      {TICKS.map((t, i) => (
        <path key={i} d={t.d} className="pw-et-tick" data-major={t.major} />
      ))}
      <path d={ARC} className="pw-et-track" pathLength={100} />
      {celsius === null ? null : (
        <path
          d={ARC}
          className="pw-et-fill"
          data-stale={stale}
          pathLength={100}
          strokeDasharray={`${(share * 100).toFixed(2)} 100`}
        />
      )}
      <text x={lx} y={ly + 26} className="pw-et-scale" textAnchor="middle">
        {label(SCALE_MIN_C)}
      </text>
      <text x={rx} y={ry + 26} className="pw-et-scale" textAnchor="middle">
        {label(SCALE_MAX_C)}
      </text>
    </svg>
  );
}

export default defineTileModule({
  declaration: {
    type: 'engine-temp', name: 'Engine temp', binding: 'follows-car', rate: 'slow',
    minSize: { w: 2, h: 2 }, defaultSize: { w: 3, h: 2 },
    densities: [{ density: 'compact', minWidth: 0, minHeight: 0 }, { density: 'regular', minWidth: 140, minHeight: 64 }],
    reads: [{ source: 'state', keys: ['cars.telemetry'] }],
  },
  Component: function EngineTemp({ car, units, width, height }: TileProps) {
    const snapshot = useSlow(s => s);
    const telemetry = boundCar(snapshot, car)?.telemetry;
    const value = mapValue(telemetry, t => {
      const q = temperature(t.engine_temperature, units);
      return `${Math.round(q.value)} ${q.unit}`;
    });
    const showGauge = width >= GAUGE_MIN.width && height >= GAUGE_MIN.height;
    const state = effectiveReadoutState(telemetry, snapshot.connection === 'open');
    const celsius = telemetry && 'value' in telemetry && (state === ReadoutState.Live || state === ReadoutState.Stale) ? telemetry.value.engine_temperature : null;
    return (
      <div className="pw-et" data-gauge={showGauge}>
        <SlowTile title="Engine temp" snapshot={snapshot} rows={[{ label: 'Engine temperature', value }]}>
          {showGauge ? <Gauge celsius={celsius} stale={state === ReadoutState.Stale} unit={units.temperature} /> : null}
        </SlowTile>
      </div>
    );
  },
});
