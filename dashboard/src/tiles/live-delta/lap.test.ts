import { expect, test } from 'vitest';
import { FrameStore } from '../../runtime/stores';
import { buildFrame } from '../../runtime/fixtures';
import { DeltaLap, deltaExtent, segments } from './lap';

test('whole lap survives ring rollover, resets at the line, retains the finished lap', () => {
  const store = new FrameStore(3);
  const lap = new DeltaLap();
  let seq = 0;
  const add = (distance: number) => {
    store.accept(buildFrame({ epoch: 1, seq: ++seq, sessionTime: seq, cars: { 3: { lapDistance: distance, deltaBest: -100, readout: 0 } } }));
    lap.ingest(store.rings[3]!, 1, 'deltaBest');
  };
  for (const distance of [0, 50, 100, 150, 200, 250]) add(distance);
  expect(lap.current.map(p => p.distance)).toEqual([0, 50, 100, 150, 200, 250]);
  add(0);
  expect(lap.current.map(p => p.distance)).toEqual([0]);
  expect(lap.previous).toHaveLength(6);
  store.reset();
  lap.ingest(store.rings[3]!, 2, 'deltaBest');
  expect(lap.previous).toHaveLength(0);
});

test('graph joins up to 100 m, breaks longer gaps and NaN, uses symmetric sensible steps', () => {
  const points = [0, 100, 201, 220, 240, 260].map((distance, i) => ({ distance, delta: i === 4 ? NaN : -0.23 }));
  expect(segments(points).map(s => s.map(p => p.distance))).toEqual([[0, 100], [201, 220], [260]]);
  expect(deltaExtent(points)).toBe(0.5);
  expect(deltaExtent([{ distance: 0, delta: 1.4 }])).toBe(2);
});
