# Native timing tiles: issue #72

This work ports four tiles to the native app: Timing tower, Gaps, Lap state and Stint.
Each lives in `crates/app/src/tiles/<tile>.rs` and follows the pattern of the frame-rate
tiles (see [frame-rate-tiles.md](frame-rate-tiles.md)): an `Inputs` struct of plain
readouts built from the model, a pure `view()` that returns a view model, and a gpui
`render()`. The tile tests port the TypeScript tests and run against the view model.
`tiles/timing_tests.rs` builds the four tiles from a model fed synthetic packets, to
check names, focus, the race status band, stint history and the reset on a new session.

The issue also lists "Gap to Driver 6", "Gap to session best", "Last lap", "Stops", "Tyre
stints" and "Current tyre set". None of these is a tile type. They are the timing tower's
gap column header (focus-relative and timed-session forms) and its Full-density Last lap
column, and the Stint tile's stop count, completed-set list and current-set block. They
are covered by the tower and Stint ports. The tower's Full-density "Wear (max)" column
(TOWER-01), deferred from #71, is part of the tower port.

## Shared pieces

- `dashboard/registry.rs` has an entry for each of the four tiles. The tower offers its
  three settings: gap reference (Interval, Leader, Focus-relative), rows (Density default,
  Full field, Around focus) and cars each side (0 to 12). A setting whose choices are
  all whole numbers now saves a JSON number, as the web build saves `radius`, and a saved
  fractional number reads as its floor. `SettingField::selected` and `SettingField::stored`
  carry this, and the dashboard's settings menu calls them.
- `tiles/memory.rs` keeps, per placed Stint tile, which completed tyre sets are expanded.
- The tower takes the window's focus car from the tile's resolved car. A session-wide
  tile has no car setting, so it resolves the default Focus target: the window's focus,
  or the player car while the window follows the player. A saved focus that is not in
  the session highlights no row.

## Differences from the web build

- **Disconnect.** The web tower and Stint tile dim live values while the socket is not
  open. In-process there is no socket; both tiles dim live values while the game is lost
  (no data for a while), which the web build also does.
- **Restricted gap cell.** A tower gap cell whose gaps readout is restricted reads
  "Restricted"; the web build printed an em dash in the restricted style. This follows
  the core tower port (#69).
- **Pit stop times.** A stop without a stopped or pit-lane time reads as an absent
  readout (em dash, muted). The web build printed the same dash in the live style.
- **Stint expand control.** Completed sets expand on click, as the web `<details>`
  element does, but gpui offers no keyboard focus or accessible name for the row, so the
  keyboard part of VISUAL-02 stays open.
- **Tyre badge name.** The view model carries the badge's accessible name ("Soft
  tyres"); gpui has no way to attach it to the drawn badge yet.

## Visual check

Screenshots in [tiles/](tiles/) come from real recordings replayed with an empty data
folder, captured from the window's own pixels:

- `timing-tiles-race-full.png`: Austria Race near the end, Race layout. The tower is at
  Full density with its three lines per car, including Wear (max); Lap state; Stint with
  the current Medium set, the completed Hard set and the lap 10 stop.
- `timing-tiles-classic-compact.png`: the same moment in Pitwall Classic, where the tower
  is Compact (position, driver, interval, tyre) and the Stint tile is shorter.
- `timing-tiles-gaps.png`: the Driver layout. The player leads, so Ahead is absent and
  Behind names P2.
- `timing-tiles-qualifying.png`: Suzuka Qualifying 1 after the end of the recording. The
  tower shows the session-best column (best time on row one, differences below), and
  the stale lap data is dimmed. At 1280 px the Race layout's tower is narrower than
  500 px and so Compact (TOWER-05).

While checking the race capture, the Gaps tile showed `+0.000 s` ahead of the race
leader: in races the game sends a zero interval for P1. `pitwall_core::gaps::gaps_view`
now reads Ahead as absent for P1 (test
`the_race_leader_has_no_car_ahead_even_though_the_game_sends_a_zero_interval`). The web
build reads the same model and probably still shows `+0.000 s` there; GAPS-01 says a
missing neighbour reads absent, so the native tile follows the parity item. The owner's
side-by-side check should confirm this.

Comparing each tile side by side with the web build on the same recording is a manual
owner check and has not been done.

## Verification

```powershell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```
