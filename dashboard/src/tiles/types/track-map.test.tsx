// @vitest-environment jsdom
import { act, cleanup, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { buildFrame, hello, snapshot, stateMsg } from '../../runtime/fixtures';
import { RuntimeProvider } from '../../runtime/react';
import { createRuntime } from '../../runtime/runtime';
import type { TileProps } from '../contract';
import tile from './track-map';

const props: TileProps = {
  id: 'map',
  settings: {},
  density: 'regular',
  width: 300,
  height: 240,
  car: null,
  units: { speed: 'kmh', temperature: 'c', pressure: 'psi' },
  editing: false,
};
const state = (revision = 0, trackId = 31) => ({
  lifecycle: 'Live',
  context: { revisions: { tracks: revision } },
  session: {
    state: 'live',
    value: {
      trackId,
      trackLength: 1000,
      sector2LapDistanceStart: 80,
      sector3LapDistanceStart: 400,
    },
  },
});
const map = (complete = false) => ({
  trackId: 31,
  trackLength: 1000,
  complete,
  revision: 0,
  outline: [
    [0, 0, 0],
    [10, 100, 0],
    [11, 100, 100],
  ],
  pit: [
    [20, 10],
    [30, 20],
  ],
});
let ctx: Record<string, ReturnType<typeof vi.fn>>;
let fetcher: ReturnType<typeof vi.fn>;
beforeEach(() => {
  vi.useFakeTimers();
  ctx = Object.fromEntries(
    [
      'setTransform',
      'clearRect',
      'beginPath',
      'moveTo',
      'lineTo',
      'stroke',
      'arc',
      'fill',
      'fillText',
      'setLineDash',
    ].map((k) => [k, vi.fn()]),
  );
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(
    ctx as unknown as CanvasRenderingContext2D,
  );
  fetcher = vi.fn().mockResolvedValue({ ok: true, json: async () => map() });
  vi.stubGlobal('fetch', fetcher);
});
afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});
async function mount() {
  let queued: (ts: number) => void = () => {};
  const runtime = createRuntime({
    url: 'ws://h/ws',
    ownBuildId: 'b1',
    createSocket: () => ({
      binaryType: '',
      onopen: null,
      onmessage: null,
      onclose: null,
      onerror: null,
      close() {},
    }),
    requestFrame: (cb) => {
      queued = cb;
      return 1;
    },
    cancelFrame() {},
    reload() {},
    storage: { getItem: () => null, setItem() {} },
  });
  runtime.loop.start();
  runtime.stores.setConnection('open');
  runtime.stores.ingest(hello(1));
  runtime.stores.ingest(snapshot(1, state()));
  act(() => vi.advanceTimersByTime(100));
  const Tile = tile.Component;
  let view: ReturnType<typeof render>;
  await act(async () => {
    view = render(
      <RuntimeProvider runtime={runtime}>
        <Tile {...props} />
      </RuntimeProvider>,
    );
  });
  const paint = () => act(() => queued(16));
  const frame = (x: number, readout = 0) =>
    runtime.stores.ingest(
      buildFrame({
        epoch: 1,
        seq: 1,
        sessionTime: 1,
        cars: { 0: { worldX: x, worldZ: 50, readout } },
      }),
    );
  const update = async (s: Record<string, unknown>) => {
    await act(async () => {
      runtime.stores.ingest(stateMsg(1, s));
      vi.advanceTimersByTime(100);
    });
  };
  return { runtime, paint, frame, update, view: view! };
}
test('fetches the track, draws learned line, pit lane and only covered sector markers', async () => {
  const { paint } = await mount();
  paint();
  expect(fetcher.mock.calls[0]![0]).toBe('/api/tracks/31/1000');
  expect(screen.getByText('Learning map')).toBeTruthy();
  expect(screen.getByRole('img', { name: /Track map/ })).toBeTruthy();
  expect(ctx.stroke).toHaveBeenCalled();
  const labels = ctx.fillText!.mock.calls.map((c) => c[0]);
  expect(labels).toContain('S/F');
  expect(labels).toContain('S2');
  expect(labels).not.toContain('S3');
  expect(ctx.setLineDash).toHaveBeenCalledWith([4, 4]);
});

