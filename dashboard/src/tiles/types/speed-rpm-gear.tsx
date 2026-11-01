import { useRef } from 'react';
import { speed as speedIn } from '../../device/units';
import { effectiveState, frameReadoutState, readoutStateName, wholeTileMessage } from '../../runtime/readout';
import { ReadoutState } from '../../runtime/wire';
import { defineTileModule, type TileProps } from '../contract';
import '../frame-tiles.css';
import {
  fitCanvas,
  grouped,
  setData,
  setText,
  showMessage,
  TileMessage,
  TileTitle,
  tokenColor,
  useCarState,
  useLifecycle,
  valueOf,
  write,
} from '../frame-kit';
import { useFrameTile } from '../use-frame-tile';

/** Segments in the shift strip. */
const SEGMENTS = 20;
/** At and above this share of the maximum the strip is full and steady: the limiter. */
const LIMITER_AT = 0.985;

const declaration = {
  type: 'speed-rpm-gear',
  name: 'Speed/RPM/gear',
  binding: 'follows-car',
  rate: 'frame',
  minSize: { w: 4, h: 3 },
  defaultSize: { w: 6, h: 6 },
  densities: [
    { density: 'compact', minWidth: 0, minHeight: 0 },
    { density: 'regular', minWidth: 230, minHeight: 150 },
    { density: 'full', minWidth: 330, minHeight: 370 },
  ],
  reads: [
    { source: 'frame', fields: ['speed', 'rpm', 'gear'] },
    { source: 'state', keys: ['cars'] },
  ],
} as const;

const gearText = (gear: number): string => (gear < 0 ? 'R' : gear === 0 ? 'N' : String(gear));

/** Lit segments for an RPM against the car's maximum; nothing is lit when the maximum is unknown. */
function litSegments(rpm: number, maxRpm: number | null): number {
  if (maxRpm === null) return 0;
  return Math.round(Math.min(1, Math.max(0, rpm / maxRpm)) * SEGMENTS);
}

function SpeedRpmGear({ car, units, density, width, height }: TileProps) {
  const root = useRef<HTMLDivElement>(null);
  const speedEl = useRef<HTMLSpanElement>(null);
  const unitEl = useRef<HTMLSpanElement>(null);
  const rpmEl = useRef<HTMLSpanElement>(null);
  const gearEl = useRef<HTMLSpanElement>(null);
  const strip = useRef<HTMLDivElement>(null);
  const dial = useRef<HTMLCanvasElement>(null);

  const status = valueOf(useCarState(car)?.status);
  const maxRpm = status && status.max_rpm > 0 ? status.max_rpm : null;
  const lifecycle = useLifecycle();
  const showStrip = density !== 'compact';
  const showDial = density === 'full';
  const dialSize = Math.max(120, Math.min(width - 32, height - 200));

  useFrameTile(
    declaration,
    car,
    ({ frame, car: index, connected }) => {
      const state =
        index === null ? ReadoutState.Absent : effectiveState(frameReadoutState(frame, index, 'speed'), connected);
      const live = index !== null && (state === ReadoutState.Live || state === ReadoutState.Stale);
      const kmh = live ? frame.speed[index]! : 0;
      const rpm = live ? frame.rpm[index]! : 0;
      const shown = speedIn(kmh, units);

      write(speedEl.current, state, String(Math.round(shown.value)));
      setText(unitEl.current, shown.unit);
      write(gearEl.current, state, live ? gearText(frame.gear[index]!) : '');
      write(rpmEl.current, state, grouped(rpm));

      const lit = live ? litSegments(rpm, maxRpm) : 0;
      const limiter = live && maxRpm !== null && rpm >= maxRpm * LIMITER_AT;
      if (strip.current) {
        const el = strip.current;
        setData(el, 'state', maxRpm === null && live ? 'absent' : readoutStateName(state));
        setData(el, 'limiter', String(limiter));
        if (el.dataset.lit !== String(lit)) {
          el.dataset.lit = String(lit);
          for (let i = 0; i < el.children.length; i++) (el.children[i] as HTMLElement).dataset.on = String(i < lit);
        }
      }
      if (dial.current) drawDial(dial.current, dialSize, live ? rpm : null, maxRpm, state === ReadoutState.Stale);

      showMessage(root.current, wholeTileMessage([state], { lifecycle, connected }));
    },
    [units.speed, density, maxRpm, lifecycle, dialSize],
  );

  return (
    <div className="pw-frame-tile pw-srg" ref={root} data-density={density}>
      <div className="pw-frame-body">
        {showStrip ? <TileTitle>Speed / RPM</TileTitle> : null}
        {showStrip ? (
          <div className="pw-shift-strip" ref={strip} data-testid="shift-strip" data-segments={SEGMENTS} data-lit="0" data-limiter="false" aria-hidden="true">
            {Array.from({ length: SEGMENTS }, (_, i) => (
              <span key={i} data-zone={i < 8 ? 'low' : i < 14 ? 'mid' : 'high'} data-on="false" />
            ))}
          </div>
        ) : null}
        <div className="pw-srg-speed">
          <span className="pw-num pw-readout pw-big" ref={speedEl} data-testid="speed" />
          <span className="pw-label" ref={unitEl} data-testid="speed-unit" />
        </div>
        {showDial ? (
          <canvas
            className="pw-dial"
            ref={dial}
            data-testid="rpm-dial"
            role="img"
            aria-label="Speed, RPM and gear: engine speed dial. The readouts beside it give the numbers."
          />
        ) : null}
        <div className="pw-srg-row">
          <div className="pw-stat">
            <span className="pw-label">Gear</span>
            <span className="pw-num pw-readout pw-metric pw-gear" ref={gearEl} data-testid="gear" />
          </div>
          {showStrip ? (
            <div className="pw-stat pw-stat-end">
              <span className="pw-label">Engine speed</span>
              <span className="pw-srg-rpm">
                <span className="pw-num pw-readout pw-metric" ref={rpmEl} data-testid="rpm" /> <span className="pw-label">RPM</span>
              </span>
            </div>
          ) : null}
        </div>
      </div>
      <TileMessage />
    </div>
  );
}

/** The RPM dial: a 270° arc, filled to the share of the maximum. A steady fill, never animated. */
function drawDial(canvas: HTMLCanvasElement, size: number, rpm: number | null, maxRpm: number | null, stale: boolean): void {
  const ctx = fitCanvas(canvas, size, size);
  if (!ctx) return;
  const line = tokenColor(canvas, '--pw-line', '#343d42');
  const ink = tokenColor(canvas, '--pw-text', '#F3F5F6');
  const start = Math.PI * 0.75;
  const sweep = Math.PI * 1.5;
  const c = size / 2;
  const radius = c - 10;
  ctx.clearRect(0, 0, size, size);
  ctx.lineWidth = 8;
  ctx.lineCap = 'butt';
  ctx.strokeStyle = line;
  ctx.beginPath();
  ctx.arc(c, c, radius, start, start + sweep);
  ctx.stroke();
  if (rpm === null || maxRpm === null) return;
  ctx.globalAlpha = stale ? 0.6 : 1;
  ctx.strokeStyle = ink;
  ctx.beginPath();
  ctx.arc(c, c, radius, start, start + sweep * Math.min(1, Math.max(0, rpm / maxRpm)));
  ctx.stroke();
  ctx.globalAlpha = 1;
}

export default defineTileModule({ declaration, Component: SpeedRpmGear });
