import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';
import { buildFrame, hello, snapshot, stateMsg } from './fixtures';
import { FrameRateMonitor, RenderLoop, type DrawContext } from './loop';
import { createStores, type Stores } from './stores';
import { defineTile, type TileDeclaration } from './tile';

const decl = (over: Partial<TileDeclaration> = {}) =>
  defineTile({
    type: 't',
    name: 'T',
    binding: 'follows-car',
    rate: 'frame',
    minSize: { w: 1, h: 1 },
    defaultSize: { w: 2, h: 2 },
    densities: [
      { density: 'compact', minWidth: 0, minHeight: 0 },
      { density: 'regular', minWidth: 100, minHeight: 100 },
    ],
    reads: [{ source: 'frame', fields: ['speed'] }],
    ...over,
  });

let stores: Stores;
let pending: ((ts: number) => void)[];
let loop: RenderLoop;
const tick = (ts: number) => {
  const cbs = pending;
  pending = [];
  cbs.forEach((cb) => cb(ts));
};
const frame = (seq: number, speed = 100) =>
  stores.ingest(buildFrame({ epoch: 1, seq, sessionTime: seq, cars: { 0: { speed, readout: 0 } } }));

beforeEach(() => {
  vi.useFakeTimers();
  stores = createStores();
  stores.ingest(hello(1));
  stores.ingest(snapshot(1, { lifecycle: 'live' }));
  pending = [];
  loop = new RenderLoop({
    stores,
    requestFrame: (cb) => {
      pending.push(cb);
      return pending.length;
    },
    cancelFrame: () => {
      pending = [];
    },
  });
});

afterEach(() => vi.useRealTimers());

describe('RenderLoop', () => {
  test('uses one rAF chain no matter how many tiles', () => {
    loop.register({ declaration: decl(), car: () => 0, draw: () => {} });
    loop.register({ declaration: decl(), car: () => 0, draw: () => {} });
    loop.start();
    loop.start();
    expect(pending).toHaveLength(1);
    tick(16);
    expect(pending).toHaveLength(1);
  });

  test('draws a new tile once, then only when its data changed', () => {
    const draw = vi.fn();
    loop.register({ declaration: decl(), car: () => 0, draw });
    loop.start();
    tick(16);
    expect(draw).toHaveBeenCalledTimes(1);
    tick(33);
    tick(50);
    expect(draw).toHaveBeenCalledTimes(1);
    frame(1);
    tick(66);
    expect(draw).toHaveBeenCalledTimes(2);
    tick(83);
    expect(draw).toHaveBeenCalledTimes(2);
  });

  test('passes the frame timestamp, latest frame data, car and rings', () => {
    let seen: DrawContext | undefined;
    loop.register({ declaration: decl(), car: () => 0, draw: (c) => (seen = c) });
    loop.start();
    frame(1, 250);
    tick(1234);
    expect(seen!.ts).toBe(1234);
    expect(seen!.car).toBe(0);
    expect(seen!.frame.speed[0]).toBe(250);
    expect(seen!.rings!.speed.last()).toBe(250);
  });

  test('draws only the latest value after several frames between paints', () => {
    const speeds: number[] = [];
    loop.register({ declaration: decl(), car: () => 0, draw: (c) => speeds.push(c.frame.speed[0]!) });
    loop.start();
    tick(1);
    frame(1, 10);
    frame(2, 20);
    frame(3, 30);
    tick(17);
    expect(speeds).toEqual([0, 30]);
  });

  test('a tile that does not read frames ignores them, but redraws on State', () => {
    const draw = vi.fn();
    loop.register({
      declaration: decl({ reads: [{ source: 'frame', fields: ['speed'] }, { source: 'state', keys: ['x'] }] }),
      car: () => 0,
      draw,
    });
    const stateOnly = vi.fn();
    loop.register({
      declaration: decl({ rate: 'frame', reads: [{ source: 'frame', fields: ['rpm'] }] }),
      car: () => 0,
      draw: stateOnly,
    });
    loop.start();
    tick(1);
    stores.ingest(stateMsg(1, { x: 1 }));
    vi.advanceTimersByTime(200);
    tick(17);
    expect(draw).toHaveBeenCalledTimes(2);
    expect(stateOnly).toHaveBeenCalledTimes(1);
  });

  test('every frame tile redraws when the connection drops or returns, with ctx.connected', () => {
    stores.setConnection('open');
    vi.advanceTimersByTime(200);
    const seen: boolean[] = [];
    loop.register({ declaration: decl(), car: () => 0, draw: (c) => seen.push(c.connected) });
    loop.start();
    tick(1);
    stores.setConnection('reconnecting');
    vi.advanceTimersByTime(200);
    tick(17);
    tick(33);
    stores.setConnection('open');
    vi.advanceTimersByTime(200);
    tick(50);
    expect(seen).toEqual([true, false, true]);
  });

  test('invalidate forces one redraw (resize, car change)', () => {
    const draw = vi.fn();
    const h = loop.register({ declaration: decl(), car: () => 0, draw });
    loop.start();
    tick(1);
    h.invalidate();
    tick(17);
    tick(33);
    expect(draw).toHaveBeenCalledTimes(2);
  });

  test('unregister stops drawing', () => {
    const draw = vi.fn();
    const h = loop.register({ declaration: decl(), car: () => 0, draw });
    loop.start();
    tick(1);
    h.unregister();
    frame(1);
    tick(17);
    expect(draw).toHaveBeenCalledTimes(1);
  });

  test('a tile with no car (session-wide or unresolved) still draws, with car null', () => {
    let car: number | null | undefined;
    loop.register({
      declaration: decl({ binding: 'session-wide' }),
      car: () => null,
      draw: (c) => (car = c.car),
    });
    loop.start();
    tick(1);
    expect(car).toBeNull();
  });

  test('a throwing tile is parked and does not stop the loop or its neighbours', () => {
    const err = vi.spyOn(console, 'error').mockImplementation(() => {});
    const bad = loop.register({
      declaration: decl(),
      car: () => 0,
      draw: () => {
        throw new Error('boom');
      },
    });
    const good = vi.fn();
    loop.register({ declaration: decl(), car: () => 0, draw: good });
    loop.start();
    tick(1);
    frame(1);
    tick(17);
    expect(good).toHaveBeenCalledTimes(2);
    expect(err).toHaveBeenCalledTimes(1);
    expect(bad.error()).toBeInstanceOf(Error);
    err.mockRestore();
  });

  test('refuses slow tiles, which belong to React', () => {
    expect(() => loop.register({ declaration: decl({ rate: 'slow' }), car: () => 0, draw: () => {} })).toThrow(
      /slow/,
    );
  });

  test('stop cancels the pending frame', () => {
    loop.start();
    loop.stop();
    expect(pending).toHaveLength(0);
  });
});

