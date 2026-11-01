// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import type { Trace } from '../../generated/wire/Trace';
import { buildFrame, hello, historyMsg, snapshot, stateMsg } from '../../runtime/fixtures';
import { RuntimeProvider } from '../../runtime/react';
import { createRuntime } from '../../runtime/runtime';
import type { Plot } from '../traces/plot';
import type { TileProps } from '../contract';
import { registry } from '../registry';
import tracesTile from './throttle-brake-traces';

// uPlot needs a real canvas. The chart adapter is the one seam we replace: the tile's job is
// to hand it the right data, and these tests read that data back.
const chart = vi.hoisted(() => ({ plots: [] as unknown[], destroyed: 0 }));
vi.mock('../traces/chart', () => ({
  createTraceChart: () => ({
    draw: (plot: unknown) => chart.plots.push(plot),
    destroy: () => chart.destroyed++,
  }),
}));

const lastPlot = () => chart.plots[chart.plots.length - 1] as Plot;
const channelValues = (channel: string, lap: string) =>
  lastPlot().series.find((s) => s.channel === channel && s.lap === lap)?.values;

const trace = (lap: number, over: Partial<Trace> = {}): Trace => ({
  epoch: 1,
  lap,
  distance: [0, 5, 10],
  time: [0, 100, 200],
  speed: [100, 150, 200],
  throttle: [1, 0.5, 0],
  brake: [0, 0, 1],
  ...over,
});

const lapOf = (n: number) => ({ lap_number: n, session_time: 0, lap_time_ms: 90000, sectors_ms: [1, 2, 3], valid: true, sector_valid: [true, true, true], authoritative: true });
const car = (current: number, best: number | null, previous: number | null, extra: Record<string, unknown> = {}) => ({
  lap: { state: 'live', value: { current_lap_num: current } },
  best: best === null ? null : lapOf(best),
  previous: previous === null ? null : lapOf(previous),
  ...extra,
});

let fetchMock: ReturnType<typeof vi.fn>;
let replies: Map<string, Trace>;

