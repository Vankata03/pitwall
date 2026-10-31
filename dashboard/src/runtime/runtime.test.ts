import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';
import type { SocketLike } from './client';
import { buildFrame, hello, snapshot } from './fixtures';
import { createRuntime } from './runtime';

class FakeSocket implements SocketLike {
  static all: FakeSocket[] = [];
  binaryType = '';
  onopen: (() => void) | null = null;
  onmessage: ((e: { data: string | ArrayBuffer }) => void) | null = null;
  onclose: (() => void) | null = null;
  onerror: (() => void) | null = null;
  constructor(readonly url: string) {
    FakeSocket.all.push(this);
  }
  close() {}
}

let reload = vi.fn<() => void>();
const mem = new Map<string, string>();
const make = (ownBuildId = 'b1') =>
  createRuntime({
    url: 'ws://h/ws',
    ownBuildId,
    createSocket: (u) => new FakeSocket(u),
    requestFrame: () => 0,
    cancelFrame: () => {},
    reload,
    storage: { getItem: (k) => mem.get(k) ?? null, setItem: (k, v) => void mem.set(k, v) },
  });

beforeEach(() => {
  vi.useFakeTimers();
  FakeSocket.all = [];
  mem.clear();
  reload = vi.fn<() => void>();
});
afterEach(() => vi.useRealTimers());

describe('createRuntime', () => {
  test('connection status reaches the slow store', () => {
    const rt = make();
    rt.start();
    expect(rt.stores.slow.getSnapshot().connection).toBe('connecting');
    FakeSocket.all[0]!.onopen!();
    vi.advanceTimersByTime(200);
    expect(rt.stores.slow.getSnapshot().connection).toBe('open');
    rt.stop();
  });

  test('messages from the socket land in the stores', () => {
    const rt = make();
    rt.start();
    const s = FakeSocket.all[0]!;
    s.onopen!();
    s.onmessage!({ data: hello(1, 'b1') });
    s.onmessage!({ data: snapshot(1, { lifecycle: 'live' }) });
    s.onmessage!({
      data: buildFrame({ epoch: 1, seq: 1, sessionTime: 1, cars: { 0: { speed: 77, readout: 0 } } }),
    });
    expect(rt.stores.frame.data.speed[0]).toBe(77);
    vi.advanceTimersByTime(200);
    expect(rt.stores.slow.getSnapshot().state).toEqual({ lifecycle: 'live' });
    rt.stop();
  });

  test('a build ID mismatch in hello reloads the page', () => {
    const rt = make('old');
    rt.start();
    FakeSocket.all[0]!.onopen!();
    FakeSocket.all[0]!.onmessage!({ data: hello(1, 'new') });
    expect(reload).toHaveBeenCalledTimes(1);
    rt.stop();
  });

  test('losing the socket keeps the last values and flags reconnecting', () => {
    const rt = make();
    rt.start();
    const s = FakeSocket.all[0]!;
    s.onopen!();
    s.onmessage!({ data: hello(1) });
    s.onmessage!({ data: snapshot(1, { lifecycle: 'live' }) });
    s.onmessage!({ data: buildFrame({ epoch: 1, seq: 1, sessionTime: 1, cars: { 0: { speed: 77, readout: 0 } } }) });
    s.onclose!();
    vi.advanceTimersByTime(200);
    expect(rt.stores.slow.getSnapshot().connection).toBe('reconnecting');
    expect(rt.stores.frame.data.speed[0]).toBe(77);
    expect(rt.stores.slow.getSnapshot().state).toEqual({ lifecycle: 'live' });
    rt.stop();
  });
});
