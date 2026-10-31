# Development

The scaffold contains one Cargo workspace member, `crates/collector`, and one
pnpm workspace package, `dashboard`. The collector library exposes per-user
paths and embedded asset lookup; its `pitwall` binary prints a startup summary
and exits. HTTP serving, telemetry, settings, tiles and packaging belong to
later tickets.

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

`AppPaths::resolve()` uses the OS per-user data directory:

- Windows: `%APPDATA%\pitwall`
- macOS: `~/Library/Application Support/pitwall`

It resolves `config.toml`, `layouts.json`, `recordings/`, `tracks/`, `liveries/`
and `logs/` under that root without creating them. `AppPaths::from_data_dir()`
accepts an isolated data directory for callers and tests.

Release targets are `x86_64-pc-windows-msvc` and `aarch64-apple-darwin`.
CI runs tests and release builds on Windows for every pull request, and on
macOS arm64 for pushes to `main`. Dependency fetching is a separate setup step;
collector tests and builds use Cargo's offline mode. An arm64 Mac can verify
its native target locally; Windows execution requires the Windows CI runner.
