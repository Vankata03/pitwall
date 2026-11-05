// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, expect, test } from 'vitest';
import { buildFrame, hello, snapshot } from '../runtime/fixtures';
import { RuntimeProvider } from '../runtime/react';
import { createRuntime } from '../runtime/runtime';
import { InputHistory } from '../tiles/traces/window-plot';
import { inputTrace, smoothLine } from './input-trace';
import { ThrottleBrakeOverlay } from './throttle-brake';

afterEach(cleanup);

const settle = () => act(() => new Promise((r) => setTimeout(r, 150)));

function setup(playerCar: number | null = 1) {
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
  runtime.stores.ingest(hello(1));
  const cars = new Array(24).fill(null);
  cars[1] = { participant: { state: 'live', value: { ai_controlled: 1, driver_id: 7, name: 'Driver', race_number: 4 } } };
  runtime.stores.ingest(snapshot(1, { lifecycle: 'live', playerCar, cars }));
  render(<RuntimeProvider runtime={runtime}><ThrottleBrakeOverlay /></RuntimeProvider>);
  return {
    paint: () => queued?.((ts += 16)),
    frame: (seq: number, throttle: number, brake: number) => runtime.stores.ingest(
      buildFrame({ epoch: 1, seq, sessionTime: seq / 10, cars: { 1: { lapDistance: seq, throttle, brake, speed: 200, readout: 0 } } }),
    ),
  };
}

test('shows the player car pedals as bars with values', async () => {
  const view = setup();
  await settle();
  view.frame(1, 80, 0);
  view.frame(2, 35, 60);
  view.paint();
  expect(screen.getByTestId('overlay-throttle-value').textContent).toBe('35');
  expect(screen.getByTestId('overlay-brake-value').textContent).toBe('60');
  expect(screen.getByTestId('overlay-throttle').style.height).toBe('35%');
  expect(screen.getByTestId('overlay-brake').style.height).toBe('60%');
  expect(document.documentElement.dataset.overlay).toBe('true');
});

test('empties the bars without a player car', async () => {
  const view = setup(null);
  await settle();
  view.frame(1, 80, 20);
  view.paint();
  expect(screen.getByTestId('overlay-throttle').style.height).toBe('0%');
  expect(screen.getByTestId('overlay-brake').style.height).toBe('0%');
});

function history(samples: Array<[time: number, throttle: number, brake: number]>) {
  const h = new InputHistory();
  for (const [time, throttle, brake] of samples) {
    h.rings.sessionTime.push(time);
    h.rings.lapDistance.push(0);
    h.rings.speed.push(0);
    h.rings.throttle.push(throttle);
    h.rings.brake.push(brake);
  }
  return h.rings;
}

test('trace keeps the last 5 s with the newest sample on the right edge', () => {
  const trace = inputTrace(history([[0, 100, 0], [10, 100, 0], [10.4, 50, 0], [10.8, 0, 100]]), 100, 50);
  expect(trace.throttle.map((line) => line.map(([x, y]) => [Math.round(x), y]))).toEqual([[[84, 0], [92, 25], [100, 50]]]);
  expect(trace.brake.map((line) => line.map(([x, y]) => [Math.round(x), y]))).toEqual([[[84, 50], [92, 50], [100, 0]]]);
});

test('trace splits at a pause in samples', () => {
  const trace = inputTrace(history([[1, 10, 0], [1.1, 20, 0], [3, 30, 0], [3.1, 40, 0]]), 100, 100);
  expect(trace.throttle).toHaveLength(2);
});

test('move mode sends drag and resize requests to the collector', async () => {
  const sent: string[] = [];
  (globalThis as { ipc?: unknown }).ipc = { postMessage: (message: string) => sent.push(message) };
  setup();
  await settle();
  const grip = document.querySelector('.pw-ov-grip[data-edge="se"]')!;
  fireEvent.mouseDown(grip, { button: 0 });
  expect(sent).toEqual([]); // locked: the page ignores presses
  document.documentElement.dataset.overlayMove = 'true';
  fireEvent.mouseDown(grip, { button: 0 });
  fireEvent.mouseDown(screen.getByRole('main'), { button: 0 });
  expect(sent).toEqual(['resize:se', 'drag']);
  delete (globalThis as { ipc?: unknown }).ipc;
});

test('trace overlaps the reference lap at the same lap distance', () => {
  const h = new InputHistory();
  for (const [time, distance, throttle] of [[10, 100, 100], [10.4, 110, 50], [10.8, 120, 0]] as const) {
    h.rings.sessionTime.push(time);
    h.rings.lapDistance.push(distance);
    h.rings.speed.push(0);
    h.rings.throttle.push(throttle);
    h.rings.brake.push(0);
  }
  // Reference covers 100 to 110 m only: throttle 20% then 40%, brake on at 110 m.
  const reference = { epoch: 1, lap: 3, distance: [100, 110], time: [0, 1], speed: [0, 0], throttle: [0.2, 0.4], brake: [0, 1] };
  const trace = inputTrace(h.rings, 100, 100, reference);
  const rounded = (lines: Array<Array<[number, number]>>) => lines.map((line) => line.map(([x, y]) => [Math.round(x), Math.round(y)]));
  expect(rounded(trace.reference.throttle)).toEqual([[[84, 80], [92, 60]]]);
  expect(rounded(trace.reference.brake)).toEqual([[[84, 100], [92, 0]]]);
  expect(inputTrace(h.rings, 100, 100).reference.throttle).toEqual([]);
});

test('smoothing averages neighbours within the radius and keeps the ends', () => {
  const stairs: Array<[number, number]> = [[0, 0], [1, 0], [2, 30], [3, 30], [4, 60]];
  expect(smoothLine(stairs, 1)).toEqual([[0, 0], [1, 10], [2, 20], [3, 40], [4, 60]]);
  expect(smoothLine(stairs, 0)).toBe(stairs);
});
