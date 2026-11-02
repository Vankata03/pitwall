import { useRef, type CSSProperties } from 'react';
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
  const full = density === 'full';
  // Regular gives the dial the tile's full height, with title, strip, gear and RPM beside it;
  // full stacks them around it.
  const dialSize = Math.round(
    full ? Math.max(120, Math.min(width - 32, height - 200)) : Math.min(height - 24, (width - 2 * 14) * 0.55),
  );
  const showDial = showStrip && dialSize >= 110;

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

  const speed = (
    <div className="pw-srg-speed">
      <span className="pw-num pw-readout pw-big" ref={speedEl} data-testid="speed" />
      <span className="pw-label" ref={unitEl} data-testid="speed-unit" />
    </div>
  );

  return (
    <div
      className="pw-frame-tile pw-srg"
      ref={root}
      data-density={density}
      data-dial={showDial ? 'true' : 'false'}
      style={{ '--pw-dial': `${dialSize}px` } as CSSProperties}
    >
      <div className="pw-frame-body">
        {showStrip ? <TileTitle>Speed / RPM</TileTitle> : null}
        {showStrip ? (
          <div className="pw-shift-strip" ref={strip} data-testid="shift-strip" data-segments={SEGMENTS} data-lit="0" data-limiter="false" aria-hidden="true">
            {Array.from({ length: SEGMENTS }, (_, i) => (
              <span key={i} data-zone={i < 8 ? 'low' : i < 14 ? 'mid' : 'high'} data-on="false" />
            ))}
          </div>
        ) : null}
        <div className="pw-srg-main">
          {showDial ? (
            <div className="pw-srg-dialbox">
              <canvas
                className="pw-dial"
                ref={dial}
                data-testid="rpm-dial"
                role="img"
                aria-label="Speed, RPM and gear: engine speed dial. The readouts beside it give the numbers."
              />
              {speed}
            </div>
          ) : (
            speed
          )}
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
      </div>
      <TileMessage />
    </div>
  );
}

/** Share of the sweep where the shift strip's zones change colour, so dial and strip agree. */
const ZONE_STOPS = [8 / SEGMENTS, 14 / SEGMENTS] as const;

/**
 * The RPM dial: tick marks and thousands labels outside a 270° track, then the track and a
 * fill to the share of the maximum, coloured by the shift strip's zones. A steady fill,
 * never animated. The track and the fill are the last two arcs drawn.
 */
function drawDial(canvas: HTMLCanvasElement, size: number, rpm: number | null, maxRpm: number | null, stale: boolean): void {
  const ctx = fitCanvas(canvas, size, size);
  if (!ctx) return;
  const empty = tokenColor(canvas, '--pw-meter-empty', '#2C363B');
  const muted = tokenColor(canvas, '--pw-text-muted', '#9AA7AD');
  const low = tokenColor(canvas, '--pw-sector-personal-best', '#7ED99C');
  const mid = tokenColor(canvas, '--pw-sector-slower', '#F2D14B');
  const high = tokenColor(canvas, '--pw-temp-hot', '#FF9C35');
  const start = Math.PI * 0.75;
  const sweep = Math.PI * 1.5;
  const c = size / 2;
  const band = Math.max(6, Math.round(size * 0.055));
  const radius = c - 24 - band / 2;
  ctx.clearRect(0, 0, size, size);

  // Ticks: 40 minor, a major one every 10, just outside the track.
  const outer = radius + band / 2 + 4;
  ctx.lineCap = 'butt';
  for (let i = 0; i <= 40; i++) {
    const angle = start + (sweep * i) / 40;
    const major = i % 10 === 0;
    const length = major ? 7 : 3;
    ctx.strokeStyle = major ? muted : empty;
    ctx.lineWidth = major ? 2 : 1;
    ctx.beginPath();
    ctx.moveTo(c + Math.cos(angle) * outer, c + Math.sin(angle) * outer);
    ctx.lineTo(c + Math.cos(angle) * (outer + length), c + Math.sin(angle) * (outer + length));
    ctx.stroke();
  }
  if (maxRpm !== null) {
    ctx.fillStyle = muted;
    ctx.font = '11px Oxanium, sans-serif';
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    for (let i = 0; i <= 4; i++) {
      const angle = start + (sweep * i) / 4;
      const at = outer + 14;
      ctx.fillText(String(Math.round((maxRpm * i) / 4 / 1000)), c + Math.cos(angle) * at, c + Math.sin(angle) * at);
    }
  }

  ctx.lineWidth = band;
  ctx.strokeStyle = empty;
  ctx.beginPath();
  ctx.arc(c, c, radius, start, start + sweep);
  ctx.stroke();
  if (rpm === null || maxRpm === null) return;
  ctx.globalAlpha = stale ? 0.6 : 1;
  const gradient = ctx.createConicGradient?.(start, c, c);
  if (gradient && !stale) {
    // The conic gradient runs a full turn from `start`; the stops are shares of 360°.
    const turn = sweep / (Math.PI * 2);
    gradient.addColorStop(0, low);
    gradient.addColorStop(turn * ZONE_STOPS[0], low);
    gradient.addColorStop(turn * ZONE_STOPS[1], mid);
    gradient.addColorStop(turn, high);
    gradient.addColorStop(1, high);
    ctx.strokeStyle = gradient;
  } else {
    ctx.strokeStyle = muted;
  }
  ctx.beginPath();
  ctx.arc(c, c, radius, start, start + sweep * Math.min(1, Math.max(0, rpm / maxRpm)));
  ctx.stroke();
  ctx.globalAlpha = 1;
}

export default defineTileModule({ declaration, Component: SpeedRpmGear });
