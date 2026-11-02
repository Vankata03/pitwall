import { expect, test } from 'vitest';
import { DEFAULT_UNITS } from '../../device/units';
import { FrameStore } from '../../runtime/stores';
import { buildFrame } from '../../runtime/fixtures';
import type { Trace } from '../../generated/wire/Trace';
import { buildWindowPlot, InputHistory } from './window-plot';

function samples() {
  const store = new FrameStore(600);
  const inputs = new InputHistory();
  const add = (time: number, distance = 0, throttle = 70, brake = 0) => {
    store.accept(buildFrame({ epoch: 1, seq: 1, sessionTime: time, cars: { 0: { lapDistance: distance, throttle, brake, speed: 200, readout: 0 } } }));
    inputs.update(store.rings[0]!, '1:0');
  };
  const plot = (reference: Trace | null = null) => buildWindowPlot(inputs.rings, reference, ['throttle', 'brake'], DEFAULT_UNITS);
  return { add, plot };
}

test('keeps exactly thirty seconds at 60 Hz, including stationary pedal changes', () => {
  const { add, plot } = samples();
  for (let i = 0; i <= 2400; i++) add(i / 60, 0, i % 2 ? 20 : 70);
  const p = plot();
  expect(p.windowSeconds).toBe(30);
  expect(p.x).toHaveLength(1801);
  expect(p.x[0]).toBe(-30);
  expect(p.x.at(-1)).toBe(0);
  expect(p.series[0]!.values.slice(-2)).toEqual([20, 70]);
});

test('a new stream keeps the full axis even before thirty seconds have arrived', () => {
  const { add, plot } = samples();
  add(120); add(120.1);
  expect(plot().windowSeconds).toBe(30);
  expect(plot().x[0]).toBeCloseTo(-0.1);
  expect(plot().x.at(-1)).toBe(0);
});

test('paused replay publications do not evict the thirty seconds already received', () => {
  const { add, plot } = samples();
  for (let i = 0; i <= 1800; i++) add(i / 60);
  for (let i = 0; i < 2000; i++) add(30);
  add(30, 0, 40, 10);
  expect(plot().x).toHaveLength(1801);
  expect(plot().x[0]).toBe(-30);
  expect(plot().series[0]!.values.at(-1)).toBe(40);
  expect(plot().series[1]!.values.at(-1)).toBe(10);
});

test('breaks lines across missing telemetry and discards history after a time reversal', () => {
  const { add, plot } = samples();
  add(10); add(12);
  expect(plot().series[0]!.values).toEqual([70, null, 70]);
  add(5, 0, 30);
  expect(plot().x).toEqual([0]);
  expect(plot().series[0]!.values).toEqual([30]);
});

test('aligns the dim reference to circuit position within the time window', () => {
  const { add, plot } = samples();
  add(10, 2.5); add(10.1, 5); add(10.2, 100);
  const reference: Trace = { epoch: 1, lap: 1, distance: [0, 5], time: [0, 100], speed: [100, 200], throttle: [1, 0], brake: [0, 1] };
  const p = plot(reference);
  expect(p.series.find(s => s.lap === 'reference' && s.channel === 'throttle')!.values).toEqual([50, 0, null]);
  expect(p.series.find(s => s.lap === 'reference' && s.channel === 'brake')!.values).toEqual([50, 100, null]);
});
