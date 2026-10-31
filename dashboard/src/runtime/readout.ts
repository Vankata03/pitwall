import {
  FRAME_FIELD_GROUP,
  READOUT_GROUPS,
  ReadoutState,
  readoutGroupState,
  type FrameData,
  type FrameField,
  type Lifecycle,
} from './wire';

export const RESTRICTED_TEXT = 'Restricted';
export const ABSENT_TEXT = '—';

export type ReadoutStateName = 'live' | 'stale' | 'restricted' | 'absent';
const NAMES: readonly ReadoutStateName[] = ['live', 'stale', 'restricted', 'absent'];

export interface ReadoutView {
  /** Drives styling: `[data-state="stale"]` dims the value and adds the marker. */
  state: ReadoutStateName;
  text: string;
}

/**
 * The single rendering rule for a readout (spec section 10, Missing data). `valueText` is
 * the tile's formatted value; it is shown for live and stale only, so zero never stands in
 * for a missing value.
 */
export function readoutView(state: ReadoutState, valueText: string): ReadoutView {
  switch (state) {
    case ReadoutState.Live:
      return { state: 'live', text: valueText };
    case ReadoutState.Stale:
      return { state: 'stale', text: valueText };
    case ReadoutState.Restricted:
      return { state: 'restricted', text: RESTRICTED_TEXT };
    default:
      return { state: 'absent', text: ABSENT_TEXT };
  }
}

/** The part of an HTMLElement `writeReadout` touches. */
export interface ReadoutElement {
  textContent: string | null;
  dataset: DOMStringMap | Record<string, string | undefined>;
}

/** Write a readout straight into the DOM, touching it only when something changed. */
export function writeReadout(el: ReadoutElement, state: ReadoutState, valueText: string): void {
  const v = readoutView(state, valueText);
  if (el.dataset.state !== v.state) el.dataset.state = v.state;
  if (el.textContent !== v.text) el.textContent = v.text;
}

/** While the collector is unreachable every live value is only the last one seen. */
export function effectiveState(state: ReadoutState, connected: boolean): ReadoutState {
  return !connected && state === ReadoutState.Live ? ReadoutState.Stale : state;
}

/** A stale value still has data to show; restricted and absent do not. */
export function hasAnyData(states: Iterable<ReadoutState>): boolean {
  for (const s of states) if (s === ReadoutState.Live || s === ReadoutState.Stale) return true;
  return false;
}

export interface ConnectionContext {
  lifecycle: Lifecycle | undefined;
  connected: boolean;
}

/** Whole-tile message, or null while any readout has data. */
export function wholeTileMessage(states: Iterable<ReadoutState>, ctx: ConnectionContext): string | null {
  if (hasAnyData(states)) return null;
  if (!ctx.connected) return 'Collector unreachable';
  if (ctx.lifecycle === 'lost') return 'No data: game paused or closed';
  if (ctx.lifecycle === 'waiting') return 'Waiting for the game';
  return 'No data';
}

const GROUP_INDEX: Record<string, number> = Object.fromEntries(READOUT_GROUPS.map((g, i) => [g, i]));

/** Readout state of one Frame field of one car. */
export function frameReadoutState(frame: FrameData, car: number, field: FrameField): ReadoutState {
  return readoutGroupState(frame.readout[car] ?? 0x3fff, GROUP_INDEX[FRAME_FIELD_GROUP[field]]!);
}

export function readoutStateName(state: ReadoutState): ReadoutStateName {
  return NAMES[state]!;
}
