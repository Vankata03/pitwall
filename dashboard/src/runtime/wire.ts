/**
 * Wire boundary: decoded Frame, State and History messages (spec section 8).
 *
 * This is the ONLY file in the dashboard that knows byte offsets or JSON message
 * shapes. Ticket #35 generates the Frame decoder and the ts-rs JSON types from the
 * Rust field table; when it lands, replace this file with a thin module that
 * re-exports the generated code under the same names. Everything else imports
 * from './wire' alone.
 *
 * Assumptions to confirm against the generated types when swapping:
 *  - the snapshot's `history` is an array of entries, each with `kind` and `id`;
 *  - State carries a `lifecycle` field (`waiting` | `live` | `ended` | `lost`);
 *  - `FRAME_FIELD_GROUP` (which readout group covers which Frame field).
 */

// ---- Frame -----------------------------------------------------------------

export const CAR_COUNT = 24;
export const FRAME_HEADER_BYTES = 16;
export const FRAME_CAR_BYTES = 40;
export const FRAME_BYTES = FRAME_HEADER_BYTES + CAR_COUNT * FRAME_CAR_BYTES; // 976

/** `gLateral` and `gLongitudinal` are sent as G x 1000. */
export const G_SCALE = 1000;

/**
 * Struct-of-arrays view of the latest Frame: one typed array per field, 24 entries
 * each, reused for every message so decoding allocates nothing per car.
 */
export interface FrameData {
  epoch: number;
  seq: number;
  sessionTime: number;
  speed: Uint16Array;
  gear: Int8Array;
  throttle: Uint8Array;
  brake: Uint8Array;
  steering: Int8Array;
  rpm: Uint16Array;
  worldX: Float32Array;
  worldZ: Float32Array;
  lapDistance: Float32Array;
  currentLapTime: Uint32Array;
  deltaBest: Int32Array;
  deltaPrevious: Int32Array;
  gLateral: Int16Array;
  gLongitudinal: Int16Array;
  ersPercent: Uint8Array;
  aeroState: Uint8Array;
  readout: Uint16Array;
}

/** Names of the per-car Frame fields (everything in FrameData except the header). */
export type FrameField = Exclude<keyof FrameData, 'epoch' | 'seq' | 'sessionTime'>;

export function createFrameData(): FrameData {
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
  return new DataView(buf).getUint32(0, true);
}

/** Decode a Frame into `out`, little-endian. Returns false, leaving `out` untouched, on a wrong size. */
export function decodeFrameInto(buf: ArrayBuffer, out: FrameData): boolean {
  if (buf.byteLength !== FRAME_BYTES) return false;
  const v = new DataView(buf);
  out.epoch = v.getUint32(0, true);
  out.seq = v.getUint32(4, true);
  out.sessionTime = v.getFloat64(8, true);
  for (let i = 0; i < CAR_COUNT; i++) {
    const o = FRAME_HEADER_BYTES + i * FRAME_CAR_BYTES;
    out.speed[i] = v.getUint16(o, true);
    out.gear[i] = v.getInt8(o + 2);
    out.throttle[i] = v.getUint8(o + 3);
    out.brake[i] = v.getUint8(o + 4);
    out.steering[i] = v.getInt8(o + 5);
    out.rpm[i] = v.getUint16(o + 6, true);
    out.worldX[i] = v.getFloat32(o + 8, true);
    out.worldZ[i] = v.getFloat32(o + 12, true);
    out.lapDistance[i] = v.getFloat32(o + 16, true);
    out.currentLapTime[i] = v.getUint32(o + 20, true);
    out.deltaBest[i] = v.getInt32(o + 24, true);
    out.deltaPrevious[i] = v.getInt32(o + 28, true);
    out.gLateral[i] = v.getInt16(o + 32, true);
    out.gLongitudinal[i] = v.getInt16(o + 34, true);
    out.ersPercent[i] = v.getUint8(o + 36);
    out.aeroState[i] = v.getUint8(o + 37);
    out.readout[i] = v.getUint16(o + 38, true);
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
  ersPercent: 'status',
  deltaBest: 'deltaBest',
  deltaPrevious: 'deltaPrevious',
  aeroState: 'aero',
  readout: 'telemetry', // the word itself; never rendered as a readout
};

// ---- Text messages ---------------------------------------------------------

export type Source = 'live' | 'replay';
export type Lifecycle = 'waiting' | 'live' | 'ended' | 'lost';

/** Bounded current values for session, cars and collector; shape owned by the generated types. */
export type WireState = Record<string, unknown>;

/** A History entry: stable `id`, replaces any earlier entry with the same id. */
export interface WireHistoryEntry {
  kind: string;
  id: string;
  [field: string]: unknown;
}

export interface HelloMessage {
  type: 'hello';
  buildId: string;
  epoch: number;
  source: Source;
}
export interface SnapshotMessage {
  type: 'snapshot';
  epoch: number;
  source: Source;
  state: WireState;
  history: WireHistoryEntry[];
}
export interface StateMessage {
  type: 'state';
  epoch: number;
  state: WireState;
}
export interface HistoryMessage {
  type: 'history';
  epoch: number;
  entries: WireHistoryEntry[];
}
export type TextMessage = HelloMessage | SnapshotMessage | StateMessage | HistoryMessage;

/** Lifecycle from a State, if it says. */
export function lifecycleOf(state: WireState | null): Lifecycle | undefined {
  const l = state?.lifecycle;
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
      return { type: 'snapshot', epoch, source: m.source, state: m.state, history: m.history };
    case 'state': {
      const { type: _type, epoch: _epoch, ...state } = m;
      return { type: 'state', epoch, state };
    }
    case 'history':
      if (!isEntries(m.entries)) return null;
      return { type: 'history', epoch, entries: m.entries };
    default:
      return null;
  }
}
