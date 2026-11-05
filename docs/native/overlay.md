# Native throttle/brake overlay (#68)

The developer entry point is `pitwall-overlay-replay`. It opens a framework-free Win32
overlay on its own thread and reads `pitwall_core::feed` in-process. It does not open or
hook the game process. The existing collector still ships the WebView2 overlay until
the native application integrates this crate and passes the release gates.

## Recovery and implementation

Recovered `5b4c637` and `c58fea3`, then copied the uncommitted `crates/overlay` from
`C:/Projects/pitwall/.claude/worktrees/agent-ada2ff256f52d0d2a`. Only this assigned
checkout was changed. The workspace manifest adds `pitwall-overlay`; Cargo regenerated
the lockfile instead of replacing it with the recovery checkout's older lockfile.

Shared trace logic lives in `pitwall-core::traces`: smoothing, bounded input history,
lap-distance reference alignment, current-lap buckets and the time-window plot. The
existing `pitwall-core::lap_traces` cache and its tests remain the #69 implementation.
Readout, binding and units modules owned by #67 were not added or changed.

`pitwall_overlay::spawn(reader, settings, optional_settings_path)` returns after the
first successful draw. `Overlay::set_moving` changes mouse handling without activating
the window; leaving move mode and closing save geometry when a path was supplied.
`Overlay::close` returns the final settings, and dropping the handle closes the window.
The caller owns visibility and tray state. Opening the standalone tool explicitly
shows the window even when stored visibility is false. Its default run writes no
settings; `--settings PATH` opts into loading and saving that file.

The renderer uses the ADR 0002 window styles. Direct2D draws premultiplied alpha into
a DirectComposition swap chain by default. If composition cannot initialize, a new
layered window uses a Direct2D software DC target and `UpdateLayeredWindow`.
`PITWALL_OVERLAY_RENDER=software` forces that fallback; `hardware` selects the DC target
with Direct2D's default render-target type. `PITWALL_OVERLAY_PROFILE=1` reports draw
timing. The waitable timer caps polling at 60 Hz; presentation does not wait for vsync.
Both feed generations drive repaint requests, so slow changes also redraw while the
fast path is unchanged. No feed changes means no drawing, though polling continues.
The core feed retains 256 publications for `Reader::fast_since`. The controller consumes
all retained frames between draws, preserving short pedal inputs that a latest-only read
would miss. Overruns clear the old trace before reading the retained tail. Fast frames
also carry the model epoch, preventing old inputs from entering a new snapshot's trace.

## Test ports and TDD evidence

| Source | Rust seam and tests |
| --- | --- |
| `tiles/traces/smooth.test.ts` | Core `traces::smooth_values`; `traces_smooth.rs` |
| `tiles/traces/current-lap.test.ts` | Core `traces::CurrentLap`; `traces_current_lap.rs` |
| `tiles/traces/window-plot.test.ts` | Core `traces::InputHistory` and `build_window_plot`; `traces_window_plot.rs` |
| `tiles/traces/lap-traces.test.ts` | Existing core `lap_traces::LapTraceCache`; `lap_traces.rs`, with HTTP exclusions recorded in `parity.md` |
| `overlay/throttle-brake.test.tsx` | Core `traces::input_trace`/`smooth_line`, plus overlay view and move/resize geometry tests |
| Collector overlay settings tests | Overlay settings and placement tests, including the old missing-size shape |

The recovered tests were run as existing work. New tests used the confirmed public
core trace and overlay controller/settings/renderer seams. Observed red/green cycles:

- Repeated renderer open/close leaked four handles across four cycles (`161 -> 165`).
  Owning the waitable timer handle made the same Windows lifecycle test pass.
- A slow-only snapshot update had no controller API. The new `OverlayController::poll`
  test initially failed to compile, then passed with generation tracking for both paths.
- Restricted pedals displayed an absent dash instead of `Restricted`. The value test
  failed with those literal values, then passed after preserving restricted text and
  zero fill.
- Renderer metrics were absent. The lifecycle test initially failed to compile, then
  passed after exposing presented-frame and render-failure counters.
- The feed had no catch-up API. Its publication-order test initially failed to compile,
  then passed with a bounded, independently seqlocked history ring. A delayed draw
  initially showed `[50]` instead of `[50, 0, 50]`; consuming the retained frames preserved
  the throttle stab. Retained generations and overrun reporting have supporting tests.
- A new-epoch snapshot initially displayed retained old inputs. The regression test
  passed after adding an epoch word to fast frames and rejecting mismatched timelines.

Startup cleanup also avoids queuing `WM_QUIT` when destroying a failed fallback
candidate, releases the window if the message pump fails, and retries drawing after a
device error. Forced device-loss and composition-initialization failure were not tested.

Verification commands, run from this checkout:

```powershell
cargo test -p pitwall-core -p pitwall-overlay --quiet
$env:PITWALL_OVERLAY_RENDER='software'
cargo test -p pitwall-overlay --test window -- --ignored --nocapture
Remove-Item Env:PITWALL_OVERLAY_RENDER
cargo clippy -p pitwall-core -p pitwall-overlay --all-targets -- -D warnings
cargo fmt --all -- --check
git diff --check
cargo build --release -p pitwall-overlay --bin pitwall-overlay-replay
```

