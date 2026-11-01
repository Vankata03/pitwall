import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdtemp, mkdir, copyFile, writeFile, rm, readdir } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { chromium } from 'playwright';
import { computeExecutablePath } from '@puppeteer/browsers';

const legacy = process.argv.includes('--chromium-127');
const output = path.resolve('artifacts/smoke', legacy ? 'chromium-127' : 'chromium-pinned');
await mkdir(output, { recursive: true });
const data = await mkdtemp(path.join(tmpdir(), 'pitwall-smoke-'));
await mkdir(path.join(data, 'pitwall/recordings'), { recursive: true });
await copyFile('tests/fixtures/assumed-full-grid.pitrec', path.join(data, 'pitwall/recordings/assumed-full-grid.pitrec'));
// Port zero is forbidden by product config. Use distinct harness ports; failures are fatal.
const port = legacy ? 18779 : 18778;
const origin = `http://127.0.0.1:${port}`;
const host = spawn(path.resolve(`target/debug/examples/smoke_host${process.platform === 'win32' ? '.exe' : ''}`),
  [data, '--console', '--bind', '127.0.0.1', '--http-port', String(port), '--udp-port', String(port + 1000)]);
let log = '';
host.stdout.on('data', b => { log += b; });
host.stderr.on('data', b => { log += b; });
let spawnError;
host.on('error', e => { spawnError = e; });
let browser;
const errors = [];
try {
  let ready = false;
  for (let i = 0; i < 200; i++) {
    if (spawnError) throw spawnError;
    assert.equal(host.exitCode, null, log);
    try { ready = (await fetch(`${origin}/api/layouts`)).ok; } catch {}
    if (ready) break;
    await new Promise(r => setTimeout(r, 100));
  }
  assert.ok(ready, `collector did not start: ${log}`);
  const post = async (route, body) => {
    const response = await fetch(`${origin}/api/replay/${route}`, {
      method: 'POST', headers: { 'Content-Type': 'application/json', Origin: origin }, body: JSON.stringify(body),
    });
    assert.ok(response.ok, `${route}: ${response.status}`);
  };
  await post('start', { name: 'assumed-full-grid.pitrec' });
  // Load is asynchronous. A successful pause proves the replay exists.
  let loaded = false;
  for (let i = 0; i < 200; i++) {
    try { await post('pause', { paused: true }); loaded = true; break; } catch {}
    await new Promise(r => setTimeout(r, 50));
  }
  assert.ok(loaded, 'replay failed to load');
  browser = await chromium.launch({ headless: true, ...(legacy ? {
    executablePath: computeExecutablePath({ browser: 'chrome', buildId: '127.0.6533.88', cacheDir: path.resolve('.cache/chrome') }),
  } : {}) });
  assert.ok(!legacy || browser.version().startsWith('127.'), browser.version());
  const { layouts } = await (await fetch(`${origin}/api/layouts`)).json();
  const covered = new Set();
  for (const preset of layouts.filter(l => l.preset)) {
    const layout = await (await fetch(`${origin}/api/layouts/${encodeURIComponent(preset.name)}`)).json();
    const expected = layout.arrangements.wide.map(t => t.type);
    expected.forEach(t => covered.add(t));
    const page = await browser.newPage({ viewport: { width: 1920, height: 1080 } });
    page.on('pageerror', e => errors.push(`${preset.name}: ${e.stack}`));
    page.on('console', m => { if (m.type() === 'error') errors.push(`${preset.name}: ${m.text()}`); });
    page.on('requestfailed', r => errors.push(`${preset.name}: ${r.url()} ${r.failure()?.errorText}`));
    page.on('response', r => { if (r.status() >= 400) errors.push(`${preset.name}: HTTP ${r.status()} ${r.url()}`); });
    // Observe the real socket without replacing messages or product stores.
    await page.addInitScript(() => {
      window.__smoke = { frames: 0, state: null, epoch: null };
      const Native = window.WebSocket;
      window.WebSocket = class extends Native {
        constructor(...args) {
          super(...args);
          this.addEventListener('message', event => {
            if (typeof event.data === 'string') {
              const message = JSON.parse(event.data);
              if (message.type === 'state') window.__smoke.state = message;
              if (message.type === 'snapshot') window.__smoke.state = { ...message.state, epoch: message.epoch };
            } else {
              window.__smoke.frames++;
              window.__smoke.epoch = new DataView(event.data).getUint32(0, true);
            }
          });
        }
      };
    });
    await page.goto(`${origin}/?layout=${encodeURIComponent(preset.name)}`);
    for (const position of [1, 15, 29]) {
      await post('seek', { position });
      await page.waitForFunction(p => window.__smoke.state?.context?.replay?.position === p && window.__smoke.epoch === window.__smoke.state.epoch && window.__smoke.state.cars.filter(Boolean).length === 24, position);
      await page.waitForFunction(n => document.querySelectorAll('.react-grid-item').length === n, expected.length);
      await page.waitForTimeout(350); // Let scheduled draw calls run after the reset.
      assert.equal(await page.locator('[data-testid="unknown-tile"], .pw-load-error').count(), 0);
      await page.waitForFunction(() => {
        const speed = document.querySelector('[data-testid="speed"]');
        return !speed || /^2[0-9]{2}$/.test(speed.textContent ?? '');
      });
      await page.screenshot({ path: path.join(output, `${preset.name.replaceAll(' ', '-')}-${position}.png`), fullPage: true });
    }
    await page.close();
  }
  const types = (await readdir('dashboard/src/tiles/types')).filter(f => f.endsWith('.tsx') && !f.endsWith('.test.tsx')).map(f => f.slice(0, -4));
  assert.deepEqual([...covered].sort(), types.sort(), 'presets must exercise every registered tile');
  assert.deepEqual(errors, [], 'browser/tile errors');
  console.log(`PASS ${browser.version()}: ${layouts.filter(l => l.preset).length} presets × 3 seeks; ${covered.size} tile types; ${output}`);
} finally {
  await writeFile(path.join(output, 'collector.log'), log);
  await writeFile(path.join(output, 'errors.json'), JSON.stringify(errors, null, 2));
  await browser?.close();
  host.kill();
  await new Promise(resolve => host.exitCode !== null || spawnError ? resolve() : host.once('exit', resolve));
  await rm(data, { recursive: true, force: true });
}
