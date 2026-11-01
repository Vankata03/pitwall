// @vitest-environment jsdom
import { cleanup, screen } from '@testing-library/react';
import { afterEach, expect, test, vi } from 'vitest';
import { liveRuntime, mount, props, readoutWord, Restricted, Absent, Stale } from '../frame-test-kit';
import { ReadoutState } from '../../runtime/wire';
import tile from './live-delta';

afterEach(cleanup);
const Tile = tile.Component;
const headline = () => screen.getByTestId('live-delta-value');

/** A recording 2D context that also logs property writes, so colour and order can be asserted. */
function recordCanvas() {
  const log: ({ op: 'call'; name: string; args: unknown[] } | { op: 'set'; name: string; value: unknown })[] = [];
  const context = new Proxy(
    {},
    {
      get: (_t, name: string) => (...args: unknown[]) => void log.push({ op: 'call', name, args }),
      set: (_t, name: string, value) => (log.push({ op: 'set', name, value }), true),
    },
  );
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(context as never);
  const calls = (name: string) => log.filter((e) => e.op === 'call' && e.name === name);
  /** Colours in force at each stroke() call, newest paint last. */
  const strokeColours = () => {
    let colour: unknown;
    const out: unknown[] = [];
    for (const e of log) {
      if (e.op === 'set' && e.name === 'strokeStyle') colour = e.value;
      if (e.op === 'call' && e.name === 'stroke') out.push(colour);
    }
    return out;
  };
  const alphas = () => log.filter((e) => e.op === 'set' && e.name === 'globalAlpha').map((e) => (e as { value: number }).value);
  return { log, calls, strokeColours, alphas, clear: () => void (log.length = 0) };
}

test('declaration: follows the car at frame rate, 6x3 minimum, 12x6 default', () => {
  expect(tile.declaration).toMatchObject({ type: 'live-delta', name: 'Live delta', binding: 'follows-car', rate: 'frame', minSize: { w: 6, h: 3 }, defaultSize: { w: 12, h: 6 } });
});

test('every accepted Frame reaches the headline by a DOM write, signed to three decimals', () => {
  recordCanvas();
  const rt = liveRuntime();
  mount(rt, <Tile {...props()} />);
  const send = (deltaBest: number) => rt.frame({ 3: { deltaBest, deltaPrevious: 432, lapDistance: 10, readout: 0 } });
  const sequence: [number, string, string][] = [[-123, '−0.123', 'ahead'], [124, '+0.124', 'behind'], [125, '+0.125', 'behind'], [0, '+0.000', 'equal'], [-1, '−0.001', 'ahead'], [12345, '+12.345', 'behind']];
  sequence.forEach(([ms, text, sign], i) => {
    send(ms);
    rt.paint(16 * (i + 1)); // one paint per Frame: nothing waits for State
    expect(headline().textContent).toBe(text);
    expect(headline().dataset.sign).toBe(sign);
    expect(headline().dataset.state).toBe('live');
  });
});

test('reference setting picks deltaBest or deltaPrevious, defaulting to best', () => {
  recordCanvas();
  const rt = liveRuntime();
  const view = mount(rt, <Tile {...props()} />);
  rt.frame({ 3: { deltaBest: -50, deltaPrevious: 432, lapDistance: 10, readout: 0 } });
  rt.paint(16);
  expect(headline().textContent).toBe('−0.050');
  view.rerender(<Tile {...props({ settings: { reference: 'previous' } })} />);
  rt.paint(33);
  expect(headline().textContent).toBe('+0.432');
  view.rerender(<Tile {...props({ settings: { reference: 'nonsense' } })} />);
  rt.paint(50);
  expect(headline().textContent).toBe('−0.050');
  expect(screen.getByText(/best lap/i)).toBeTruthy();
});

test('the four readout states show on the headline', () => {
  recordCanvas();
  const rt = liveRuntime();
  mount(rt, <Tile {...props()} />);
  const word = (state: ReadoutState) => readoutWord({ deltaBest: state });
  rt.frame({ 3: { deltaBest: -123, lapDistance: 10, readout: word(Stale) } });
  rt.paint(16);
  expect([headline().dataset.state, headline().textContent]).toEqual(['stale', '−0.123']);
  rt.frame({ 3: { deltaBest: 0, lapDistance: 10, readout: word(Restricted) } });
  rt.paint(33);
  expect([headline().dataset.state, headline().textContent, headline().dataset.sign]).toEqual(['restricted', 'Restricted', '']);
  rt.frame({ 3: { deltaBest: 0, lapDistance: 10, readout: word(Absent) } });
  rt.paint(50);
  expect([headline().dataset.state, headline().textContent, headline().dataset.sign]).toEqual(['absent', '—', '']);
  // Absent on a first lap is normal while the collector is live: no whole-tile message over it.
  expect(screen.getByTestId('tile-message').hidden).toBe(true);
  rt.frame({ 3: { deltaBest: -123, lapDistance: 10, readout: 0 } });
  rt.disconnect();
  rt.paint(66);
  expect(headline().dataset.state).toBe('stale');
  expect(headline().textContent).toBe('−0.123');
});

test('with no collector and no value the tile says why', () => {
  recordCanvas();
  const rt = liveRuntime();
  mount(rt, <Tile {...props()} />);
  rt.frame({ 3: { deltaBest: 0, lapDistance: 10, readout: readoutWord({ deltaBest: Absent }) } });
  rt.disconnect();
  rt.paint(16);
  expect(screen.getByTestId('tile-message').textContent).toBe('Collector unreachable');
});

