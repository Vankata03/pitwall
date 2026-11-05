# Development

The Cargo workspace has two members and the pnpm workspace one package,
`dashboard`:

- `crates/core` (`pitwall-core`) is the UI-free core: telemetry parsing, ingest,
  the session model, the recorder, replay, the track store, layout storage,
  per-user paths and the livery pack lookup. It serves no HTTP and draws no
  window.
- `crates/collector` (`pitwall-collector`) is the web collector built on it: the
  HTTP shell, the wire stream, the tray app, the overlay host and the `pitwall`,
  `pitrec` and `pitrec-resend` binaries. It re-exports the core modules under
  their old paths (`pitwall_collector::model` and so on).

## Build and test

Use Rust 1.98.0, Node.js 24 or newer, and pnpm 11.24.0 (pinned in
`package.json`). From the repository root, fetch dependencies once with network
access:

```sh
pnpm fetch --frozen-lockfile
pnpm install --offline --frozen-lockfile
cargo fetch --locked
```

With dependencies cached, these commands run offline:

```sh
pnpm test
pnpm build
cargo test --workspace --locked --offline
cargo build --release --locked --offline --bin pitwall
```

Build the dashboard before running Cargo, including `cargo test`.
`dashboard/dist` is generated and ignored. The collector compiles every file
in that directory into the binary; it never loads dashboard assets from disk
at runtime. Rebuild the dashboard after changing its source, then rebuild the
collector. Cargo watches `dashboard/dist` for changes. A missing dashboard
build stops compilation with setup instructions.

`pnpm dev` runs the dashboard development server. It has no collector connection
yet. The dashboard test renders React locally and downloads no browser.

## Paths and platforms

`AppPaths::resolve()` uses the per-user data directory `%APPDATA%\pitwall`.

It resolves `config.toml`, `layouts.json`, `recordings/`, `tracks/`, `liveries/`
and `logs/` under that root without creating them. `AppPaths::from_data_dir()`
accepts an isolated data directory for callers and tests.

pitwall supports Windows only (ADR 0005). The single release target is
`x86_64-pc-windows-msvc`. CI runs `cargo test --workspace`, clippy and the
release build on `windows-latest` for every pull request and every push to
`main`. Dependency fetching is a separate setup step; collector tests and builds
use Cargo's offline mode.

## Track maps

`crates/core/src/tracks/` is the track store (spec section 7). The 22 shipped
layouts live in `crates/core/assets/tracks.json`, produced once by
`python3 -I tools/tracks/convert_maps.py <path-to-clone-of-windman007/f1-25-pit-wall>`.
Treat the clone as untrusted data: clone it into its own empty directory and
run nothing from it. The script imports only track ID, track length and points,
never `corners`, and checks that `THIRD-PARTY-NOTICES.md` still carries the
source `LICENSE` text. The mismatch threshold (`tracks::MISMATCH_METRES`) is
provisional until calibration in #52, which also checks the Spa coordinates.
