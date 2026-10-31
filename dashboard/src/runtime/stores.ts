import { frameReadoutState } from './readout';
import { RingBuffer } from './ring';
import {
  CAR_COUNT,
  createFrameData,
  decodeFrameInto,
  frameEpoch,
  parseTextMessage,
  type FrameData,
  type HelloMessage,
  type Source,
  type WireHistoryEntry,
  type WireState,
} from './wire';

export type ConnectionStatus = 'connecting' | 'open' | 'reconnecting';

/** Slow-path notifications go out at most this often (10 Hz). */
const SLOW_NOTIFY_MS = 100;
/** Frame ring depth: 10 s at 60 Hz. */
const DEFAULT_RING_CAPACITY = 600;

// ---- Frame store -----------------------------------------------------------

/** Recent samples of one car, appended by every Frame. Used to build the current lap. */
export interface CarRings {
  sessionTime: RingBuffer;
  speed: RingBuffer;
  throttle: RingBuffer;
  brake: RingBuffer;
  lapDistance: RingBuffer;
}

/**
 * Mutable store for 60 Hz data. Never touches React state: tiles read `data` inside
 * `draw()` and the render loop dirty-checks on `version`.
 */
export class FrameStore {
  readonly data: FrameData = createFrameData();
  readonly rings: CarRings[];
  /** Bumped by every accepted Frame and every reset. */
  version = 0;

  constructor(ringCapacity: number) {
    this.rings = Array.from({ length: CAR_COUNT }, () => ({
      sessionTime: new RingBuffer(ringCapacity),
      speed: new RingBuffer(ringCapacity),
      throttle: new RingBuffer(ringCapacity),
      brake: new RingBuffer(ringCapacity),
      lapDistance: new RingBuffer(ringCapacity),
    }));
  }

  /** @internal Decode a Frame in and feed the rings. */
  accept(buf: ArrayBuffer): void {
    if (!decodeFrameInto(buf, this.data)) return;
    const d = this.data;
    for (let i = 0; i < CAR_COUNT; i++) {
      if ((d.readout[i]! & 0x3fff) === 0x3fff) continue; // empty slot: absent throughout
      const r = this.rings[i]!;
      r.sessionTime.push(d.sessionTime);
      // Keep channels aligned; missing data must break a trace rather than draw zero.
      for (const field of ['speed', 'throttle', 'brake', 'lapDistance'] as const) {
        r[field].push(frameReadoutState(d, i, field) < 2 ? d[field][i]! : NaN);
      }
    }
    this.version++;
  }

  /** @internal Back to the empty state: every slot absent, rings cleared. */
  reset(): void {
    const fresh = createFrameData();
    const d = this.data as unknown as Record<string, unknown>;
    for (const key of Object.keys(fresh) as (keyof FrameData)[]) {
      const f = fresh[key];
      if (typeof f === 'number') d[key] = f;
      else (this.data[key] as unknown as { set(a: ArrayLike<number>): void }).set(f as ArrayLike<number>);
    }
    for (const r of this.rings) {
      r.sessionTime.clear();
      r.speed.clear();
      r.throttle.clear();
      r.brake.clear();
      r.lapDistance.clear();
    }
    this.version++;
  }
}

// ---- Slow store ------------------------------------------------------------

export interface SlowSnapshot {
  connection: ConnectionStatus;
  /** Null until the first hello. */
  epoch: number | null;
  source: Source | null;
  /** Null until the first snapshot. */
  state: WireState | null;
  history: readonly WireHistoryEntry[];
}

/**
 * State, History and connection status for React, through `useSyncExternalStore`.
 * Snapshots are immutable and rebuilt at most every 100 ms, so subscribers see 4 to 10 Hz.
 */
export class SlowStore {
  private published: SlowSnapshot = {
    connection: 'connecting',
    epoch: null,
    source: null,
    state: null,
    history: [],
  };
  private readonly listeners = new Set<() => void>();
  private timer: ReturnType<typeof setTimeout> | null = null;
  private lastFlush = 0;
  private dirty = false;