test('car dots use raw X/Z before any map exists and refit only when bounds widen', async () => {
  fetcher.mockResolvedValue({ ok: false, status: 404 });
  const { paint, frame } = await mount();
  frame(20);
  paint();
  const first = ctx.arc!.mock.calls.at(-1)!;
  expect(first.slice(0, 2)).toEqual([102, 100]);
  // Identical raw point keeps the same projection; another dot moves inside the fit.
  frame(30);
  paint();
  expect(ctx.arc!.mock.calls.at(-1)!.slice(0, 2)).toEqual([118, 100]);
  frame(150);
  paint();
  expect(ctx.arc!.mock.calls.at(-1)!.slice(0, 2)).toEqual([215, 100]);
});

test('refetches on track revision and removes the learning label when complete', async () => {
  const { update, paint } = await mount();
  fetcher.mockResolvedValue({ ok: true, json: async () => map(true) });
  await update(state(1));
  paint();
  expect(fetcher).toHaveBeenCalledTimes(2);
  expect(screen.queryByText('Learning map')).toBeNull();
  await update(state(1));
  expect(fetcher).toHaveBeenCalledTimes(2);
});

test('stale dots keep their positions and marker; absent and restricted dots never draw as zero', async () => {
  const { runtime, paint, frame } = await mount();
  frame(20);
  paint();
  ctx.arc!.mockClear();
  frame(20, 4);
  paint(); // motion group stale
  expect(ctx.arc!.mock.calls.map((c) => c[2])).toEqual([4, 6]);
  expect(screen.getByText(/1 car · stale/)).toBeTruthy();
  ctx.arc!.mockClear();
  frame(0, 8);
  paint(); // motion restricted
  expect(ctx.arc).not.toHaveBeenCalled();
  expect(screen.getByText('Restricted')).toBeTruthy();
  ctx.arc!.mockClear();
  frame(0, 12);
  paint(); // motion absent
  expect(ctx.arc).not.toHaveBeenCalled();
  frame(20);
  paint();
  act(() => {
    runtime.stores.setConnection('reconnecting');
    vi.advanceTimersByTime(100);
  });
  paint();
  expect(screen.getByText(/stale/)).toBeTruthy();
});

test('an old HTTP response cannot install the previous session map', async () => {
  let finish: (value: unknown) => void = () => {};
  fetcher.mockReturnValueOnce(
    new Promise((resolve) => {
      finish = resolve;
    }),
  );
  const { update, paint } = await mount();
  fetcher.mockResolvedValue({ ok: false, status: 404 });
  await update(state(1, 32));
  await act(async () => {
    finish({ ok: true, json: async () => map(true) });
  });
  paint();
  expect(screen.getByText('Learning map')).toBeTruthy();
  expect(ctx.fillText!.mock.calls.map((c) => c[0])).not.toContain('S/F');
});

test('resize redraws and unmount stops drawing', async () => {
  const { runtime, paint, frame, view } = await mount();
  frame(20);
  paint();
  const old = ctx.arc!.mock.calls.at(-1)![0];
  const Tile = tile.Component;
  view.rerender(
    <RuntimeProvider runtime={runtime}>
      <Tile {...props} width={400} />
    </RuntimeProvider>,
  );
  paint();
  expect(ctx.arc!.mock.calls.at(-1)![0]).not.toBe(old);
  view.unmount();
  ctx.clearRect!.mockClear();
  frame(50);
  paint();
  expect(ctx.clearRect).not.toHaveBeenCalled();
});

test('sector marker appears on a short supported segment between learned buckets', async () => {
  fetcher.mockResolvedValue({
    ok: true,
    json: async () => ({
      ...map(),
      outline: [
        [9, 90, 0],
        [11, 110, 20],
      ],
    }),
  });
  const { paint } = await mount();
  paint();
  const labels = ctx.fillText!.mock.calls.map((c) => c[0]);
  expect(labels).toContain('S2');
  expect(labels).not.toContain('S/F');
  expect(labels).not.toContain('S3');
});
