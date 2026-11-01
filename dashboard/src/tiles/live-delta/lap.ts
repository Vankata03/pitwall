import type { CarRings } from '../../runtime/stores';

export type Point = { distance: number; delta: number };
/** Retains a whole lap beyond the short transport rings, consuming every received sample. */
export class DeltaLap {
  current: Point[] = [];
  previous: Point[] = [];
  private seen = 0;
  private epoch = -1;
  private lastTime = -Infinity;
  private lastDistance = NaN;

  ingest(rings: CarRings, epoch: number, reference: 'deltaBest' | 'deltaPrevious'): void {
    const ring = rings[reference];
    if (epoch !== this.epoch || ring.version < this.seen || ring.version - this.seen > ring.length) {
      this.current = []; this.previous = []; this.seen = ring.version - ring.length;
      this.lastTime = -Infinity; this.lastDistance = NaN; this.epoch = epoch;
    }
    const start = Math.max(0, ring.length - (ring.version - this.seen));
    for (let i = start; i < ring.length; i++) {
      const time = rings.sessionTime.at(i);
      const distance = rings.lapDistance.at(i);
      const delta = ring.at(i) / 1000;
      if (time < this.lastTime) { this.current = []; this.previous = []; this.lastDistance = NaN; }
      this.lastTime = time;
      if (Number.isFinite(distance) && distance >= 0) {
        if (distance < this.lastDistance - 100) {
          this.previous = this.current; this.current = [];
        } else if (distance < this.lastDistance) {
          // Small reversals are not a new lap. Break the trace while reversing.
          this.current.push({ distance, delta: NaN });
          this.lastDistance = distance;
          continue;
        }
        this.lastDistance = distance;
      }
      // One point per distance suffices; telemetry Frames often repeat Lap Data.
      const last = this.current.at(-1);
      if (last && !Number.isFinite(last.distance) && !Number.isFinite(distance)) continue;
      if (last && last.distance === distance && (Number.isFinite(last.delta) || !Number.isFinite(delta))) last.delta = delta;
      else this.current.push({ distance, delta });
    }
    this.seen = ring.version;
  }
}

/** NaN or a gap over 100 m starts a new segment; shorter gaps interpolate linearly. */
export function segments(points: readonly Point[]): Point[][] {
  const out: Point[][] = [];
  let segment: Point[] | null = null;
  for (const point of points) {
    if (!Number.isFinite(point.distance) || point.distance < 0 || !Number.isFinite(point.delta)) { segment = null; continue; }
    const last = segment?.at(-1);
    if (!segment || (last && (point.distance - last.distance > 100 || point.distance < last.distance))) {
      segment = []; out.push(segment);
    }
    segment.push(point);
  }
  return out;
}

/** Symmetric 1/2/5 steps, with a useful floor of ±0.1 s. */
export function deltaExtent(points: readonly Point[]): number {
  let max = 0.1;
  for (const p of points) if (Number.isFinite(p.delta)) max = Math.max(max, Math.abs(p.delta));
  const power = 10 ** Math.floor(Math.log10(max));
  return [1, 2, 5, 10].find(n => n * power >= max)! * power;
}