describe('FrameRateMonitor: slow-draw notice under 24 fps for 5 s', () => {
  const run = (m: FrameRateMonitor, fps: number, fromMs: number, seconds: number) => {
    const step = 1000 / fps;
    let t = fromMs;
    for (; t < fromMs + seconds * 1000; t += step) m.sample(t);
    return t;
  };

  test('stays quiet at 60 fps', () => {
    const m = new FrameRateMonitor();
    run(m, 60, 0, 20);
    expect(m.slow).toBe(false);
  });

  test('raises only after 5 s continuously under 24 fps', () => {
    const m = new FrameRateMonitor();
    let t = run(m, 20, 0, 4);
    expect(m.slow).toBe(false);
    t = run(m, 20, t, 2);
    expect(m.slow).toBe(true);
  });

  test('a brief dip does not trigger it', () => {
    const m = new FrameRateMonitor();
    let t = run(m, 60, 0, 3);
    t = run(m, 15, t, 3);
    t = run(m, 60, t, 3);
    t = run(m, 15, t, 3);
    expect(m.slow).toBe(false);
  });

  test('clears when the rate recovers', () => {
    const m = new FrameRateMonitor();
    let t = run(m, 20, 0, 7);
    expect(m.slow).toBe(true);
    t = run(m, 60, t, 3);
    expect(m.slow).toBe(false);
  });

  test('notifies subscribers only when the flag flips', () => {
    const m = new FrameRateMonitor();
    const l = vi.fn();
    m.subscribe(l);
    const t = run(m, 20, 0, 8);
    expect(l).toHaveBeenCalledTimes(1);
    run(m, 60, t, 3);
    expect(l).toHaveBeenCalledTimes(2);
  });

  test('reset forgets a stall such as a hidden tab', () => {
    const m = new FrameRateMonitor();
    run(m, 60, 0, 2);
    m.reset();
    m.sample(60000); // first frame after the tab came back
    run(m, 60, 60016, 2);
    expect(m.slow).toBe(false);
  });

  test('the loop feeds it and exposes the flag', () => {
    loop.start();
    let t = 0;
    for (let i = 0; i < 6 * 20; i++) tick((t += 50));
    expect(loop.monitor.slow).toBe(true);
  });
});
