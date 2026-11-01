// @vitest-environment jsdom
import { cleanup, screen } from '@testing-library/react';
import { afterEach, expect, test } from 'vitest';
import { Absent, carState, liveReadout, liveRuntime, mount, props, readoutWord, Restricted, Stale, stubCanvas } from '../frame-test-kit';
import tile from './speed-rpm-gear';

afterEach(cleanup);
const Tile = tile.Component;
const text = (id: string) => screen.getByTestId(id).textContent;
const state = (id: string) => screen.getByTestId(id).dataset.state;

const status = (maxRpm: number) =>
  liveReadout({ max_rpm: maxRpm, idle_rpm: 4000, max_gears: 8 } as never);
const withMaxRpm = (rt: ReturnType<typeof liveRuntime>, maxRpm = 13000) =>
  rt.state({ cars: { 3: carState({ status: status(maxRpm) }) } });

test('declares the catalogue entry: follows the car, 60 Hz, 4×3 minimum, 6×6 default', () => {
  expect(tile.declaration).toMatchObject({
    type: 'speed-rpm-gear',
    binding: 'follows-car',
    rate: 'frame',
    minSize: { w: 4, h: 3 },
    defaultSize: { w: 6, h: 6 },
  });
  expect(tile.declaration.densities.map((d) => d.density)).toEqual(['compact', 'regular', 'full']);
});

test('draws speed, RPM and gear of the followed car, in the device’s units', () => {
  const rt = liveRuntime();
  mount(rt, <Tile {...props()} />);
  rt.frame({ 3: { speed: 286, rpm: 11240, gear: 7, readout: readoutWord() } });
  rt.paint(16);
  expect(text('speed')).toBe('286');
  expect(text('speed-unit')).toBe('km/h');
  expect(text('rpm')).toBe('11,240');
  expect(text('gear')).toBe('7');
});

test('a units change redraws with no new frame', () => {
  const rt = liveRuntime();
  const view = mount(rt, <Tile {...props()} />);
  rt.frame({ 3: { speed: 200, rpm: 9000, gear: 5, readout: readoutWord() } });
  rt.paint(16);
  view.rerender(<Tile {...props({ units: { speed: 'mph', temperature: 'c', pressure: 'psi' } })} />);
  rt.paint(33);
  expect(text('speed')).toBe('124');
  expect(text('speed-unit')).toBe('mph');
});

test('gear reads R, N and 1 to 8', () => {
  const rt = liveRuntime();
  mount(rt, <Tile {...props()} />);
  for (const [gear, shown] of [[-1, 'R'], [0, 'N'], [8, '8']] as const) {
    rt.frame({ 3: { speed: 10, rpm: 5000, gear, readout: readoutWord() } });
    rt.paint(16);
    expect(text('gear')).toBe(shown);
  }
});

test('the four readout states: value, dimmed value, label, em dash', () => {
  const rt = liveRuntime();
  mount(rt, <Tile {...props()} />);
  const draw = (telemetry: 0 | 1 | 2 | 3) => {
    rt.frame({ 3: { speed: 180, rpm: 9000, gear: 4, readout: readoutWord({ telemetry }) } });
    rt.paint(16);
  };
  draw(0);
  expect([state('speed'), text('speed')]).toEqual(['live', '180']);
  draw(Stale);
  expect([state('speed'), text('speed'), state('gear'), state('rpm')]).toEqual(['stale', '180', 'stale', 'stale']);
  draw(Restricted);
  expect([state('speed'), text('speed')]).toEqual(['restricted', 'Restricted']);
  draw(Absent);
  // Zero never stands in for a missing value, whatever the frame bytes hold.
  expect([state('speed'), text('speed'), text('gear'), text('rpm')]).toEqual(['absent', '—', '—', '—']);
});

test('values read stale the moment the collector goes quiet', () => {
  const rt = liveRuntime();
  mount(rt, <Tile {...props()} />);
  rt.frame({ 3: { speed: 180, rpm: 9000, gear: 4, readout: readoutWord() } });
  rt.paint(16);
  rt.disconnect();
  rt.paint(33);
  expect([state('speed'), text('speed')]).toEqual(['stale', '180']);
});

test('with no readout holding data, one whole-tile message replaces the values and the shape stays', () => {
  const rt = liveRuntime();
  mount(rt, <Tile {...props()} />);
  rt.frame({ 3: { readout: readoutWord({ telemetry: Absent }) } });
  rt.paint(16);
  const message = screen.getByTestId('tile-message');
  expect(message.hidden).toBe(false);
  expect(message.textContent).toBe('No data');
  // The readouts are still in the document, so nothing reflows.
  expect(screen.getByTestId('speed')).toBeTruthy();

  rt.frame({ 3: { speed: 100, rpm: 9000, gear: 3, readout: readoutWord() } });
  rt.paint(33);
  expect(screen.getByTestId('tile-message').hidden).toBe(true);
});

