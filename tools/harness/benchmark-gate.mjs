import assert from 'node:assert/strict';

export function compare(samples) {
  assert.equal(samples.length, 2);
  for (const values of samples) {
    assert.equal(values.length, 7);
    assert.ok(values.every(v => Number.isFinite(v) && v > 0), 'invalid benchmark sample');
  }
  const [baseMs, headMs] = samples.map(values => values.toSorted((a, b) => a - b)[3]);
  const ratio = headMs / baseMs;
  assert.ok(ratio < 1.25, `ingest regression: ${(ratio * 100 - 100).toFixed(1)}%; threshold is 25%`);
  return { baseMs, headMs, ratio };
}
