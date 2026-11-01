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
  const size = Math.max(80, Math.min(height - 24, Math.round(width * 0.5)));

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
        <div className="pw-gf-values">
          <div className="pw-stat">
            <span className="pw-label">Lateral</span>
            <span>
              <span className="pw-num pw-readout pw-value" ref={lateral} data-testid="g-lateral" /> <span className="pw-label">G</span>
            </span>
          </div>
          <div className="pw-stat">
            <span className="pw-label">Longitudinal</span>
            <span>
              <span className="pw-num pw-readout pw-value" ref={longitudinal} data-testid="g-longitudinal" /> <span className="pw-label">G</span>
            </span>
          </div>
          {showCircle ? (
            <div className="pw-stat">
              <span className="pw-label">Total</span>
              <span>
                <span className="pw-num pw-readout pw-value" ref={total} data-testid="g-total" /> <span className="pw-label">G</span>
              </span>
            </div>
          ) : null}
        </div>
      </div>
      <TileMessage />
    </div>
  );
}

/** Rings at each whole G, and a dot for the car: lateral to the right, acceleration upward. */
function drawCircle(canvas: HTMLCanvasElement, size: number, g: { lat: number; lon: number } | null, stale: boolean): void {
  const ctx = fitCanvas(canvas, size, size);
  if (!ctx) return;
  const line = tokenColor(canvas, '--pw-line', '#343d42');
  const dot = tokenColor(canvas, '--pw-energy', '#56D9CA');
  const c = size / 2;
  const radius = c - DOT_RADIUS - 2;
  ctx.clearRect(0, 0, size, size);
  ctx.lineWidth = 1;
  ctx.strokeStyle = line;
  for (let ring = 1; ring <= RANGE_G; ring++) {
    ctx.beginPath();
    ctx.arc(c, c, (radius * ring) / RANGE_G, 0, Math.PI * 2);
    ctx.stroke();
  }
  ctx.beginPath();
  ctx.moveTo(c - radius, c);
  ctx.lineTo(c + radius, c);
  ctx.moveTo(c, c - radius);
  ctx.lineTo(c, c + radius);
  ctx.stroke();
  if (!g) return;
  // Stop at the rim: the number beside the circle still tells the truth.
  const reach = Math.hypot(g.lat, g.lon) / RANGE_G;
  const scale = reach > 1 ? 1 / reach : 1;
  ctx.globalAlpha = stale ? 0.6 : 1;
  ctx.fillStyle = dot;
  ctx.beginPath();
  ctx.arc(c + (g.lat / RANGE_G) * scale * radius, c - (g.lon / RANGE_G) * scale * radius, DOT_RADIUS, 0, Math.PI * 2);
  ctx.fill();
  ctx.globalAlpha = 1;
}

export default defineTileModule({ declaration, Component: GForce });
