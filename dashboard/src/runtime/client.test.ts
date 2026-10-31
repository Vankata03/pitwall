import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';
import { backoffDelay, CollectorClient, type SocketLike } from './client';

class FakeSocket implements SocketLike {
  static all: FakeSocket[] = [];
  binaryType = '';
  onopen: (() => void) | null = null;
  onmessage: ((e: { data: string | ArrayBuffer }) => void) | null = null;
  onclose: (() => void) | null = null;
  onerror: (() => void) | null = null;
  closed = false;
  constructor(readonly url: string) {
    FakeSocket.all.push(this);
  }
  close() {
    this.closed = true;
    this.onclose?.();
  }
  // test drivers
  open() {
    this.onopen?.();
  }
  send(data: string | ArrayBuffer) {
    this.onmessage?.({ data });
  }
  drop() {
    this.onclose?.();
  }
}

const noJitter = () => 0.5; // jitter factor 1.0

let statuses: string[];
let received: unknown[];
function make(over: Partial<ConstructorParameters<typeof CollectorClient>[0]> = {}) {
  return new CollectorClient({
    url: 'ws://host/ws',
    createSocket: (u) => new FakeSocket(u),
    onMessage: (d) => received.push(d),
    onStatus: (s) => statuses.push(s),
    random: noJitter,
    ...over,
  });
}

beforeEach(() => {
  vi.useFakeTimers();
  FakeSocket.all = [];
  statuses = [];
  received = [];
});
afterEach(() => vi.useRealTimers());

describe('backoffDelay', () => {
  test('doubles from 250 ms and caps at 5 s', () => {
    const d = (n: number) => backoffDelay(n, noJitter);
    expect([0, 1, 2, 3, 4, 5, 6, 20].map(d)).toEqual([250, 500, 1000, 2000, 4000, 5000, 5000, 5000]);
  });

  test('jitters within +-20%', () => {
    expect(backoffDelay(2, () => 0)).toBe(800);
    expect(backoffDelay(2, () => 1)).toBe(1200);
  });
});

describe('CollectorClient', () => {
  test('connects with binary frames as ArrayBuffer and reports status', () => {
    const c = make();
    c.start();
    const s = FakeSocket.all[0]!;
    expect(s.url).toBe('ws://host/ws');
    expect(s.binaryType).toBe('arraybuffer');
    expect(statuses).toEqual(['connecting']);
    s.open();
    expect(statuses).toEqual(['connecting', 'open']);
  });

  test('forwards text and binary messages', () => {
    const c = make();
    c.start();
    const s = FakeSocket.all[0]!;
    s.open();
    const buf = new ArrayBuffer(4);
    s.send('hi');
    s.send(buf);
    expect(received).toEqual(['hi', buf]);
  });

  test('reconnects forever with growing backoff, resetting after a good connection', () => {
    const c = make();
    c.start();
    FakeSocket.all[0]!.drop(); // never opened
    expect(statuses.at(-1)).toBe('reconnecting');
    vi.advanceTimersByTime(249);
    expect(FakeSocket.all).toHaveLength(1);
    vi.advanceTimersByTime(1);
    expect(FakeSocket.all).toHaveLength(2);
    FakeSocket.all[1]!.drop();
    vi.advanceTimersByTime(499);
    expect(FakeSocket.all).toHaveLength(2);
    vi.advanceTimersByTime(1);
    expect(FakeSocket.all).toHaveLength(3);
    // a connection that opens and delivers resets the ladder
    FakeSocket.all[2]!.open();
    FakeSocket.all[2]!.send('hello');
    FakeSocket.all[2]!.drop();
    vi.advanceTimersByTime(250);
    expect(FakeSocket.all).toHaveLength(4);
    // many failures never give up
    for (let i = 0; i < 30; i++) {
      FakeSocket.all.at(-1)!.drop();
      vi.advanceTimersByTime(5000);
    }
    expect(FakeSocket.all.length).toBeGreaterThan(30);
  });

  test('a silent open socket is declared unreachable and replaced', () => {
    const c = make({ silenceMs: 3000 });
    c.start();
    const s = FakeSocket.all[0]!;
    s.open();
    vi.advanceTimersByTime(2000);
    s.send('x'); // traffic rearms the watchdog
    vi.advanceTimersByTime(2999);
    expect(s.closed).toBe(false);
    vi.advanceTimersByTime(1);
    expect(s.closed).toBe(true);
    expect(statuses.at(-1)).toBe('reconnecting');
    vi.advanceTimersByTime(250);
    expect(FakeSocket.all).toHaveLength(2);
  });

  test('stale socket events after replacement are ignored', () => {
    const c = make({ silenceMs: 1000 });
    c.start();
    const old = FakeSocket.all[0]!;
    old.open();
    vi.advanceTimersByTime(1000);
    vi.advanceTimersByTime(250);
    const next = FakeSocket.all[1]!;
    old.send('ghost');
    old.drop();
    expect(received).toEqual([]);
    vi.advanceTimersByTime(10000);
    expect(FakeSocket.all).toHaveLength(2);
    next.open();
    expect(statuses.at(-1)).toBe('open');
  });

  test('stop closes the socket and cancels pending reconnects', () => {
    const c = make();
    c.start();
    FakeSocket.all[0]!.open();
    c.stop();
    expect(FakeSocket.all[0]!.closed).toBe(true);
    vi.advanceTimersByTime(60000);
    expect(FakeSocket.all).toHaveLength(1);
  });
});
