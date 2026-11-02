import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';
import { buildFrame, hello, historyMsg, snapshot, stateMsg } from './fixtures';
import { createStores, type Stores } from './stores';

let s: Stores;
beforeEach(() => {
  vi.useFakeTimers();
  s = createStores({ ringCapacity: 4 });
});
afterEach(() => vi.useRealTimers());

const live = (epoch = 1) => {
  s.ingest(hello(epoch));
  s.ingest(snapshot(epoch, { lifecycle: 'live' }));
};

describe('epoch handling', () => {
  test('snapshot installs state and history and marks the stores ready', () => {
    s.ingest(hello(2, 'build-x', 'replay'));
    s.ingest(snapshot(2, { lifecycle: 'live' }, [{ kind: 'lap', id: 'a' }], 'replay'));
    vi.advanceTimersByTime(200);
    const snap = s.slow.getSnapshot();
    expect(snap.epoch).toBe(2);
    expect(snap.source).toBe('replay');
    expect(snap.state).toEqual({ lifecycle: 'live' });
    expect(snap.history).toEqual([{ kind: 'lap', id: 'a' }]);
  });

  test('messages from an older epoch are dropped', () => {
    live(5);
    const v = s.frame.version;
    s.ingest(stateMsg(4, { lifecycle: 'lost' }));
    s.ingest(historyMsg(4, [{ kind: 'lap', id: 'old' }]));
    s.ingest(buildFrame({ epoch: 4, seq: 1, sessionTime: 1, cars: { 0: { speed: 99, readout: 0 } } }));
    vi.advanceTimersByTime(200);
    expect(s.slow.getSnapshot().state).toEqual({ lifecycle: 'live' });
    expect(s.slow.getSnapshot().history).toEqual([]);
    expect(s.frame.version).toBe(v);
    expect(s.frame.data.speed[0]).toBe(0);
  });

  test('current-epoch frame, state and history are applied', () => {
    live(5);
    const v = s.frame.version;
    s.ingest(stateMsg(5, { lifecycle: 'ended', packetRate: 60 }));
    s.ingest(historyMsg(5, [{ kind: 'lap', id: 'l1' }]));
    s.ingest(buildFrame({ epoch: 5, seq: 1, sessionTime: 1, cars: { 0: { speed: 250, readout: 0 } } }));
    vi.advanceTimersByTime(200);
    expect(s.slow.getSnapshot().state).toEqual({ lifecycle: 'ended', packetRate: 60 });
    expect(s.slow.getSnapshot().history).toEqual([{ kind: 'lap', id: 'l1' }]);
    expect(s.frame.data.speed[0]).toBe(250);
    expect(s.frame.version).toBe(v + 1);
  });

  test('a new epoch discards everything; frames wait for the snapshot', () => {
    live(1);
    s.ingest(historyMsg(1, [{ kind: 'lap', id: 'l1' }]));
    s.ingest(buildFrame({ epoch: 1, seq: 1, sessionTime: 1, cars: { 0: { speed: 250, readout: 0 } } }));
    const v = s.frame.version;
    s.ingest(buildFrame({ epoch: 2, seq: 1, sessionTime: 0, cars: { 0: { speed: 10, readout: 0 } } }));
    expect(s.frame.version).toBe(v); // newer epoch, snapshot not yet seen: ignored
    s.ingest(snapshot(2, { lifecycle: 'waiting' }));
    vi.advanceTimersByTime(200);
    expect(s.slow.getSnapshot().history).toEqual([]);
    expect(s.slow.getSnapshot().state).toEqual({ lifecycle: 'waiting' });
    expect(s.frame.data.speed[0]).toBe(0);
    expect(s.frame.version).toBeGreaterThan(v);
    s.ingest(buildFrame({ epoch: 2, seq: 2, sessionTime: 1, cars: { 0: { speed: 30, readout: 0 } } }));
    expect(s.frame.data.speed[0]).toBe(30);
  });

  test('a snapshot older than the current epoch is dropped', () => {
    live(5);
    s.ingest(snapshot(3, { lifecycle: 'waiting' }));
    vi.advanceTimersByTime(200);
    expect(s.slow.getSnapshot().epoch).toBe(5);
    expect(s.slow.getSnapshot().state).toEqual({ lifecycle: 'live' });
  });

  test('after hello, state and frames hold until the snapshot lands', () => {
    live(1);
    s.ingest(hello(1));
    s.ingest(stateMsg(1, { lifecycle: 'lost' }));
    vi.advanceTimersByTime(200);
    expect(s.slow.getSnapshot().state).toEqual({ lifecycle: 'live' });
  });
});

describe('History ids', () => {
  test('an appended entry with a known id replaces it in place', () => {
    live(1);
    s.ingest(historyMsg(1, [{ kind: 'lap', id: 'a', ms: 1 }, { kind: 'lap', id: 'b', ms: 2 }]));
    s.ingest(historyMsg(1, [{ kind: 'lap', id: 'a', ms: 99 }, { kind: 'lap', id: 'c', ms: 3 }]));
    vi.advanceTimersByTime(200);
    expect(s.slow.getSnapshot().history).toEqual([
      { kind: 'lap', id: 'a', ms: 99 },
      { kind: 'lap', id: 'b', ms: 2 },
      { kind: 'lap', id: 'c', ms: 3 },
    ]);
  });

  test('snapshot history with duplicate ids keeps the last', () => {
    s.ingest(hello(1));
    s.ingest(snapshot(1, {}, [{ kind: 'lap', id: 'a', ms: 1 }, { kind: 'lap', id: 'a', ms: 2 }]));
    vi.advanceTimersByTime(200);
    expect(s.slow.getSnapshot().history).toEqual([{ kind: 'lap', id: 'a', ms: 2 }]);
  });
});

