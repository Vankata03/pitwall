// @vitest-environment jsdom
import { act, cleanup, render, screen, within } from '@testing-library/react';
import { afterEach, expect, test, vi } from 'vitest';
import { RuntimeProvider } from '../../runtime/react';
import { createRuntime } from '../../runtime/runtime';
import { hello, snapshot } from '../../runtime/fixtures';
import type { TileProps } from '../contract';
import stint from './stint';
afterEach(() => {
  cleanup();
  vi.useRealTimers();
});
const live = <T,>(value: T) => ({ state: 'live', value });
const props: TileProps = {
  id: 'stint',
  settings: {},
  density: 'regular',
  width: 400,
  height: 350,
  car: { status: 'ok', index: 0 },
  units: { speed: 'kmh', temperature: 'c', pressure: 'psi' },
  editing: false,
};
function show(car = 0, restricted = false) {
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
  const condition = restricted ? { state: 'restricted' } : live([80, 70, 60, 50]);
  runtime.stores.ingest(
    snapshot(
      1,
      {
        lifecycle: 'Live',
        playerCar: 0,
        cars: [
          {
            lap: live({ current_lap_num: 14 }),
            status: live({ visual_tyre_compound: 17, tyres_age_laps: 4 }),
            tyreCondition: condition,
          },
          {
            lap: live({ current_lap_num: 14 }),
            status: live({ visual_tyre_compound: 17, tyres_age_laps: 4 }),
            tyreCondition: condition,
          },
        ],
        pitWindow: live({ car_index: 0, ideal_lap: 18, latest_lap: 22, rejoin_position: 8 }),
      },
      [
        {
          id: 'stint:0:1',
          kind: 'stint',
          car,
          body: {
            id: 1,
            start_lap: 1,
            end_lap: 10,
            visual_compound: 16,
            starting_age: 2,
            tyre_age_laps: 12,
            tyre_condition: condition,
          },
        },
        {
          id: 'stint:0:2',
          kind: 'stint',
          car,
          body: {
            id: 2,
            start_lap: 11,
            end_lap: null,
            visual_compound: 17,
            starting_age: 0,
            tyre_age_laps: 4,
            tyre_condition: condition,
          },
        },
        {
          id: 'stop:0:1',
          kind: 'pitStop',
          car,
          body: { id: 1, lap_number: 10, exit_time: 100, lane_time_ms: 22000, stopped_time_ms: 2300 },
        },
      ],
    ),
  );
  act(() => vi.advanceTimersByTime(200));
  const Tile = stint.Component;
  render(
    <RuntimeProvider runtime={runtime}>
      <Tile {...props} car={{ status: 'ok', index: car }} />
    </RuntimeProvider>,
  );
  return runtime;
}
test('stint shows completed and current sets, used starting age, stops and player pit window', () => {
  show();
  expect(screen.getByText('Laps 1–10 · 10 laps')).toBeTruthy();
  expect(screen.getByText('Laps 11–14 · current')).toBeTruthy();
  expect(screen.getByText('Used set · starting age 2')).toBeTruthy();
  expect(screen.getByText('Stop · lap 10')).toBeTruthy();
  expect(screen.getByText('2.300 s')).toBeTruthy();
  expect(screen.getByText('22.000 s')).toBeTruthy();
  expect(screen.getByText('Laps 18–22 · rejoin P8')).toBeTruthy();
});

test('restricted rival keeps stint history but no wear values or player pit window', () => {
  show(1, true);
  expect(screen.getAllByText('Restricted')).toHaveLength(2);
  expect(screen.getByText('Laps 1–10 · 10 laps')).toBeTruthy();
  expect(screen.queryByText('Laps 18–22 · rejoin P8')).toBeNull();
  expect(screen.queryByText('20% / 30% / 40% / 50%')).toBeNull();
  expect(screen.queryByText('Game pit window')).toBeNull();
});

test('current set comes first with labelled wheel wear and distinct pit timing values', () => {
  show();
  const active = screen.getByRole('region', { name: 'Current tyre set' });
  expect(within(active).getByText('Medium')).toBeTruthy();
  expect(within(active).getByTestId('stint-laps').textContent).toBe('03');
  for (const [wheel, wear] of [['FL', '40%'], ['FR', '50%'], ['RL', '20%'], ['RR', '30%']]) {
    expect(within(active).getByText(wheel!).parentElement!.textContent).toContain(wear);
  }
  expect(within(screen.getByRole('list', { name: 'Tyre stints' })).getByText('Soft')).toBeTruthy();
  expect(screen.getByText('Stopped').parentElement!.textContent).toContain('2.300 s');
  expect(screen.getByText('Pit lane').parentElement!.textContent).toContain('22.000 s');
});
test('new epoch removes old stints and stops instead of retaining another session', () => {
  const runtime = show();
  act(() => {
    runtime.stores.ingest(
      snapshot(2, {
        lifecycle: 'Live',
        playerCar: 0,
        cars: [{ lap: { state: 'absent' }, status: { state: 'absent' }, tyreCondition: { state: 'absent' } }],
        pitWindow: { state: 'absent' },
      }),
    );
    vi.advanceTimersByTime(200);
  });
  expect(screen.queryByText('Laps 1–10 · 10 laps')).toBeNull();
  expect(screen.queryByText('Stop · lap 10')).toBeNull();
  expect(screen.getByText('No stint history')).toBeTruthy();
});
