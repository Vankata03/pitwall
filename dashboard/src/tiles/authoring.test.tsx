// @vitest-environment jsdom
/**
 * Worked examples for tile authors. A tile is one file in `src/tiles/types/` whose default
 * export is `defineTileModule(...)`; these two show the shapes the catalogue tiles take.
 * They are tests so that they keep compiling and keep working.
 */
import { cleanup, render, screen } from '@testing-library/react';
import { useRef } from 'react';
import { afterEach, expect, test } from 'vitest';
import { buildFrame, hello, snapshot } from '../runtime/fixtures';
import { effectiveState, frameReadoutState, writeReadout } from '../runtime/readout';
import { Readout, RuntimeProvider, useSlow } from '../runtime/react';
import { createRuntime } from '../runtime/runtime';
import { carSettingOf } from '../session/cars';
import { speed } from '../device/units';
import { defineTileModule, type TileProps, type TileSettingsProps } from './contract';
import { useFrameTile } from './use-frame-tile';

afterEach(cleanup);

// 1. A frame tile (60 Hz): declare what it reads, draw into the DOM, never into React state.
const speedTile = defineTileModule({
  declaration: {
    type: 'speed-example',
    name: 'Speed example',
    binding: 'follows-car',
    rate: 'frame',
    minSize: { w: 2, h: 2 },
    defaultSize: { w: 3, h: 3 },
    densities: [
      { density: 'compact', minWidth: 0, minHeight: 0 },
      { density: 'regular', minWidth: 200, minHeight: 120 },
    ],
    reads: [{ source: 'frame', fields: ['speed'] }],
  },
  Component: function SpeedExample({ car, units }: TileProps) {
    const el = useRef<HTMLSpanElement>(null);
    useFrameTile(
      speedTile.declaration,
      car,
      ({ frame, car: index, connected }) => {
        if (index === null || !el.current) return;
        const state = effectiveState(frameReadoutState(frame, index, 'speed'), connected);
        const { value, unit } = speed(frame.speed[index]!, units);
        writeReadout(el.current, state, `${Math.round(value)} ${unit}`);
      },
      [units.speed],
    );
    return <span ref={el} className="pw-readout" data-testid="speed" />;
  },
});

// 2. A slow tile (4 to 10 Hz): read State through useSlow, show readouts through <Readout/>,
// and keep tile-specific settings next to the car setting the host already provides.
interface WeatherSettings {
  showForecast: boolean;
}
const weatherSettings = (s: Record<string, unknown>): WeatherSettings => ({ showForecast: s.showForecast !== false });

const weatherTile = defineTileModule({
  declaration: {
    type: 'weather-example',
    name: 'Weather example',
    binding: 'session-wide',
    rate: 'slow',
    minSize: { w: 4, h: 2 },
    defaultSize: { w: 6, h: 2 },
    densities: [
      { density: 'compact', minWidth: 0, minHeight: 0 },
      { density: 'regular', minWidth: 240, minHeight: 64 },
    ],
    reads: [{ source: 'state', keys: ['session'] }],
  },
  Component: function WeatherExample({ settings }: TileProps) {
    const air = useSlow((s) => (s.state?.airTemperature as number | undefined) ?? null);
    return (
      <div>
        <Readout state={air === null ? 3 : 0} value={String(air)} />
        {weatherSettings(settings).showForecast ? <span>forecast</span> : null}
      </div>
    );
  },
  Settings: function WeatherSettingsPanel({ settings, onChange }: TileSettingsProps) {
    return (
      <label>
        <input
          type="checkbox"
          checked={weatherSettings(settings).showForecast}
          onChange={(e) => onChange({ ...settings, showForecast: e.target.checked })}
        />
        Show forecast
      </label>
    );
  },
});

