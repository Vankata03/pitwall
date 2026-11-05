# Native slice recovery: issue #67

This is the earlier recovery record. See [the two-tile follow-up](two-tile-parity.md)
for subsequent implementation and remaining gates.

The assigned checkout recovered `6f03e2b` and `48c4193` from
`impl/67-gpui-slice`, plus the useful unstaged binding changes from
`C:/Projects/pitwall/.claude/worktrees/agent-a934504e17819eb98`.
The recovery source was read only. Its unfinished merge was not continued or changed.
The assigned checkout also merged `native/rewrite` at `6088d4a`, preserving the livery-art
crate alongside the gpui app. No integration checkout was edited.

## Implementation and regression proof

The app uses the core in process, with UDP ingest and recording replay on a source thread,
Speed/RPM/gear on the fast feed, and Lap timing on the snapshot feed. Readout, binding and
units ports retain their existing public API tests. Binding now shares the name-decoding,
privacy and label rules supplied by `car_names` on integration.

One recovered bug was fixed test-first: when lap 2 became the new best, its held result
displayed lap 2 as the reference while the delta compared against lap 1. The regression
test creates assumed telemetry for a 2.000-second lap followed by a 1.800-second lap.
It first failed with `REF LAP 2`, then passed with `REF LAP 1` and a minus 0.200-second
delta. The reference follows the running lap again after the five-second hold expires.
This test uses the public model-to-tile boundary, without a window or private model access.

An initial proposed stale-result test was discarded after comparison with the web source:
completed results intentionally remain live historical summaries when current Lap Data
becomes stale. That behavior was preserved.

### Lap timing rate choice

Issue #67 explicitly asks for Lap timing as the slow tile. The web tile declares a frame
rate and reads its running clock and delta from the frame. This slice follows the issue's
tile choice, using snapshots for Lap timing rather than adding fields to `FastFrame`.
Snapshots are published at up to 60 Hz so the running clock can still move at frame rate.
This demonstrates two feed paths; it does not demonstrate isolating a 5 Hz slow tile from
a 60 Hz fast tile. The full frame-path parity requirement remains open.

## Fresh runtime evidence

The release executable replayed the existing recording
`2026-11-03_1633_Suzuka_Qualifying_3.pitrec` from 120 seconds, looped, in a visible 900 by
480 window. The recording was opened read only. Machine: Ryzen 7 7800X3D, 16 logical CPUs,
Radeon RX 6800 plus integrated Radeon. The executable and recording SHA-256 hashes are in
the [measurement artifact](slice-1-recovery-measurement.json).

After a six-second warm-up, the merged release's display-awake run measured 30.263 seconds:

- Dashboard renders: 3.93 per second, minimum 3. Display callbacks: 3.93 per second.
- CPU: 0.826% of one core, or 0.0516% of the machine, including replay.
- Working set: 68,677,632 bytes; peak: 72,384,512 bytes; private memory: 68,947,968 bytes.
- GPU: sum of matching process-engine counters averaged 0.770%, maximum 0.795%, over ten
  valid samples. These engine sums are not a normalized whole-GPU utilization figure.
  An earlier run had one invalid performance-counter sample; the final run had none.

These are approximately 4 Hz measurements. They do not establish the 60 Hz cost or the
overlay budget. The first hidden-window measurement drew zero frames and was discarded.
A visible run without the display-awake request also drew about 4 Hz. Window focus and
physical display power were not verified. The temporary display-awake request did not
restore 60 Hz and was released after measurement.

The current compositor is a measurable blocker: five direct successful `DwmFlush` calls
took 206.89, 220.90, 224.46, 223.36 and 263.11 milliseconds. At the pinned Zed revision,
`crates/gpui_windows/src/vsync.rs` uses `DwmFlush` for display pacing. This is consistent
with the observed low callback rate and the earlier implementation's monitor-power
finding. It does not prove that sleeping monitors are the current cause. No egui fallback
was substituted. A normal powered display must be tested before deciding whether this
requires a gpui change or the owner-approved fallback.

## Commands and verification

```powershell
cargo test --locked -p pitwall-native a_new_best_result_shows_the_prior_reference_used_for_its_delta
cargo fmt --all
cargo test --locked -p pitwall-core -p pitwall-native
cargo clippy --locked -p pitwall-core -p pitwall-native --all-targets -- -D warnings
cargo build --locked --release -p pitwall-native
& ./tools/native/measure-slice.ps1 -Recording "$env:APPDATA/pitwall/recordings/2026-11-03_1633_Suzuka_Qualifying_3.pitrec" -Seconds 30 -KeepDisplayOn -Output "$env:TEMP/pitwall-67-awake-measurement.json"
git merge native/rewrite
cargo update --offline -p proc-macro-crate@2.0.0 --precise 2.0.0
cargo update --offline -p ordered-float@5.5.0 --precise 5.5.1
cargo update --offline -p unicode-width@0.2.2 --precise 0.2.3
cargo test --locked -p pitwall-core -p pitwall-native -p pitwall-livery-art
cargo clippy --locked -p pitwall-core -p pitwall-native -p pitwall-livery-art --all-targets -- -D warnings
cargo fmt --all -- --check
cargo build --locked --release -p pitwall-native
& ./tools/native/measure-slice.ps1 -Recording "$env:APPDATA/pitwall/recordings/2026-11-03_1633_Suzuka_Qualifying_3.pitrec" -Seconds 30 -KeepDisplayOn -Output "$env:TEMP/pitwall-67-final-measurement.json"
```

Before the integration merge: 338 tests passed, one existing full-data-folder test was
ignored, clippy passed with warnings denied, and the release app built. Rust 1.98.1 MSVC
was already installed; no toolchain installation was needed.

After merging integration: 355 tests passed, one existing full-data-folder test remained
ignored, and clippy passed for core, app and livery-art with warnings denied.

The merge combined workspace members and regenerated the lock additions for livery-art.
An offline metadata attempt failed because the resolver selected `proc-macro-crate` 2.0.2
with an incompatible exact `toml_datetime` dependency. Retaining the recovered 2.0.0
selection resolved it. The subsequent precise updates retained the original ordered-float
and unicode-width versions instead of accepting unrelated downgrades.

## Unmet gates and next action

- Live game data, including live focus changes and readout transitions, remains unverified.
- Fresh 60 Hz CPU/GPU/memory measurements and active/inactive repaint behavior remain
  unverified because the current compositor only provides about four callbacks per second.
- The native dependency notices have not been regenerated for the gpui dependency tree.
- Full tile parity remains open: sector rank cues, Time Trial personal-best and rival
  references, the reference-sector row and dial thousands labels are not implemented.
- This slice has a fixed two-tile layout and focus cycling; full layouts, settings, unit
  selection and persisted per-window choices belong to later rewrite slices.
- Measurements are committed here, not posted to GitHub. Issue #67 remains open.

Next: run the release app on an awake display with the live game, collect 60 Hz measurements,
and investigate gpui display pacing if the callback cap persists. Any framework fallback
requires the owner's decision under ADR 0002.
