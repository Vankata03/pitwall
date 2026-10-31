/** Generated collector contracts adapted to the runtime's stable store interface. */
import { Frame, FRAME_SIZE } from '../generated/wire/frame';
import type { TextMessage as GeneratedTextMessage } from '../generated/wire/TextMessage';
import type { Lifecycle as WireLifecycle } from '../generated/wire/Lifecycle';
export type { Trace } from '../generated/wire/Trace';
export type { Readout } from '../generated/wire/Readout';

// ---- Frame -----------------------------------------------------------------

export const CAR_COUNT = 24;
export const FRAME_HEADER_BYTES = 16;
export const FRAME_CAR_BYTES = 40;
export const FRAME_BYTES = FRAME_SIZE;

/** `gLateral` and `gLongitudinal` are sent as G x 1000. */
export const G_SCALE = 1000;

/** Mutable store arrays; transport field names come from the generated decoder. */
export type FrameData = ReturnType<typeof createFrameData>;
export type FrameField = Exclude<keyof Frame, 'view' | 'epoch' | 'seq' | 'sessionTime'>;

export function createFrameData() {
  return {
    epoch: 0,
    seq: 0,
    sessionTime: 0,
    speed: new Uint16Array(CAR_COUNT),
    gear: new Int8Array(CAR_COUNT),
    throttle: new Uint8Array(CAR_COUNT),
    brake: new Uint8Array(CAR_COUNT),
    steering: new Int8Array(CAR_COUNT),
    rpm: new Uint16Array(CAR_COUNT),
    worldX: new Float32Array(CAR_COUNT),
    worldZ: new Float32Array(CAR_COUNT),
    lapDistance: new Float32Array(CAR_COUNT),
    currentLapTime: new Uint32Array(CAR_COUNT),
    deltaBest: new Int32Array(CAR_COUNT),
    deltaPrevious: new Int32Array(CAR_COUNT),
    gLateral: new Int16Array(CAR_COUNT),
    gLongitudinal: new Int16Array(CAR_COUNT),
    // Until the first Frame every slot reads absent in every group.
    readout: new Uint16Array(CAR_COUNT).fill(0x3fff),
    ersPercent: new Uint8Array(CAR_COUNT),
    aeroState: new Uint8Array(CAR_COUNT),
  };
}

/** Epoch of a Frame, or null if the buffer is not a Frame. Cheap pre-check before decoding. */
export function frameEpoch(buf: ArrayBuffer): number | null {
  if (buf.byteLength !== FRAME_BYTES) return null;
  return new Frame(buf).epoch;
}

/** Decode a Frame into `out`, little-endian. Returns false, leaving `out` untouched, on a wrong size. */
export function decodeFrameInto(buf: ArrayBuffer, out: FrameData): boolean {
  if (buf.byteLength !== FRAME_BYTES) return false;
  const frame = new Frame(buf);
  out.epoch = frame.epoch;
  out.seq = frame.seq;
  out.sessionTime = frame.sessionTime;
  for (let i = 0; i < CAR_COUNT; i++) {
    for (const field of FRAME_FIELDS) {
      out[field][i] = frame[field](i);
    }
  }
  return true;
}

// ---- Readout states --------------------------------------------------------

export const ReadoutState = {
  Live: 0,
  Stale: 1,
  Restricted: 2,
  Absent: 3,
} as const;
export type ReadoutState = (typeof ReadoutState)[keyof typeof ReadoutState];

/** Readout groups in bit order from bit 0, two bits each. Bits 14 and 15 are reserved. */
export const READOUT_GROUPS = [
  'telemetry',
  'motion',
  'lap',
  'status',
  'deltaBest',
  'deltaPrevious',
  'aero',
] as const;
export type ReadoutGroup = (typeof READOUT_GROUPS)[number];

/** State of group number `group` (index into READOUT_GROUPS) in a `readout` word. */
export function readoutGroupState(readout: number, group: number): ReadoutState {
  return ((readout >> (group * 2)) & 3) as ReadoutState;
}

/** Which readout group governs each Frame field. */
export const FRAME_FIELD_GROUP: Record<FrameField, ReadoutGroup> = {
  speed: 'telemetry',
  gear: 'telemetry',
  throttle: 'telemetry',
  brake: 'telemetry',
  steering: 'telemetry',
  rpm: 'telemetry',
  worldX: 'motion',
  worldZ: 'motion',
  gLateral: 'motion',
  gLongitudinal: 'motion',
  lapDistance: 'lap',
  currentLapTime: 'lap',
  // Collector frame::encode packs ers_percentage(), not the raw status readout.
  ersPercent: 'status',
  deltaBest: 'deltaBest',
  deltaPrevious: 'deltaPrevious',
  aeroState: 'aero',
  readout: 'telemetry', // the word itself; never rendered as a readout
};

const FRAME_FIELDS = Object.keys(FRAME_FIELD_GROUP) as FrameField[];

// ---- Text messages ---------------------------------------------------------

export type Source = Extract<GeneratedTextMessage, { type: 'hello' }>['source'];
/** Presentation vocabulary retained for existing banner and tile callers. */
export type Lifecycle = Lowercase<WireLifecycle>;
export type WireState = Extract<GeneratedTextMessage, { type: 'snapshot' }>['state'];
export type WireHistoryEntry = Extract<GeneratedTextMessage, { type: 'history' }>['entries'][number];
export type HelloMessage = Extract<GeneratedTextMessage, { type: 'hello' }>;
export type SnapshotMessage = Extract<GeneratedTextMessage, { type: 'snapshot' }>;
export type StateMessage = { type: 'state'; epoch: number; state: WireState };
export type HistoryMessage = Extract<GeneratedTextMessage, { type: 'history' }>;
export type TextMessage = HelloMessage | SnapshotMessage | StateMessage | HistoryMessage;

/** Lifecycle from a State, if it says. */
export function lifecycleOf(state: WireState | null): Lifecycle | undefined {
  const l = state?.lifecycle?.toLowerCase();
  return l === 'waiting' || l === 'live' || l === 'ended' || l === 'lost' ? l : undefined;
}

function isObject(x: unknown): x is Record<string, unknown> {
  return typeof x === 'object' && x !== null && !Array.isArray(x);
}
function isSource(x: unknown): x is Source {
  return x === 'live' || x === 'replay';
}
function isEntries(x: unknown): x is WireHistoryEntry[] {
  return Array.isArray(x) && x.every((e) => isObject(e) && typeof e.kind === 'string' && typeof e.id === 'string');
}

/** Parse a text message; null for anything malformed or unknown. Never throws. */
export function parseTextMessage(text: string): TextMessage | null {
  let m: unknown;
  try {
    m = JSON.parse(text);
  } catch {
    return null;
  }
  if (!isObject(m) || typeof m.epoch !== 'number') {
    return null;
  }
  const epoch = m.epoch;
  switch (m.type) {
    case 'hello':
      if (typeof m.buildId !== 'string' || !isSource(m.source)) return null;
      return { type: 'hello', buildId: m.buildId, epoch, source: m.source };
    case 'snapshot':
      if (!isSource(m.source) || !isObject(m.state) || !isEntries(m.history)) return null;
      return { type: 'snapshot', epoch, source: m.source, state: m.state as WireState, history: m.history };
    case 'state': {
      const { type: _type, epoch: _epoch, ...state } = m;
      return { type: 'state', epoch, state: state as WireState };
    }
    case 'history':
      if (!isEntries(m.entries)) return null;
      return { type: 'history', epoch, entries: m.entries };
    default:
      return null;
  }
}
