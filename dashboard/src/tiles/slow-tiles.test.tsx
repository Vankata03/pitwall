// @vitest-environment jsdom
import { act, cleanup, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { RuntimeProvider } from '../runtime/react';
import { createRuntime } from '../runtime/runtime';
import { hello, snapshot } from '../runtime/fixtures';
import { registry } from './registry';
import type { TileProps } from './contract';

beforeEach(() => vi.useFakeTimers());
afterEach(() => { cleanup(); vi.useRealTimers(); });
const live = (value: unknown) => ({ state: 'live', value });
const props: TileProps = { id: 'test', car: { status: 'ok', index: 0 }, settings: {}, density: 'regular', width: 320, height: 160, units: { speed: 'kmh', temperature: 'c', pressure: 'psi' }, editing: false };
function setup(type: string, state: Record<string, unknown>, over: Partial<TileProps> = {}, history: Parameters<typeof snapshot>[2] = []) {
  const runtime = createRuntime({ url: 'ws://test/ws', ownBuildId: 'b1', createSocket: () => ({ binaryType: '', onopen: null, onmessage: null, onclose: null, onerror: null, close() {} }), requestFrame: () => 1, cancelFrame() {}, reload() {}, storage: { getItem: () => null, setItem() {} } });
  runtime.stores.setConnection('open');
  runtime.stores.ingest(hello(1));
  runtime.stores.ingest(snapshot(1, { lifecycle: 'live', ...state }, history));
  vi.advanceTimersByTime(100);
  const Tile = registry.get(type)?.Component;
  expect(Tile, `${type} registered`).toBeDefined();
  if (!Tile) throw new Error(type);
  const view = render(<RuntimeProvider runtime={runtime}><Tile {...props} {...over} /></RuntimeProvider>);
  return { runtime, ...view };
}
function row(label: string) { return screen.getByText(label).closest('div')!; }

test('engine temperature follows its bound car and device temperature units', () => {
  setup('engine-temp', { cars: [{ telemetry: live({ engine_temperature: 100 }) }, { telemetry: live({ engine_temperature: 110 }) }] }, { car: { status: 'ok', index: 1 }, units: { ...props.units, temperature: 'f' } });
  expect(row('Engine temperature').textContent).toContain('230 °F');
});

test('fuel shows tank kilograms and a signed laps delta, never a unit conversion', () => {
  setup('fuel', { cars: [{ fuel: live({ in_tank: 18.25, remaining_laps: -1.2 }) }] });
  expect(row('Fuel in tank').textContent).toContain('18.3 kg');
  expect(row('Fuel delta').textContent).toContain('−1.20 laps');
});

test('track discipline keeps penalties, warnings and invalid status distinct', () => {
  setup('track-discipline', { cars: [{ penalties: live({ seconds: 5, total_warnings: 3, corner_cutting_warnings: 2, drive_throughs: 1, stop_gos: 0 }), lap: live({ current_lap_invalid: 1 }) }] });
  expect(row('Corner cuts').textContent).toContain('2');
  expect(row('Warnings').textContent).toContain('3');
  expect(row('Penalties').textContent).toContain('5 s');
  expect(row('Drive-through').textContent).toContain('1');
  expect(row('Stop-go').textContent).toContain('0');
  expect(row('Lap validity').textContent).toContain('Invalid');
});

test('gaps uses the next position’s interval for behind, irrespective of car-array order', () => {
  setup('gaps', { cars: [{ lap: live({ car_position: 2 }), gaps: live({ to_car_in_front_ms: 1250 }) }, { lap: live({ car_position: 4 }), gaps: live({ to_car_in_front_ms: 9000 }) }, { lap: live({ car_position: 3 }), gaps: live({ to_car_in_front_ms: 2750 }) }] });
  expect(row('Ahead').textContent).toContain('+1.250 s');
  expect(row('Behind').textContent).toContain('−2.750 s');
});

test('lap state shows lap total, position, one-based sector and named pit status', () => {
  setup('lap-state', { session: live({ totalLaps: 44 }), cars: [{ lap: live({ current_lap_num: 12, car_position: 3, sector: 1, pit_status: 2 }) }] });
  expect(row('Lap').textContent).toContain('12');
  expect(row('Total').textContent).toContain('44');
  expect(row('Position').textContent).toContain('3');
  expect(row('Sector').textContent).toContain('2');
  expect(row('Pit status').textContent).toContain('In pit');
});

test('weather shows sky, temperatures and forecast rain with its horizon', () => {
  setup('weather', { session: live({ sessionType: 10, weather: 2, airTemperature: 20, trackTemperature: 30, forecast: [{ session_type: 10, time_offset: 5, weather: 4, rain_percentage: 70, air_temperature: 19, track_temperature: 27 }] }) }, { car: null });
  expect(row('Air').textContent).toContain('20 °C');
  expect(row('Track').textContent).toContain('30 °C');
  expect(row('Sky').textContent).toContain('Overcast');
  expect(row('Forecast').textContent).toContain('+5 min: Heavy rain');
  expect(row('Rain chance').textContent).toContain('70%');
});

const raceEntry = (id: number, kind: unknown, cars: number[] = [], rewound = false) => ({ kind: 'raceControl', id: `race-control:${id}`, car: null, body: { id, session_time: id * 10, lap_number: 2, cars, source: 'Derived', kind, rewound } });
test('race-control filters entry types, names hidden drivers by race number, and marks rewound entries', () => {
  setup('race-control-feed', { cars: [{ participant: live({ name: Array.from(new TextEncoder().encode('private-id')), race_number: 27, show_online_names: 0 }) }] }, { car: null, settings: { entryTypes: ['PitEntry'] } }, [raceEntry(1, 'PitEntry', [0], true), raceEntry(2, { WeatherChanged: { previous: 0, weather: 4 } })]);
  expect(screen.getByRole('list').textContent).toContain('#27');
  expect(screen.getByRole('list').textContent).toContain('Rewound');
  expect(screen.getByRole('list').textContent).not.toContain('private-id');
  expect(screen.getByRole('list').textContent).not.toContain('Weather');
});

for (const type of ['engine-temp', 'fuel', 'track-discipline', 'gaps', 'lap-state', 'weather']) {
  for (const state of ['stale', 'restricted', 'absent'] as const) {
    test(`${type} keeps ${state} readouts explicit and never substitutes zero`, () => {
      const telemetry = { state, value: { engine_temperature: 100 } };
      const fuel = { state, value: { in_tank: 10, remaining_laps: 1 } };
      const penalties = { state, value: { seconds: 5, total_warnings: 3, corner_cutting_warnings: 2, drive_throughs: 1, stop_gos: 0 } };
      const lap = { state, value: { current_lap_num: 2, car_position: 1, sector: 0, pit_status: 0, current_lap_invalid: 0 } };
      const gaps = { state, value: { to_car_in_front_ms: 1234 } };
      const session = { state, value: { totalLaps: 44, sessionType: 10, weather: 0, airTemperature: 20, trackTemperature: 30, forecast: [] } };
      // Missing readouts genuinely carry no value, matching the generated wire contract.
      const wrap = (r: { state: string; value: unknown }) => state === 'stale' ? r : { state };
      const { container } = setup(type, { cars: [{ telemetry: wrap(telemetry), fuel: wrap(fuel), penalties: wrap(penalties), lap: wrap(lap), gaps: wrap(gaps) }], session: wrap(session) });
      const readouts = [...container.querySelectorAll('.pw-readout')];
      expect(readouts.length).toBeGreaterThan(0);
      expect(readouts.some(r => r.getAttribute('data-state') === state)).toBe(true);
      if (state !== 'stale') {
        expect(readouts.every(r => r.textContent === (state === 'restricted' ? 'Restricted' : '—') || r.textContent === '—')).toBe(true);
        expect(screen.getByRole('status').textContent).toBe('No data');
      } else expect(screen.queryByRole('status')).toBeNull();
    });
  }
}

test('a slow tile redraws on State updates and dims live values after disconnect', () => {
  const { runtime } = setup('engine-temp', { cars: [{ telemetry: live({ engine_temperature: 90 }) }] });
  act(() => {
    runtime.stores.ingest(JSON.stringify({ type: 'state', epoch: 1, lifecycle: 'Live', cars: [{ telemetry: live({ engine_temperature: 105 }) }] }));
    vi.advanceTimersByTime(100);
  });
  expect(row('Engine temperature').textContent).toContain('105 °C');
  act(() => { runtime.stores.setConnection('reconnecting'); vi.advanceTimersByTime(100); });
  expect(row('Engine temperature').querySelector('.pw-readout')?.getAttribute('data-state')).toBe('stale');
});

test('the race-control filter preserves unknown settings and unknown entry types', () => {
  const Settings = registry.get('race-control-feed')!.Settings!;
  const save = vi.fn();
  render(<Settings settings={{ extra: { keep: true }, entryTypes: ['FutureKind', 'PitEntry'] }} onChange={save} />);
  screen.getByLabelText('Pit entry').click();
  expect(save).toHaveBeenCalledWith({ extra: { keep: true }, entryTypes: ['FutureKind'] });
});

test('race-control history updates and epoch resets replace visible entries', () => {
  const { runtime } = setup('race-control-feed', {}, { car: null }, [raceEntry(1, 'PitEntry')]);
  act(() => {
    runtime.stores.ingest(JSON.stringify({ type: 'history', epoch: 1, entries: [raceEntry(2, 'PitExit')] }));
    vi.advanceTimersByTime(100);
  });
  expect(screen.getByRole('list').textContent).not.toContain('Pit exit');
  act(() => vi.advanceTimersByTime(2900));
  expect(screen.getByRole('list').textContent).toContain('Pit exit');
  act(() => { runtime.stores.ingest(snapshot(2, { lifecycle: 'Live' }, [])); vi.advanceTimersByTime(100); });
  expect(screen.getByRole('list').textContent).toBe('');
});

test.each([74, 320])('race-control ignores controller and mode telemetry in a %ipx tile', (height) => {
  const game = (details: unknown) => ({ Game: { code: [66, 85, 84, 78], details } });
  const { runtime } = setup('race-control-feed', {}, { car: null, height }, [
    raceEntry(1, { Game: { code: [76, 71, 79, 84], details: 'LightsOut' } }),
    raceEntry(2, game({ Buttons: { button_status: 1 } })),
    raceEntry(3, game('PartialAeroEnabled')),
    raceEntry(4, game('OvertakeEnabled')),
    raceEntry(5, game('Unknown')),
    raceEntry(6, game({ SpeedTrap: { speed: 250, vehicle_idx: 0 } })),
  ]);
  expect(screen.getAllByRole('listitem')).toHaveLength(1);
  const before = screen.getByRole('list').textContent;
  act(() => {
    runtime.stores.ingest(JSON.stringify({ type: 'history', epoch: 1, entries: [raceEntry(7, game({ Buttons: { button_status: 2 } }))] }));
    vi.advanceTimersByTime(4000);
  });
  expect(screen.getByRole('list').textContent).toBe(before);
});

test('race-control safety announcements bypass the routine hold and a seek clears pending headlines', () => {
  const { runtime } = setup('race-control-feed', {}, { car: null, height: 320 }, [raceEntry(1, 'PitEntry')]);
  const append = (entries: unknown[]) => act(() => {
    runtime.stores.ingest(JSON.stringify({ type: 'history', epoch: 1, entries }));
    vi.advanceTimersByTime(100);
  });
  append([raceEntry(2, 'PitExit')]);
  expect(screen.getByRole('list').textContent).not.toContain('Pit exit');
  append([raceEntry(3, { Game: { code: [82, 69, 68, 70], details: 'RedFlag' } })]);
  expect(screen.getAllByRole('listitem')[0]!.textContent).toContain('Red Flag');
  append([raceEntry(4, { WeatherChanged: { previous: 0, weather: 4 } })]);
  expect(screen.getByRole('list').textContent).not.toContain('Heavy rain');
  act(() => { runtime.stores.ingest(snapshot(2, { lifecycle: 'Live' }, [])); vi.advanceTimersByTime(100); });
  act(() => vi.advanceTimersByTime(4000));
  expect(screen.getByRole('list').textContent).toBe('');
});

test('per-car flag bursts are batched while the track status updates immediately', () => {
  const { runtime } = setup('race-control-feed', { session: live({ safetyCarStatus: 0 }) }, { car: null, height: 74 }, [raceEntry(1, 'PitEntry')]);
  const append = (entries: unknown[]) => act(() => {
    runtime.stores.ingest(JSON.stringify({ type: 'history', epoch: 1, entries }));
    vi.advanceTimersByTime(100);
  });
  append([raceEntry(2, { MarshalFlagChanged: { zone: 1, previous: 1, flag: 3 } })]);
  expect(screen.getByTestId('race-control-status').textContent).toBe('Yellow flag');
  expect(screen.getByRole('list').textContent).not.toContain('Yellow flag');
  for (let i = 3; i < 15; i++) append([raceEntry(i, { FlagChanged: { previous: 1, flag: 3 } }, [i])]);
  expect(screen.getAllByRole('listitem')).toHaveLength(1);
  act(() => vi.advanceTimersByTime(1700));
  expect(screen.getByRole('list').textContent).toContain('Yellow flag');
  append([raceEntry(15, { MarshalFlagChanged: { zone: 1, previous: 3, flag: 1 } })]);
  expect(screen.getByTestId('race-control-status').textContent).toBe('Track clear');
});

test('gaps at the field boundary stays absent and stale neighbor positions taint the behind gap', () => {
  const { container } = setup('gaps', { cars: [{ lap: live({ car_position: 1 }), gaps: live({ to_car_in_front_ms: null }) }, { lap: { state: 'stale', value: { car_position: 2 } }, gaps: live({ to_car_in_front_ms: 2000 }) }] });
  expect(row('Ahead').textContent).toContain('—');
  expect(row('Behind').querySelector('.pw-readout')?.getAttribute('data-state')).toBe('stale');
  expect(container.querySelector('[role="status"]')).toBeNull();
});

test('weather never invents forecast rain when no matching session sample exists', () => {
  setup('weather', { session: live({ sessionType: 10, weather: 0, airTemperature: 22, trackTemperature: 34, forecast: [{ session_type: 11, time_offset: 5, weather: 5, rain_percentage: 100 }] }) });
  expect(row('Rain chance').textContent).toContain('—');
  expect(row('Forecast').textContent).toContain('—');
  expect(screen.queryByRole('status')).toBeNull();
});

test('compact densities drop secondary readouts without changing the primary values', () => {
  setup('track-discipline', { cars: [{ penalties: live({ seconds: 5, total_warnings: 3, corner_cutting_warnings: 2 }), lap: live({ current_lap_invalid: 0 }) }] }, { density: 'compact' });
  expect(row('Penalties').textContent).toContain('5 s');
  expect(screen.queryByText('Stop-go')).toBeNull();
});

test('race-control uses words for flags and safety car states, and latest entries come first', () => {
  setup('race-control-feed', {}, { car: null, height: 600 }, [raceEntry(1, { FlagChanged: { previous: 0, flag: 3 } }), raceEntry(2, { SafetyCarChanged: { previous: 0, status: 2 } }), raceEntry(3, { Game: { code: [82, 69, 68, 70], details: 'RedFlag' } })]);
  const items = screen.getAllByRole('listitem');
  expect(items[0]!.textContent).toContain('Red Flag');
  expect(items[1]!.textContent).toContain('VSC');
  expect(items[2]!.textContent).toContain('Flag Y');
});

test('a short, wide race-control tile is a one-line strip: track status, then the newest events first with names', () => {
  const names = (n: string) => live({ name: Array.from(new TextEncoder().encode(n)), race_number: 1, show_online_names: 1 });
  setup('race-control-feed', { cars: [{ participant: names('Alpha') }, { participant: names('Bravo') }], session: live({ safetyCarStatus: 0, sessionType: 10, weather: 0, forecast: [] }) }, { car: null, width: 940, height: 74 }, [
    raceEntry(1, { Game: { code: [76, 71, 79, 84], details: 'LightsOut' } }),
    raceEntry(2, { Game: { code: [79, 86, 84, 75], details: { Overtake: { overtaking_vehicle_idx: 0, being_overtaken_vehicle_idx: 1 } } } }),
  ]);
  expect(screen.getByTestId('race-control-status').textContent).toBe('Track clear');
  const items = screen.getAllByRole('listitem');
  expect(items[0]!.textContent).toContain('OVTK');
  expect(items[0]!.textContent).toContain('Alpha passes Bravo');
  expect(items[1]!.textContent).toContain('Lights Out');
});
