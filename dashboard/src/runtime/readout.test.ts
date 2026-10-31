import { describe, expect, test } from 'vitest';
import { buildFrame } from './fixtures';
import {
  ABSENT_TEXT,
  jsonReadoutView,
  RESTRICTED_TEXT,
  effectiveState,
  frameReadoutState,
  hasAnyData,
  readoutView,
  wholeTileMessage,
  writeReadout,
} from './readout';
import { ReadoutState, createFrameData, decodeFrameInto } from './wire';

describe('readoutView: one rendering for the four states', () => {
  test('live shows the value', () => {
    expect(readoutView(ReadoutState.Live, '312')).toEqual({ state: 'live', text: '312' });
  });
  test('stale keeps the last value and is flagged for dimming and a marker', () => {
    expect(readoutView(ReadoutState.Stale, '312')).toEqual({ state: 'stale', text: '312' });
  });
  test('restricted shows a label, never the value', () => {
    expect(readoutView(ReadoutState.Restricted, '0')).toEqual({ state: 'restricted', text: RESTRICTED_TEXT });
  });
  test('absent shows an em dash, never zero', () => {
    expect(ABSENT_TEXT).toBe('—');
    expect(readoutView(ReadoutState.Absent, '0')).toEqual({ state: 'absent', text: '—' });
  });
});

describe('writeReadout', () => {
  test('writes text and state attribute to a DOM-like element, nothing else', () => {
    const el = { textContent: '', dataset: {} as Record<string, string> };
    writeReadout(el, ReadoutState.Stale, '88');
    expect(el).toEqual({ textContent: '88', dataset: { state: 'stale' } });
    writeReadout(el, ReadoutState.Absent, '0');
    expect(el).toEqual({ textContent: '—', dataset: { state: 'absent' } });
  });

  test('skips the DOM write when nothing changed', () => {
    let writes = 0;
    const el = {
      _t: '',
      get textContent() {
        return this._t;
      },
      set textContent(v: string) {
        writes++;
        this._t = v;
      },
      dataset: {} as Record<string, string>,
    };
    writeReadout(el, ReadoutState.Live, '5');
    writeReadout(el, ReadoutState.Live, '5');
    expect(writes).toBe(1);
  });
});

describe('connection override', () => {
  test('a live readout reads stale while the collector is unreachable', () => {
    expect(effectiveState(ReadoutState.Live, false)).toBe(ReadoutState.Stale);
    expect(effectiveState(ReadoutState.Live, true)).toBe(ReadoutState.Live);
  });
  test('restricted and absent stay as they are', () => {
    expect(effectiveState(ReadoutState.Restricted, false)).toBe(ReadoutState.Restricted);
    expect(effectiveState(ReadoutState.Absent, false)).toBe(ReadoutState.Absent);
  });
});

describe('whole-tile message', () => {
  test('only when no readout has data', () => {
    expect(hasAnyData([ReadoutState.Absent, ReadoutState.Restricted])).toBe(false);
    expect(hasAnyData([ReadoutState.Absent, ReadoutState.Stale])).toBe(true);
    expect(hasAnyData([ReadoutState.Live])).toBe(true);
    expect(hasAnyData([])).toBe(false);
  });

  test('none while any readout has data', () => {
    expect(wholeTileMessage([ReadoutState.Live], { lifecycle: 'live', connected: true })).toBeNull();
  });

  test('wording follows why there is no data', () => {
    const none = [ReadoutState.Absent];
    expect(wholeTileMessage(none, { lifecycle: 'live', connected: false })).toBe('Collector unreachable');
    expect(wholeTileMessage(none, { lifecycle: 'lost', connected: true })).toBe(
      'No data: game paused or closed',
    );
    expect(wholeTileMessage(none, { lifecycle: 'waiting', connected: true })).toBe('Waiting for the game');
    expect(wholeTileMessage(none, { lifecycle: 'live', connected: true })).toBe('No data');
    expect(wholeTileMessage([ReadoutState.Restricted], { lifecycle: 'live', connected: true })).toBe(
      'No data',
    );
  });
});

describe('frameReadoutState: Frame field to its group', () => {
  test('reads the group that governs the field', () => {
    const f = createFrameData();
    // car 1: telemetry live, motion stale, lap restricted, status absent, deltas live, aero stale
    const readout = 0 | (1 << 2) | (2 << 4) | (3 << 6) | (0 << 8) | (0 << 10) | (1 << 12);
    decodeFrameInto(buildFrame({ epoch: 1, seq: 1, sessionTime: 0, cars: { 1: { readout } } }), f);
    expect(frameReadoutState(f, 1, 'speed')).toBe(ReadoutState.Live);
    expect(frameReadoutState(f, 1, 'worldX')).toBe(ReadoutState.Stale);
    expect(frameReadoutState(f, 1, 'lapDistance')).toBe(ReadoutState.Restricted);
    expect(frameReadoutState(f, 1, 'ersPercent')).toBe(ReadoutState.Absent);
    expect(frameReadoutState(f, 1, 'deltaBest')).toBe(ReadoutState.Live);
    expect(frameReadoutState(f, 1, 'deltaPrevious')).toBe(ReadoutState.Live);
    expect(frameReadoutState(f, 1, 'aeroState')).toBe(ReadoutState.Stale);
  });

  test('an empty slot is absent for every field', () => {
    const f = createFrameData();
    expect(frameReadoutState(f, 20, 'speed')).toBe(ReadoutState.Absent);
  });
});

// Assumed JSON gap values: snake_case members are the generated collector contract.
test('JSON readouts render gap values and never format missing placeholders', () => {
  const format = (g: { to_leader_ms: number }) => `${g.to_leader_ms} ms`;
  expect(jsonReadoutView({ state: 'stale', value: { to_leader_ms: 1234 } }, format))
    .toEqual({ state: 'stale', text: '1234 ms' });
  expect(jsonReadoutView({ state: 'restricted' }, format)).toEqual({ state: 'restricted', text: 'Restricted' });
  expect(jsonReadoutView({ state: 'absent' }, format)).toEqual({ state: 'absent', text: '—' });
});
