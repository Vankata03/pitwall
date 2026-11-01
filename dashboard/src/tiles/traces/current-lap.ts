import type { Trace } from '../../generated/wire/Trace';

/** Lap samples on the 5 m bucket grid, in the shape of the trace route's columns. */
export interface LapSamples {
  /** Metres. */
  distance: number[];
  /** km/h; null where the game withheld it. */
  speed: (number | null)[];
  /** Fractions, 0 to 1. */
  throttle: (number | null)[];
  brake: (number | null)[];
}

export const BUCKET_METRES = 5;
/** A fall of lap distance this large is the start line, not a spin or a reverse. */
const NEW_LAP_DROP_METRES = 500;
const MAX_BUCKETS = 20_000;

/**
 * The lap in progress, built from Frame (spec section 8: a screen builds the current lap
 * from Frame). One slot per 5 m bucket of lap distance, the latest sample winning, so
 * memory stays flat however long the lap runs and nothing is allocated per sample.
 */
export class CurrentLap {
  /** Bumped on every change, so the render loop can tell the trace is stale. */
  version = 0;
  private speed = new Float64Array(2048);
  private throttle = new Float64Array(2048);
  private brake = new Float64Array(2048);
  private filled = new Uint8Array(2048);
  private lastBucket = -1;
  private firstBucket = -1;

  /** One sample; throttle and brake in percent, any of the three NaN when withheld. */
  record(distance: number, throttlePercent: number, brakePercent: number, speedKmh: number): void {
    if (!(distance >= 0)) return; // negative lap distance is dropped; so is NaN
    const bucket = Math.floor(distance / BUCKET_METRES);
    if (bucket >= MAX_BUCKETS) return;
    if (this.lastBucket >= 0 && (this.lastBucket - bucket) * BUCKET_METRES > NEW_LAP_DROP_METRES) this.clear();
    this.grow(bucket);
    if (this.firstBucket < 0) this.firstBucket = bucket;
    this.put(bucket, speedKmh, throttlePercent / 100, brakePercent / 100);
    this.lastBucket = bucket;
    this.version++;
  }

  /** Fill in what a fetched lap-so-far has that the live samples do not. */
  seed(trace: Trace): void {
    for (let i = 0; i < trace.distance.length; i++) {
      const bucket = Math.round(trace.distance[i]! / BUCKET_METRES);
      if (bucket < 0 || bucket >= MAX_BUCKETS) continue;
      this.grow(bucket);
      if (this.filled[bucket]) continue;
      this.put(bucket, trace.speed[i] ?? NaN, trace.throttle[i] ?? NaN, trace.brake[i] ?? NaN);
    }
    this.version++;
  }

  /**
   * True when the first sample of this lap was well past the line, so the start of the lap was
   * never seen: the screen joined mid-lap and should fetch the lap so far.
   */
  get joinedMidLap(): boolean {
    return this.firstBucket > 1;
  }

  reset(): void {
    this.clear();
    this.version++;
  }

  samples(): LapSamples {
    const out: LapSamples = { distance: [], speed: [], throttle: [], brake: [] };
    for (let b = 0; b < this.filled.length; b++) {
      if (!this.filled[b]) continue;
      out.distance.push(b * BUCKET_METRES);
      out.speed.push(orNull(this.speed[b]!));
      out.throttle.push(orNull(this.throttle[b]!));
      out.brake.push(orNull(this.brake[b]!));
    }
    return out;
  }

  private put(bucket: number, speed: number, throttle: number, brake: number): void {
    this.speed[bucket] = speed;
    this.throttle[bucket] = throttle;
    this.brake[bucket] = brake;
    this.filled[bucket] = 1;
  }

  private clear(): void {
    this.filled.fill(0);
    this.lastBucket = -1;
    this.firstBucket = -1;
  }

  private grow(bucket: number): void {
    if (bucket < this.filled.length) return;
    const size = Math.min(MAX_BUCKETS, Math.max(bucket + 1, this.filled.length * 2));
    const widen = <T extends Float64Array | Uint8Array>(a: T): T => {
      const next = new (a.constructor as new (n: number) => T)(size);
      next.set(a);
      return next;
    };
    this.speed = widen(this.speed);
    this.throttle = widen(this.throttle);
    this.brake = widen(this.brake);
    this.filled = widen(this.filled);
  }
}

const orNull = (v: number): number | null => (Number.isNaN(v) ? null : v);
