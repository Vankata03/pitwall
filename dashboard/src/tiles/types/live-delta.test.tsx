// @vitest-environment jsdom
import { cleanup, screen } from '@testing-library/react';
import { afterEach, expect, test } from 'vitest';
import { liveRuntime, mount, props, readoutWord, stubCanvas, Restricted, Absent } from '../frame-test-kit';
import tile from './live-delta';

afterEach(cleanup);
const Tile = tile.Component;
test('delta-only Frames redraw immediately at frame rate, with four states and selectable reference', () => {
  stubCanvas();
  const rt = liveRuntime();
  const view = mount(rt, <Tile {...props()} />);
  const send = (deltaBest: number, word = 0) => rt.frame({ 3: { deltaBest, deltaPrevious: 432, lapDistance: 10, readout: word } });
  send(-123); rt.paint(16);
  expect(screen.getByTestId('live-delta-value').textContent).toBe('−0.123');
  send(124); rt.paint(33);
  expect(screen.getByTestId('live-delta-value').textContent).toBe('+0.124');
  view.rerender(<Tile {...props({ settings: { reference: 'previous' } })} />); rt.paint(50);
  expect(screen.getByTestId('live-delta-value').textContent).toBe('+0.432');
  view.rerender(<Tile {...props()} />);
  send(0, readoutWord({ deltaBest: Restricted })); rt.paint(66);
  expect(screen.getByTestId('live-delta-value').dataset.state).toBe('restricted');
  send(0, readoutWord({ deltaBest: Absent })); rt.paint(83);
  expect(screen.getByTestId('live-delta-value').dataset.state).toBe('absent');
  send(-123); rt.disconnect(); rt.paint(100);
  expect(screen.getByTestId('live-delta-value').dataset.state).toBe('stale');
});

test('every accepted Frame retains aligned delta samples, including missing values', () => {
  const rt = liveRuntime();
  rt.frame({ 3: { deltaBest: -10, deltaPrevious: 20, lapDistance: 10, readout: 0 } });
  rt.frame({ 3: { deltaBest: -30, deltaPrevious: 40, lapDistance: 20, readout: 0 } });
  rt.frame({ 3: { readout: 0x3fff } });
  const rings = rt.runtime.stores.frame.rings[3]!;
  expect(rings.deltaBest.toArray()).toEqual([-10, -30, NaN]);
  expect(rings.deltaPrevious.toArray()).toEqual([20, 40, NaN]);
  expect(rings.lapDistance.toArray()).toEqual([10, 20, NaN]);
});

test('regular draws all samples received between paints; compact keeps the headline without a graph', () => {
  const canvas = stubCanvas();
  const rt = liveRuntime();
  rt.state({ session: { state: 'live', value: { trackLength: 1000, sector2LapDistanceStart: 300, sector3LapDistanceStart: 700 } } });
  const view = mount(rt, <Tile {...props({ width: 600, height: 300 })} />);
  for (const [lapDistance, deltaBest] of [[0, -100], [50, -200], [100, -300]]) {
    rt.frame({ 3: { lapDistance, deltaBest, readout: 0 } });
  }
  rt.paint(16);
  expect(canvas.calls.filter(c => c.method === 'lineTo').length).toBeGreaterThan(5);
  expect(canvas.calls.some(c => c.method === 'fill')).toBe(true);
  expect(screen.getByRole('img').getAttribute('aria-label')).toContain('lap distance');
  view.rerender(<Tile {...props({ density: 'compact' })} />); rt.paint(33);
  expect(screen.queryByRole('img')).toBeNull();
  expect(screen.getByTestId('live-delta-value').textContent).toBe('−0.300');
});
