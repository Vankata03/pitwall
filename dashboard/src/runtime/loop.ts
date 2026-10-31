import type { CarRings, Stores } from './stores';
import type { TileDeclaration } from './tile';
import type { FrameData } from './wire';

/** What `draw()` receives. Everything here is live, mutable store data: do not keep references. */
export interface DrawContext {
  /** The requestAnimationFrame timestamp. */
  ts: number;
  frame: FrameData;
  /** Car this tile follows, or null for a session-wide tile or an unresolved binding. */
  car: number | null;
  /** Recent samples of `car`; null when `car` is. */
  rings: CarRings | null;
}

export interface FrameTile {
  declaration: TileDeclaration;
  /** Resolved each draw, so a focus change needs only an `invalidate()`. */
  car: () => number | null;
  draw: (ctx: DrawContext) => void;
}

export interface TileHandle {
  /** Draw again on the next animation frame (resize, density, car or settings change). */
  invalidate(): void;
  unregister(): void;
  /** Error that parked the tile, if its `draw()` threw. Cleared by `invalidate()`. */
  error(): unknown;
}

const WINDOW_MS = 1000;
const SLOW_FPS = 24;
const SLOW_AFTER_MS = 5000;

/**
 * Measures the rate of animation frames. Raises `slow` once the rate has stayed under
 * 24 fps for 5 s, and clears it on the first full second at or above 24.
 */
export class FrameRateMonitor {
  slow = false;
  private windowStart: number | null = null;
  private count = 0;
  private lowMs = 0;
  private readonly listeners = new Set<() => void>();

  subscribe = (l: () => void): (() => void) => {
    this.listeners.add(l);
    return () => {
      this.listeners.delete(l);
    };
  };
  getSnapshot = (): boolean => this.slow;

  sample(ts: number): void {
    if (this.windowStart === null) {
      this.windowStart = ts;
      this.count = 0;
      return;
    }
    this.count++;
    const elapsed = ts - this.windowStart;
    if (elapsed < WINDOW_MS) return;
    const fps = (this.count * 1000) / elapsed;
    this.lowMs = fps < SLOW_FPS ? this.lowMs + elapsed : 0;
    this.windowStart = ts;
    this.count = 0;
    this.set(this.lowMs >= SLOW_AFTER_MS);
  }

  /** Forget the window, for a gap that is not slowness (hidden tab). */
  reset(): void {
    this.windowStart = null;
    this.count = 0;
    this.lowMs = 0;
    this.set(false);
  }

  private set(slow: boolean): void {
    if (slow === this.slow) return;
    this.slow = slow;
    for (const l of [...this.listeners]) l();
  }
}

interface Entry {
  tile: FrameTile;
  readsFrame: boolean;
  readsSlow: boolean;
  seenFrame: number;
  seenSlow: unknown;
  forced: boolean;
  error: unknown;
  parked: boolean;
}

export interface LoopOptions {
  stores: Stores;
  requestFrame?: (cb: (ts: number) => void) => number;
  cancelFrame?: (id: number) => void;
}

/**
 * The one shared requestAnimationFrame loop. Each animation frame it calls `draw()` on
 * the registered tiles whose data changed since their last draw, and on no others. A frame
 * that arrives late draws the latest value; nothing is queued or interpolated.
 */
export class RenderLoop {
  readonly monitor = new FrameRateMonitor();
  private readonly entries = new Set<Entry>();
  private raf: number | null = null;
  private readonly request: (cb: (ts: number) => void) => number;
  private readonly cancel: (id: number) => void;

  constructor(private readonly options: LoopOptions) {
    this.request = options.requestFrame ?? ((cb) => requestAnimationFrame(cb));
    this.cancel = options.cancelFrame ?? ((id) => cancelAnimationFrame(id));
  }

  register(tile: FrameTile): TileHandle {
    if (tile.declaration.rate !== 'frame') {
      throw new Error(`tile ${tile.declaration.type}: slow tiles render through React, not the frame loop`);
    }
    const reads = tile.declaration.reads;
    const entry: Entry = {
      tile,
      readsFrame: reads.some((r) => r.source === 'frame'),
      readsSlow: reads.some((r) => r.source === 'state' || r.source === 'history'),
      seenFrame: -1,
      seenSlow: null,
      forced: true,
      error: undefined,
      parked: false,
    };
    this.entries.add(entry);
    return {
      invalidate: () => {
        entry.forced = true;
        entry.parked = false;
        entry.error = undefined;
      },
      unregister: () => {
        this.entries.delete(entry);
      },
      error: () => entry.error,
    };
  }

  start(): void {
    if (this.raf !== null) return;
    this.raf = this.request(this.onFrame);
  }

  stop(): void {
    if (this.raf !== null) this.cancel(this.raf);
    this.raf = null;
  }

  /** Call when the page is hidden or shown: a hidden tab's missing frames are not slowness. */
  visibilityChanged(): void {
    this.monitor.reset();
  }

  private readonly onFrame = (ts: number): void => {
    this.raf = this.request(this.onFrame);
    this.monitor.sample(ts);
    const { frame, slow } = this.options.stores;
    const snapshot = slow.getSnapshot();
    for (const e of this.entries) {
      if (e.parked) continue;
      const dirty =
        e.forced ||
        (e.readsFrame && frame.version !== e.seenFrame) ||
        (e.readsSlow && snapshot !== e.seenSlow);
      if (!dirty) continue;
      e.forced = false;
      e.seenFrame = frame.version;
      e.seenSlow = snapshot;
      const car = e.tile.car();
      try {
        e.tile.draw({ ts, frame: frame.data, car, rings: car === null ? null : (frame.rings[car] ?? null) });
      } catch (err) {
        e.error = err;
        e.parked = true;
        console.error(`tile ${e.tile.declaration.type} draw() threw; parked until invalidated`, err);
      }
    }
  };
}
