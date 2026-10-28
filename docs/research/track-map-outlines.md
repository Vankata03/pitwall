# How do we get track map outlines and sector markers?

Research for the pitwall ticket of the same name. Question: how can the track-map tile draw an accurate outline for every F1 25 circuit (including the reverse layouts), with sector boundaries, start/finish and pit lane, and what is the cheapest reliable way to cover all F1 25 tracks?

Researched 2026-10-29. The EA forum page that hosts the official spec sits behind a Cloudflare challenge, so spec quotes come from a verbatim text extraction of EA's "Data Output from F1 25 v3" document mirrored on GitHub ([MatMridul/APX-IQ `docs/Data Output from F1 25 v3.txt`](https://github.com/MatMridul/APX-IQ/blob/HEAD/docs/Data%20Output%20from%20F1%2025%20v3.txt), cited below as **Spec**). The original is [EA's F1 25 UDP Specification post](https://forums.ea.com/blog/f1-games-game-info-hub-en/f1%c2%ae-25-udp-specification/12187347).

## Short answer

Learn the outline from the game's own coordinates, indexed by lap distance. Every car's world position arrives in the motion packet, and every car's lap distance arrives in the lap-data packet. Bucketing positions by lap distance gives a polyline in which "metres from the start line" is the index. Sector boundaries, the start/finish line and the pit-lane mouth then become lookups at known lap distances, so no per-track sector data has to be stored. Ship pre-built outlines that we generate ourselves by running the same learner over our own recordings (one short session per layout, 27 layouts), and keep the learner running live so that any missing or changed track fills itself in during its first lap.

Real-world geometry (GeoJSON, OSM) adds alignment work and still needs game samples to fit, so it buys nothing. The one community dataset in game coordinates is MIT-licensed but covers only 22 of the 27 layouts and has no pit lanes, so at most it could serve as a temporary seed.

## What the F1 25 UDP spec gives us

| Need | Field | Packet / rate | Notes |
|---|---|---|---|
| Car position | `m_worldPositionX/Y/Z` (float, "World space X position - metres") | Motion, `m_carMotionData[22]`, "Rate as specified in menus" | Spec lines 172–233. All 22 cars. |
| Ground plane | X and Z | — | The spec does not say which axis is up. Two independent implementations treat X/Z as the ground plane: pits-n-giggles' radar uses `world_position.x`/`.z` (`apps/backend/rust/crates/hud-renderer/src/track_radar.rs:106-107`), and windman007's learner stores `{x, z}` ([`shared/track.ts`](https://github.com/windman007/f1-25-pit-wall/blob/HEAD/shared/track.ts)). Confirm with a recording. |
| Distance round the lap | `m_lapDistance` (float, metres, "could be negative if line hasn't been crossed yet") | Lap data, menu rate | Spec lines 582–585. |
| Track length | `m_trackLength` (uint16, "Track length in metres") | Session, 2 per second | Spec lines 249, 315–316. |
| Layout identity | `m_trackId` (int8, -1 unknown) | Session | Spec line 321. The appendix lists 24 circuits plus reverse layouts as separate IDs: 39 Silverstone (Reverse), 40 Austria (Reverse), 41 Zandvoort (Reverse) (Spec ~2790–2911). The 2026 Season Pack format adds 42 Madrid ([MacManley/f1-26-udp README, Track IDs](https://github.com/MacManley/f1-26-udp)). |
| Sector boundaries | `m_sector2LapDistanceStart`, `m_sector3LapDistanceStart` (float) | Session | Spec lines 501–502. The F1 26 header comments read "Distance in m around track where sector 2 starts" / "…sector 3 starts" ([MacManley/f1-26-udp README](https://github.com/MacManley/f1-26-udp), lines 268–269). |
| Current sector per car | `m_sector` (0/1/2) | Lap data | Spec lines 622–623. |
| Marshal zones | `m_zoneStart`: "Fraction (0..1) of way through the lap the marshal zone starts", up to 21 zones, with flag colour | Session | Spec lines 252–259, 350–354. Can colour yellow-flag stretches on the same lap-distance axis. |
| Pit lane | `m_pitStatus` (0 none, 1 pitting, 2 in pit area), `m_pitLaneTimerActive`, `m_pitLaneTimeInLaneInMS`, `m_driverStatus` (0 = in garage) | Lap data | Spec lines 618–647. **The spec carries no pit-lane geometry**: no entry/exit distances and no polyline. Only these per-car states. |
| Online visibility | Positions are not on the "Your Telemetry" restricted list | — | The list covers car status and car damage fields only (Spec 1889–1990), so the map works in online lobbies. |

### Mapping lap distance onto markers

With an outline sampled as `lapDistance → (x, z)` for one layout:

- **Start/finish** sits at lap distance 0 (by definition of `m_lapDistance`; negative values before the line is first crossed confirm 0 is the line).
- **S1/S2 boundary** sits at `m_sector2LapDistanceStart`, and the **S2/S3 boundary** at `m_sector3LapDistanceStart`. S3 runs to `m_trackLength`, which wraps back to 0.
- **Car dots** can be drawn either from raw world X/Z, which is exact and also works in the pit lane, or by interpolating the outline at the car's `m_lapDistance`, which snaps cleanly to the line. Use raw X/Z, and fall back to lap distance only when needed.
- **Marshal zones** sit at `m_zoneStart × m_trackLength`.

So sector markers cost nothing per track: they come from the live session packet and are drawn at render time.

### Pit lane

The spec only says *whether* a car is in the pit lane, never *where* it is. Pit-lane geometry therefore has to be learned too:

- Positions of cars with `m_pitStatus != 0` go into a separate pit-lane trace instead of the outline. windman007's learner, for one, skips `pit` cars and non-active `m_resultStatus` cars when building the outline (`learnTrack` in [`shared/track.ts`](https://github.com/windman007/f1-25-pit-wall/blob/HEAD/shared/track.ts)).
- Pit entry and exit on the racing line are the `m_lapDistance` values at which `m_pitStatus` changes 0→1 and back to 0. Whether lap distance stays monotonic through the pit lane is not documented, and that affects how the trace is ordered. Verify with a recording (see open questions).
- AI cars pit during races, so a race session with AI fills the pit lane without the player driving it. In practice sessions the player car leaves the garage through the pit lane (`m_driverStatus` 0 = in garage).

## Options compared

| Option | Accuracy | F1 25 coverage | Licence | Effort |
|---|---|---|---|---|
| **1. Learn from motion + lap data (record a lap)** | Exact, because it is the game's own geometry in the same frame as the live car dots. Sector and start markers line up by construction. | All 27 layouts (24 + 3 reverse), plus any future track (e.g. Madrid ID 42), as soon as one lap is observed. Pit lane needs one observed pit pass. | Our own data. | Low to medium: a bucketed learner (about 100 lines), a completeness check, persistence keyed by `(trackId, trackLength)`, plus about 27 short recording sessions to pre-build assets. |
| **2a. Community dataset in game coordinates: [windman007/f1-25-pit-wall `tracks/maps/`](https://github.com/windman007/f1-25-pit-wall/tree/HEAD/tracks/maps)** | Game coordinates, `[bucket(8 m), x, z]` per point, built by playing recordings through their learner ([`scripts/build-tracks.js`](https://github.com/windman007/f1-25-pit-wall/blob/HEAD/scripts/build-tracks.js)). Quality unverified. Austria has 537 points for a 4323 m lap. | 22 of 27. **Missing Miami, Las Vegas and all 3 reverse layouts** (README "Track map" bullet). No pit lane. | MIT. The LICENSE file is MIT © 2021 racehub-io, with a note that Delta Zero modified it, and the README says "Licensed under the MIT License". GitHub reports `NOASSERTION` because of the extra text. Usable with attribution. | Very low to import, but it is a 0-star fork pushed days ago, so treat it as unvetted. Its corner numbers come from the MultiViewer API (see 3c). |
| **2b. pits-n-giggles `assets/track-maps/*.svg`** (reference only) | Real-world shapes as 1000×600 SVG polylines, not in game coordinates, so car dots cannot be placed on them without alignment. Their track-map overlay still has a `TODO` for placing cars (`apps/hud/ui/overlays/track_map/track_map.py`). | 27 SVGs. Reverse layouts reuse the forward SVG (`_Reverse` suffix stripped). | The repo is MIT, but the file comment says the SVGs were "fetched and constructed from https://api.multiviewer.app/api/v1/circuits/{track_id}/2025", so the underlying data is MultiViewer's (see 3c). | Not usable as-is. |
| **3a. [bacinger/f1-circuits](https://github.com/bacinger/f1-circuits) GeoJSON** | Real-world WGS84 LineStrings, hand-traced (the README credits a Google My Maps source). No sectors, no pit lane, no guaranteed start point or direction. Lengths are real-world (Red Bull Ring `length: 4318`) and differ from the game's `m_trackLength` (windman007's Austria file: 4323). | Covers the F1 calendar (2020–2026 location lists). The same geometry applies to reverse layouts, but the direction and start line are unknown. | MIT © Tomislav Bacinger. The upstream Google My Maps provenance is unclear. | Medium to high: project to metres, then fit a similarity transform (rotation, scale, translation, possibly a mirror for axis handedness) to game samples by Procrustes/ICP. That needs a recorded game lap anyway, and residual error shows wherever the game model deviates. |
| **3b. [TUMFTM/racetrack-database](https://github.com/TUMFTM/racetrack-database)** | Smoothed OSM centrelines plus satellite-derived widths, in local metres. "Quality … varies greatly depending on the location" (README). | About 20 F1 tracks. Missing several F1 25 circuits (Monaco, Singapore, Baku, Jeddah, Miami, Las Vegas, Losail, Imola, Zandvoort as an F1 track, …). | Repo LGPL-3.0, but the centrelines come from OpenStreetMap (README), so ODbL attribution and share-alike apply to derived data. | As 3a, plus licence bookkeeping. |
| **3c. MultiViewer circuits API** (`api.multiviewer.app/api/v1/circuits/{key}/{year}`) | F1 live-timing coordinates (`x`, `y`, `rotation`), plus corners, marshal sectors and pit-loss times. The response names its source lap (`candidateLap`, e.g. 2022 FP1 at Spielberg). | Real-world calendar only, with no reverse layouts. | **No licence**: an undocumented API of a proprietary app, carrying F1 live-timing-derived data. Not safe to bundle. | Same alignment problem as 3a. |

## Recommended approach

1. **Collector learns the outline** for each `(trackId, trackLength)`. From every active (`m_resultStatus` 2), non-pit car with `m_lapDistance ≥ 0`, put world `(X, Z)` into lap-distance buckets of a few metres. Reject teleports (flashbacks, garage-to-grid moves) that land far from neighbouring buckets, and lightly average repeat passes. Mark the outline complete at about 98% bucket coverage with no long gap. Prior art by windman007 (MIT) uses an 8 m bucket, a 6-bucket teleport guard, 0.8/0.2 smoothing and 98% coverage with a gap of at most 4 buckets; we can reference that approach without reusing code.
2. **Collector learns the pit lane** from cars with `m_pitStatus != 0` into a separate trace, plus the lap distances of pit entry and exit.
3. **Persist** learned outlines and pit lanes, and **ship pre-built ones** generated by the same code from our own recordings. Keying by `trackLength` as well as `trackId` catches a game patch that changes a layout.
4. **Dashboard draws** the outline polyline, with sector boundaries and start/finish from the live session packet (`m_sector2LapDistanceStart`, `m_sector3LapDistanceStart`, 0) interpolated on the lap-distance axis. Car dots come from raw world X/Z. The tile only needs a fit-to-box transform and a user rotation, because the game's world frame has an arbitrary heading. windman007 likewise lets the user rotate and remembers the angle per track.
5. **Recording campaign** to pre-build the assets: for each of the 27 layouts, one practice session (out of the garage via the pit lane, one full lap, back into the pits). That yields outline, sectors and pit lane in about 3–5 minutes per layout, roughly 2 hours in total. Time Trial gives the outline but probably not the pit lane.
6. Optionally, until our own recordings exist, seed the 22 tracks from windman007's MIT data with attribution. This is not needed if the live learner is in place, since any race with AI cars completes the outline during lap 1.

## Facts affecting other tickets

- **Recording:** recordings must keep motion (all 22 cars), lap data and session packets. Those are enough to rebuild maps offline, which windman007 also does by replaying recordings. One recording per layout with a pit pass is the asset-build input.
- **Collector model:** the live session state needs per-car `{worldX, worldZ, lapDistance, pitStatus, resultStatus, driverStatus}` and per-session `{trackId, trackLength, sector2Start, sector3Start, marshalZones}`. Track geometry is a separate, persisted store keyed by `(trackId, trackLength)` that outlives a session.
- **Packet rates:** the session packet is 2 Hz; motion and lap data arrive at the menu rate and in the same frame ("on the frame that these get sent they will all get sent together", Spec ~2115). Positions and lap distances can therefore be joined per frame without interpolation.
- **Online:** world positions are not restricted by "Your Telemetry", so the map works for all cars in multiplayer.
- **2026 Season Pack:** it adds track 42 (Madrid) and a 2026 UDP format; the learner covers it without new data.

## Open questions (verify with a first recording)

1. Is Y the vertical axis, with X/Z the ground plane? Two implementations assume so; the spec is silent.
2. Are world coordinates for a `trackId` stable across sessions and game modes (practice, race, time trial)? Bundled outlines depend on it.
3. Does `m_lapDistance` stay monotonic and meaningful inside the pit lane, and what happens to it in a pit lane that crosses the start line, as at Silverstone, Abu Dhabi or Baku? This decides how the pit-lane trace is ordered.
4. Do the reverse layouts report their own sector distances and a start line at a different place from the forward layout? If they do, which is expected, they are simply separate layouts.
5. Does `m_trackLength` exactly equal the lap distance at which `m_lapDistance` wraps? Small mismatches would show as a gap or overlap at the start line.
6. How large is the outline drift between cars on different lines? Averaging gives a racing-line trace rather than a centreline, which is fine for a schematic map.
