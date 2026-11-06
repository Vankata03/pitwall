# Native remaining tiles: issue #73

This work ports six tiles to the native app: Track map, Weather, Race-control feed, Track
discipline, SM and Notice. Each tile lives in `crates/app/src/tiles/<tile>.rs` and follows
the pattern described in [frame-rate-tiles.md](frame-rate-tiles.md): an `Inputs` struct
built from core data, a pure `view()` that returns a view model, and a gpui `render()`.
The tests port the TypeScript tests and run against the view model. Two model-driven tests
in `tiles/remaining_tests.rs` feed synthetic (assumed) packets through a session model.

The issue also lists "Team". No tile type has that name; the 19 registered types are the
baseline (DEC-01). Team colours appear as the Track map's dot colours, through
`pitwall_core::livery::team_colour`, and as the timing tower's team marker (#72).

## Shared pieces

- Each tile has an entry in `dashboard/registry.rs`. The feed's `entryTypes` now has
  independent checkboxes in the layout dialog. Empty selections and unknown saved types
  survive edits; Cancel discards the draft and Done saves it with the other settings.
- The Track map reads maps from the host's track store in-process. `host::Runtime` now
  exposes the store (`tracks`), and `main` hands it to the tile through
  `track_map::TRACKS`. The tile asks the store again only when the track or the store's
  revision changes, so a map the learner is still building grows on screen.
- `tiles/memory.rs` gained the feed's headline hold and the map's fit and last lookup.

## Differences from the web build

- **Track map lookup.** The web tile fetches `/api/tracks/<id>/<length>` and rejects a
  response for an old session. The native tile reads the store directly, keyed by track,
  so no stale response can arrive. "Map unavailable" shows only when no store is set.
- **Race-control resets.** The web feed clears pending headlines on a transport epoch
  change. In-process there is no transport epoch, so the native feed resets on a model
  epoch change (new session or flashback) or when the log no longer starts with the
  entries already shown, which is what a replay seek does.
- **Held headlines.** The dashboard redraws when new data arrives. A routine headline
  held when a replay pauses shows on the next redraw, not exactly three seconds later.
- **Weather icon.** The sky glyphs are drawn with canvas strokes in the same shapes as the
  web SVG, slightly simplified.
- **Notice link.** In an ordinary dashboard, the exact short line in the Notice tile
  opens the existing status surface scrolled to its full notice. It is keyboard
  focusable. In stream output and edit mode it remains text without an activation
  control. The dashboard's independent short line remains visible.
- **Accessibility.** The pinned gpui supports accessible names and Windows AccessKit.
  The map now exposes its track name and car-state summary as an image name, without a
  live region. Screen-reader behavior remains unverified. Source evidence and tests
  are recorded in [tile-interactions.md](tile-interactions.md).

## Visual check

Screenshots in [tiles/](tiles/) come from the Shanghai Race recording
(`2026-11-02_1744_Shanghai_Race.pitrec`), replayed from 400 s with an empty data folder.
The shipped Shanghai outline supplies the map; no pack art is present.

- `remaining-tiles-classic.png`: Pitwall Classic. The feed is a one-line strip (track
  status, then the newest entry; entries that do not fit wrap out of sight whole), with
  SM (DRS), Weather at its two-row height and Track map.
- `remaining-tiles-race.png`: Race. The feed is a list, newest first, with each entry's
  source at Full density. Weather shows its forecast line.
- `remaining-tiles-driver.png`: Driver, with Track discipline at Regular density.

Comparing each tile side by side with the web build on the same recording is a manual
owner check and has not been done.

## Verification

```powershell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```
