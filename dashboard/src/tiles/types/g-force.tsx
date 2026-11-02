import { useRef } from 'react';
import { effectiveState, frameReadoutState, wholeTileMessage } from '../../runtime/readout';
import { G_SCALE, ReadoutState } from '../../runtime/wire';
import { defineTileModule, type TileProps } from '../contract';
import '../frame-tiles.css';
import { fitCanvas, showMessage, signed, TileMessage, tokenColor, useLifecycle, write } from '../frame-kit';
import { useFrameTile } from '../use-frame-tile';

/** The circle spans this many G from the centre to the rim. */
const RANGE_G = 4;
const DOT_RADIUS = 5;

const declaration = {
  type: 'g-force',
  name: 'G-force',
  binding: 'follows-car',
  rate: 'frame',
  minSize: { w: 2, h: 2 },
  defaultSize: { w: 3, h: 2 },
  densities: [
    { density: 'compact', minWidth: 0, minHeight: 0 },
    { density: 'regular', minWidth: 170, minHeight: 120 },
  ],
  reads: [{ source: 'frame', fields: ['gLateral', 'gLongitudinal'] }],
} as const;

function GForce({ car, density, width, height }: TileProps) {
  const root = useRef<HTMLDivElement>(null);
  const lateral = useRef<HTMLSpanElement>(null);
  const longitudinal = useRef<HTMLSpanElement>(null);
  const total = useRef<HTMLSpanElement>(null);
  const circle = useRef<HTMLCanvasElement>(null);
  const lifecycle = useLifecycle();
  const showCircle = density !== 'compact';
  const size = Math.max(80, Math.min(height - 24, Math.round(width * 0.62)));

  useFrameTile(
    declaration,
    car,
    ({ frame, car: index, connected }) => {
      const state =
        index === null ? ReadoutState.Absent : effectiveState(frameReadoutState(frame, index, 'gLateral'), connected);
      const live = index !== null && (state === ReadoutState.Live || state === ReadoutState.Stale);
      const lat = live ? frame.gLateral[index]! / G_SCALE : 0;
      const lon = live ? frame.gLongitudinal[index]! / G_SCALE : 0;
      write(lateral.current, state, signed(lat, 1));
      write(longitudinal.current, state, signed(lon, 1));
      write(total.current, state, Math.hypot(lat, lon).toFixed(1));
      if (circle.current) drawCircle(circle.current, size, live ? { lat, lon } : null, state === ReadoutState.Stale);
      showMessage(root.current, wholeTileMessage([state], { lifecycle, connected }));
    },
    [density, size, lifecycle],
  );

  return (
    <div className="pw-frame-tile pw-gf" ref={root} data-density={density}>
      <div className="pw-frame-body pw-gf-body">
        {showCircle ? (
          <canvas
            className="pw-gf-circle"
            ref={circle}
            data-testid="g-circle"
            role="img"
            aria-label="G-force: lateral against longitudinal acceleration on a circle. The readouts beside it give the numbers."
          />
        ) : null}
        <div className="pw-stat pw-gf-total">
          <span className="pw-label">G-force</span>
          <span className="pw-gf-unit">
            <span className="pw-num pw-readout pw-metric" ref={total} data-testid="g-total" /> <span className="pw-label">G</span>
          </span>
        </div>
        <div className="pw-gf-values">
          <div className="pw-gf-axis">
            <span className="pw-label">Lat</span>
            <span className="pw-num pw-readout pw-value" ref={lateral} data-testid="g-lateral" />
          </div>
          <div className="pw-gf-axis">
            <span className="pw-label">Lon</span>
            <span className="pw-num pw-readout pw-value" ref={longitudinal} data-testid="g-longitudinal" />
          </div>
        </div>
      </div>
      <TileMessage />
    </div>
  );
}

/**
 * A polar plot: a disc with rings at each whole G and spokes every 45°, and a dot for the
 * car, lateral to the right and acceleration upward, joined to the centre by a vector line.
 * The dot is the last arc drawn.
 */
function drawCircle(canvas: HTMLCanvasElement, size: number, g: { lat: number; lon: number } | null, stale: boolean): void {
  const ctx = fitCanvas(canvas, size, size);
  if (!ctx) return;
  const line = tokenColor(canvas, '--pw-line', '#343d42');
  const disc = tokenColor(canvas, '--pw-tile-bg-raised', '#242B2E');
  const muted = tokenColor(canvas, '--pw-text-muted', '#9AA7AD');
  const dot = tokenColor(canvas, '--pw-brake', '#F55C72');
  const c = size / 2;
  const radius = c - DOT_RADIUS - 2;
  ctx.clearRect(0, 0, size, size);
  ctx.fillStyle = disc;
  ctx.beginPath();
  ctx.arc(c, c, radius, 0, Math.PI * 2);
  ctx.fill();
  ctx.lineWidth = 1;
  ctx.strokeStyle = line;
  for (let ring = 1; ring <= RANGE_G; ring++) {
    ctx.beginPath();
    ctx.arc(c, c, (radius * ring) / RANGE_G, 0, Math.PI * 2);
    ctx.stroke();
  }
  ctx.beginPath();
  for (let spoke = 0; spoke < 4; spoke++) {
    const a = (spoke * Math.PI) / 4;
    ctx.moveTo(c - Math.cos(a) * radius, c - Math.sin(a) * radius);
    ctx.lineTo(c + Math.cos(a) * radius, c + Math.sin(a) * radius);
  }
  ctx.stroke();
  if (!g) return;
  // Stop at the rim: the number beside the circle still tells the truth.
  const reach = Math.hypot(g.lat, g.lon) / RANGE_G;
  const scale = reach > 1 ? 1 / reach : 1;
  const x = c + (g.lat / RANGE_G) * scale * radius;
  const y = c - (g.lon / RANGE_G) * scale * radius;
  const ink = stale ? muted : dot;
  ctx.strokeStyle = ink;
  ctx.lineWidth = 2;
  ctx.beginPath();
  ctx.moveTo(c, c);
  ctx.lineTo(x, y);
  ctx.stroke();
  ctx.globalAlpha = stale ? 0.25 : 0.3;
  ctx.fillStyle = ink;
  ctx.beginPath();
  ctx.arc(x, y, DOT_RADIUS * 2.2, 0, Math.PI * 2);
  ctx.fill();
  ctx.globalAlpha = stale ? 0.6 : 1;
  ctx.beginPath();
  ctx.arc(x, y, DOT_RADIUS, 0, Math.PI * 2);
  ctx.fill();
  ctx.globalAlpha = 1;
}

export default defineTileModule({ declaration, Component: GForce });