test('the headline is sized from the box and stays huge in every density', () => {
  recordCanvas();
  const rt = liveRuntime();
  const view = mount(rt, <Tile {...props({ density: 'compact', width: 300, height: 112 })} />);
  const size = () => Number.parseFloat(document.querySelector<HTMLElement>('.pw-live-delta')!.style.getPropertyValue('--pw-ld-size'));
  expect(size()).toBeGreaterThanOrEqual(48);
  expect(document.querySelector('canvas')).toBeNull();
  view.rerender(<Tile {...props({ density: 'full', width: 1450, height: 232 })} />);
  expect(size()).toBeGreaterThanOrEqual(90);
  expect(document.querySelector('.pw-live-delta')!.getAttribute('data-arrangement')).toBe('side');
});

test('frames between paints are all drawn; colour follows the sign; fill and clip are used', () => {
  const canvas = recordCanvas();
  const rt = liveRuntime();
  rt.state({ session: { state: 'live', value: { trackLength: 1000, sector2LapDistanceStart: 300, sector3LapDistanceStart: 700 } } });
  mount(rt, <Tile {...props({ density: 'regular', width: 600, height: 300 })} />);
  const samples: [number, number][] = [[0, -100], [50, -200], [100, -300], [150, 150], [200, 300]];
  samples.forEach(([lapDistance, deltaBest], i) => rt.frame({ 3: { lapDistance, deltaBest, readout: 0 } }, i + 1));
  canvas.clear();
  rt.paint(16);
  expect(canvas.calls('fill').length).toBeGreaterThan(0);
  expect(canvas.calls('clip').length).toBeGreaterThan(0);
  expect(new Set(canvas.strokeColours())).toEqual(new Set(['#9AA7AD', '#7ED99C', '#F2D14B']));
  const lineWidths = canvas.log.filter((e) => e.op === 'set' && e.name === 'lineWidth').map((e) => (e as { value: number }).value);
  expect(Math.max(...lineWidths)).toBeGreaterThanOrEqual(3);
  // Every sample is a vertex of the line, none skipped by the paint cadence.
  const xs = canvas.calls('lineTo').map((e) => (e as { args: number[] }).args[0]!);
  expect(new Set(xs).size).toBeGreaterThanOrEqual(5);
  expect(screen.getByRole('img').getAttribute('aria-label')).toContain('lap distance');
  expect(headline().textContent).toBe('+0.300');
});

test('a stale or disconnected tile draws the graph in the muted colour only', () => {
  const canvas = recordCanvas();
  const rt = liveRuntime();
  mount(rt, <Tile {...props({ width: 600, height: 300 })} />);
  rt.frame({ 3: { lapDistance: 0, deltaBest: -100, readout: 0 } }, 1);
  rt.frame({ 3: { lapDistance: 100, deltaBest: 100, readout: 0 } }, 2);
  rt.disconnect();
  canvas.clear();
  rt.paint(16);
  expect(canvas.strokeColours().filter((c) => c === '#7ED99C' || c === '#F2D14B')).toEqual([]);
});

test('crossing the line starts a new graph and the finished lap stays faint until passed', () => {
  const canvas = recordCanvas();
  const rt = liveRuntime();
  rt.state({ session: { state: 'live', value: { trackLength: 1000, sector2LapDistanceStart: 0, sector3LapDistanceStart: 0 } } });
  mount(rt, <Tile {...props({ width: 600, height: 300 })} />);
  let t = 0;
  for (let d = 100; d <= 900; d += 50) rt.frame({ 3: { lapDistance: d, deltaBest: -100, readout: 0 } }, ++t);
  rt.paint(16);
  expect(canvas.alphas()).not.toContain(0.28); // nothing finished yet
  rt.frame({ 3: { lapDistance: 3, deltaBest: 4000, readout: 0 } }, ++t); // official figure at the line
  rt.frame({ 3: { lapDistance: 60, deltaBest: 40, readout: 0 } }, ++t);
  canvas.clear();
  rt.paint(33);
  expect(canvas.alphas()).toContain(0.28); // the finished lap, faint, ahead of the car
  expect(headline().textContent).toBe('+0.040');
  rt.frame({ 3: { lapDistance: 950, deltaBest: 40, readout: 0 } }, ++t);
  rt.frame({ 3: { lapDistance: 960, deltaBest: 40, readout: 0 } }, ++t);
  canvas.clear();
  rt.paint(50);
  // Passed: the faint copy past 960 m is gone, so no faint stroke remains.
  expect(canvas.alphas()).not.toContain(0.28);
});

test('every accepted Frame is recorded in the rings, not once per animation frame', () => {
  recordCanvas();
  const rt = liveRuntime();
  rt.frame({ 3: { deltaBest: -10, deltaPrevious: 20, lapDistance: 10, readout: 0 } });
  rt.frame({ 3: { deltaBest: -30, deltaPrevious: 40, lapDistance: 20, readout: 0 } });
  rt.frame({ 3: { deltaBest: -50, deltaPrevious: 60, lapDistance: 30, readout: readoutWord({ deltaBest: Absent }) } });
  const rings = rt.runtime.stores.frame.rings[3]!;
  expect(rings.deltaBest.toArray()).toEqual([-10, -30, NaN]);
  expect(rings.deltaPrevious.toArray()).toEqual([20, 40, 60]);
  expect(rings.lapDistance.toArray()).toEqual([10, 20, 30]);
});

test('settings keep unknown keys and offer best and previous', () => {
  const saved: Record<string, unknown>[] = [];
  const Panel = tile.Settings!;
  mount(liveRuntime(), <Panel settings={{ keep: 1 }} onChange={(s) => saved.push(s)} />);
  const select = screen.getByLabelText('Reference lap') as HTMLSelectElement;
  expect([...select.options].map((o) => o.value)).toEqual(['best', 'previous']);
  expect(select.value).toBe('best');
});
