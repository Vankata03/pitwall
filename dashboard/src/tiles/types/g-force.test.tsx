// @vitest-environment jsdom
import { cleanup, screen } from '@testing-library/react';
import { afterEach, expect, test } from 'vitest';
import { Absent, liveRuntime, mount, props, readoutWord, Restricted, Stale, stubCanvas } from '../frame-test-kit';
import tile from './g-force';

afterEach(cleanup);
const Tile = tile.Component;
const text = (id: string) => screen.getByTestId(id).textContent;
const state = (id: string) => screen.getByTestId(id).dataset.state;

const drive = (rt: ReturnType<typeof liveRuntime>, cars: Record<number, Record<string, number>>, ts = 16) => {
  rt.frame(cars);
  rt.paint(ts);
};

test('declares the catalogue entry: follows the car, 60 Hz, 2×2 minimum, 3×2 default', () => {
  expect(tile.declaration).toMatchObject({
    type: 'g-force',
    binding: 'follows-car',
    rate: 'frame',
    minSize: { w: 2, h: 2 },
    defaultSize: { w: 3, h: 2 },
  });
});

test('shows lateral and longitudinal G, signed, from the wire’s thousandths', () => {
  const rt = liveRuntime();
  mount(rt, <Tile {...props()} />);
  drive(rt, { 3: { gLateral: 1830, gLongitudinal: -2140, readout: readoutWord() } });
  expect(text('g-lateral')).toBe('+1.8');
  expect(text('g-longitudinal')).toBe('−2.1');
  expect(screen.getAllByText('G').length).toBeGreaterThan(0);
});

test('a reading of zero carries no sign and a large one is shown as sent, never clamped', () => {
  const rt = liveRuntime();
  mount(rt, <Tile {...props()} />);
  drive(rt, { 3: { gLateral: 0, gLongitudinal: 10400, readout: readoutWord() } });
  expect(text('g-lateral')).toBe('+0.0');
  expect(text('g-longitudinal')).toBe('+10.4');
});

test('the four readout states: value, dimmed value, label, em dash', () => {
  const rt = liveRuntime();
  mount(rt, <Tile {...props()} />);
  const draw = (motion: 0 | 1 | 2 | 3) =>
    drive(rt, { 3: { gLateral: 1500, gLongitudinal: 500, readout: readoutWord({ motion }) } });
  draw(0);
  expect([state('g-lateral'), text('g-lateral')]).toEqual(['live', '+1.5']);
  draw(Stale);
  expect([state('g-lateral'), text('g-lateral'), state('g-longitudinal')]).toEqual(['stale', '+1.5', 'stale']);
  draw(Restricted);
  expect([state('g-lateral'), text('g-lateral')]).toEqual(['restricted', 'Restricted']);
  draw(Absent);
  expect([state('g-lateral'), text('g-lateral'), text('g-longitudinal')]).toEqual(['absent', '—', '—']);
});

test('values read stale while the collector is unreachable', () => {
  const rt = liveRuntime();
  mount(rt, <Tile {...props()} />);
  drive(rt, { 3: { gLateral: 1500, gLongitudinal: 500, readout: readoutWord() } });
  rt.disconnect();
  rt.paint(33);
  expect([state('g-lateral'), text('g-lateral')]).toEqual(['stale', '+1.5']);
});

test('with no data the whole tile says so and keeps its shape', () => {
  const rt = liveRuntime();
  mount(rt, <Tile {...props()} />);
  drive(rt, { 3: { readout: readoutWord({ motion: Absent }) } });
  expect(screen.getByTestId('tile-message').textContent).toBe('No data');
  expect(screen.getByTestId('g-lateral')).toBeTruthy();
});

test('compact keeps the two numbers; regular adds the G circle with a name and summary for screen readers', () => {
  const rt = liveRuntime();
  const view = mount(rt, <Tile {...props({ density: 'compact', width: 150, height: 70 })} />);
  expect(screen.queryByTestId('g-circle')).toBeNull();
  expect(screen.getByTestId('g-lateral')).toBeTruthy();
  view.rerender(<Tile {...props({ density: 'regular', width: 200, height: 200 })} />);
  const circle = screen.getByTestId('g-circle');
  expect(circle.getAttribute('role')).toBe('img');
  expect(circle.getAttribute('aria-label')).toMatch(/^G-force: /);
  drive(rt, { 3: { gLateral: 3000, gLongitudinal: 4000, readout: readoutWord() } });
  expect(text('g-total')).toBe('5.0');
});

test('the circle puts a dot where the car’s G puts it, and none while there is no value', () => {
  const canvas = stubCanvas();
  const rt = liveRuntime();
  mount(rt, <Tile {...props({ density: 'regular', width: 200, height: 200 })} />);
  const dot = () => canvas.calls.filter((c) => c.method === 'arc').at(-1)!.args as number[];

  drive(rt, { 3: { gLateral: 2000, gLongitudinal: 0, readout: readoutWord() } });
  const right = dot();
  drive(rt, { 3: { gLateral: -2000, gLongitudinal: 0, readout: readoutWord() } }, 33);
  const left = dot();
  expect(right[0]! - left[0]!).toBeGreaterThan(0); // positive lateral sits right of negative
  expect(right[1]).toBeCloseTo(left[1]!);

  // Beyond the circle's range the dot stops at the rim; the number still shows the truth.
  drive(rt, { 3: { gLateral: 10000, gLongitudinal: 0, readout: readoutWord() } }, 50);
  const rim = dot();
  expect(rim[0]!).toBeLessThanOrEqual((screen.getByTestId('g-circle') as HTMLCanvasElement).width);

  const before = canvas.calls.length;
  drive(rt, { 3: { gLateral: 1000, gLongitudinal: 0, readout: readoutWord({ motion: Absent }) } }, 66);
  const drawn = canvas.calls.slice(before).filter((c) => c.method === 'arc');
  expect(drawn.every((c) => (c.args as number[])[2]! > 8)).toBe(true); // rings only, no dot
});
