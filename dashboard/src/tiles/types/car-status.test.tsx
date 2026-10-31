// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import type { CarState } from '../../generated/wire/CarState';
import type { State } from '../../generated/wire/State';
import { hello, snapshot } from '../../runtime/fixtures';
import { RuntimeProvider } from '../../runtime/react';
import { createRuntime } from '../../runtime/runtime';
import type { TileProps } from '../contract';
import { registry } from '../registry';
import carStatus from './car-status';

const four = <T,>(a: T, b: T, c: T, d: T): [T, T, T, T] => [a, b, c, d];

const participant = (over: Record<string, unknown> = {}) => ({
  ai_controlled: 0,
  driver_id: 9,
  network_id: 255,
  team_id: 3,
  my_team: 0,
  race_number: 4,
  name: [],
  show_online_names: 1,
  num_colours: 2,
  livery_colour: [
    { red: 255, green: 128, blue: 0 },
    { red: 0, green: 128, blue: 255 },
    { red: 0, green: 0, blue: 0 },
    { red: 0, green: 0, blue: 0 },
  ],
  ...over,
});

const carState = (over: Record<string, unknown> = {}): CarState =>
  ({
    participant: { state: 'live', value: participant() },
    retired: false,
    telemetry: {
      state: 'live',
      value: {
        brakes_temperature: four(569, 571, 587, 588),
        tyres_surface_temperature: four(97, 95, 94, 92),
        tyres_inner_temperature: four(78, 78, 80, 80),
        tyres_pressure: four(21.8, 21.7, 23.5, 23.4),
      },
    },
    status: { state: 'live', value: { actual_tyre_compound: 18, visual_tyre_compound: 17, tyres_age_laps: 9 } },
    damage: {
      state: 'live',
      value: {
        brakes_damage: four(0, 0, 0, 0),
        front_left_wing_damage: 12,
        front_right_wing_damage: 0,
        rear_wing_damage: 4,
        floor_damage: 1,
        diffuser_damage: 0,
        sidepod_damage: 0,
        gear_box_damage: 0,
        engine_damage: 0,
      },
    },
    tyreCondition: { state: 'live', value: four(73, 76, 79, 82) },
    ...over,
  }) as unknown as CarState;

function runtimeWith(cars: (CarState | null)[]) {
  const runtime = createRuntime({
    url: 'ws://h/ws',
    ownBuildId: 'b1',
    createSocket: () => ({ binaryType: '', onopen: null, onmessage: null, onclose: null, onerror: null, close() {} }),
    requestFrame: () => 1,
    cancelFrame: () => {},
    reload: () => {},
    storage: { getItem: () => null, setItem: () => {} },
  });
  // The slow store publishes at most every 100 ms; step the clock so each message lands at once.
  const tick = () => vi.setSystemTime(Date.now() + 200);
  runtime.stores.setConnection('open');
  tick();
  runtime.stores.ingest(hello(1));
  tick();
  runtime.stores.ingest(snapshot(1, { lifecycle: 'live', playerCar: 0, cars } as unknown as Partial<State>));
  return runtime;
}

const props = (over: Partial<TileProps> = {}): TileProps => ({
  id: 'car-status',
  settings: {},
  density: 'regular',
  width: 700,
  height: 460,
  car: { status: 'ok', index: 0 },
  units: { speed: 'kmh', temperature: 'c', pressure: 'psi' },
  editing: false,
  ...over,
});

const show = (runtime: ReturnType<typeof runtimeWith>, over: Partial<TileProps> = {}) => {
  const Tile = carStatus.Component;
  return render(
    <RuntimeProvider runtime={runtime}>
      <Tile {...props(over)} />
    </RuntimeProvider>,
  );
};

const wheel = (id: string) => within(document.querySelector(`[data-wheel="${id}"]`) as HTMLElement);
const field = (container: ReturnType<typeof within>, name: string) =>
  (container.getByTestId(name) as HTMLElement).textContent;

