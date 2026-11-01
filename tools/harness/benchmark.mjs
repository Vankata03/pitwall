import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { copyFile, mkdir } from 'node:fs/promises';
import path from 'node:path';
import { goldens, manifest, cache } from './goldens.mjs';
import { compare } from './benchmark-gate.mjs';

await goldens();
const id = manifest.slots['2026-spa-practice-full'];
if (!id) {
  console.log('SKIP ingest benchmark: no full Spa golden pinned; awaiting #52. Synthetic data is not a baseline.');
  process.exit(0);
}
const entry = manifest.recordings.find(e => e.id === id);
assert.ok(entry, 'full Spa slot must reference a manifest recording');
const base = process.argv[2];
assert.ok(base, 'usage: node tools/harness/benchmark.mjs BASE_CHECKOUT');
const roots = [path.resolve(base), process.cwd()];
await mkdir(path.join(roots[0], 'crates/collector/examples'), { recursive: true });
await copyFile('crates/collector/examples/ingest_bench.rs', path.join(roots[0], 'crates/collector/examples/ingest_bench.rs'));
const binaries = roots.map(root => {
  execFileSync('cargo', ['build', '--locked', '--release', '--example', 'ingest_bench'], { cwd: root, stdio: 'inherit' });
  return path.join(root, 'target/release/examples', `ingest_bench${process.platform === 'win32' ? '.exe' : ''}`);
});
const run = binary => JSON.parse(execFileSync(binary, [path.join(cache, entry.file)], { encoding: 'utf8' }));
binaries.forEach(run); // warm both paths outside measurement
const samples = [[], []];
let records;
for (let round = 0; round < 7; round++) {
  for (const index of round % 2 ? [1, 0] : [0, 1]) {
    const result = run(binaries[index]);
    records ??= result.records;
    assert.equal(result.records, records, 'base and head must ingest identical record counts');
    assert.ok(Number.isFinite(result.milliseconds) && result.milliseconds > 0);
    samples[index].push(result.milliseconds);
  }
}
const result = compare(samples);
console.log(JSON.stringify({ records, samples, ...result }, null, 2));