describe('slow path notifications', () => {
  test('are coalesced to at most 10 Hz and snapshot identity is stable between changes', () => {
    live(1);
    vi.advanceTimersByTime(200);
    const listener = vi.fn();
    s.slow.subscribe(listener);
    const before = s.slow.getSnapshot();
    for (let i = 0; i < 10; i++) s.ingest(stateMsg(1, { n: i }));
    vi.advanceTimersByTime(100);
    const after = s.slow.getSnapshot();
    expect(after).not.toBe(before);
    expect(after.state).toEqual({ n: 9 });
    expect(listener.mock.calls.length).toBeLessThanOrEqual(2);
    expect(s.slow.getSnapshot()).toBe(after);
    vi.advanceTimersByTime(1000);
    expect(s.slow.getSnapshot()).toBe(after);
  });

  test('unsubscribe stops notifications', () => {
    live(1);
    vi.advanceTimersByTime(200);
    const listener = vi.fn();
    const off = s.slow.subscribe(listener);
    off();
    s.ingest(stateMsg(1, { n: 1 }));
    vi.advanceTimersByTime(500);
    expect(listener).not.toHaveBeenCalled();
  });

  test('connection status changes reach the snapshot', () => {
    s.setConnection('reconnecting');
    vi.advanceTimersByTime(200);
    expect(s.slow.getSnapshot().connection).toBe('reconnecting');
  });
});

describe('frame store and rings', () => {
  test('bumps version per accepted frame and ignores wrong sizes', () => {
    live(1);
    const v = s.frame.version;
    s.ingest(buildFrame({ epoch: 1, seq: 1, sessionTime: 1 }));
    s.ingest(buildFrame({ epoch: 1, seq: 2, sessionTime: 2 }));
    s.ingest(new ArrayBuffer(12));
    expect(s.frame.version).toBe(v + 2);
  });

  test('rings hold recent samples for present cars only', () => {
    live(1);
    for (let i = 1; i <= 6; i++) {
      s.ingest(
        buildFrame({
          epoch: 1,
          seq: i,
          sessionTime: i,
          cars: { 2: { speed: i * 10, throttle: i, brake: 0, lapDistance: i * 100, readout: 0 } },
        }),
      );
    }
    const car = s.frame.rings[2]!;
    expect(car.speed.toArray()).toEqual([30, 40, 50, 60]); // capacity 4
    expect(car.sessionTime.toArray()).toEqual([3, 4, 5, 6]);
    expect(s.frame.rings[7]!.speed.length).toBe(0); // absent slot
  });

  test('rings reset with the epoch', () => {
    live(1);
    s.ingest(buildFrame({ epoch: 1, seq: 1, sessionTime: 1, cars: { 0: { speed: 5, readout: 0 } } }));
    s.ingest(hello(2));
    s.ingest(snapshot(2));
    expect(s.frame.rings[0]!.speed.length).toBe(0);
  });
});

describe('hello listener', () => {
  test('is told the build ID on every hello', () => {
    const onHello = vi.fn();
    const stores = createStores({ onHello });
    stores.ingest(hello(1, 'zzz'));
    expect(onHello).toHaveBeenCalledWith({ type: 'hello', buildId: 'zzz', epoch: 1, source: 'live' });
  });
});

// Assumed mixed availability: motion exists while telemetry/lap placeholders are zero.
test('rings preserve aligned gaps instead of treating absent or restricted values as zero', () => {
  live();
  s.ingest(buildFrame({ epoch: 1, seq: 1, sessionTime: 1,
    cars: { 0: { readout: 0x3fcf } } })); // telemetry absent, motion absent, lap live
  const rings = s.frame.rings[0]!;
  expect(rings.speed.last()).toBeNaN();
  expect(rings.throttle.last()).toBeNaN();
  expect(rings.lapDistance.last()).toBe(0);
  expect(rings.sessionTime.last()).toBe(1);
});

test('same-epoch history correction preserves live frame and input rings', () => {
  live(1);
  s.ingest(buildFrame({ epoch: 1, seq: 1, sessionTime: 10, cars: { 0: { throttle: 70, readout: 0 } } }));
  const version = s.frame.version;
  s.ingest(snapshot(1, { lifecycle: 'live' }, []));
  expect(s.frame.version).toBe(version);
  expect(s.frame.data.throttle[0]).toBe(70);
  expect(s.frame.rings[0]!.sessionTime.length).toBe(1);
});

test('history removal delta deletes old entries without clearing input samples', () => {
  live(1);
  s.ingest(historyMsg(1, [{ kind: 'lap', id: 'old' }, { kind: 'lap', id: 'keep' }]));
  s.ingest(buildFrame({ epoch: 1, seq: 1, sessionTime: 10, cars: { 0: { throttle: 70, readout: 0 } } }));
  s.ingest(JSON.stringify({ type: 'history', epoch: 1, entries: [], removed: ['old'] }));
  vi.advanceTimersByTime(200);
  expect(s.slow.getSnapshot().history.map(entry => entry.id)).toEqual(['keep']);
  expect(s.frame.rings[0]!.sessionTime.length).toBe(1);
  s.ingest(historyMsg(1, [{ kind: 'lap', id: 'keep', car: 2 }]));
  vi.advanceTimersByTime(200);
  expect(s.slow.getSnapshot().history).toHaveLength(1);
  expect(s.slow.getSnapshot().history[0]!.car).toBe(2);
});
