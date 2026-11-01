import { expect, test } from 'vitest';
import type { Trace } from '../../generated/wire/Trace';
import { BUCKET_METRES, CurrentLap } from './current-lap';

test('buckets hold the value at their exact distance; samples between boundaries only shape it', () => {
  const lap = new CurrentLap();
  lap.record(0, 100, 0, 200);
  lap.record(5, 80, 0, 210);
  lap.record(6.9, 60, 10, 215); // between boundaries: no bucket of its own
  lap.record(10, 0, 100, 150);
  expect(lap.samples()).toEqual({
    distance: [0, 5, 10],
    speed: [200, 210, 150],
    throttle: [1, 0.8, 0],
    brake: [0, 0, 1],
  });
});

test('a bucket between two samples is interpolated to its own distance, not copied from either', () => {
  const lap = new CurrentLap();
  lap.record(0, 0, 100, 100);
  lap.record(8, 80, 20, 180);
  const s = lap.samples();
  expect(s.distance).toEqual([0, 5]);
  expect(s.throttle[1]).toBeCloseTo(0.5);
  expect(s.brake[1]).toBeCloseTo(0.5);
  expect(s.speed[1]).toBeCloseTo(150);
});

test('missing data leaves a gap and never a zero, and a negative distance is dropped', () => {
  const lap = new CurrentLap();
  lap.record(0, 100, 0, 200);
  lap.record(5, NaN, NaN, NaN);
  lap.record(-3, 50, 0, 100);
  lap.record(10, 50, 0, 100);
  lap.record(15, 50, 0, 100);
  const s = lap.samples();
  expect(s.distance).toEqual([0, 5, 10, 15]);
  // The withheld sample breaks both steps that touch it.
  expect(s.throttle).toEqual([1, null, null, 0.5]);
  expect(s.speed).toEqual([200, null, null, 100]);
});

test('a drop of distance back to the start line begins a new lap', () => {
  const lap = new CurrentLap();
  lap.record(5000, 100, 0, 300);
  lap.record(5300, 100, 0, 300);
  lap.record(0, 90, 0, 120);
  lap.record(10, 90, 0, 120);
  expect(lap.samples().distance).toEqual([0, 5, 10]);
  expect(lap.samples().throttle).toEqual([0.9, 0.9, 0.9]);
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

for (const hz of [10, 20, 30, 60]) {
  test(`${hz} Hz at 300 km/h gives one continuous, correctly interpolated sample per bucket`, () => {
    const lap = new CurrentLap();
    const metresPerSample = 300 / 3.6 / hz;
    // Throttle ramps with distance, so any bucket the trace skipped or mis-filled shows.
    for (let n = 0; n * metresPerSample <= 1000; n++) {
      const d = n * metresPerSample;
      lap.record(d, d / 10, 100 - d / 10, 300);
    }
    const s = lap.samples();
    const last = Math.floor(Math.floor(1000 / metresPerSample) * metresPerSample / BUCKET_METRES);
    expect(s.distance).toEqual(Array.from({ length: last + 1 }, (_, i) => i * BUCKET_METRES));
    expect(s.throttle.every((v) => v !== null && v >= 0 && v <= 1)).toBe(true);
    for (let i = 0; i <= last; i++) expect(s.throttle[i]! * 100).toBeCloseTo(s.distance[i]! / 10, 6);
  });
}

test('a gap of exactly 100 m is joined and one past it is not', () => {
  const joined = new CurrentLap();
  joined.record(0, 0, 0, 300);
  joined.record(100, 100, 0, 300);
  expect(joined.samples().throttle.every((v) => v !== null)).toBe(true);
  const broken = new CurrentLap();
  broken.record(0, 0, 0, 300);
  broken.record(100.5, 100, 0, 300);
  expect(broken.samples().distance).toEqual([0]);
});

test('a withheld channel breaks only itself and recovers on the next pair of real samples', () => {
  const lap = new CurrentLap();
  lap.record(0, 0, 100, 300);
  lap.record(20, NaN, 100, 300);
  lap.record(40, 100, 100, 300);
  lap.record(60, 100, 100, 300);
  const s = lap.samples();
  expect(s.brake.every((v) => v === 1)).toBe(true);
  expect(s.throttle[0]).toBe(0);
  expect(s.throttle.slice(1, 9)).toEqual(Array(8).fill(null));
  expect(s.throttle.slice(9)).toEqual([1, 1, 1, 1]);
});
