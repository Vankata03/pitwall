import { describe, expect, test } from 'vitest';
import {
  CAR_COUNT,
  FRAME_BYTES,
  READOUT_GROUPS,
  ReadoutState,
  createFrameData,
  decodeFrameInto,
  frameEpoch,
  lifecycleOf,
  parseTextMessage,
  readoutGroupState,
} from './wire';
import { buildFrame } from './fixtures';

describe('Frame decoder', () => {
  test('layout constants match the spec', () => {
    expect(FRAME_BYTES).toBe(976);
    expect(CAR_COUNT).toBe(24);
  });

  test('decodes header and every field of a car entry', () => {
    const buf = buildFrame({
      epoch: 7,
      seq: 123456,
      sessionTime: 301.25,
      cars: {
        3: {
          speed: 312,
          gear: -1,
          throttle: 87,
          brake: 12,
          steering: -100,
          rpm: 11800,
          worldX: -123.5,
          worldZ: 456.25,
          lapDistance: 1999.5,
          currentLapTime: 83456,
          deltaBest: -234,
          deltaPrevious: 1500,
          gLateral: -2750,
          gLongitudinal: 1200,
          ersPercent: 64,
          aeroState: 1,
          readout: 0,
        },
      },
    });
    const out = createFrameData();
    expect(decodeFrameInto(buf, out)).toBe(true);
    expect(out.epoch).toBe(7);
    expect(out.seq).toBe(123456);
    expect(out.sessionTime).toBe(301.25);
    expect(out.speed[3]).toBe(312);
    expect(out.gear[3]).toBe(-1);
    expect(out.throttle[3]).toBe(87);
    expect(out.brake[3]).toBe(12);
    expect(out.steering[3]).toBe(-100);
    expect(out.rpm[3]).toBe(11800);
    expect(out.worldX[3]).toBe(-123.5);
    expect(out.worldZ[3]).toBe(456.25);
    expect(out.lapDistance[3]).toBe(1999.5);
    expect(out.currentLapTime[3]).toBe(83456);
    expect(out.deltaBest[3]).toBe(-234);
    expect(out.deltaPrevious[3]).toBe(1500);
    expect(out.gLateral[3]).toBe(-2750);
    expect(out.gLongitudinal[3]).toBe(1200);
    expect(out.ersPercent[3]).toBe(64);
    expect(out.aeroState[3]).toBe(1);
    expect(out.readout[3]).toBe(0);
  });

  test('entries are 40 bytes apart and do not bleed into neighbours', () => {
    const buf = buildFrame({
      epoch: 1,
      seq: 1,
      sessionTime: 0,
      cars: { 0: { speed: 100, readout: 0 }, 1: { speed: 200, readout: 0 }, 23: { speed: 65535, readout: 0 } },
    });
    const out = createFrameData();
    decodeFrameInto(buf, out);
    expect(out.speed[0]).toBe(100);
    expect(out.speed[1]).toBe(200);
    expect(out.speed[2]).toBe(0);
    expect(out.speed[23]).toBe(65535);
  });

  test('reuses the same typed arrays across frames', () => {
    const out = createFrameData();
    const speedRef = out.speed;
    decodeFrameInto(buildFrame({ epoch: 1, seq: 1, sessionTime: 0, cars: { 0: { speed: 10 } } }), out);
    decodeFrameInto(buildFrame({ epoch: 1, seq: 2, sessionTime: 1, cars: { 0: { speed: 20 } } }), out);
    expect(out.speed).toBe(speedRef);
    expect(out.speed[0]).toBe(20);
    expect(out.seq).toBe(2);
  });

  test('rejects a buffer that is not 976 bytes and leaves the target untouched', () => {
    const out = createFrameData();
    decodeFrameInto(buildFrame({ epoch: 4, seq: 9, sessionTime: 2, cars: { 0: { speed: 55 } } }), out);
    expect(decodeFrameInto(new ArrayBuffer(975), out)).toBe(false);
    expect(decodeFrameInto(new ArrayBuffer(977), out)).toBe(false);
    expect(out.epoch).toBe(4);
    expect(out.speed[0]).toBe(55);
  });

  test('frameEpoch reads the epoch without decoding', () => {
    expect(frameEpoch(buildFrame({ epoch: 42, seq: 0, sessionTime: 0 }))).toBe(42);
    expect(frameEpoch(new ArrayBuffer(10))).toBeNull();
  });

  test('empty car slots read absent in every group', () => {
    const out = createFrameData();
    decodeFrameInto(buildFrame({ epoch: 1, seq: 1, sessionTime: 0 }), out);
    for (let g = 0; g < READOUT_GROUPS.length; g++) {
      expect(readoutGroupState(out.readout[5]!, g)).toBe(ReadoutState.Absent);
    }
  });
});

