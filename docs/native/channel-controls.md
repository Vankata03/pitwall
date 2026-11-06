# Input channel controls and feed-history cost

This slice follows `native/rewrite` at `e267df6758f704cce833dafa9bec7d0cf1af4c58` for issues #71 and #64. It completes the settings control required by INPUT-01. It does not establish release readiness or close performance and manual verification gates.

## Settings behavior

The existing list-setting control now offers Channels for Throttle/brake traces. It reads `settings_of` and applies `toggle_channel`, so the dialog and plot use the same rules: Throttle, Brake, Speed in canonical order; pedals by default; unknown saved channels ignored; pedals used when no valid channel remains; and no action when the user tries to deselect the last channel. The controller changes only `channels`. Reference lap and unrelated settings survive draft preview, Cancel, Done, and reload. Race-control feed settings retain unknown entries and allow an empty selection.

The control retains the feed checkbox role, label, checked state, 44-pixel target, tab stop, and visible focus ring. In the pinned gpui revision `654efae441bc8eb117d0042bb28cd7cf3570bdc9`, `crates/gpui/src/elements/div.rs` maps an unmodified Enter/Space down/up pair to `on_click` while focus stays on the element (around lines 3078-3155). No extra key handler is needed. This is source verification; physical keyboard and screen-reader checks remain open.

Parent-confirmed test seams are registry choices/actions, controller draft/save/cancel, and the prepared trace view. The first test failed because the registry offered zero channel groups before the implementation. Removing the last-channel guard subsequently made both new seam tests fail. The guard was restored.

Named native evidence:

- `input_channel_choices_match_web_defaults_canonical_order_and_last_selection_guard` covers the dialog choices, defaults, malformed/empty/unknown values, duplicates, canonical order, unsupported actions, and last-channel protection.
- `input_channels_use_draft_save_cancel_and_prepare_the_selected_trace_view` follows a channel action through the real layout controller, preview, Cancel, Done, disk reload, the Speed-only plot, previous-lap legend, and unchanged THR/BRK meters.
- `feed_settings_toggle_known_types_without_losing_future_types_or_empty_selection` and `feed_checkbox_changes_use_the_layout_draft_and_preserve_other_settings_on_save` retain the existing feed behavior.
- `draws_recent_frames_against_time_with_throttle_and_brake_in_percent`, `the_channels_setting_chooses_what_is_plotted_with_speed_in_the_devices_unit`, and `native_publication_rates_keep_the_full_thirty_second_window_beyond_feed_history` cover INPUT-01 plot behavior and retention.

## Feed storage and catch-up allocation

The feed retains 2,400 publications, representing the ten-second tail at the native 240 Hz publication budget. The input tile keeps its separate 30-second history. Neither limit nor overrun handling changes here.

The encoded history has 146 atomic words plus one sequence word per slot: `2400 * 147 * 8 = 2,822,400` bytes (2.692 MiB), excluding allocator metadata and the rest of the shared feed. The earlier 256-slot, 98-word history used `256 * 99 * 8 = 202,752` bytes (0.193 MiB). The storage increase is 2,619,648 bytes per feed. Readers clone the shared Arc; a window does not duplicate the encoded ring merely by cloning its reader. Distinct feed channels each own a ring.

The decoded `FastFrame` is 1,192 bytes. A full catch-up batch needs 2,860,800 bytes of element storage. Repeated Vec growth previously retained capacity for 4,096 frames, or 4,882,432 bytes. Reserving the bounded observed batch size avoids those growth allocations and copies. Idle and future cursors allocate no frame storage. `catch_up_allocation_is_bounded_by_one_decoded_history_ring` failed on the old capacity and now checks allocation bounds, idle/future cursors, and the same ordered overflow tail. The existing concurrent-reader test still verifies untorn generations.

## Reproducible feed benchmark

The documented `fast_path_bench` example was absent at the starting revision. Its original source was restored from `688b213:crates/core/examples/fast_path_bench.rs`, retaining the synthetic 22-car fixture and original timings. Added measurements cover channel construction and 0/1/4/2400-frame catch-up reads, with assertions on count and overrun state.

```powershell
cargo run --release -p pitwall-core --example fast_path_bench --locked
```

Machine: AMD Ryzen 7 7800X3D, 16 logical processors, Windows, rustc 1.96.0. Results below are medians of five batches from sequential runs on this machine, using the same harness and frame/history shape. The only feed change between these two runs was the bounded reservation. They are short microbenchmarks, not app or game budgets.

| Operation | Before reservation | After reservation |
| --- | ---: | ---: |
| Channel allocation, initialization, and drop | 468,415 ns | 463,774 ns |
| `publish_fast` | 363.6 ns | 465.9 ns |
| Latest-frame read | 114.6 ns | 114.9 ns |
| Generation read | 1.0 ns | 1.0 ns |
| Catch up 0 frames | 7 ns | 7 ns |
| Catch up 1 frame | 221 ns | 208 ns |
| Catch up 4 frames | 717 ns | 561 ns |
| Catch up 2,400 frames | 1,038,271 ns | 617,420 ns |

Reservation changes only the catch-up allocation. The publication timing variation does not prove a writer regression or improvement. The old 265.8 ns publish / 71.7 ns latest-read figures in `fast-path.md` used a smaller frame and ring; they are historical context, not a matched baseline. The full retained batch is an overrun/late-join cost, not the ordinary per-frame read size.