function liveRuntime() {
  let queued: ((ts: number) => void) | null = null;
  const runtime = createRuntime({
    url: 'ws://h/ws',
    ownBuildId: 'b1',
    createSocket: () => ({ binaryType: '', onopen: null, onmessage: null, onclose: null, onerror: null, close() {} }),
    requestFrame: (cb) => ((queued = cb), 1),
    cancelFrame: () => {},
    reload: () => {},
    storage: { getItem: () => null, setItem: () => {} },
  });
  runtime.loop.start();
  runtime.stores.setConnection('open');
  runtime.stores.ingest(hello(1));
  runtime.stores.ingest(snapshot(1, { lifecycle: 'live' }));
  return { runtime, paint: (ts: number) => queued!(ts) };
}

const props = (over: Partial<TileProps> = {}): TileProps => ({
  id: 'speed-example',
  settings: {},
  density: 'regular',
  width: 300,
  height: 200,
  car: { status: 'ok', index: 3 },
  units: { speed: 'kmh', temperature: 'c', pressure: 'psi' },
  editing: false,
  ...over,
});

test('a frame tile draws the car it was given, in the device’s units', () => {
  const { runtime, paint } = liveRuntime();
  const Tile = speedTile.Component;
  const view = render(
    <RuntimeProvider runtime={runtime}>
      <Tile {...props()} />
    </RuntimeProvider>,
  );
  runtime.stores.ingest(buildFrame({ epoch: 1, seq: 1, sessionTime: 1, cars: { 3: { speed: 200, readout: 0 } } }));
  paint(16);
  expect(screen.getByTestId('speed').textContent).toBe('200 km/h');

  view.rerender(
    <RuntimeProvider runtime={runtime}>
      <Tile {...props({ units: { speed: 'mph', temperature: 'c', pressure: 'psi' } })} />
    </RuntimeProvider>,
  );
  paint(33); // a units change redraws with no new frame
  expect(screen.getByTestId('speed').textContent).toBe('124 mph');
});

test('a frame tile follows a changed car without remounting', () => {
  const { runtime, paint } = liveRuntime();
  const Tile = speedTile.Component;
  runtime.stores.ingest(
    buildFrame({ epoch: 1, seq: 1, sessionTime: 1, cars: { 3: { speed: 200, readout: 0 }, 5: { speed: 150, readout: 0 } } }),
  );
  const view = render(
    <RuntimeProvider runtime={runtime}>
      <Tile {...props()} />
    </RuntimeProvider>,
  );
  paint(16);
  expect(screen.getByTestId('speed').textContent).toBe('200 km/h');
  view.rerender(
    <RuntimeProvider runtime={runtime}>
      <Tile {...props({ car: { status: 'ok', index: 5 } })} />
    </RuntimeProvider>,
  );
  paint(33);
  expect(screen.getByTestId('speed').textContent).toBe('150 km/h');
});

test('a frame tile stops drawing once unmounted', () => {
  const { runtime, paint } = liveRuntime();
  const Tile = speedTile.Component;
  const view = render(
    <RuntimeProvider runtime={runtime}>
      <Tile {...props()} />
    </RuntimeProvider>,
  );
  view.unmount();
  expect(() => paint(16)).not.toThrow();
});

test('a slow tile reads State, and its own settings sit beside the host’s car setting', () => {
  const { runtime } = liveRuntime();
  runtime.stores.ingest(JSON.stringify({ type: 'state', epoch: 1, lifecycle: 'live', airTemperature: 22 }));
  const Tile = weatherTile.Component;
  const Panel = weatherTile.Settings!;
  const saved: Record<string, unknown>[] = [];
  render(
    <RuntimeProvider runtime={runtime}>
      <Tile {...props({ car: null, settings: { car: { target: 'pinned' }, showForecast: false } })} />
      <Panel settings={{ other: 1 }} onChange={(s) => saved.push(s)} />
    </RuntimeProvider>,
  );
  expect(screen.queryByText('forecast')).toBeNull();
  // Settings the tile does not know must survive: the panel spreads what it was given.
  screen.getByRole('checkbox').click();
  expect(saved[0]).toEqual({ other: 1, showForecast: false });
  expect(carSettingOf({ car: { target: 'pinned' } })).toEqual({ target: 'pinned' });
});