test('the whole-tile message says why: game paused, waiting for the game', () => {
  const rt = liveRuntime();
  rt.state({ lifecycle: 'Lost', cars: { 3: carState() } });
  mount(rt, <Tile {...props()} />);
  rt.frame({ 3: { readout: readoutWord({ telemetry: Absent }) } });
  rt.paint(16);
  expect(screen.getByTestId('tile-message').textContent).toBe('No data: game paused or closed');
});

test('the shift strip fills with RPM against the car’s maximum', () => {
  const rt = liveRuntime();
  withMaxRpm(rt, 12000);
  mount(rt, <Tile {...props()} />);
  const lit = () => Number(screen.getByTestId('shift-strip').dataset.lit);
  const total = Number(screen.getByTestId('shift-strip').dataset.segments);
  rt.frame({ 3: { speed: 100, rpm: 6000, gear: 3, readout: readoutWord() } });
  rt.paint(16);
  expect(lit()).toBe(Math.round(total / 2));
  rt.frame({ 3: { speed: 100, rpm: 0, gear: 3, readout: readoutWord() } });
  rt.paint(33);
  expect(lit()).toBe(0);
});

test('at the limiter the strip is a steady full fill: it never changes between frames', () => {
  const rt = liveRuntime();
  withMaxRpm(rt, 12000);
  mount(rt, <Tile {...props()} />);
  const strip = screen.getByTestId('shift-strip');
  const total = Number(strip.dataset.segments);
  const seen = new Set<string>();
  // RPM bouncing off the limiter, even beyond the nominal maximum.
  [12000, 12150, 11950, 12100, 12000].forEach((rpm, i) => {
    rt.frame({ 3: { speed: 300, rpm, gear: 7, readout: readoutWord() } });
    rt.paint(16 * (i + 1));
    seen.add(`${strip.dataset.lit}|${strip.dataset.limiter}`);
  });
  expect(seen.size).toBe(1);
  expect(strip.dataset.lit).toBe(String(total));
});

test('the strip lights nothing when the car’s maximum RPM is unknown, rather than guessing', () => {
  const rt = liveRuntime();
  mount(rt, <Tile {...props()} />);
  rt.frame({ 3: { speed: 100, rpm: 9000, gear: 3, readout: readoutWord() } });
  rt.paint(16);
  expect(screen.getByTestId('shift-strip').dataset.lit).toBe('0');
  expect(text('rpm')).toBe('9,000');
});

test('density drops readouts and never shrinks text: compact keeps speed and gear only', () => {
  const rt = liveRuntime();
  mount(rt, <Tile {...props({ density: 'compact', width: 160, height: 110 })} />);
  expect(screen.getByTestId('speed')).toBeTruthy();
  expect(screen.getByTestId('gear')).toBeTruthy();
  expect(screen.queryByTestId('shift-strip')).toBeNull();
  expect(screen.queryByTestId('rpm')).toBeNull();
  expect(screen.queryByTestId('rpm-dial')).toBeNull();
});

test('full density adds the RPM dial, and the canvas exposes the tile name and a summary', () => {
  const rt = liveRuntime();
  mount(rt, <Tile {...props({ density: 'full', width: 420, height: 380 })} />);
  const dial = screen.getByTestId('rpm-dial');
  expect(dial.getAttribute('role')).toBe('img');
  expect(dial.getAttribute('aria-label')).toMatch(/^Speed, RPM and gear: /);
});

test('the dial draws a track and a fill to the share of the maximum RPM, steadily', () => {
  const canvas = stubCanvas();
  const rt = liveRuntime();
  withMaxRpm(rt, 12000);
  mount(rt, <Tile {...props({ density: 'full', width: 420, height: 380 })} />);
  rt.frame({ 3: { speed: 100, rpm: 6000, gear: 3, readout: readoutWord() } });
  rt.paint(16);
  const arcs = canvas.calls.filter((c) => c.method === 'arc').slice(-2);
  const [track, fill] = arcs.map((c) => c.args as number[]);
  expect(track![3]! - track![4]!).toBeCloseTo(-Math.PI * 1.5);
  expect(fill![4]! - fill![3]!).toBeCloseTo(Math.PI * 0.75); // half of the 270° sweep
});