describe('readout bits', () => {
  test('groups are in spec order from bit 0', () => {
    expect(READOUT_GROUPS).toEqual([
      'telemetry',
      'motion',
      'lap',
      'status',
      'deltaBest',
      'deltaPrevious',
      'aero',
    ]);
  });

  test('two bits per group: 0 live, 1 stale, 2 restricted, 3 absent', () => {
    // telemetry=live(0), motion=stale(1), lap=restricted(2), status=absent(3),
    // deltaBest=live(0), deltaPrevious=stale(1), aero=restricted(2)
    const readout = 0 | (1 << 2) | (2 << 4) | (3 << 6) | (0 << 8) | (1 << 10) | (2 << 12);
    expect(readoutGroupState(readout, 0)).toBe(ReadoutState.Live);
    expect(readoutGroupState(readout, 1)).toBe(ReadoutState.Stale);
    expect(readoutGroupState(readout, 2)).toBe(ReadoutState.Restricted);
    expect(readoutGroupState(readout, 3)).toBe(ReadoutState.Absent);
    expect(readoutGroupState(readout, 4)).toBe(ReadoutState.Live);
    expect(readoutGroupState(readout, 5)).toBe(ReadoutState.Stale);
    expect(readoutGroupState(readout, 6)).toBe(ReadoutState.Restricted);
  });

  test('reserved bits 14 and 15 never leak into a group', () => {
    expect(readoutGroupState(0xc000, 6)).toBe(ReadoutState.Live);
  });
});

describe('text messages', () => {
  test('hello', () => {
    expect(parseTextMessage('{"type":"hello","buildId":"abc","epoch":3,"source":"live"}')).toEqual({
      type: 'hello',
      buildId: 'abc',
      epoch: 3,
      source: 'live',
    });
  });

  test('snapshot carries state and history', () => {
    const m = parseTextMessage(
      JSON.stringify({
        type: 'snapshot',
        epoch: 3,
        source: 'replay',
        state: { lifecycle: 'live' },
        history: [{ kind: 'lap', id: 'lap-1' }],
      }),
    );
    expect(m).toEqual({
      type: 'snapshot',
      epoch: 3,
      source: 'replay',
      state: { lifecycle: 'live' },
      history: [{ kind: 'lap', id: 'lap-1' }],
    });
  });

  test('state fields sit beside epoch on the wire and are split out', () => {
    const m = parseTextMessage('{"type":"state","epoch":3,"lifecycle":"waiting","packetRate":60}');
    expect(m).toEqual({ type: 'state', epoch: 3, state: { lifecycle: 'waiting', packetRate: 60 } });
  });

  test('history', () => {
    const m = parseTextMessage('{"type":"history","epoch":3,"entries":[{"kind":"lap","id":"x"}]}');
    expect(m).toEqual({ type: 'history', epoch: 3, entries: [{ kind: 'lap', id: 'x' }] });
  });

  test('garbage and unknown types are rejected, not thrown', () => {
    expect(parseTextMessage('not json')).toBeNull();
    expect(parseTextMessage('{"type":"nope","epoch":1}')).toBeNull();
    expect(parseTextMessage('{"type":"state"}')).toBeNull();
    expect(parseTextMessage('[]')).toBeNull();
  });
});

// Assumed collector JSON: Rust Lifecycle serializes with PascalCase.
test('collector lifecycle drives the existing banner vocabulary', () => {
  const message = parseTextMessage(JSON.stringify({
    type: 'snapshot', epoch: 1, source: 'live',
    state: { lifecycle: 'Lost' }, history: [],
  }));
  expect(message?.type === 'snapshot' && lifecycleOf(message.state)).toBe('lost');
});
