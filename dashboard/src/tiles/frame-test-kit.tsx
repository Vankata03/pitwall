/**
 * Test support for tiles that draw from wire data: a live runtime fed real wire messages,
 * a `CarState` builder typed by the generated contract, and a Frame readout-word builder.
 * Tests assert on what the tile puts in the DOM, never on its internals.
 */
import { act, render } from '@testing-library/react';
import type { ReactElement } from 'react';
import { afterEach, vi } from 'vitest';
import { buildFrame, hello, snapshot } from '../runtime/fixtures';
import { RuntimeProvider } from '../runtime/react';
import { createRuntime, type Runtime } from '../runtime/runtime';
import { READOUT_GROUPS, ReadoutState, type ReadoutGroup } from '../runtime/wire';
import type { CarState } from '../generated/wire/CarState';
import type { TileProps } from './contract';

export type { CarState };

// The slow store publishes at most every 100 ms; fake timers let a test flush it at once.
afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

export const Live = ReadoutState.Live;
export const Stale = ReadoutState.Stale;
export const Restricted = ReadoutState.Restricted;
export const Absent = ReadoutState.Absent;

/** A Frame `readout` word: each group is Live unless named. */
export function readoutWord(states: Partial<Record<ReadoutGroup, ReadoutState>> = {}): number {
  return READOUT_GROUPS.reduce((word, group, i) => word | ((states[group] ?? Live) << (i * 2)), 0);
}

export interface LiveRuntime {
  runtime: Runtime;
  /** Run the next animation frame. */
  paint(ts: number): void;
  /** Ingest a Frame carrying these cars, on the current epoch. */
  frame(cars: Record<number, Partial<Record<string, number>>>, sessionTime?: number): void;
  /** Replace State: `cars` are indexed by vehicle index. */
  state(patch: { lifecycle?: string; session?: unknown; playerCar?: number | null; cars?: Record<number, CarState | null> }, history?: HistoryInput[]): void;
  disconnect(): void;
}
type HistoryInput = { kind: string; id: string; car?: number | null; body?: unknown };

export function liveRuntime(): LiveRuntime {
  vi.useFakeTimers();
  let queued: ((ts: number) => void) | null = null;
  let seq = 0;
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
  runtime.stores.ingest(snapshot(1, { lifecycle: 'Live', cars: [] }));
  vi.advanceTimersByTime(100);
  const stateOf = (patch: Parameters<LiveRuntime['state']>[0]) => {
    const cars: (CarState | null)[] = Array.from({ length: 24 }, () => null);
    for (const [i, c] of Object.entries(patch.cars ?? {})) cars[Number(i)] = c;
    return { lifecycle: 'Live', session: { state: 'absent' }, playerCar: null, cars, ...patch };
  };
  return {
    runtime,
    paint: (ts) => act(() => queued!(ts)),
    frame: (cars, sessionTime = 1) =>
      act(() => runtime.stores.ingest(buildFrame({ epoch: 1, seq: ++seq, sessionTime, cars }))),
    state: (patch, history = []) =>
      act(() => {
        runtime.stores.ingest(
          snapshot(
            1,
            stateOf(patch),
            history.map((h) => ({ car: null, body: {}, ...h })),
          ),
        );
        vi.advanceTimersByTime(100);
      }),
    disconnect: () =>
      act(() => {
        runtime.stores.setConnection('reconnecting');
        vi.advanceTimersByTime(100);
      }),
  };
}

export const props = (over: Partial<TileProps> = {}): TileProps => ({
  id: 'tile',
  settings: {},
  density: 'regular',
  width: 300,
  height: 200,
  car: { status: 'ok', index: 3 },
  units: { speed: 'kmh', temperature: 'c', pressure: 'psi' },
  editing: false,
  ...over,
});

export function mount(rt: LiveRuntime, ui: ReactElement) {
  const view = render(<RuntimeProvider runtime={rt.runtime}>{ui}</RuntimeProvider>);
  return {
    ...view,
    rerender: (next: ReactElement) => view.rerender(<RuntimeProvider runtime={rt.runtime}>{next}</RuntimeProvider>),
  };
}

/** Live readouts for every group of a car, with no data in the optional blocks. */
export function carState(over: Partial<CarState> = {}): CarState {
  return {
    participant: { state: 'absent' },
    regulations: 'Pre2026',
    retired: false,
    lap: { state: 'absent' },
    telemetry: { state: 'absent' },
    status: { state: 'absent' },
    fuel: { state: 'absent' },
    ers: { state: 'absent' },
    damage: { state: 'absent' },
    aero: { state: 'absent' },
    gaps: { state: 'absent' },
    tyreCondition: { state: 'absent' },
    frontBrakeBias: { state: 'absent' },
    penalties: { state: 'absent' },
    best: null,
    previous: null,
    completedBest: null,
    completedPrevious: null,
    personalBest: null,
    rival: null,
    ...over,
  };
}

export const liveReadout = <T,>(value: T) => ({ state: 'live' as const, value });
export const staleReadout = <T,>(value: T) => ({ state: 'stale' as const, value });

/**
 * jsdom has no canvas. This installs a recording 2D context so a tile's canvas drawing can be
 * observed: `calls` lists every method called, with its arguments. Restored after each test.
 */
export function stubCanvas(): { calls: { method: string; args: unknown[] }[]; props: Record<string, unknown> } {
  const calls: { method: string; args: unknown[] }[] = [];
  const props: Record<string, unknown> = {};
  const context = new Proxy(
    {},
    {
      get: (_t, name: string) => (...args: unknown[]) => void calls.push({ method: name, args }),
      set: (_t, name: string, value) => ((props[name] = value), true),
    },
  );
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(context as never);
  return { calls, props };
}
