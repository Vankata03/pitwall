# Recording test harness

## Browser smoke

Run from the repository root:

```sh
pnpm install --frozen-lockfile
pnpm exec playwright install chromium
pnpm exec browsers install chrome@127.0.6533.88 --path .cache/chrome
pnpm build
cargo build --locked --example smoke_host
pnpm test:smoke
pnpm test:smoke --chromium-127
node --test tools/harness/goldens.test.mjs
```

Playwright is pinned in `package.json` and the lockfile; its bundled Chromium
revision is fixed by that version. The compatibility run uses Chrome for Testing
127.0.6533.88. Both runs use headless mode and the production collector runtime,
with a temporary data directory. They require free HTTP ports 18778/18779 and UDP
ports 19778/19779. No user recordings, layouts or livery packs are accessed.

Every shipped preset loads at 1920 × 1080, then seeks the recording to 1, 15 and
29 seconds. The harness waits for replay State and Frame traffic and rendered
speed, checks tile coverage, and fails on browser, network or tile draw errors.
Screenshots go to `artifacts/smoke/` and CI uploads them without image comparison.
This is a rendering smoke check, not device, latency or visual acceptance proof.

`tests/fixtures/assumed-full-grid.pitrec` is synthetic, not a golden. It contains
24 invented AI drivers over three laps; car 23 has restricted private telemetry.
Regenerate it with a new temporary capture path and remove the previous output
before conversion (the tools refuse overwrites):

```sh
python3 tools/telemetry/smoke_assumed.py /tmp/assumed-full-grid.pwcap
cargo run --bin pitrec -- convert /tmp/assumed-full-grid.pwcap tests/fixtures/assumed-full-grid.pitrec
```

## Goldens and benchmark

`tests/goldens/manifest.json` starts empty. Ticket #52 owns real captures and
approval. Each future recording entry has `id`, `file` (Release asset basename)
and lowercase `sha256`; slots reference entry IDs. The three reserved slots are
the short Spa clip, full Spa recording and original capture. Assets come from the
`goldens` Release in `Vankata03/pitwall`. CI caches `.cache/goldens` by manifest
hash and verifies SHA-256 on every use, including cache hits. Corrupt files fail
and are evicted. Release uploads require separate owner approval.

`pnpm bench:ingest BASE_CHECKOUT` skips with a message until the full Spa slot is
pinned. It builds the same benchmark helper against base and head, then measures
pitrec decompression, parsing, ingest gating and model updates, excluding disk
reads and compilation. It warms both binaries and alternates seven samples each.
A median increase of 25% or more fails. Windows CI runs it on pushes to `main`
and PRs carrying `perf`, including when that label is added. No synthetic fixture
is substituted for the full golden.
