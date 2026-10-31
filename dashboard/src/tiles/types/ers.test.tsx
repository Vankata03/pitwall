// @vitest-environment jsdom
import { cleanup, screen } from '@testing-library/react';
import { afterEach, expect, test } from 'vitest';
import { Absent, carState, liveReadout, liveRuntime, mount, props, readoutWord, Restricted, Stale } from '../frame-test-kit';
import tile from './ers';

afterEach(cleanup);
const Tile = tile.Component;
const text = (id: string) => screen.getByTestId(id).textContent;
const state = (id: string) => screen.getByTestId(id).dataset.state;
const full = { density: 'full', width: 380, height: 200 } as const;

const ers = (over: Record<string, unknown> = {}) =>
  liveReadout({
    store_energy: 2_560_000,
    deploy_mode: 1,
    harvested_this_lap_mguk: 143_000,
    harvested_this_lap_mguh: 144_000,
    deployed_this_lap: 106_000,
    harvest_limit_per_lap: null,
    engine_power_ice: 0,
    engine_power_mguk: 0,
    ...over,
  } as never);
const aero = (over: Record<string, unknown> = {}) =>
  liveReadout({ active_aero_mode: 0, active_aero_available: 0, overtake_available: 0, overtake_active: 0, ...over } as never);

const percent = (rt: ReturnType<typeof liveRuntime>, ersPercent: number, groups: Parameters<typeof readoutWord>[0] = {}, ts = 16) => {
  rt.frame({ 3: { ersPercent, readout: readoutWord(groups) } });
  rt.paint(ts);
};

test('declares the catalogue entry: follows the car, 60 Hz, 4×2 minimum, 6×4 default', () => {
  expect(tile.declaration).toMatchObject({
    type: 'ers',
    binding: 'follows-car',
    rate: 'frame',
    minSize: { w: 4, h: 2 },
    defaultSize: { w: 6, h: 4 },
  });
});

test('draws the ERS percentage and a battery of segments from the frame', () => {
  const rt = liveRuntime();
  mount(rt, <Tile {...props({ density: 'compact', width: 220, height: 72 })} />);
  percent(rt, 64);
  expect(text('ers-percent')).toBe('64%');
  const battery = screen.getByTestId('ers-battery');
  expect(battery.dataset.lit).toBe('13'); // 64% of 20 segments
  percent(rt, 0, {}, 33);
  expect(text('ers-percent')).toBe('0%');
  expect(battery.dataset.lit).toBe('0');
});

test('the four readout states of the percentage: value, dimmed value, label, em dash', () => {
  const rt = liveRuntime();
  mount(rt, <Tile {...props({ density: 'compact', width: 220, height: 72 })} />);
  percent(rt, 64, { status: 0 });
  expect([state('ers-percent'), text('ers-percent')]).toEqual(['live', '64%']);
  percent(rt, 64, { status: Stale }, 33);
  expect([state('ers-percent'), text('ers-percent')]).toEqual(['stale', '64%']);
  percent(rt, 64, { status: Restricted }, 50);
  expect([state('ers-percent'), text('ers-percent'), screen.getByTestId('ers-battery').dataset.lit]).toEqual(['restricted', 'Restricted', '0']);
  percent(rt, 64, { status: Absent }, 66);
  // Zero is not a value: the bar is empty because there is nothing, and the number is a dash.
  expect([state('ers-percent'), text('ers-percent'), screen.getByTestId('ers-battery').dataset.lit]).toEqual(['absent', '—', '0']);
});

test('values read stale while the collector is unreachable', () => {
  const rt = liveRuntime();
  rt.state({ cars: { 3: carState({ ers: ers() }) } });
  mount(rt, <Tile {...props({ density: 'regular', width: 260, height: 120 })} />);
  percent(rt, 64);
  rt.disconnect();
  rt.paint(33);
  expect([state('ers-percent'), state('ers-mgu-k')]).toEqual(['stale', 'stale']);
});

test('regular adds the deploy mode, harvest per MGU and energy deployed, in kJ', () => {
  const rt = liveRuntime();
  rt.state({ cars: { 3: carState({ ers: ers() }) } });
  mount(rt, <Tile {...props({ density: 'regular', width: 260, height: 120 })} />);
  percent(rt, 64);
  expect(text('ers-mode')).toBe('Medium');
  expect(text('ers-mgu-k')).toBe('143 kJ');
  expect(text('ers-mgu-h')).toBe('144 kJ');
  expect(text('ers-deployed')).toBe('106 kJ');
});

test('deploy modes are named, and a mode it does not know is shown by number', () => {
  const rt = liveRuntime();
  const names: [number, string][] = [[0, 'None'], [2, 'Hotlap'], [3, 'Overtake'], [9, 'Mode 9']];
  for (const [mode, name] of names) {
    cleanup();
    rt.state({ cars: { 3: carState({ ers: ers({ deploy_mode: mode }) }) } });
    mount(rt, <Tile {...props({ density: 'regular', width: 260, height: 120 })} />);
    expect(text('ers-mode')).toBe(name);
  }
});

