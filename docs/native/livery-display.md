# Native livery display investigation

The existing pack renders in native Car status at regular and full density. The
investigation found a separate refresh defect: native retained the pack listing
and decoded image after the car image unmounted. It did not reproduce missing
art on an initial mount with the installed pack.

## Local evidence

The source folder was `%APPDATA%/pitwall/liveries`. It contained 21 PNGs:
`0.png` through `9.png`, and `476.png` through `486.png`. Every file decoded through
`pitwall_livery_art::decode_png` to 960×720 RGBA with visible pixels. The selected
`484.png` had SHA-256
`3B01BFD54EF6240C5F059CB77AF029F2F2E167FA7DBD2D01D0ADCA5899E13BB7`.

The read-only probe examined the first participant packet in each of 31 local
recordings. Twenty-five contained participant packets, all in telemetry format
2026. Their active participants used team IDs 476–486, with no My Team flags.
Every participant selected its exact decimal PNG filename. The probe compared
every decoded RGBA pixel with the native `RenderImage` BGRA frame and checked
the player livery through `SessionModel` and `car_status::Inputs::from_model`.
It did not scan later participant changes after that check.

Six recordings had no participant packet: `2026-11-02_1815_Unknown_Session.pitrec`,
`2026-11-02_2023_Unknown_Session.pitrec`,
`2026-11-02_2128_Unknown_Session.pitrec`,
`2026-11-03_1613_Unknown_Session.pitrec`,
`2026-11-03_1615_Unknown_Session.pitrec`, and
`2026-11-03_1739_Unknown_Session.pitrec`. Their livery choice is unverified.

For GPU drawing, an isolated native host replayed a copy of
`2026-11-01_1015_Melbourne_Time_Trial.pitrec` from five seconds. Its SHA-256 was
`B579E950D3CF3A35B9F0A5073065813E309AC17D5B03B5D857A13761B09FDCA8`.
Its player was index 0, team 484. Stream windows containing a single Car status
tile were captured with Win32 `PrintWindow`; the inspected images show the pack
car at regular density (700×500 window client area) and full density (950×740).
Those captures used the debug build; a separate inspected regular-density
capture from the optimized native release also showed the pack car.
Removing only the isolated pack produced the recoloured fallback. The original
pack remained installed, confirming that `--data-dir DIR` uses
`DIR/pitwall/liveries` rather than the default folder.

These private captures are retained locally under
`%TEMP%/pitwall-livery-evidence-bb2e1270-b847-466d-bbd1-5c7ef7601c08/` as
`regular-pack.png`, `full-pack.png`, and `regular-empty-data-dir.png`. The supplied
art and these captures are not bundled with the repository. The host used data
copies; the probe opened source files only for reading.

## Refresh correction

The web tile unmounts `CarImage` at Compact and lists the pack again when it
remounts. Native now refreshes its listing and decoded images at that same
transition. Unchanged mounted frames still reuse their image upload. Retained
tile paint is scoped to layout, tile ID and tile type; removed, hidden or
unavailable tiles release their caches instead of reviving an old mount later.

The regressions first failed on added pack art after Compact and on retained
paint surviving a layout change. They pass with the correction. Existing
livery-art tests cover failed decoding, missing files, replacements and My Team
selection. No folder watcher or automatic refresh while an image stays mounted
is claimed. Compact still hides the car, as in the web build.

## Verification and limits

Run the local evidence probe explicitly:

```powershell
cargo test --locked -p pitwall-native --bin pitwall-native actual_pack_and_recorded_teams -- --ignored --nocapture
```

`PITWALL_REAL_DATA` can select another pitwall root containing `liveries/` and
`recordings/`; it must supply pack art for the sampled teams. Ordinary tests do
not require these private files. Workspace tests use fresh `TEMP` and `TMP`.

`cargo test --workspace --locked` passed 789 tests, with 0 failures and 9 ignored.
`cargo clippy --workspace --all-targets --locked -- -D warnings`,
`cargo fmt --all --check`, `cargo build --release -p pitwall-native --locked`, and
`pnpm build` passed. The first workspace attempt required building the missing
`dashboard/dist`; the frozen-lockfile install and web build resolved that prerequisite.

The reported running instance remains unidentified: executable revision,
resolved data folder, selected layout, tile content dimensions/density, followed
car and mount history are unknown. The confirmed stale-cache mechanism explains
art added or replaced after a prior mount; it does not establish the cause of
that instance's initial missing-art symptom. These checks do not validate live
game access, My Team captures, telemetry format 2025, or every dashboard size.