let files: string[] = [];
beforeEach(() => {
  vi.useFakeTimers({ toFake: ['Date'] });
  files = [];
  vi.stubGlobal(
    'fetch',
    vi.fn(async () => new Response(JSON.stringify({ files }), { headers: { 'content-type': 'application/json' } })),
  );
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

test('is the Car status tile of the catalogue: follows a car, slow, 8×6 up to 12×12, one file', () => {
  expect(registry.get('car-status')).toBe(carStatus);
  expect(carStatus.declaration).toMatchObject({
    type: 'car-status',
    name: 'Car status',
    binding: 'follows-car',
    rate: 'slow',
    minSize: { w: 8, h: 6 },
    defaultSize: { w: 12, h: 12 },
  });
  expect(carStatus.declaration.densities.map((d) => d.density)).toEqual(['compact', 'regular', 'full']);
});

test('draws each wheel’s readouts beside its own position, from the followed car', () => {
  show(runtimeWith([carState()]));
  const fr = wheel('FR');
  expect(field(fr, 'condition')).toContain('82%');
  expect(field(fr, 'brake')).toContain('588');
  expect(field(fr, 'surface')).toContain('92');
  expect(field(fr, 'inner')).toContain('80');
  expect(field(fr, 'pressure')).toContain('23.4');
  expect(field(wheel('RL'), 'condition')).toContain('73%');
  expect(screen.getByTestId('compound').textContent).toContain('Medium');
  expect(screen.getByTestId('age').textContent).toContain('9');
  expect(screen.getByTestId('damage-front-left-wing').textContent).toContain('12%');
});

test('follows the car it was given, not the player', () => {
  const other = carState({ tyreCondition: { state: 'live', value: four(11, 12, 13, 14) } });
  show(runtimeWith([carState(), other]), { car: { status: 'ok', index: 1 } });
  expect(field(wheel('RL'), 'condition')).toContain('11%');
});

test('shows a tyre bar sized by condition, colour only repeating the number', () => {
  show(runtimeWith([carState({ tyreCondition: { state: 'live', value: four(30, 55, 100, 71) } })]));
  const bar = (id: string) => wheel(id).getByTestId('condition-bar');
  expect(bar('RL').style.width).toBe('30%');
  expect(bar('RL').dataset.level).toBe('low');
  expect(bar('RR').dataset.level).toBe('mid');
  expect(bar('FL').style.width).toBe('100%');
});

test('shows cold and hot with an arrow glyph and a band, an unbanded number with neither', () => {
  const telemetry = {
    brakes_temperature: four(569, 571, 587, 588),
    tyres_surface_temperature: four(40, 95, 130, 92),
    tyres_inner_temperature: four(78, 78, 80, 80),
    tyres_pressure: four(21.8, 21.7, 23.5, 23.4),
  };
  show(runtimeWith([carState({ telemetry: { state: 'live', value: telemetry } })]));
  expect(field(wheel('RL'), 'surface')).toBe('40▼');
  expect(wheel('RL').getByTestId('surface').dataset.band).toBe('cold');
  expect(field(wheel('FL'), 'surface')).toBe('130▲');
  expect(field(wheel('RR'), 'surface')).toBe('95');
  expect(wheel('RR').getByTestId('surface').dataset.band).toBe('window');
});

test('restricted wear and damage show their label, never a number, and temperatures stay', () => {
  show(runtimeWith([carState({ tyreCondition: { state: 'restricted' }, damage: { state: 'restricted' } })]));
  expect(field(wheel('FR'), 'condition')).toBe('Restricted');
  expect(wheel('FR').getByTestId('condition-bar').style.width).toBe('0%');
  expect(screen.getByTestId('damage-front-left-wing').textContent).toBe('Restricted');
  expect(field(wheel('FR'), 'surface')).toContain('92');
});

test('uses the device’s temperature and pressure units', () => {
  show(runtimeWith([carState()]), { units: { speed: 'kmh', temperature: 'f', pressure: 'bar' } });
  expect(field(wheel('RL'), 'surface')).toContain('207');
  expect(field(wheel('RL'), 'pressure')).toContain('1.50');
  expect(screen.getByTestId('car-status').textContent).toContain('°F');
});

test('compact density drops the car and keeps every wheel readout', () => {
  show(runtimeWith([carState()]), { density: 'compact', width: 360, height: 240 });
  expect(screen.queryByTestId('car-image')).toBeNull();
  expect(field(wheel('RL'), 'condition')).toContain('73%');
  expect(field(wheel('FL'), 'pressure')).toContain('23.5');
});

test('regular density shows the car; full adds every damage part and brake damage', () => {
  const view = show(runtimeWith([carState()]));
  expect(screen.getByTestId('car-image')).toBeTruthy();
  expect(screen.queryByTestId('damage-floor')).toBeNull();
  view.unmount();
  show(runtimeWith([carState()]), { density: 'full', width: 900, height: 640 });
  expect(screen.getByTestId('damage-floor').textContent).toContain('1%');
  expect(screen.getByTestId('damage-engine')).toBeTruthy();
  expect(wheel('RL').getByTestId('brake-damage').textContent).toContain('0%');
});

test('the car is the fallback, recoloured with the game’s livery colours, when there is no pack image', async () => {
  show(runtimeWith([carState()]));
  const img = screen.getByTestId('car-image') as HTMLImageElement;
  await waitFor(() => expect(vi.mocked(fetch)).toHaveBeenCalledWith('/api/liveries'));
  expect(img.src.startsWith('data:image/svg+xml')).toBe(true);
  const svg = decodeURIComponent(img.src.slice(img.src.indexOf(',') + 1));
  expect(svg).toContain('rgb(255,128,0)'); // livery colour 0
  expect(img.getAttribute('alt')).toBe('');
});

test('uses the pack image for the team when the host has one', async () => {
  files = ['3.png', '7.png'];
  show(runtimeWith([carState()]));
  await waitFor(() => expect((screen.getByTestId('car-image') as HTMLImageElement).getAttribute('src')).toBe('/liveries/3.png'));
});

test('uses my-team.png for the player’s custom team', async () => {
  files = ['my-team.png', '3.png'];
  show(runtimeWith([carState({ participant: { state: 'live', value: participant({ my_team: 1 }) } })]));
  await waitFor(() => expect((screen.getByTestId('car-image') as HTMLImageElement).getAttribute('src')).toBe('/liveries/my-team.png'));
});

test('an unreachable livery list leaves the fallback car', async () => {
  vi.stubGlobal('fetch', vi.fn(async () => Promise.reject(new Error('offline'))));
  show(runtimeWith([carState()]));
  await act(async () => {});
  expect((screen.getByTestId('car-image') as HTMLImageElement).src.startsWith('data:image/svg+xml')).toBe(true);
});

test('shows a whole-tile message only when no readout has data, and keeps the shape', () => {
  const empty = carState({
    telemetry: { state: 'absent' },
    status: { state: 'absent' },
    damage: { state: 'absent' },
    tyreCondition: { state: 'absent' },
  });
  show(runtimeWith([empty]));
  expect(screen.getByRole('status').textContent).toBe('No data');
  expect(wheel('FR').getByTestId('surface').textContent).toBe('—');
});

test('while the collector is unreachable, values stay, dimmed as stale', () => {
  const runtime = runtimeWith([carState()]);
  show(runtime);
  act(() => {
    vi.setSystemTime(Date.now() + 200);
    runtime.stores.setConnection('reconnecting');
  });
  expect(wheel('FR').getByTestId('surface').dataset.state).toBe('stale');
  expect(field(wheel('FR'), 'surface')).toContain('92');
});

test('says so when the followed car has retired, and keeps its last values', () => {
  show(runtimeWith([carState({ retired: true })]), { car: { status: 'retired', index: 0 } });
  expect(screen.getByTestId('retired').textContent).toBe('Retired');
  expect(field(wheel('FR'), 'condition')).toContain('82%');
});

test('a pack image that fails to load falls back to the original car', async () => {
  files = ['3.png'];
  show(runtimeWith([carState()]));
  const img = () => screen.getByTestId('car-image') as HTMLImageElement;
  await waitFor(() => expect(img().getAttribute('src')).toBe('/liveries/3.png'));
  fireEvent.error(img());
  expect(img().src.startsWith('data:image/svg+xml')).toBe(true);
});