## Recording measurements

The existing harnesses ran against original recordings opened read-only, after validation and release compilation finished. The native harness now always passes an isolated temporary data directory and `--console`; it queries only GPU engine instances for its own process. It cannot write layouts, config, logs, window state, or overlay settings into the original user data folder. The overlay harness received no `--settings`, so it did not save geometry. Executables were copied once into `artifacts/channel-controls/bin` before these runs and were not moved during measurement. No game process was opened or inspected.

Exact commands from the repository root:

```powershell
New-Item -ItemType Directory -Force artifacts/channel-controls/bin
cargo build --release --locked -p pitwall-native -p pitwall-overlay
Copy-Item target/release/pitwall-native.exe artifacts/channel-controls/bin/pitwall-native.exe
Copy-Item target/release/pitwall-overlay-replay.exe artifacts/channel-controls/bin/pitwall-overlay-replay.exe
& ./tools/native/measure-slice.ps1 -Recording "$env:APPDATA/pitwall/recordings/2026-11-03_1633_Suzuka_Qualifying_3.pitrec" -Executable ./artifacts/channel-controls/bin/pitwall-native.exe -Seconds 20 -KeepDisplayOn -Output ./artifacts/channel-controls/native-measurement-final.json
$env:PITWALL_OVERLAY_PROFILE='1'
Remove-Item Env:PITWALL_OVERLAY_RENDER -ErrorAction SilentlyContinue
& ./artifacts/channel-controls/bin/pitwall-overlay-replay.exe "$env:APPDATA/pitwall/recordings/2026-11-01_1015_Melbourne_Time_Trial.pitrec" --start 120 --seconds 20
```

The GPU-sampled overlay repeat used the same arguments with `Start-Process -WindowStyle Hidden -PassThru`, redirected output, a two-second startup wait, then `Get-Counter -Counter "\GPU Engine(pid_$($overlayRun.Id)_*)\Utilization Percentage" -SampleInterval 1 -MaxSamples 8`. It waited for only that child process to exit. The first GPU query ran too late and returned zero valid samples; it is excluded.

Raw results, executable hashes, recording hashes, and limitations are preserved in [channel-controls-measurements.json](channel-controls-measurements.json). The native run replayed Suzuka Qualifying 3 from second 120 in the default Pitwall Classic layout, at 900 by 480 logical pixels. It measured 20.805 seconds after a six-second warmup: 33.19 changed renders/s, 163.05 display callbacks/s, 14.12% of one core (0.882% of the machine), 190,836,736 private bytes, and 102,768,640 working-set bytes. Its ten counter polls averaged a process-engine sum of 9.547%, maximum 10.502%; engine sums are not total adapter utilization. Display wake was requested, but physical display state and focus were not confirmed. Changed render count is not display callback count and does not prove 60 Hz content rendering.

The overlay ran Melbourne Time Trial seconds 120-140, normal speed, default composition renderer, 360 by 110 physical pixels. Both runs reported zero render failures:

| Run | Presented fps | Overlay thread, % of one core | Whole process, % of one core | Private / peak MiB | Mean draw ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| First | 58.8 | 1.80 | 2.50 | 48.0 / 48.0 | 0.421 |
| GPU-sampled repeat | 57.8 | 0.08 | 0.16 | 47.7 / 47.7 | 0.407 |

The repeat returned 128 valid individual engine-instance samples, mean 0.06157% and maximum 1.00932%. These are neither summed adapter utilization nor a GPU comparison with WebView2. CPU variation across the two short runs prevents a stable CPU claim. Overlay thread CPU excludes driver worker threads; whole-process CPU includes replay and publication. Private bytes include replay/checkpoints, both feed storage and graphics resources. They do not isolate ring storage. The initial native run overlapped compilation and is excluded from the reported native result.

The historical native measurement used an earlier two-tile slice, a different feed and repaint behavior. Historical overlay measurements used smaller feed frames/history, and no contemporaneous WebView2 run was made here. Those conditions do not match, so no app or overlay regression ratio is reported. Nothing here proves the tenfold CPU/memory budget, total GPU cost, live datagram-to-draw latency, zero collector drops, multiwindow cost, performance under game load, or long-run stability. Side-by-side web/native checks, physical keyboard checks, screen-reader checks, and game interaction remain open.

## Validation

```powershell
pnpm install --frozen-lockfile
pnpm build
$env:TEMP=Join-Path $env:LOCALAPPDATA ('Temp/pitwall-channels-verified-'+[guid]::NewGuid())
$env:TMP=$env:TEMP
New-Item -ItemType Directory $env:TEMP
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all -- --check
cargo build --release --locked -p pitwall-native -p pitwall-overlay
```

The fresh TEMP/TMP workspace run passed 787 tests, with zero failures and eight ignored tests. Strict Clippy, formatting, the native/overlay release build, and the existing web build passed. The first workspace attempt stopped because this fresh worktree lacked dashboard build assets; the web build supplied them before the successful rerun. INPUT-01 is complete at the implementation/test level. VERIFY-02, VERIFY-03, RENDER, OVERLAY and other manual/performance gates remain open.
