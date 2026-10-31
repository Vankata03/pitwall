import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { buildFrame, hello, snapshot } from './fixtures';
import { frameReadoutState, effectiveState, writeReadout } from './readout';
import { createStores } from './stores';
import { RenderLoop } from './loop';
import { defineTile } from './tile';

// A reference frame tile, written the way tile tickets should write one: declare what it
// reads, register with the loop, and in draw() write straight into the DOM.

const speedTile = defineTile({
  type: 'speed',
  name: 'Speed',
  binding: 'follows-car',
  rate: 'frame',
  minSize: { w: 2, h: 2 },
  defaultSize: { w: 3, h: 3 },
  densities: [
    { density: 'compact', minWidth: 0, minHeight: 0 },
    { density: 'regular', minWidth: 200, minHeight: 120 },
  ],
  reads: [{ source: 'frame', fields: ['speed'] }],
});

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

test('a frame tile shows live, stale, restricted and absent through the shared rendering', () => {
  const stores = createStores();
  let queued: ((ts: number) => void) | null = null;
  const loop = new RenderLoop({
    stores,
    requestFrame: (cb) => ((queued = cb), 1),
    cancelFrame: () => {},
  });
  const el = { textContent: '', dataset: {} as Record<string, string> };
  loop.register({
    declaration: speedTile,
    car: () => 0,
    draw: ({ frame, car, connected }) => {
      const state = effectiveState(frameReadoutState(frame, car!, 'speed'), connected);
      writeReadout(el, state, String(frame.speed[car!]));
    },
  });
  stores.setConnection('open');
  stores.ingest(hello(1));
  stores.ingest(snapshot(1, { lifecycle: 'live' }));
  loop.start();
  const paint = (ts: number) => queued!(ts);
  const send = (seq: number, readoutBits: number) =>
    stores.ingest(
      buildFrame({ epoch: 1, seq, sessionTime: seq, cars: { 0: { speed: 312, readout: readoutBits } } }),
    );

  paint(1); // nothing received yet: absent, never 0
  expect(el).toEqual({ textContent: '—', dataset: { state: 'absent' } });
  send(1, 0b00); // telemetry live
  paint(17);
  expect(el).toEqual({ textContent: '312', dataset: { state: 'live' } });
  send(2, 0b01);
  paint(33);
  expect(el).toEqual({ textContent: '312', dataset: { state: 'stale' } });
  send(3, 0b10);
  paint(50);
  expect(el).toEqual({ textContent: 'Restricted', dataset: { state: 'restricted' } });
  send(4, 0b00);
  paint(66);
  expect(el.dataset.state).toBe('live');
  stores.setConnection('reconnecting'); // collector gone: live values dim with no new frame
  vi.advanceTimersByTime(200);
  paint(83);
  expect(el).toEqual({ textContent: '312', dataset: { state: 'stale' } });
});
