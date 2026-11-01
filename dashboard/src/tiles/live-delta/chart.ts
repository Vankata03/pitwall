import { fitCanvas, signed, tokenColor } from '../frame-kit';
import { deltaExtent, segments, type DeltaLap, type Point } from './lap';

export function drawDelta(canvas: HTMLCanvasElement | null, width: number, height: number, lap: DeltaLap, length: number, sectors: number[], stale: boolean): void {
  const ctx = fitCanvas(canvas, width, height);
  if (!ctx || !canvas) return;
  ctx.clearRect(0, 0, width, height);
  if (!(length > 0) || width < 80 || height < 45) return;
  const left = 58, right = width - 12, top = 12, bottom = height - 24;
  const passed = lap.current.filter(p => Number.isFinite(p.distance)).at(-1)?.distance ?? 0;
  const previous = lap.previous.filter(p => p.distance >= passed);
  const extent = deltaExtent([...lap.current, ...previous]);
  const x = (d: number) => left + d / length * (right - left);
  const y = (d: number) => top + (extent - d) / (extent * 2) * (bottom - top);
  const muted = tokenColor(canvas, '--pw-text-muted', '#9AA7AD');
  const ahead = tokenColor(canvas, '--pw-sector-personal-best', '#7ED99C');
  const behind = tokenColor(canvas, '--pw-sector-slower', '#F2D14B');
  ctx.strokeStyle = muted; ctx.lineWidth = 1; ctx.globalAlpha = 0.35;
  ctx.beginPath(); ctx.moveTo(left, y(0)); ctx.lineTo(right, y(0)); ctx.stroke();
  ctx.globalAlpha = 0.15;
  for (const d of sectors.filter(d => d > 0 && d < length)) {
    ctx.beginPath(); ctx.moveTo(x(d), top); ctx.lineTo(x(d), bottom); ctx.stroke();
  }
  ctx.globalAlpha = 1; ctx.fillStyle = muted; ctx.font = '11px sans-serif';
  ctx.fillText(`${signed(extent, 1)} s`, 0, top + 8);
  ctx.fillText('0', 35, y(0) + 4);
  ctx.fillText(`${signed(-extent, 1)} s`, 0, bottom);
  ctx.fillText('0 m', left, height - 5);
  ctx.textAlign = 'right'; ctx.fillText(`${Math.round(length)} m`, right, height - 5); ctx.textAlign = 'left';
  const draw = (points: Point[], alpha: number) => {
    for (const segment of segments(points)) {
      if (segment.length < 2) continue;
      ctx.beginPath();
      ctx.moveTo(x(segment[0]!.distance), y(0));
      for (const p of segment) ctx.lineTo(x(p.distance), y(p.delta));
      ctx.lineTo(x(segment.at(-1)!.distance), y(0)); ctx.closePath();
      ctx.fillStyle = muted; ctx.globalAlpha = alpha * 0.12; ctx.fill();
      // Colour each edge by sign, splitting exactly at zero when it crosses.
      ctx.lineWidth = 3; ctx.lineJoin = 'round'; ctx.lineCap = 'round'; ctx.globalAlpha = alpha;
      for (let i = 1; i < segment.length; i++) {
        const a = segment[i - 1]!, b = segment[i]!;
        const edge = (a: Point, b: Point, value: number) => {
          ctx.strokeStyle = stale ? muted : value < 0 ? ahead : behind;
          ctx.beginPath(); ctx.moveTo(x(a.distance), y(a.delta)); ctx.lineTo(x(b.distance), y(b.delta)); ctx.stroke();
        };
        if (a.delta * b.delta < 0) {
          const zero = { distance: a.distance + (b.distance - a.distance) * -a.delta / (b.delta - a.delta), delta: 0 };
          edge(a, zero, a.delta); edge(zero, b, b.delta);
        } else edge(a, b, b.delta || a.delta);
      }
    }
  };
  draw(previous, 0.18); draw(lap.current, 1); ctx.globalAlpha = 1;
}
