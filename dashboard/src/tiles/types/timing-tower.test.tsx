// @vitest-environment jsdom
import { act, cleanup, render, screen, within } from '@testing-library/react';
import { afterEach, expect, test, vi } from 'vitest';
import { RuntimeProvider } from '../../runtime/react';
import { createRuntime } from '../../runtime/runtime';
import { hello, snapshot } from '../../runtime/fixtures';
import { pickDensity } from '../../runtime/tile';
import { cellsToPixels } from '../../layout/screen';
import type { TileProps } from '../contract';
import tower, { TowerFocusProvider } from './timing-tower';

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});
const live = <T,>(value: T) => ({ state: 'live', value });
const cars = () =>
  Array.from({ length: 12 }, (_, i) => ({
    participant: live({
      name: Array.from(new TextEncoder().encode(`Driver ${i + 1}`)),
      race_number: i + 1,
      show_online_names: i === 2 ? 0 : 1,
      livery_colour: [{ red: 100, green: 120, blue: 140 }],
    }),
    retired: false,
    lap: live({
      car_position: i + 1,
      result_status: 2,
      pit_status: 0,
      num_pit_stops: 1,
      grid_position: i + 2,
      last_lap_time_in_ms: 91234,
    }),
    status: live({ visual_tyre_compound: 16, tyres_age_laps: 4 }),
    gaps: live({
      to_leader_ms: i * 2500,
      to_car_in_front_ms: i ? 2500 : null,
      laps_behind: i === 11 ? 1 : 0,
    }),
    tyreCondition: { state: 'restricted' },
    penalties: live({ seconds: 5, corner_cutting_warnings: 2, drive_throughs: 0, stop_gos: 0 }),
    best: { lap_time_ms: 90000 + i * 100 },
  }));
const props: TileProps = {
  id: 'tower',
  settings: {},
  density: 'full',
  width: 700,
  height: 1000,
  car: null,
  units: { speed: 'kmh', temperature: 'c', pressure: 'psi' },
  editing: false,
};
function show(
  options: Partial<TileProps> = {},
  state: Record<string, unknown> = {},
  focus: number | null = 5,
  history: unknown[] = [],
) {
  vi.useFakeTimers();
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
    requestFrame: () => 1,
    cancelFrame() {},
    reload() {},
    storage: { getItem: () => null, setItem() {} },
  });
  runtime.stores.setConnection('open');
  runtime.stores.ingest(hello(1));
  runtime.stores.ingest(
    snapshot(
      1,
      {
        lifecycle: 'Live',
        playerCar: 0,
        session: live({ sessionType: 15, safetyCarStatus: 1 }),
        cars: cars(),
        ...state,
      },
      history as never[],
    ),
  );
  act(() => vi.advanceTimersByTime(200));
  const Tile = tower.Component;
  const view = render(
    <RuntimeProvider runtime={runtime}>
      <TowerFocusProvider value={focus}>
        <Tile {...props} {...options} />
      </TowerFocusProvider>
    </RuntimeProvider>,
  );
  return { runtime, view };
}
test('Full race tower keeps fixed columns, hidden names, lapped gaps, focus and status', () => {
  show();
  expect(screen.getByRole('columnheader', { name: 'Last lap' })).toBeTruthy();
  expect(screen.getByText('#3')).toBeTruthy();
  expect(screen.queryByText('Driver 3')).toBeNull();
  expect(screen.getByText('+1 LAP')).toBeTruthy();
  expect(screen.getByText('Safety car')).toBeTruthy();
  const row = screen.getByText('Driver 6').closest('tr')!;
  expect(row.getAttribute('aria-current')).toBe('true');
  expect(within(row).getByText('Restricted')).toBeTruthy();
  const size = cellsToPixels('wide', 1249, { w: 9, h: 16 });
  expect(pickDensity(tower.declaration, size.width, size.height)).toBe('compact');
  expect(pickDensity(tower.declaration, 500, size.height)).toBe('full');
});

test('AI driver names appear in the tower even when online names are disabled', () => {
  const cs = cars();
  Object.assign(cs[2]!.participant.value, { ai_controlled: 1, show_online_names: 0 });
  show({}, { cars: cs });
  expect(screen.getByText('Driver 3')).toBeTruthy();
  expect(screen.queryByText('#3')).toBeNull();
});

