import { createHash } from 'node:crypto';
import { readFile, mkdir, rm } from 'node:fs/promises';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { pathToFileURL } from 'node:url';

export const cache = path.resolve('.cache/goldens');
export const manifest = JSON.parse(await readFile('tests/goldens/manifest.json', 'utf8'));
export async function goldens() {
  if (manifest.version !== 1 || !Array.isArray(manifest.recordings)) throw Error('Invalid golden manifest');
  await mkdir(cache, { recursive: true });
  for (const entry of manifest.recordings) {
    if (!/^[a-zA-Z0-9][a-zA-Z0-9._-]*$/.test(entry.file) || !/^[a-f0-9]{64}$/.test(entry.sha256)) throw Error('Invalid golden filename or SHA-256');
    const file = path.join(cache, entry.file);
    let bytes;
    try { bytes = await readFile(file); } catch (e) { if (e.code !== 'ENOENT') throw e; }
    if (!bytes) {
      execFileSync('gh', ['release', 'download', manifest.release, '--repo', 'Vankata03/pitwall', '--pattern', entry.file, '--dir', cache], { stdio: 'inherit' });
      bytes = await readFile(file);
    }
    if (createHash('sha256').update(bytes).digest('hex') !== entry.sha256) {
      await rm(file);
      throw Error(`SHA-256 mismatch: ${entry.file}`);
    }
    console.log(`Verified ${entry.file}`);
  }
  if (!manifest.recordings.length) console.log('SKIP goldens: manifest is empty; real captures await #52.');
}
if (import.meta.url === pathToFileURL(process.argv[1]).href) await goldens();