beforeEach(() => {
  chart.plots.length = 0;
  chart.destroyed = 0;
  replies = new Map();
  fetchMock = vi.fn((url: string) => {
    const t = replies.get(url);
    return Promise.resolve(t ? new Response(JSON.stringify(t), { status: 200 }) : new Response(null, { status: 404 }));
  });
  vi.stubGlobal('fetch', fetchMock);
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

function setup(cars: Record<number, unknown> = { 0: car(5, 3, 4) }, epoch = 1) {
  let queued: ((ts: number) => void) | null = null;
  let ts = 0;
  const runtime = createRuntime({
    url: 'ws://h/ws',
    ownBuildId: 'b1',
    createSocket: () => ({ binaryType: '', onopen: null, onmessage: null, onclose: null, onerror: null, close() {} }),
    requestFrame: (cb) => ((queued = cb), 1),
    cancelFrame: () => {},
    reload: () => {},
    storage: { getItem: () => null, setItem: () => {} },
  });
  runtime.loop.start();
  runtime.stores.setConnection('open');
  runtime.stores.ingest(hello(epoch));
  runtime.stores.ingest(snapshot(epoch, { lifecycle: 'live', playerCar: 0, cars: Object.assign(new Array(24).fill(null), cars) }));
  return {
    runtime,
    paint: () => queued!((ts += 16)),
    frame: (seq: number, distance: number, throttle: number, brake: number, speed = 200, e = epoch) =>
      runtime.stores.ingest(
        buildFrame({ epoch: e, seq, sessionTime: seq, cars: { 0: { lapDistance: distance, throttle, brake, speed, readout: 0 } } }),
      ),
  };
}

const props = (over: Partial<TileProps> = {}): TileProps => ({
  id: 'traces',
  settings: {},
  density: 'regular',
  width: 700,
  height: 400,
  car: { status: 'ok', index: 0 },
  units: { speed: 'kmh', temperature: 'c', pressure: 'psi' },
  editing: false,
  ...over,
});

const show = (runtime: ReturnType<typeof setup>['runtime'], over: Partial<TileProps> = {}) => {
  const Tile = tracesTile.Component;
  const tree = (p: Partial<TileProps>) => (
    <RuntimeProvider runtime={runtime}>
      <Tile {...props(p)} />
    </RuntimeProvider>
  );
  const view = render(tree(over));
  return { ...view, again: (p: Partial<TileProps>) => view.rerender(tree(p)) };
};

const url = (carIndex: number, lap: number, epoch = 1) => `/api/cars/${carIndex}/laps/${lap}/trace?epoch=${epoch}`;
const settle = () => act(() => new Promise((r) => setTimeout(r, 150)));

test('is the Throttle/brake traces tile of the catalogue', () => {
  const d = tracesTile.declaration;
  expect(registry.get('throttle-brake-traces')).toBe(tracesTile);
  expect(d).toMatchObject({ binding: 'follows-car', rate: 'frame', minSize: { w: 6, h: 4 }, defaultSize: { w: 10, h: 7 } });
});

test('draws the current lap from Frame against distance, throttle and brake in percent', async () => {
  const { runtime, paint, frame } = setup();
  show(runtime);
  await settle();
  frame(1, 0, 100, 0);
  frame(2, 10, 50, 0);
  frame(3, 20, 0, 80);
  paint();
  expect(lastPlot().x).toEqual([0, 5, 10, 15, 20]);
  expect(channelValues('throttle', 'current')).toEqual([100, 75, 50, 25, 0]);
  expect(channelValues('brake', 'current')).toEqual([0, 0, 0, 40, 80]);
  expect(lastPlot().series.every((s) => s.channel !== 'speed')).toBe(true);
});

test('draws a continuous current lap from 10 Hz Frames at 300 km/h, 8 m apart and off the bucket grid', async () => {
  const { runtime, paint, frame } = setup();
  show(runtime);
  await settle();
  for (let n = 0; n <= 12; n++) frame(n + 1, n * (300 / 3.6 / 10), 100 - n * 5, n * 5);
  paint();
  const x = lastPlot().x;
  expect(x.length).toBeGreaterThan(15);
  expect(x).toEqual(x.map((_, i) => i * 5));
  expect(channelValues('throttle', 'current')!.every((v) => v !== null)).toBe(true);
  expect(channelValues('brake', 'current')!.every((v) => v !== null)).toBe(true);
});

test('fetches the best lap as the default reference and overlays it', async () => {
  replies.set(url(0, 3), trace(3));
  const { runtime, paint, frame } = setup();
  show(runtime);
  frame(1, 0, 100, 0);
  await vi.waitFor(() => expect(fetchMock).toHaveBeenCalledWith(url(0, 3)));
  await settle();
  paint();
  expect(channelValues('throttle', 'reference')).toEqual([100, 50, 0]);
  expect(channelValues('brake', 'reference')).toEqual([0, 0, 100]);
});

test('the reference setting switches to the previous lap', async () => {
  replies.set(url(0, 4), trace(4, { throttle: [0.2, 0.2, 0.2] }));
  const { runtime, paint, frame } = setup();
  show(runtime, { settings: { reference: 'previous' } });
  frame(1, 0, 100, 0);
  await vi.waitFor(() => expect(fetchMock).toHaveBeenCalledWith(url(0, 4)));
  expect(fetchMock).not.toHaveBeenCalledWith(url(0, 3));
  await settle();
  paint();
  expect(channelValues('throttle', 'reference')).toEqual([20, 20, 20]);
});

test('a lap completing fetches the new previous and best lap', async () => {
  const { runtime, paint, frame } = setup({ 0: car(5, 3, 4) });
  show(runtime, { settings: { reference: 'previous' } });
  await vi.waitFor(() => expect(fetchMock).toHaveBeenCalledWith(url(0, 4)));
  replies.set(url(0, 5), trace(5, { throttle: [0.9, 0.9, 0.9] }));
  runtime.stores.ingest(historyMsg(1, [{ kind: 'lap', id: 'lap-0-5', car: 0, body: lapOf(5) }]));
  runtime.stores.ingest(stateMsg(1, { lifecycle: 'live', playerCar: 0, cars: Object.assign(new Array(24).fill(null), { 0: car(6, 3, 5) }) }));
  await vi.waitFor(() => expect(fetchMock).toHaveBeenCalledWith(url(0, 5)));
  frame(1, 0, 100, 0);
  await settle();
  paint();
  expect(channelValues('throttle', 'reference')).toEqual([90, 90, 90]);
});

test('a lap that was not ready at lap-complete is asked for again on the next History entry', async () => {
  const { runtime } = setup({ 0: car(5, 3, 4) });
  show(runtime, { settings: { reference: 'previous' } });
  await vi.waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(1)); // lap 4: 404, not held yet
  await settle();
  replies.set(url(0, 4), trace(4));
  runtime.stores.ingest(historyMsg(1, [{ kind: 'lap', id: 'lap-0-4', car: 0, body: lapOf(4) }]));
  await vi.waitFor(() => expect(fetchMock.mock.calls.filter(([path]) => path === url(0, 4))).toHaveLength(2));
});

test('a late joiner fetches the current lap so far and fills in what Frame has not seen', async () => {
  replies.set(url(0, 5), trace(5, { distance: [0, 5, 10], throttle: [1, 1, 1], brake: [0, 0, 0] }));
  const { runtime, paint, frame } = setup({ 0: car(5, null, null) });
  show(runtime);
  frame(1, 10, 40, 0); // joined mid-lap: the first sample is not at the line
  paint();
  await vi.waitFor(() => expect(fetchMock).toHaveBeenCalledWith(url(0, 5)));
  await settle();
  paint();
  expect(lastPlot().x).toEqual([0, 5, 10]);
  expect(channelValues('throttle', 'current')).toEqual([100, 100, 40]); // live bucket wins
});

test('a car seen from the line needs no fetch for its current lap', async () => {
  const { runtime, paint, frame } = setup({ 0: car(5, null, null) });
  show(runtime);
  frame(1, 0, 100, 0);
  paint();
  await settle();
  expect(fetchMock).not.toHaveBeenCalled();
});

test('a reply from an older epoch is dropped', async () => {
  const { runtime, paint, frame } = setup();
  let reply!: (r: Response) => void;
  fetchMock.mockImplementation(() => new Promise<Response>((r) => (reply = r)));
  show(runtime);
  await vi.waitFor(() => expect(fetchMock).toHaveBeenCalled());
  // A flashback: the collector bumps the epoch and sends a new snapshot.
  runtime.stores.ingest(hello(2));
  runtime.stores.ingest(snapshot(2, { lifecycle: 'live', playerCar: 0, cars: Object.assign(new Array(24).fill(null), { 0: car(5, null, null) }) }));
  frame(1, 0, 100, 0, 200, 2);
  reply(new Response(JSON.stringify(trace(3)), { status: 200 }));
  await act(() => new Promise((resolve) => setTimeout(resolve, 0)));
  paint(); // Frame already advanced; slow State publication may still be waiting.
  expect(channelValues('throttle', 'reference')).toBeUndefined();
  await settle();
  paint();
  expect(channelValues('throttle', 'reference')).toBeUndefined();
});

test('a new epoch clears the current lap', async () => {
  const { runtime, paint, frame } = setup();
  show(runtime);
  frame(1, 0, 100, 0);
  frame(2, 100, 100, 0);
  paint();
  expect(lastPlot().x.length).toBe(21);
  runtime.stores.ingest(hello(2));
  runtime.stores.ingest(snapshot(2, { lifecycle: 'live', playerCar: 0, cars: Object.assign(new Array(24).fill(null), { 0: car(5, null, null) }) }));
  frame(1, 5, 70, 0, 200, 2);
  paint();
  expect(lastPlot().x).toEqual([0, 5]);
  expect(channelValues('throttle', 'current')).toEqual([null, 70]);
});

test('following another car starts a fresh lap for that car', async () => {
  const { runtime, paint, frame } = setup({ 0: car(5, 3, 4), 1: car(5, 2, 4) });
  const view = show(runtime);
  frame(1, 0, 100, 0);
  paint();
  runtime.stores.ingest(
    buildFrame({ epoch: 1, seq: 9, sessionTime: 9, cars: { 1: { lapDistance: 50, throttle: 30, brake: 0, speed: 100, readout: 0 } } }),
  );
  view.again({ car: { status: 'ok', index: 1 } });
  paint();
  expect(lastPlot().x).toEqual([0, 5, 10, 15, 20, 25, 30, 35, 40, 45, 50]);
  expect(channelValues('throttle', 'current')!.at(-1)).toBe(30);
  await vi.waitFor(() => expect(fetchMock).toHaveBeenCalledWith(url(1, 2)));
});

test('the channels setting chooses what is plotted, speed in the device’s unit', async () => {
  const { runtime, paint, frame } = setup();
  show(runtime, { settings: { channels: ['speed'] }, units: { speed: 'mph', temperature: 'c', pressure: 'psi' } });
  frame(1, 0, 100, 0, 100);
  paint();
  expect(lastPlot().series.filter((s) => s.lap === 'current').map((s) => s.channel)).toEqual(['speed']);
  expect(channelValues('speed', 'current')![0]).toBeCloseTo(62.14, 2);
  expect(lastPlot().speedUnit).toBe('mph');
});

test('says why there is nothing to draw until the car has data, keeping the tile’s shape', async () => {
  const { runtime, paint, frame } = setup();
  show(runtime);
  paint();
  const message = screen.getByRole('status');
  expect(message.hidden).toBe(false);
  expect(message.textContent).toBe('No data');
  frame(1, 0, 100, 0);
  paint();
  expect(message.hidden).toBe(true);
});

test('the chart is destroyed with the tile', () => {
  const { runtime } = setup();
  const view = show(runtime);
  view.unmount();
  expect(chart.destroyed).toBeGreaterThan(0);
});

test('settings choose the reference and the channels, and leave unknown keys alone', () => {
  const Panel = tracesTile.Settings!;
  const saved: Record<string, unknown>[] = [];
  const { rerender } = render(<Panel settings={{ other: 1 }} onChange={(s) => saved.push(s)} />);
  expect((screen.getByLabelText('Best lap') as HTMLInputElement).checked).toBe(true);
  fireEvent.click(screen.getByLabelText('Previous lap'));
  expect(saved[0]).toEqual({ other: 1, reference: 'previous' });

  fireEvent.click(screen.getByLabelText('Speed'));
  expect(saved[1]).toEqual({ other: 1, channels: ['throttle', 'brake', 'speed'] });

  rerender(<Panel settings={{ channels: ['throttle'] }} onChange={(s) => saved.push(s)} />);
  fireEvent.click(screen.getByLabelText('Throttle')); // the last channel stays
  expect(saved).toHaveLength(2);
});

test('a completion fetches that lap even when State still names the old previous lap', async () => {
  const { runtime } = setup();
  show(runtime);
  await settle();
  runtime.stores.ingest(historyMsg(1, [{ kind: 'lap', id: 'lap-0-5', car: 0, body: lapOf(5) }]));
  await vi.waitFor(() => expect(fetchMock).toHaveBeenCalledWith(url(0, 5)));
});

test('crossing the line clears the old current lap without waiting for State', async () => {
  const { runtime, paint, frame } = setup();
  show(runtime);
  await settle();
  frame(1, 0, 100, 0);
  frame(2, 5000, 80, 0);
  paint();
  frame(3, 0, 30, 0);
  paint();
  expect(channelValues('throttle', 'current')).toEqual([30]);
});

test('disconnect dims the last trace and marks it stale without new telemetry', async () => {
  const { runtime, paint, frame } = setup();
  show(runtime);
  await settle();
  frame(1, 0, 100, 0);
  paint();
  runtime.stores.setConnection('reconnecting');
  await settle();
  paint();
  expect(screen.getByRole('status').textContent).toBe('Stale ◷');
  expect(screen.getByRole('img').style.opacity).toBe('0.55');
  expect(channelValues('throttle', 'current')).toEqual([100]);
});
