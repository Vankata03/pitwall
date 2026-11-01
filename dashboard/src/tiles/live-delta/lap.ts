import type { CarRings } from '../../runtime/stores';

export type Point = { distance: number; delta: number };
export type Reference = 'deltaBest' | 'deltaPrevious';

/** Longest gap in lap distance (m) the graph joins with a straight line (spec section 6). */
export const MAX_JOIN_M = 100;
/** A fall in lap distance larger than this is the start line, not a car rolling back. */
const LINE_M = 100;
const MAX_POINTS = 20_000;
const BREAK: Point = { distance: NaN, delta: NaN };

/**
 * One car's delta against lap distance over the whole lap, in seconds. The transport rings
 * keep ten seconds; this keeps the lap. It reads every sample the rings gained since its last
 * call, so a Frame that arrives between two paints is never skipped. The rings are written
 * per accepted Frame, never per animation frame.
 */
export class DeltaLap {
  current: Point[] = [];
  /** The finished lap, kept until the new lap passes it. */
  previous: Point[] = [];
  private seen = 0;
  private epoch = -1;
  private lastTime = -Infinity;
  private lastDistance = NaN;
  /** Distance of the first sample after the line, repeated by Frames until the next Lap Data. */
  private atLine = NaN;

  ingest(rings: CarRings, epoch: number, reference: Reference): void {
    const ring = rings[reference];
    if (epoch !== this.epoch) {
      this.restart();
      this.epoch = epoch;
      this.seen = ring.version - ring.length;
    }
    const gained = ring.version - this.seen;
    // Frames lost to an overflow (a hidden tab, say) leave a hole: break the line, keep the lap.
    if (gained > ring.length || gained < 0) this.add(BREAK);
    const start = Math.max(0, ring.length - Math.max(0, gained));
    for (let i = start; i < ring.length; i++) this.sample(rings.sessionTime.at(i), rings.lapDistance.at(i), ring.at(i) / 1000);
    this.seen = ring.version;
  }

  private restart(): void {
    this.current = [];
    this.previous = [];
    this.lastTime = -Infinity;
    this.lastDistance = NaN;
    this.atLine = NaN;
  }

  private sample(time: number, distance: number, delta: number): void {
    if (time < this.lastTime) this.restart();
    this.lastTime = time;
    if (!Number.isFinite(distance) || distance < 0) {
      this.add(BREAK);
      return;
    }
    if (distance < this.lastDistance - LINE_M) {
      // The line. The first Lap Data of the new lap carries the official lap delta, which is
      // the headline's business: it belongs to neither lap's graph.
      if (this.current.filter((p) => Number.isFinite(p.delta)).length > 1) this.previous = this.current;
      this.current = [];
      this.lastDistance = distance;
      this.atLine = distance;
      return;
    }
    if (distance === this.atLine) return;
    this.atLine = NaN;
    if (distance < this.lastDistance) {
      // A small reversal is not a new lap: break the line while the car goes back over ground.
      this.add({ distance, delta: NaN });
      this.lastDistance = distance;
      return;
    }
    this.lastDistance = distance;
    const last = this.current.at(-1);
    if (last && last.distance === distance) last.delta = delta; // one point per distance
    else this.add({ distance, delta });
  }

  private add(point: Point): void {
    const last = this.current.at(-1);
    if (!Number.isFinite(point.distance) && last && !Number.isFinite(last.distance)) return;
    this.current.push(point === BREAK ? { ...BREAK } : point);
    if (this.current.length > MAX_POINTS) this.current.splice(0, MAX_POINTS / 4);
  }
}

/** NaN or a gap over 100 m starts a new segment; shorter gaps interpolate linearly. */
export function segments(points: readonly Point[]): Point[][] {
  const out: Point[][] = [];
  let segment: Point[] | null = null;
  for (const point of points) {
    if (!Number.isFinite(point.distance) || point.distance < 0 || !Number.isFinite(point.delta)) {
      segment = null;
      continue;
    }
    const last = segment?.at(-1);
    if (!segment || (last && (point.distance - last.distance > MAX_JOIN_M || point.distance < last.distance))) {
      segment = [];
      out.push(segment);
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
  return [1, 2, 5, 10].find((n) => n * power >= max)! * power;
}
