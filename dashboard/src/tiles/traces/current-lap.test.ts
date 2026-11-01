import { expect, test } from 'vitest';
import type { Trace } from '../../generated/wire/Trace';
import { CurrentLap } from './current-lap';

test('samples are placed in 5 m buckets, the latest sample in a bucket winning', () => {
  const lap = new CurrentLap();
  lap.record(0.4, 100, 0, 200);
  lap.record(5.2, 80, 0, 210);
  lap.record(6.9, 60, 10, 215); // same bucket as 5.2
  lap.record(10, 0, 100, 150);
  expect(lap.samples()).toEqual({
    distance: [0, 5, 10],
    speed: [200, 215, 150],
    throttle: [1, 0.6, 0],
    brake: [0, 0.1, 1],
  });
});

test('missing data leaves a gap and never a zero, and a negative distance is dropped', () => {
  const lap = new CurrentLap();
  lap.record(0, 100, 0, 200);
  lap.record(5, NaN, NaN, NaN);
  lap.record(-3, 50, 0, 100);
  lap.record(10, 50, 0, 100);
  const s = lap.samples();
  expect(s.distance).toEqual([0, 5, 10]);
  expect(s.throttle).toEqual([1, null, 0.5]);
  expect(s.speed).toEqual([200, null, 100]);
});

test('a drop of distance back to the start line begins a new lap', () => {
  const lap = new CurrentLap();
  lap.record(5000, 100, 0, 300);
  lap.record(5300, 100, 0, 300);
  lap.record(2, 90, 0, 120);
  expect(lap.samples().distance).toEqual([0]);
  expect(lap.samples().throttle).toEqual([0.9]);
});

test('a small step back, as when a car spins, overwrites instead of resetting', () => {
  const lap = new CurrentLap();
  lap.record(100, 100, 0, 100);
  lap.record(120, 100, 0, 100);
  lap.record(110, 0, 100, 50);
  expect(lap.samples().distance).toEqual([100, 105, 110, 115, 120]);
});

test('a fetched lap-so-far fills only what the live samples have not covered', () => {
  const lap = new CurrentLap();
  lap.record(20, 10, 0, 90);
  const fetched: Trace = {
    epoch: 1,
    lap: 2,
    distance: [0, 5, 10, 15, 20],
    time: [0, 1, 2, 3, 4],
    speed: [80, 82, 84, 86, 88],
    throttle: [1, 1, 1, 1, 1],
    brake: [0, 0, 0, 0, 0],
  };
  lap.seed(fetched);
  const s = lap.samples();
  expect(s.distance).toEqual([0, 5, 10, 15, 20]);
  expect(s.throttle).toEqual([1, 1, 1, 1, 0.1]); // live bucket 20 kept
  expect(s.speed).toEqual([80, 82, 84, 86, 90]);
});

test('reset empties the lap and bumps the version for the render loop', () => {
  const lap = new CurrentLap();
  lap.record(0, 100, 0, 100);
  const before = lap.version;
  lap.reset();
  expect(lap.samples().distance).toEqual([]);
  expect(lap.version).toBeGreaterThan(before);
});

for (const steps of [[0, 0.05, 0.1, 0.15, 0.2, 0.25, 0.3], [0, 0.05, 0.183, 0.23, 0.41]]) {
  test(`300 km/h sparse samples fill every bucket: ${steps}`, () => {
    const lap = new CurrentLap();
    for (const t of steps) lap.record(t * 300 / 3.6, 100 - t * 100, t * 100, 300);
    const s = lap.samples();
    expect(s.distance).toEqual(Array.from({ length: Math.floor(steps.at(-1)! * 300 / 3.6 / 5) + 1 }, (_, i) => i * 5));
    expect(s.throttle.every(v => v !== null)).toBe(true);
  });
}
test('interpolation keeps missing channels and gaps over 100 m broken', () => {
  const lap = new CurrentLap();
  lap.record(0, 100, 0, 300);
  lap.record(20, NaN, 0, 300);
  lap.record(40, 50, 0, 300);
  expect(lap.samples().throttle.slice(1, 8)).toEqual(Array(7).fill(null));
  lap.record(150, 100, 0, 300);
  expect(lap.samples().distance).not.toContain(100);
});
