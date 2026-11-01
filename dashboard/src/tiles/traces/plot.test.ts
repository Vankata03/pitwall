import { expect, test } from 'vitest';
import { DEFAULT_UNITS } from '../../device/units';
import type { LapSamples } from './current-lap';
import { buildPlot } from './plot';

const lap = (over: Partial<LapSamples> = {}): LapSamples => ({
  distance: [0, 5, 10],
  speed: [100, 200, null],
  throttle: [1, 0.5, 0],
  brake: [0, 0, 0.25],
  ...over,
});

const values = (plot: ReturnType<typeof buildPlot>, id: string, which: string) =>
  plot.series.find((s) => s.channel === id && s.lap === which)?.values;

test('throttle and brake are plotted in percent against distance on a shared 5 m axis', () => {
  const plot = buildPlot({ channels: ['throttle', 'brake'], units: DEFAULT_UNITS, current: lap(), reference: null });
  expect(plot.x).toEqual([0, 5, 10]);
  expect(plot.distanceUnit).toBe('m');
  expect(values(plot, 'throttle', 'current')).toEqual([100, 50, 0]);
  expect(values(plot, 'brake', 'current')).toEqual([0, 0, 25]);
  expect(plot.series.map((s) => s.channel)).toEqual(['throttle', 'brake']);
});

test('only the chosen channels are plotted, speed in the device’s unit', () => {
  const kmh = buildPlot({ channels: ['speed'], units: DEFAULT_UNITS, current: lap(), reference: null });
  expect(kmh.series.map((s) => s.channel)).toEqual(['speed']);
  expect(values(kmh, 'speed', 'current')).toEqual([100, 200, null]);
  expect(kmh.speedUnit).toBe('km/h');

  const mph = buildPlot({ channels: ['speed'], units: { ...DEFAULT_UNITS, speed: 'mph' }, current: lap(), reference: null });
  expect(values(mph, 'speed', 'current')![1]).toBeCloseTo(124.27, 2);
  expect(mph.distanceUnit).toBe('ft');
  expect(mph.x[1]).toBeCloseTo(16.4, 1); // 5 m
});

test('the reference lap is overlaid on the same axis and may be longer than the current lap', () => {
  const reference = lap({ distance: [0, 5, 10, 15], speed: [1, 2, 3, 4], throttle: [0, 0, 0, 1], brake: [1, 1, 1, 0] });
  const plot = buildPlot({ channels: ['throttle'], units: DEFAULT_UNITS, current: lap(), reference });
  expect(plot.x).toEqual([0, 5, 10, 15]);
  expect(values(plot, 'throttle', 'reference')).toEqual([0, 0, 0, 100]);
  expect(values(plot, 'throttle', 'current')).toEqual([100, 50, 0, null]);
  // The reference is drawn first so the live lap stays on top.
  expect(plot.series.map((s) => s.lap)).toEqual(['reference', 'current']);
});

test('a bucket the lap never reached is a gap, not zero', () => {
  const sparse = lap({ distance: [0, 15], speed: [1, 2], throttle: [1, 0.5], brake: [0, 0] });
  const plot = buildPlot({ channels: ['throttle'], units: DEFAULT_UNITS, current: sparse, reference: null });
  expect(plot.x).toEqual([0, 5, 10, 15]);
  expect(values(plot, 'throttle', 'current')).toEqual([100, null, null, 50]);
});

test('with no samples at all there is nothing to plot', () => {
  const empty: LapSamples = { distance: [], speed: [], throttle: [], brake: [] };
  const plot = buildPlot({ channels: ['throttle', 'brake'], units: DEFAULT_UNITS, current: empty, reference: null });
  expect(plot.x).toEqual([]);
  expect(plot.hasData).toBe(false);
});
