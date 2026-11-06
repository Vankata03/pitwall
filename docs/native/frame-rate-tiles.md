# Native frame-rate tiles: issue #71

This work ports seven tiles to the native app: G-force, Throttle/brake traces, Live delta,
ERS, Car status, Engine temp and Fuel. Each tile lives in `crates/app/src/tiles/<tile>.rs`
and follows the Speed/RPM/gear and Lap timing pattern: an `Inputs` struct built from core
data, a pure `view()` that returns a view model, and a gpui `render()` of that view model.
The tile tests port the TypeScript tests and run against the view model, so they need no
window.

The issue also lists "Wear (max)". No tile has that name. It is the Full-density column of
the timing tower (TOWER-01), so it belongs with the tower port, not here.

## Shared pieces

- Each tile has an entry in `dashboard/registry.rs` (#70), which pairs the saved type with
  its render function and the settings its dialog offers. Live delta and Throttle/brake
  traces offer the best or previous reference lap there. Render functions now also take
  a `tiles::memory::Memory`, which the dashboard keeps per placed tile, by tile id. It holds
  the observed delta lap, the inputs history for Throttle/brake traces and the livery
  pictures for Car status.
  `TileInputs` gained the feed `reader`, so a tile can catch up on every fast frame.
- `tiles/slow.rs` is the slow-tile row helper (label over value, tone, whole-tile message).
- `tiles/paint.rs` draws the discs, arcs, polylines and filled polygons that gpui paths
  lack as primitives.
- Car status reads the livery pack from the host's data folder (`--data-dir` applies).

## Delta and trace sampling equivalence

DELTA-03 and INPUT-03 remain port requirements. No owner decision dropped their behavior.
HTTP requests and the browser cache are replaced by local ownership under ADR 0001;
their visible data guarantees still apply.

Live delta consumes `FastFrame::delta_best` or `delta_previous` through
`Reader::fast_since`, rather than reconstructing the graph from historical lap traces.
The publisher reads both from `CarView::live_delta`, so reference selection and the
official headline remain model decisions. Per-tile `live_delta::Memory` keeps observed
points beyond feed rollover, replaces repeated positions, excludes the first position
after the line even when repeated, and retains the finished graph faintly. Missing
values, negative distance, short reversals, gaps over 100 m and lost feed samples break
the line. A backwards clock, car/reference change or new epoch starts fresh history.

Opening a tile midlap starts with the retained recent feed tail, as the web tile reads
its existing frame rings. It does not expose the whole lap stored in the model. The feed
holds 2,400 publications for the ten-second lookback at its 240 Hz publication budget;
consumers also trim the tail by session time. Above that rate, bounded-history overflow
still breaks the graph. The old 256-publication feed could not preserve the web ring's
ten-second lookback. Open input tiles retain thirty seconds by elapsed time, with a
20,000-sample safety bound. The web's 1,801-frame bound retained only 7.5 seconds at 240
native publications per second. Repeated paused timestamps still replace the last sample.

Throttle/brake traces reads `CarView::lap_trace` from the frame model on every preparation.
It keeps no separate reference cache. The model owns the partial current trace, completed
traces, authoritative lap summaries and named-reference selection. Completion and
same-epoch corrections change the next preparation without waiting for the 5 Hz slow
snapshot or copying a partial trace into a second cache. An unavailable named reference
stays absent; the next preparation sees it when completion supplies it. Reference changes
preserve current input history. A new epoch clears history even when the reset frame has
no car data. A frame/model epoch mismatch suppresses the old model's reference. Held
snapshots stay immutable. There are no asynchronous trace requests or replies on this path.

Both tiles keep their stale presentation. Input traces show No data, Restricted or
Stale (clock marker), with stale opacity 0.55. Live delta mutes stale graphs, permits
ordinary first-lap absence without an error, and explains Waiting and Lost. The web's
separate "Collector unreachable" connection state has no separate failure boundary when
collector and tile share one process; no connection or retry protocol was added.

### Native evidence

`crates/app/src/tiles/live_delta_tests.rs` applies synthetic datagrams to the real model,
publishes through the real feed, and observes tile inputs/views. These are deterministic
seam tests, not recording captures or visual/runtime verification.

- `delta_consumes_feed_samples_and_keeps_the_lap_beyond_feed_rollover` covers samples
  between paints, retention, the finished lap's original reference and a late join.
- `opening_a_tile_uses_the_recent_feed_tail_instead_of_the_whole_historical_lap` covers
  ten-second lookback; `opening_delta_midlap_does_not_reconstruct_unobserved_model_history`
  prevents historical reconstruction through the model adapter.
- `repeated_position_replaces_its_delta_and_fast_feed_keeps_best_and_previous_distinct`
  checks deduplication, newer values at the same position and both named deltas.
- `official_finish_is_headline_only_including_repeated_first_next_lap_positions`,
  `delta_breaks_missing_values_negative_distances_short_reversals_and_large_gaps`, and
  `overflow_keeps_old_lap_samples_but_breaks_before_the_retained_tail` cover discontinuities.
- `changing_car_or_reference_restarts_from_retained_observations_and_epoch_discards_old_laps`
  covers binding/reference changes and the model's time-reversal epoch reset.
- `shared_model_completion_and_best_correction_replace_references_without_slow_state_or_cache`
  proves partial-to-complete replacement, immutable snapshots, new Previous and same-epoch
  Best correction/restoration while slow state remains unchanged.
- `named_reference_unavailable_until_completion_is_retried_from_shared_model_without_fallback`
  covers unavailability followed by completion, without silently using another lap.
- `input_epoch_mismatch_clears_history_and_reference_before_the_model_catches_up`,
  `input_car_switch_without_a_new_publication_starts_from_the_latest_observation`, and
  `input_overflow_breaks_even_when_lost_publications_span_less_than_half_a_second` cover
  invalidation, redraw without new telemetry and explicit breaks in both series.
- `late_input_join_keeps_observed_time_only_and_silence_marks_the_same_trace_stale` and
  `stale_delta_keeps_observed_graph_muted_and_waiting_and_lost_explain_absence` cover states.

Core tests in `crates/core/tests/traces_window_plot.rs` cover missing channels, stationary
inputs, pause deduplication, time reversal and reference alignment.
`native_publication_rates_keep_the_full_thirty_second_window_beyond_feed_history` adds
240 Hz retention evidence. `crates/core/tests/feed.rs` checks publication ordering,
overflow, independent snapshots and untorn reads. Existing view tests cover Restricted,
channels, units, smoothing, density and chart geometry.

The larger feed and bounded catch-up allocation are measured in [channel-controls.md](channel-controls.md).
Those short measurements do not establish the native CPU/memory budget.
Manual side-by-side recording checks remain open. The full native port is unfinished.

## Differences from the web build

- **Engine temp gauge.** The web gauge fills with a horizontal gradient; the native gauge
  fills in four solid steps of the same colours.
- **Settings.** Throttle/brake traces now uses the existing list-setting control for channels.
  INPUT-01 implementation and named tests are recorded in [channel-controls.md](channel-controls.md).
- **Accessibility.** The G-force circle does not yet expose a screen-reader summary,
  so GFORCE-01 stays open.

## Visual check

Screenshots in [tiles/](tiles/) come from real recordings, replayed before the merge with
#70 in the slice window, which took a tile list on the command line. The data folder was
empty, so Car status draws the fallback car rather than pack art. After the merge, the
Car status was checked again inside the Pitwall Classic layout.

- `frame-tiles-time-trial.png`: Melbourne Time Trial, lap 4 against best lap 3, with a live
  delta graph and the dim reference traces.
- `frame-tiles-race.png`: Melbourne Race. The best lap's trace was not rebuilt after the
  seek, so Live delta correctly reads absent and the traces show no reference.
- `frame-tiles-compact.png`: the six tiles at Compact density.
- `car-status-full.png`, `car-status-regular.png`, `car-status-compact.png`: the three
  Car status densities.

The captures are darker than the window on screen, as noted for the slice-1 screenshot.
Comparing each tile side by side with the web build on the same recording is a manual
owner check and has not been done.

## Verification

```powershell
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all -- --check
cargo build --locked --release -p pitwall-native
pnpm build
```

These checks passed for the delta/trace sampling follow-up. Workspace tests used a fresh
`TEMP` and `TMP`. The ignored real-data and runtime tests were not run. No manual visual,
OBS, accessibility or performance gate is marked passed by these results.
