import { expect, test, vi } from 'vitest';
import type { Trace } from '../../generated/wire/Trace';
import { createLapTraces } from './lap-traces';

const trace = (over: Partial<Trace> = {}): Trace => ({
  epoch: 1,
  lap: 3,
  distance: [0, 5, 10],
  time: [0, 200, 400],
  speed: [100, 110, null],
  throttle: [1, 0.5, 0],
  brake: [0, 0, 0.8],
  ...over,
});

const json = (status: number, body?: unknown) =>
  Promise.resolve(new Response(body === undefined ? null : JSON.stringify(body), { status }));

function setup(epoch: number | null = 1) {
  let current = epoch;
  const fetch = vi.fn<typeof globalThis.fetch>();
  const traces = createLapTraces({ fetch, epoch: () => current });
  return {
    fetch,
    traces,
    setEpoch: (e: number | null) => (current = e),
  };
}

test('requests a lap from the trace route with the current epoch and then serves it', async () => {
  const { fetch, traces } = setup(7);
  fetch.mockReturnValue(json(200, trace({ epoch: 7 })));
  const changed = vi.fn();
  traces.subscribe(changed);

  traces.request(4, 3);
  expect(traces.get(4, 3)).toBeUndefined();
  expect(fetch).toHaveBeenCalledWith('/api/cars/4/laps/3/trace?epoch=7');

  await vi.waitFor(() => expect(traces.get(4, 3)).toEqual(trace({ epoch: 7 })));
  expect(changed).toHaveBeenCalled();
});

test('a lap already held or in flight is not fetched again', async () => {
  const { fetch, traces } = setup();
  fetch.mockReturnValue(json(200, trace()));
  traces.request(0, 3);
  traces.request(0, 3);
  expect(fetch).toHaveBeenCalledTimes(1);
  await vi.waitFor(() => expect(traces.get(0, 3)).toBeDefined());
  traces.request(0, 3);
  expect(fetch).toHaveBeenCalledTimes(1);
});

test('a reply that arrives after the epoch moved on is dropped', async () => {
  const { fetch, traces, setEpoch } = setup(1);
  let reply!: (r: Response) => void;
  fetch.mockReturnValue(new Promise<Response>((r) => (reply = r)));
  traces.request(0, 3);
  setEpoch(2); // a flashback or new session while the request was out
  reply(new Response(JSON.stringify(trace({ epoch: 1 })), { status: 200 }));
  await new Promise((r) => setTimeout(r, 0));
  expect(traces.get(0, 3)).toBeUndefined();
});

test('a reply stamped with an older epoch than requested is dropped', async () => {
  const { fetch, traces } = setup(5);
  fetch.mockReturnValue(json(200, trace({ epoch: 4 })));
  traces.request(0, 3);
  await new Promise((r) => setTimeout(r, 0));
  expect(traces.get(0, 3)).toBeUndefined();
});

test('held laps are forgotten when the epoch changes', async () => {
  const { fetch, traces, setEpoch } = setup(1);
  fetch.mockReturnValue(json(200, trace()));
  traces.request(0, 3);
  await vi.waitFor(() => expect(traces.get(0, 3)).toBeDefined());
  setEpoch(2);
  expect(traces.get(0, 3)).toBeUndefined();
  fetch.mockReturnValue(json(200, trace({ epoch: 2 })));
  traces.request(0, 3);
  expect(fetch).toHaveBeenLastCalledWith('/api/cars/0/laps/3/trace?epoch=2');
});

test('nothing is requested before the first hello', () => {
  const { fetch, traces } = setup(null);
  traces.request(0, 3);
  expect(fetch).not.toHaveBeenCalled();
});

test('an unavailable lap is not retried until invalidated', async () => {
  const { fetch, traces } = setup();
  fetch.mockImplementation(() => json(404));
  traces.request(0, 3);
  await new Promise((r) => setTimeout(r, 0));
  traces.request(0, 3);
  expect(fetch).toHaveBeenCalledTimes(1);
  expect(traces.get(0, 3)).toBeUndefined();

  traces.invalidate(); // a lap-complete History entry says new data may exist now
  fetch.mockImplementation(() => json(200, trace()));
  traces.request(0, 3);
  expect(fetch).toHaveBeenCalledTimes(2);
  await vi.waitFor(() => expect(traces.get(0, 3)).toBeDefined());
});

test('a 409 or a network failure leaves nothing held and may be retried after invalidation', async () => {
  const { fetch, traces } = setup();
  fetch.mockImplementationOnce(() => json(409));
  traces.request(0, 3);
  await new Promise((r) => setTimeout(r, 0));
  fetch.mockImplementationOnce(() => Promise.reject(new TypeError('offline')));
  traces.invalidate();
  traces.request(0, 3);
  await new Promise((r) => setTimeout(r, 0));
  expect(traces.get(0, 3)).toBeUndefined();
  traces.invalidate();
  fetch.mockImplementation(() => json(200, trace()));
  traces.request(0, 3);
  await vi.waitFor(() => expect(traces.get(0, 3)).toBeDefined());
});

test('an old request failing cannot cancel the same lap requested in a new epoch', async () => {
  const { fetch, traces, setEpoch } = setup();
  let rejectOld!: (reason: Error) => void;
  let finishNew!: (response: Response) => void;
  fetch.mockImplementationOnce(() => new Promise((_, reject) => { rejectOld = reject; }));
  traces.request(0, 3);
  setEpoch(2);
  fetch.mockImplementationOnce(() => new Promise((resolve) => { finishNew = resolve; }));
  traces.request(0, 3);
  rejectOld(new Error('old connection closed'));
  await new Promise((r) => setTimeout(r, 0));
  finishNew(new Response(JSON.stringify(trace({ epoch: 2 })), { status: 200 }));
  await vi.waitFor(() => expect(traces.get(0, 3)?.epoch).toBe(2));
});

test('lap completion replaces a cached partial current lap with the complete trace', async () => {
  const { fetch, traces } = setup();
  fetch.mockImplementation(() => json(200, trace({ distance: [0], throttle: [1] })));
  traces.request(0, 3);
  await vi.waitFor(() => expect(traces.get(0, 3)?.distance).toEqual([0]));
  traces.invalidate(0, 3);
  fetch.mockImplementation(() => json(200, trace()));
  traces.request(0, 3);
  await vi.waitFor(() => expect(traces.get(0, 3)?.distance).toEqual([0, 5, 10]));
});