The normal suite skips the interactive Windows lifecycle test and the unrelated
real-data-folder test. The lifecycle test was run explicitly and passed. Initial
strict clippy found `ticks % 6 == 0` in the recovered tool; `is_multiple_of(6)` fixed it.

## Recording measurements: 2026-11-05

The recording was opened read-only. Each run replayed seconds 120–140 at normal speed,
publishing at 60 Hz into a 360×110 physical-pixel overlay at 96 DPI. No game was running
as part of these checks. These are short desktop recording runs, not an F1 benchmark.

```powershell
$recording="$env:APPDATA/pitwall/recordings/2026-11-01_1015_Melbourne_Time_Trial.pitrec"
$env:PITWALL_OVERLAY_PROFILE='1'
Remove-Item Env:PITWALL_OVERLAY_RENDER -ErrorAction SilentlyContinue
& target/release/pitwall-overlay-replay.exe $recording --start 120 --seconds 20
# Repeat with each alternative:
$env:PITWALL_OVERLAY_RENDER='software'
& target/release/pitwall-overlay-replay.exe $recording --start 120 --seconds 20
$env:PITWALL_OVERLAY_RENDER='hardware'
& target/release/pitwall-overlay-replay.exe $recording --start 120 --seconds 20
```

| Run | Presented fps | Overlay thread CPU, % of one core | Whole process CPU, % of one core | Peak private MiB | Private MiB at end | Draw mean, ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Composition, first run after removing vsync wait | 58.3 | 0.70 | 1.02 | 45.3 | 45.3 | 0.433 |
| Composition, GPU-sampled run | 58.9 | 1.48 | 2.27 | 44.8 | 44.8 | 0.435 |
| Software DC, final run | 58.1 | 2.11 | 3.91 | 22.4 | 21.6 | 1.414 |
| Default-type DC | 58.4 | 5.16 | 6.09 | 42.8 | 41.3 | 1.125 |
| Composition, final catch-up build with GPU sampling | 59.1 | 0.55 | 0.55 | 44.9 | 44.9 | 0.515 |
| Software DC, final catch-up build | 59.1 | 1.56 | 4.06 | 21.6 | 20.7 | 1.011 |

The first four runs precede the bounded catch-up history change; the last two use the
final implementation. Every listed run reported zero render failures. Private memory is the whole replay
process, including core/checkpoints and graphics resources. The composition run with
GPU sampling on the final build added 31.7 MiB after opening the overlay; the final
software run added 8.9 MiB.
MiB means bytes divided by 1,048,576; the earlier tool output labelled this value MB.
Overlay CPU excludes startup and measures only its owning thread, so it excludes any
graphics driver worker threads. Whole-process CPU also includes replay and publication.

Before the vsync change, composition rendered only about four frames per second,
averaging 251 ms per draw. That measurement is not evidence of a low CPU cost at 60 fps.

GPU sampling queried only the replay process's engine instances, after waiting two
seconds for startup:

```powershell
$taskProcess=Start-Process -FilePath (Join-Path $PWD 'target/release/pitwall-overlay-replay.exe') `
    -ArgumentList @($recording,'--start','120','--seconds','20') `
    -WindowStyle Hidden -PassThru `
    -RedirectStandardOutput "$env:TEMP/pitwall-68-final.txt" `
    -RedirectStandardError "$env:TEMP/pitwall-68-final-errors.txt"
Start-Sleep -Seconds 2
$taskCounter="\GPU Engine(pid_$($taskProcess.Id)_*)\Utilization Percentage"
Get-Counter -Counter $taskCounter -SampleInterval 1 -MaxSamples 12
$taskProcess.WaitForExit()
```

The final query returned 192 engine-instance samples: mean 0.061% and maximum
1.045% for an individual engine instance. The earlier successful query returned 192
samples with mean 0.059% and maximum 1.043%. These are not total adapter utilization, nor a
comparison with a contemporaneous WebView2 GPU measurement. The first query, made
before startup completed, returned no samples and was discarded.

## Unmet acceptance gates

- F1 25 windowed/borderless visibility, click-through, retained game focus, real pointer
  move/resize and exclusive-fullscreen behavior have not been tested. Fullscreen behavior
  remains unknown for this renderer. Do not infer it from the old WebView2 overlay.
- The tenfold budget is not met across CPU and memory. Against the issue's approximate
  260 MB/14%-of-one-core baseline, composition exceeds the roughly 26 MB memory target;
  an earlier GPU-sampled CPU run also exceeded 1.4%. Software meets the approximate memory target
  but exceeds the CPU target. No contemporaneous WebView2 or F1 GPU comparison exists.
- Measurements are short runs on this desktop. They do not establish stable 60 fps,
  long-run memory bounds, performance at saved larger sizes or other DPIs, or behavior
  under game load.
- Native application/tray visibility, failure notification and retry-after-toggle wiring
  still need integration. The crate supplies window lifecycle, move mode and persistence;
  it does not replace the collector's existing tray implementation in this slice.
- Catch-up history is bounded at 256 publications. Longer stalls discard the overwritten
  portion and restart the trace with the retained tail. Live/replay source switching must
  provide a distinct timeline or a new feed; this crate compares model epochs, not source IDs.

Keep OVERLAY, PEDALS and DRAW release gates unchecked pending the missing integration,
game checks and performance evidence. The next step is to integrate this crate with the
native app and run the game checks and longer side-by-side measurements on the gaming PC.