test('compact keeps the percentage and battery and drops every other row', () => {
  const rt = liveRuntime();
  rt.state({ cars: { 3: carState({ ers: ers() }) } });
  mount(rt, <Tile {...props({ density: 'compact', width: 220, height: 72 })} />);
  for (const id of ['ers-mode', 'ers-mgu-k', 'ers-mgu-h', 'ers-deployed', 'ers-overtake', 'ers-boost', 'ers-harvest-limit']) {
    expect(screen.queryByTestId(id), id).toBeNull();
  }
});

test('a 2026 car adds overtake state, boost and the harvest limit; a pre-2026 car hides those rows', () => {
  const rt = liveRuntime();
  rt.state({
    cars: {
      3: carState({ regulations: 'Regulations2026', ers: ers({ harvest_limit_per_lap: 8_500_000 }), aero: aero() }),
      5: carState({ regulations: 'Pre2026', ers: ers(), aero: aero() }),
    },
  });
  const view = mount(rt, <Tile {...props({ ...full, car: { status: 'ok', index: 3 } })} />);
  expect(screen.getByTestId('ers-overtake')).toBeTruthy();
  expect(screen.getByTestId('ers-boost')).toBeTruthy();
  expect(text('ers-harvest-limit')).toBe('8,500 kJ');
  view.rerender(<Tile {...props({ ...full, car: { status: 'ok', index: 5 } })} />);
  for (const id of ['ers-overtake', 'ers-boost', 'ers-harvest-limit']) expect(screen.queryByTestId(id), id).toBeNull();
  expect(screen.getByTestId('ers-mgu-k')).toBeTruthy();
});

test('overtake is active, available or off; boost is the overtake deploy mode while overtake is not active', () => {
  const rt = liveRuntime();
  const show = (aeroOver: Record<string, unknown>, mode: number) => {
    cleanup();
    rt.state({ cars: { 3: carState({ regulations: 'Regulations2026', ers: ers({ deploy_mode: mode }), aero: aero(aeroOver) }) } });
    mount(rt, <Tile {...props(full)} />);
    return [text('ers-overtake'), text('ers-boost')];
  };
  expect(show({ overtake_active: 1 }, 3)).toEqual(['Active', 'Off']);
  expect(show({ overtake_active: 0, overtake_available: 1 }, 3)).toEqual(['Available', 'On']);
  expect(show({}, 3)).toEqual(['Off', 'On']);
  expect(show({}, 1)).toEqual(['Off', 'Off']);
});

test('a harvest limit the game did not send is absent, never zero', () => {
  const rt = liveRuntime();
  rt.state({ cars: { 3: carState({ regulations: 'Regulations2026', ers: ers({ harvest_limit_per_lap: null }), aero: aero() }) } });
  mount(rt, <Tile {...props(full)} />);
  expect([state('ers-harvest-limit'), text('ers-harvest-limit')]).toEqual(['absent', '—']);
});

test('the slow rows show the four states of the State readout they read', () => {
  const rt = liveRuntime();
  const rows = ['ers-mode', 'ers-mgu-k', 'ers-mgu-h', 'ers-deployed'];
  const states = () => rows.map(state);
  rt.state({ cars: { 3: carState({ ers: { state: 'stale', value: (ers() as { value: never }).value } }) } });
  mount(rt, <Tile {...props({ density: 'regular', width: 260, height: 120 })} />);
  expect(states()).toEqual(['stale', 'stale', 'stale', 'stale']);
  rt.state({ cars: { 3: carState({ ers: { state: 'restricted' } }) } });
  expect(states()).toEqual(['restricted', 'restricted', 'restricted', 'restricted']);
  expect(text('ers-mgu-k')).toBe('Restricted');
  rt.state({ cars: { 3: carState({ ers: { state: 'absent' } }) } });
  expect(states()).toEqual(['absent', 'absent', 'absent', 'absent']);
  expect(text('ers-mgu-k')).toBe('—');
});

test('the whole-tile message appears only when no readout has data', () => {
  const rt = liveRuntime();
  rt.state({ cars: { 3: carState({ ers: ers() }) } });
  mount(rt, <Tile {...props({ density: 'regular', width: 260, height: 120 })} />);
  percent(rt, 0, { status: Absent });
  // The rows still hold data, so the tile keeps showing them.
  expect(screen.getByTestId('tile-message').hidden).toBe(true);
  rt.state({ cars: { 3: carState({ ers: { state: 'absent' } }) } });
  percent(rt, 0, { status: Absent }, 33);
  expect(screen.getByTestId('tile-message').textContent).toBe('No data');
  expect(screen.getByTestId('ers-percent')).toBeTruthy();
});
