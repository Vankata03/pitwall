import { expect, test } from 'vitest';
import { smoothValues } from './smooth';

test('averages within the radius, keeps both ends exact and never reaches across a gap', () => {
  const x = [0, 1, 2, 3, 4, 5, 6, 7];
  expect(smoothValues(x, [0, 0, 30, 30, 60, null, 90, 0], 1)).toEqual([0, 10, 20, 40, 60, null, 90, 0]);
});

test('a zero radius changes nothing', () => {
  expect(smoothValues([0, 1, 2], [0, 50, 0], 0)).toEqual([0, 50, 0]);
});
