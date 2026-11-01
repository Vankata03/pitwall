// @vitest-environment jsdom
import { cleanup, screen } from '@testing-library/react';
import { afterEach, expect, test } from 'vitest';
import { Absent, carState, liveReadout, liveRuntime, mount, props, readoutWord, Restricted, Stale } from '../frame-test-kit';
import tile from './sm';

afterEach(cleanup);
const Tile = tile.Component;
const text = (id: string) => screen.getByTestId(id).textContent;
const state = (id: string) => screen.getByTestId(id).dataset.state;

// Car 3 runs 2026 regulations (active aero), car 5 a pre-2026 car (DRS).
const aero = (available: number) =>
  liveReadout({ active_aero_mode: 0, active_aero_available: available, active_aero_activation_distance: 0, overtake_available: 0, overtake_active: 0, overtake_activation_distance: 0, regulations_2026: 1, driving_wrong_way: 0 } as never);
const status = (drsAllowed: number) => liveReadout({ drs_allowed: drsAllowed } as never);

function twoCarSession(rt: ReturnType<typeof liveRuntime>, opts: { aeroAvailable?: number; drsAllowed?: number } = {}) {
  rt.state({
    cars: {
      3: carState({ regulations: 'Regulations2026', aero: aero(opts.aeroAvailable ?? 0) }),
      5: carState({ regulations: 'Pre2026', status: status(opts.drsAllowed ?? 0) }),
    },
  });
}
const drive = (rt: ReturnType<typeof liveRuntime>, car: number, aeroState: number, groups: Parameters<typeof readoutWord>[0] = {}, ts = 16) => {
  rt.frame({ [car]: { aeroState, readout: readoutWord(groups) } });
  rt.paint(ts);
};

test('declares the catalogue entry: follows the car, 60 Hz, 2×2 minimum, 4×2 default', () => {
  expect(tile.declaration).toMatchObject({
    type: 'sm',
    name: 'SM',
    binding: 'follows-car',
    rate: 'frame',
    minSize: { w: 2, h: 2 },
    defaultSize: { w: 4, h: 2 },
  });
});

test('a 2026-regulations car shows SM, the active aero state', () => {
  const rt = liveRuntime();
  twoCarSession(rt);
  mount(rt, <Tile {...props({ car: { status: 'ok', index: 3 } })} />);
  drive(rt, 3, 1);
  expect(text('sm-label')).toBe('SM');
  expect(text('sm-state')).toBe('ACTIVE');
  drive(rt, 3, 0, {}, 33);
  expect(text('sm-state')).toBe('INACTIVE');
});

test('a pre-2026 car shows DRS in the same slot, from the same byte', () => {
  const rt = liveRuntime();
  twoCarSession(rt);
  mount(rt, <Tile {...props({ car: { status: 'ok', index: 5 } })} />);
  drive(rt, 5, 1);
  expect(text('sm-label')).toBe('DRS');
  expect(text('sm-state')).toBe('OPEN');
  drive(rt, 5, 0, {}, 33);
  expect(text('sm-state')).toBe('CLOSED');
});

test('availability, from State, shows between inactive and active', () => {
  const rt = liveRuntime();
  twoCarSession(rt, { aeroAvailable: 1, drsAllowed: 1 });
  const view = mount(rt, <Tile {...props({ car: { status: 'ok', index: 3 } })} />);
  drive(rt, 3, 0);
  expect(text('sm-state')).toBe('AVAILABLE');
  drive(rt, 3, 1, {}, 33);
  expect(text('sm-state')).toBe('ACTIVE');
  view.rerender(<Tile {...props({ car: { status: 'ok', index: 5 } })} />);
  drive(rt, 5, 0, {}, 50);
  expect(text('sm-state')).toBe('AVAILABLE');
});

test('the tile follows a car of the other regulations without remounting', () => {
  const rt = liveRuntime();
  twoCarSession(rt);
  const view = mount(rt, <Tile {...props({ car: { status: 'ok', index: 3 } })} />);
  drive(rt, 3, 1);
  expect(text('sm-label')).toBe('SM');
  view.rerender(<Tile {...props({ car: { status: 'ok', index: 5 } })} />);
  drive(rt, 5, 1, {}, 33);
  expect(text('sm-label')).toBe('DRS');
});

test('a mode the tile does not know is named, not guessed', () => {
  const rt = liveRuntime();
  twoCarSession(rt);
  mount(rt, <Tile {...props({ car: { status: 'ok', index: 3 } })} />);
  drive(rt, 3, 2);
  expect(text('sm-state')).toBe('MODE 2');
});

test('without the car’s regulations the byte cannot be read, so the readout is absent', () => {
  const rt = liveRuntime();
  rt.state({ cars: { 3: carState({ regulations: null }) } });
  mount(rt, <Tile {...props()} />);
  drive(rt, 3, 1);
  expect([state('sm-state'), text('sm-state')]).toEqual(['absent', '—']);
});

test('the four readout states: value, dimmed value, label, em dash', () => {
  const rt = liveRuntime();
  twoCarSession(rt);
  mount(rt, <Tile {...props()} />);
  drive(rt, 3, 1, { aero: 0 });
  expect([state('sm-state'), text('sm-state')]).toEqual(['live', 'ACTIVE']);
  drive(rt, 3, 1, { aero: Stale }, 33);
  expect([state('sm-state'), text('sm-state')]).toEqual(['stale', 'ACTIVE']);
  drive(rt, 3, 1, { aero: Restricted }, 50);
  expect([state('sm-state'), text('sm-state')]).toEqual(['restricted', 'Restricted']);
  drive(rt, 3, 1, { aero: Absent }, 66);
  expect([state('sm-state'), text('sm-state')]).toEqual(['absent', '—']);
});

test('values read stale while the collector is unreachable', () => {
  const rt = liveRuntime();
  twoCarSession(rt);
  mount(rt, <Tile {...props()} />);
  drive(rt, 3, 1);
  rt.disconnect();
  rt.paint(33);
  expect([state('sm-state'), text('sm-state')]).toEqual(['stale', 'ACTIVE']);
});

test('with no data the whole tile says so and keeps its shape', () => {
  const rt = liveRuntime();
  twoCarSession(rt);
  mount(rt, <Tile {...props()} />);
  drive(rt, 3, 0, { aero: Absent });
  expect(screen.getByTestId('tile-message').textContent).toBe('No data');
  expect(screen.getByTestId('sm-state')).toBeTruthy();
});

test('compact keeps the state; regular adds what the abbreviation means', () => {
  const rt = liveRuntime();
  twoCarSession(rt);
  const view = mount(rt, <Tile {...props({ density: 'compact', width: 100, height: 70 })} />);
  expect(screen.queryByTestId('sm-detail')).toBeNull();
  view.rerender(<Tile {...props({ density: 'regular', width: 220, height: 80 })} />);
  expect(text('sm-detail')).toBe('Straight mode');
  view.rerender(<Tile {...props({ density: 'regular', width: 220, height: 80, car: { status: 'ok', index: 5 } })} />);
  expect(text('sm-detail')).toBe('Drag reduction');
});
