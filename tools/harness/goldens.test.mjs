import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, rm, access } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';

const script = path.resolve('tools/harness/goldens.mjs');
test('cached golden is verified on every run; corrupt bytes fail and are evicted', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'pitwall-hash-test-'));
  try {
    await mkdir(path.join(root, 'tests/goldens'), { recursive: true });
    await mkdir(path.join(root, '.cache/goldens'), { recursive: true });
    const bytes = Buffer.from('hash verifier test input, not a recording');
    const cached = path.join(root, '.cache/goldens/test.pitrec');
    await writeFile(path.join(root, 'tests/goldens/manifest.json'), JSON.stringify({ version: 1, recordings: [
      { id: 'test', file: 'test.pitrec', sha256: createHash('sha256').update(bytes).digest('hex') },
    ] }));
    await writeFile(cached, bytes);
    assert.equal(spawnSync(process.execPath, [script], { cwd: root }).status, 0);
    await writeFile(cached, 'corrupted cache');
    const result = spawnSync(process.execPath, [script], { cwd: root, encoding: 'utf8' });
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /SHA-256 mismatch/);
    await assert.rejects(access(cached));
  } finally { await rm(root, { recursive: true, force: true }); }
});
