import { test } from 'node:test';
import assert from 'node:assert/strict';
import { compare } from './benchmark-gate.mjs';

test('25% regression blocks; just below passes; one outlier does not move the median', () => {
  const base = [10, 10, 10, 10, 10, 10, 1000];
  assert.equal(compare([base, [12.49, 12.49, 12.49, 12.49, 12.49, 12.49, 5000]]).headMs, 12.49);
  assert.throws(() => compare([base, Array(7).fill(12.5)]), /25.0%/);
  assert.throws(() => compare([base, Array(7).fill(NaN)]), /invalid benchmark sample/);
});
