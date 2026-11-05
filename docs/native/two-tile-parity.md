# Two-tile parity follow-up: issue #67

The implementation branch recovered `t3/native-gpui-slice` at `b068f43` and merged
`native/rewrite` at `089dfcd`. The source and integration checkouts were not edited.
This work covers Speed/RPM/gear and Lap timing only.

Lap timing now shows session-best and personal-best sector colours with diamond and
circle glyphs. Ranking excludes the displayed car's current and later laps, ignores
invalid comparison sectors, and uses each displayed sector's validity. Equal session
sectors qualify as personal best, matching the web implementation.

Personal-best and rival references use the core's player-only Time Trial summaries.
They show plain reference lap and sector times, never seed a live distance delta, and
leave missing or zero reference sectors absent. Completed summary comparisons expire
at 5000 ms. The tile header's comparison control cycles available references without
changing other tile settings; Time Trial choices are skipped outside Time Trial.

The RPM dial labels show quarter-sweep values in rounded thousands. The Speed/RPM/gear
source adapter rejects fast frames from another model epoch before combining values
with car binding and maximum RPM. Lap timing rebuilds from the selected model car, so
held results cannot survive a car or model reset through a tile cache.

Regression tests live in `crates/app/src/tiles/lap_timing_tests.rs` and the two tile
modules. Existing readout, binding and units ports remain in `crates/core/tests`.
Fixtures are synthetic packet-table inputs, not live-game evidence.

The existing notices generator now emits the pinned native dependency tree in UTF-8
and resolves Windows checkout licence placeholders to full texts. Cached upstream
licences retain source URLs; the SeaHash and simd_helpers caches explain the missing
published licence files and the alternative upstream revisions used.

Remaining gates: live-game verification, representative GPU measurement, visual checks
of the two tiles at each density, and the broader native release inventory. The existing
[confirmed-display recording replay](slice-1-confirmed-display-measurement.json) measured 60.87 renders per second; this work does
not repeat that benchmark. Lap timing retains the slice's snapshot feed at up to 60 Hz.
Full layouts, persisted window settings, status and multiple windows remain later work.

Verification on the merged tree: 504 Rust tests passed, with 7 existing manual tests
ignored; 599 web tests passed. The dashboard production build, workspace clippy with
warnings denied, formatting check and native release build passed. Notices regenerate
identically offline, and the GPUI sections contain complete Apache licence texts.

```powershell
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo build --locked --release -p pitwall-native
python tools/notices/generate.py
pnpm build
```
