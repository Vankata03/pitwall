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

- `tiles/registry.rs` maps each saved tile type (`g-force`, `live-delta` and so on) to its
  display name and a single `render(kind, &Context, &mut Memory)`. A layout host resolves
  the car binding, then calls `render` with the bound car's index. `Memory` holds what a
  tile keeps between renders: the inputs history for Throttle/brake traces and the livery
  pictures for Car status.
- `tiles/slow.rs` is the slow-tile row helper (label over value, tone, whole-tile message).
- `tiles/paint.rs` draws the discs, arcs, polylines and filled polygons that gpui paths
  lack as primitives.
- `pitwall-native --tiles g-force,ers,...` picks the tiles the slice window shows, and
  `--data-dir DIR` replaces the OS data folder, which holds the livery pack. The window
  wraps tiles three to a row. Layouts (#70) will replace this.

## Differences from the web build

- **Live delta graph.** The web tile fills its graph from frame rings it samples while
  open. The native tile derives the graph from the core's 5 m lap traces: each sample's lap
  time against the reference lap's time at the same distance. The finished lap is compared
  with the reference it was driven against: the lap before it for Previous, and the best
  valid lap before it for Best. The whole lap shows even when the window opens mid-lap, and
  the paint cadence cannot drop samples. The ring-transport rules of DELTA-03 (overflow
  breaks, ring rollover, the official figure on the first sample after the line) do not
  apply in-process, so DELTA-03 stays open for an owner decision. The core gained
  `CarView::completed_lap_trace(lap_number)` for this.
- **Reference traces.** Throttle/brake traces reads its reference lap from the in-process
  model instead of fetching it over HTTP. The trace cache and retry rules of INPUT-03 have
  no native counterpart, so INPUT-03 stays open.
- **Engine temp gauge.** The web gauge fills with a horizontal gradient; the native gauge
  fills in four solid steps of the same colours.
- **Settings.** The tile header's "Compare" control cycles best and previous for Live delta
  and Throttle/brake traces, as it does for Lap timing. The channel setting is read and its
  rules are tested (`toggle_channel`), but there is no settings panel until layout editing
  lands, so INPUT-01 stays open.
- **Accessibility.** gpui offers no accessible name for a drawn element, so the G-force
  circle has no screen-reader summary and GFORCE-01 stays open.

## Visual check

Screenshots in [tiles/](tiles/) come from real recordings replayed with `--data-dir` set to
an empty folder, so Car status draws the fallback car rather than pack art:

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
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```
