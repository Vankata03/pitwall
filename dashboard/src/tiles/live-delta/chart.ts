import { fitCanvas, signed, tokenColor } from '../frame-kit';
import { deltaExtent, segments, type DeltaLap, type Point } from './lap';

export interface ChartBox {
  width: number;
  height: number;
}

interface Palette {
  muted: string;
  ahead: string;
  behind: string;
  font: string;
}
const palettes = new WeakMap<HTMLCanvasElement, Palette>();
/** Read once per canvas: a style query on every animation frame would cost more than the drawing. */
function paletteOf(canvas: HTMLCanvasElement): Palette {
  let p = palettes.get(canvas);
  if (!p) {
    const family = getComputedStyle(canvas).fontFamily;
    p = {
      muted: tokenColor(canvas, '--pw-text-muted', '#9AA7AD'),
      ahead: tokenColor(canvas, '--pw-sector-personal-best', '#7ED99C'),
      behind: tokenColor(canvas, '--pw-sector-slower', '#F2D14B'),
      font: `11px ${family || 'sans-serif'}`,
    };
    palettes.set(canvas, p);
  }
  return p;
}

const LEFT = 48; // gutter for the delta labels
const RIGHT = 8;
const TOP = 8;
const BOTTOM = 20; // room for the distance labels
const LINE = 3;
const FILL_ALPHA = 0.22;
const FAINT = 0.18;

/**
 * Delta against lap distance for the whole lap. Ahead (negative) is drawn above the zero line
 * and labelled in words, so the sign never rests on colour alone. The line is one thick path per
 * run, clipped either side of zero so each half takes its colour and its area to the line.
 */
export function drawDelta(
  canvas: HTMLCanvasElement | null,
  box: ChartBox,
  lap: DeltaLap,
  length: number,
  sectors: readonly number[],
  muted: boolean,
): void {
  const ctx = fitCanvas(canvas, box.width, box.height);
  if (!ctx || !canvas) return;
  const { width, height } = box;
  ctx.clearRect(0, 0, width, height);
  if (!(length > 0)) return;
  const colours = paletteOf(canvas);
  const left = LEFT;
  const right = width - RIGHT;
  const top = TOP;
  const bottom = height - BOTTOM;

  let passed = 0;
  let head: Point | null = null;
  for (let i = lap.current.length - 1; i >= 0; i--) {
    const p = lap.current[i]!;
    if (Number.isFinite(p.distance) && Number.isFinite(p.delta)) {
      head = p;
      passed = p.distance;
      break;
    }
  }
  // The finished lap stays only ahead of the car; the new lap overwrites it as it passes.
  const ahead = lap.previous.filter((p) => !(p.distance < passed));
  const extent = deltaExtent([...lap.current, ...ahead]);
  const x = (distance: number) => left + (distance / length) * (right - left);
  const y = (delta: number) => top + ((extent + delta) / (extent * 2)) * (bottom - top);
  const zero = y(0);

  ctx.font = colours.font;
  ctx.fillStyle = colours.muted;
  ctx.strokeStyle = colours.muted;
  ctx.lineWidth = 1;
  ctx.globalAlpha = 0.3;
  for (const distance of sectors) {
    if (!(distance > 0 && distance < length)) continue;
    ctx.beginPath();
    ctx.moveTo(x(distance), top);
    ctx.lineTo(x(distance), bottom);
    ctx.stroke();
  }
  ctx.globalAlpha = 0.5;
  ctx.beginPath();
  ctx.moveTo(left, zero);
  ctx.lineTo(right, zero);
  ctx.stroke();
  ctx.globalAlpha = 1;
  ctx.textAlign = 'left';
  ctx.fillText(`${signed(-extent, 1)} s`, 0, top + 10);
  ctx.fillText('ahead', 0, top + 23);
  ctx.fillText('0', LEFT - 14, zero + 4);
  ctx.fillText('behind', 0, bottom - 2);
  ctx.fillText(`${signed(extent, 1)} s`, 0, bottom - 15);
  ctx.fillText('0 m', left, height - 5);
  ctx.textAlign = 'right';
  ctx.fillText(`${Math.round(length)} m`, right, height - 5);

  const halves = [
    { colour: muted ? colours.muted : colours.ahead, from: top, to: zero },
    { colour: muted ? colours.muted : colours.behind, from: zero, to: bottom },
  ];
  const run = (points: readonly Point[], alpha: number) => {
    for (const segment of segments(points)) {
      if (segment.length < 2) continue;
      // A point closer than a pixel to the last drawn one adds cost and no information.
      const trace = () => {
        let drawn = x(segment[0]!.distance);
        ctx.moveTo(drawn, y(segment[0]!.delta));
        for (let i = 1; i < segment.length; i++) {
          const px = x(segment[i]!.distance);
          if (px - drawn < 1 && i < segment.length - 1) continue;
          ctx.lineTo(px, y(segment[i]!.delta));
          drawn = px;
        }
      };
      for (const half of halves) {
        ctx.save();
        ctx.beginPath();
        ctx.rect(left, half.from, right - left, half.to - half.from);
        ctx.clip();
        ctx.beginPath();
        trace();
        ctx.lineTo(x(segment.at(-1)!.distance), zero);
        ctx.lineTo(x(segment[0]!.distance), zero);
        ctx.closePath();
        ctx.fillStyle = half.colour;
        ctx.globalAlpha = alpha * FILL_ALPHA;
        ctx.fill();
        ctx.beginPath();
        trace();
        ctx.strokeStyle = half.colour;
        ctx.lineWidth = LINE;
        ctx.lineJoin = 'round';
        ctx.lineCap = 'round';
        ctx.globalAlpha = alpha;
        ctx.stroke();
        ctx.restore();
      }
    }
  };
  run(ahead, FAINT);
  run(lap.current, 1);

  if (head) {
    ctx.beginPath();
    ctx.arc(x(head.distance), y(head.delta), 4, 0, Math.PI * 2);
    ctx.fillStyle = muted ? colours.muted : head.delta < 0 ? colours.ahead : colours.behind;
    ctx.globalAlpha = 1;
    ctx.fill();
  }
  ctx.globalAlpha = 1;
}
