import type { Runtime } from '../../runtime/runtime';
import type { Trace } from '../../generated/wire/Trace';

export interface LapTracesOptions {
  fetch: typeof globalThis.fetch;
  /** The epoch the screen is on now; null before the first hello. */
  epoch: () => number | null;
}

/**
 * Lap samples fetched per car and lap (spec section 8, Lap traces). Held only for the epoch
 * they were fetched in, and a reply that arrives after the epoch moved on is dropped, so a
 * flashback or new session never shows a lap from the old timeline.
 */
export interface LapTraces {
  /** Fetch a lap unless it is held, in flight or known unavailable. A no-op before hello. */
  request(car: number, lap: number): void;
  get(car: number, lap: number): Trace | undefined;
  /** Allow laps that were unavailable or failed to be requested again (a lap just completed). */
  invalidate(car?: number, lap?: number): void;
  subscribe(listener: () => void): () => void;
  /** Bumped whenever a lap arrives or the held laps are forgotten. */
  readonly version: number;
}

export function createLapTraces(options: LapTracesOptions): LapTraces {
  const held = new Map<string, Trace>();
  const pending = new Map<string, object>();
  const unavailable = new Set<string>();
  const listeners = new Set<() => void>();
  let heldEpoch: number | null = null;
  let version = 0;

  const key = (car: number, lap: number) => `${car}:${lap}`;
  const notify = () => {
    version++;
    for (const l of [...listeners]) l();
  };

  /** Forget everything held when the epoch moved. */
  function sync(): number | null {
    const epoch = options.epoch();
    if (epoch !== heldEpoch) {
      const hadData = held.size > 0;
      held.clear();
      pending.clear();
      unavailable.clear();
      heldEpoch = epoch;
      if (hadData) notify();
    }
    return epoch;
  }

  async function load(epoch: number, car: number, lap: number, token: object): Promise<void> {
    const k = key(car, lap);
    try {
      const response = await options.fetch(`/api/cars/${car}/laps/${lap}/trace?epoch=${epoch}`);
      if (pending.get(k) !== token || heldEpoch !== epoch || options.epoch() !== epoch) return;
      if (!response.ok) {
        pending.delete(k);
        unavailable.add(k);
        return;
      }
      const trace = (await response.json()) as Trace;
      if (pending.get(k) !== token || heldEpoch !== epoch || options.epoch() !== epoch) return;
      pending.delete(k);
      if (trace.epoch !== epoch || trace.lap !== lap) { unavailable.add(k); return; }
      held.set(k, trace);
      notify();
    } catch {
      if (pending.get(k) === token) { pending.delete(k); unavailable.add(k); }
    }
  }

  return {
    request(car, lap) {
      const epoch = sync();
      const k = key(car, lap);
      if (epoch === null || held.has(k) || pending.has(k) || unavailable.has(k)) return;
      const token = {};
      pending.set(k, token);
      void load(epoch, car, lap, token);
    },
    get(car, lap) {
      sync();
      return held.get(key(car, lap));
    },
    invalidate(car, lap) {
      sync();
      unavailable.clear();
      if (car !== undefined && lap !== undefined) {
        const k = key(car, lap);
        pending.delete(k);
        if (held.delete(k)) notify();
      }
    },
    subscribe(listener) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    get version() {
      return version;
    },
  };
}

const byRuntime = new WeakMap<Runtime, LapTraces>();

/** The lap traces of one screen's runtime: shared by every tile on it, reading its epoch. */
export function lapTracesFor(runtime: Runtime): LapTraces {
  let traces = byRuntime.get(runtime);
  if (!traces) {
    traces = createLapTraces({
      fetch: (...args) => globalThis.fetch(...args),
      epoch: () => {
        const slowEpoch = runtime.stores.slow.getSnapshot().epoch;
        const frameEpoch = runtime.stores.frame.data.epoch;
        // Frame can advance before the throttled State snapshot reaches subscribers.
        return slowEpoch === null ? (frameEpoch || null) : Math.max(slowEpoch, frameEpoch);
      },
    });
    byRuntime.set(runtime, traces);
  }
  return traces;
}