  // Working copy, folded into `published` on flush.
  private working: { -readonly [K in keyof SlowSnapshot]: SlowSnapshot[K] } & { entries: WireHistoryEntry[] } = {
    ...this.published,
    entries: [],
  };
  private readonly index = new Map<string, number>();

  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };

  getSnapshot = (): SlowSnapshot => this.published;

  /** @internal */
  setConnection(connection: ConnectionStatus): void {
    if (this.working.connection === connection) return;
    this.working.connection = connection;
    this.markDirty();
  }

  /** @internal */
  begin(epoch: number, source: Source): void {
    this.working.epoch = epoch;
    this.working.source = source;
    this.markDirty();
  }

  /** @internal */
  install(epoch: number, source: Source, state: WireState, history: readonly WireHistoryEntry[]): void {
    this.working.epoch = epoch;
    this.working.source = source;
    this.working.state = state;
    this.working.entries = [];
    this.index.clear();
    this.append(history);
    this.markDirty();
  }

  /** @internal */
  setState(state: WireState): void {
    this.working.state = state;
    this.markDirty();
  }

  /** @internal */
  appendHistory(entries: readonly WireHistoryEntry[]): void {
    this.append(entries);
    this.markDirty();
  }

  private append(entries: readonly WireHistoryEntry[]): void {
    for (const e of entries) {
      const at = this.index.get(e.id);
      if (at === undefined) {
        this.index.set(e.id, this.working.entries.length);
        this.working.entries.push(e);
      } else {
        this.working.entries[at] = e;
      }
    }
  }

  private markDirty(): void {
    this.dirty = true;
    if (this.timer !== null) return;
    const wait = Math.max(0, this.lastFlush + SLOW_NOTIFY_MS - Date.now());
    if (wait === 0) {
      this.flush();
    } else {
      this.timer = setTimeout(() => {
        this.timer = null;
        this.flush();
      }, wait);
    }
  }

  private flush(): void {
    if (!this.dirty) return;
    this.dirty = false;
    this.lastFlush = Date.now();
    const w = this.working;
    this.published = {
      connection: w.connection,
      epoch: w.epoch,
      source: w.source,
      state: w.state,
      history: w.entries.slice(),
    };
    for (const l of [...this.listeners]) l();
  }
}

// ---- Facade ----------------------------------------------------------------

export interface StoresOptions {
  ringCapacity?: number;
  /** Called on every hello, for the build ID check. */
  onHello?: (hello: HelloMessage) => void;
}

export interface Stores {
  readonly frame: FrameStore;
  readonly slow: SlowStore;
  /** Route one WebSocket message: text is JSON, an ArrayBuffer is a Frame. */
  ingest(data: string | ArrayBuffer): void;
  setConnection(status: ConnectionStatus): void;
}

/**
 * Wires both stores to the epoch rules: a screen drops any message from an older epoch,
 * and after a hello (or epoch bump) ignores everything until the snapshot arrives.
 */
export function createStores(options: StoresOptions = {}): Stores {
  const frame = new FrameStore(options.ringCapacity ?? DEFAULT_RING_CAPACITY);
  const slow = new SlowStore();
  let epoch: number | null = null;
  let awaitingSnapshot = true;

  const current = (messageEpoch: number): boolean => !awaitingSnapshot && messageEpoch === epoch;

  function ingestText(text: string): void {
    const m = parseTextMessage(text);
    if (!m) return;
    switch (m.type) {
      case 'hello':
        epoch = m.epoch;
        awaitingSnapshot = true;
        slow.begin(m.epoch, m.source);
        options.onHello?.(m);
        break;
      case 'snapshot':
        if (epoch !== null && m.epoch < epoch) return;
        epoch = m.epoch;
        awaitingSnapshot = false;
        frame.reset();
        slow.install(m.epoch, m.source, m.state, m.history);
        break;
      case 'state':
        if (current(m.epoch)) slow.setState(m.state);
        break;
      case 'history':
        if (current(m.epoch)) slow.appendHistory(m.entries);
        break;
    }
  }

  return {
    frame,
    slow,
    ingest(data) {
      if (typeof data === 'string') {
        ingestText(data);
        return;
      }
      const e = frameEpoch(data);
      if (e !== null && current(e)) frame.accept(data);
    },
    setConnection: (status) => slow.setConnection(status),
  };
}
