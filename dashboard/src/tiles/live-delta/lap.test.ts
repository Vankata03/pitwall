import { expect, test } from 'vitest';
import { FrameStore } from '../../runtime/stores';
import { buildFrame } from '../../runtime/fixtures';
import { DeltaLap, deltaExtent, segments } from './lap';

function setup(capacity = 600) {
  const store = new FrameStore(capacity);
  const lap = new DeltaLap();
  let seq = 0;
  const add = (distance: number, deltaBest = -100, readout = 0) => {
    seq++;
    store.accept(buildFrame({ epoch: 1, seq, sessionTime: seq, cars: { 3: { lapDistance: distance, deltaBest, readout } } }));
  };
  const ingest = (epoch = 1) => lap.ingest(store.rings[3]!, epoch, 'deltaBest');
  return { store, lap, add, ingest };
}

test('whole lap survives ring rollover, resets at the line, retains the finished lap', () => {
  const { store, lap, add, ingest } = setup(3);
  for (const distance of [0, 50, 100, 150, 200, 250]) {
    add(distance);
    ingest(); // one ingest per paint, with a ring far shorter than the lap
  }
  expect(lap.current.map((p) => p.distance)).toEqual([0, 50, 100, 150, 200, 250]);
  add(0);
  ingest();
  expect(lap.current).toEqual([]);
  expect(lap.previous).toHaveLength(6);
  store.reset();
  ingest(2);
  expect(lap.previous).toHaveLength(0);
});

test('every Frame between two paints is consumed, not only the newest', () => {
  const { lap, add, ingest } = setup();
  for (const distance of [10, 20, 30, 40, 50]) add(distance, distance * -10);
  ingest();
  expect(lap.current.map((p) => [p.distance, p.delta])).toEqual([[10, -0.1], [20, -0.2], [30, -0.3], [40, -0.4], [50, -0.5]]);
  ingest(); // nothing new: nothing added twice
  expect(lap.current).toHaveLength(5);
});

test('repeated Frames of one Lap Data sample add one point', () => {
  const { lap, add, ingest } = setup();
  for (let i = 0; i < 4; i++) add(10, -5);
  add(10, -7); // same distance, newer delta
  ingest();
  expect(lap.current).toEqual([{ distance: 10, delta: -0.007 }]);
});

test('the official figure at the line is not plotted on either lap', () => {
  const { lap, add, ingest } = setup();
  for (const distance of [1000, 2000, 3000]) add(distance, -100);
  add(2, 8000); // first Lap Data of the new lap carries the official lap delta
  add(2, 8000); // Frames repeat it until the next Lap Data
  add(40, -90);
  ingest();
  expect(lap.previous.map((p) => p.distance)).toEqual([1000, 2000, 3000]);
  expect(lap.current).toEqual([{ distance: 40, delta: -0.09 }]);
});

test('samples lost to a ring overflow break the line instead of resetting the lap', () => {
  const { lap, add, ingest } = setup(4);
  add(10); add(20);
  ingest();
  for (const d of [30, 40, 50, 60, 70, 80]) add(d); // tab hidden: ring keeps only the last four
  ingest();
  const distances = lap.current.map((p) => p.distance);
  expect(distances.slice(0, 2)).toEqual([10, 20]);
  expect(segments(lap.current).map((s) => s.map((p) => p.distance))).toEqual([[10, 20], [50, 60, 70, 80]]);
});

test('missing delta or distance breaks the line and negative distance is dropped', () => {
  const { lap, add, ingest } = setup();
  add(10); add(20, 0, 0x3 << 8); // delta best absent
  add(-5); add(-6); // before the line in the pit lane
  add(30);
  ingest();
  expect(segments(lap.current).map((s) => s.map((p) => p.distance))).toEqual([[10], [30]]);
  expect(lap.current.filter((p) => p.distance < 0)).toEqual([]);
});

test('a short reversal breaks the line without starting a new lap', () => {
  const { lap, add, ingest } = setup();
  for (const d of [100, 120, 110, 130]) add(d);
  ingest();
  expect(lap.previous).toEqual([]);
  expect(segments(lap.current).map((s) => s.map((p) => p.distance))).toEqual([[100, 120], [130]]);
});

test('a time that goes backwards starts over', () => {
  const { store, lap, add, ingest } = setup();
  add(10); add(20);
  ingest();
  store.accept(buildFrame({ epoch: 1, seq: 99, sessionTime: 0.5, cars: { 3: { lapDistance: 30, deltaBest: 0, readout: 0 } } }));
  ingest();
  expect(lap.current.map((p) => p.distance)).toEqual([30]);
});

test('graph joins up to 100 m, breaks longer gaps and NaN, uses symmetric sensible steps', () => {
  const points = [0, 100, 201, 220, 240, 260].map((distance, i) => ({ distance, delta: i === 4 ? NaN : -0.23 }));
  expect(segments(points).map((s) => s.map((p) => p.distance))).toEqual([[0, 100], [201, 220], [260]]);
  expect(deltaExtent(points)).toBe(0.5);
  expect(deltaExtent([{ distance: 0, delta: 1.4 }])).toBe(2);
  expect(deltaExtent([])).toBe(0.1);
  expect(deltaExtent([{ distance: 0, delta: -3.2 }])).toBe(5);
});