test('compact pins the leader around focus and regular falls back without scrolling', () => {
  const { view } = show({ density: 'compact', height: 280 });
  expect(screen.getAllByRole('row')).toHaveLength(8);
  expect(screen.getByText('Driver 1')).toBeTruthy();
  expect(screen.getByText('Driver 6')).toBeTruthy();
  expect(screen.queryByRole('columnheader', { name: 'Stops' })).toBeNull();
  view.unmount();
  show({ density: 'regular', height: 260 });
  expect(screen.getAllByRole('row').length).toBeLessThan(8); // header and six one-line cars
  expect(screen.getByText('Driver 6')).toBeTruthy();
});
test('focus-relative compares leader gaps and timed sessions name the fallback reference', () => {
  const { view } = show({ settings: { gap: 'focus' } });
  expect(within(screen.getByText('Driver 5').closest('tr')!).getByText('−2.500')).toBeTruthy();
  expect(screen.getByRole('columnheader', { name: 'Gap to Driver 6' })).toBeTruthy();
  view.unmount();
  const cs = cars();
  cs[5]!.best = null as never;
  show({ settings: { gap: 'focus' } }, { cars: cs, session: live({ sessionType: 5, safetyCarStatus: 0 }) });
  expect(screen.getByRole('columnheader', { name: 'Gap to session best' })).toBeTruthy();
  expect(within(screen.getByText('Driver 1').closest('tr')!).getAllByText('1:30.000').length).toBe(2);
  expect(within(screen.getByText('Driver 6').closest('tr')!).getAllByText('—').length).toBeGreaterThan(0);
});
test('pit rows retain gaps, DSQ and retired cars sink, Time Trial replaces the tower', () => {
  const cs = cars();
  cs[0]!.retired = true;
  cs[1]!.lap.value.result_status = 5; // m_resultStatus 5 = disqualified
  cs[3]!.lap.value.pit_status = 1;
  const { view } = show({}, { cars: cs });
  const rows = screen.getAllByRole('row');
  expect(rows.at(-2)!.textContent).toContain('Driver 1');
  expect(rows.at(-1)!.textContent).toContain('Driver 2');
  expect(screen.getByText('DSQ')).toBeTruthy();
  expect(within(screen.getByText('Driver 4').closest('tr')!).getByText('PIT')).toBeTruthy();
  expect(within(screen.getByText('Driver 4').closest('tr')!).getByText('+2.500')).toBeTruthy();
  view.unmount();
  show({}, { session: live({ sessionType: 18 }) });
  expect(screen.getByText('Time Trial — only the player car runs')).toBeTruthy();
  expect(screen.queryByRole('table')).toBeNull();
});

test('result statuses follow the game codes: DNF and not classified also sink with their own text', () => {
  const cs = cars();
  cs[0]!.lap.value.result_status = 4; // did not finish
  cs[2]!.lap.value.result_status = 6; // not classified
  show({}, { cars: cs });
  const rows = screen.getAllByRole('row');
  expect(rows.at(-2)!.textContent).toContain('Driver 1');
  expect(rows.at(-1)!.textContent).toContain('#3'); // fixture hides this online name
  expect(within(screen.getByText('Driver 1').closest('tr')!).getByText('Retired')).toBeTruthy();
  expect(within(screen.getByText('#3').closest('tr')!).getByText('NC')).toBeTruthy();
  expect(screen.queryByText('DSQ')).toBeNull();
});

test('settings preserve unrelated keys and disconnect marks displayed values stale', () => {
  const { runtime } = show();
  const saved: Record<string, unknown>[] = [];
  const Settings = tower.Settings!;
  render(<Settings settings={{ other: 'keep' }} onChange={(s) => saved.push(s)} />);
  const selects = screen.getAllByRole('combobox');
  act(() => {
    const select = selects[0] as HTMLSelectElement;
    select.value = 'focus';
    select.dispatchEvent(new Event('change', { bubbles: true }));
  });
  expect(saved[0]).toEqual({ other: 'keep', gap: 'focus' });
  act(() => {
    runtime.stores.setConnection('reconnecting');
    vi.advanceTimersByTime(200);
  });
  expect(screen.getByText('Driver 6').closest('.pw-readout')!.getAttribute('data-state')).toBe('stale');
  expect(screen.getAllByText('Restricted')[0]!.getAttribute('data-state')).toBe('restricted');
});
test('red flag band keeps gaps visible and ignores rewound events', () => {
  const event = (id: string, rewound: boolean) => ({
    id,
    car: null,
    kind: 'raceControl',
    body: { rewound, kind: { Game: { details: 'RedFlag' } } },
  });
  const { view } = show({}, {}, 5, [event('red', false)]);
  expect(screen.getByText('Red flag')).toBeTruthy();
  expect(screen.getByText('+1 LAP')).toBeTruthy();
  view.unmount();
  show({}, {}, 5, [event('red', true)]);
  expect(screen.queryByText('Red flag')).toBeNull();
  expect(screen.getByText('Safety car')).toBeTruthy();
});
test('Full density shows the highest wheel wear with an explicit label', () => {
  const cs = cars();
  cs[0]!.tyreCondition = live([80, 70, 60, 50]) as never;
  show({}, { cars: cs });
  expect(screen.getByRole('columnheader', { name: 'Wear (max)' })).toBeTruthy();
  expect(screen.getByText('50%')).toBeTruthy();
});

test('Around focus means ±N, without adding distant cars at field edges', () => {
  show({ density: 'compact', height: 500, settings: { rows: 'around', radius: 3 } }, {}, 0);
  expect(screen.getAllByRole('row')).toHaveLength(5);
  expect(screen.queryByText('Driver 5')).toBeNull();
});

test.each(['compact', 'regular', 'full'] as const)('team colour sits beside the driver without a spare column in %s density', (density) => {
  show({ density });
  expect(screen.queryByRole('columnheader', { name: 'Team' })).toBeNull();
  const cell = screen.getByText('Driver 6').closest('td')!;
  expect(cell.querySelector('.pw-team-marker')?.getAttribute('style')).toContain('rgb(100, 120, 140)');
  expect(cell.querySelector('.pw-team-marker')?.getAttribute('aria-hidden')).toBe('true');
  const columns = screen.getAllByRole('columnheader').length;
  for (const row of screen.getAllByRole('row').slice(1)) expect(within(row).getAllByRole('cell')).toHaveLength(columns);
});

 test('tyres have a coloured letter badge and an accessible compound name', () => {
  show({ density: 'regular' });
  const badge = within(screen.getByText('Driver 6').closest('tr')!).getByLabelText('Soft tyres');
  expect(badge.textContent).toBe('S');
  expect(badge.getAttribute('data-compound')).toBe('S');
});
