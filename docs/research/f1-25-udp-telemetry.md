# What does F1 25 UDP telemetry give us?

Research note for the pitwall wayfinder ticket "What does F1 25 UDP telemetry give us?". Researched 2026-10-29.

Vocabulary follows `GLOSSARY.md`. The ticket says "hero car", which is the glossary's **focus car**. Where a field exists only for the person driving, this note says **player car**.

## Sources

The EA forum pages that host the official specification return HTTP 403 to automated fetches, so they could not be read directly on 2026-10-29. Every field-level claim below comes from verbatim community mirrors of EA's own documents. Where the two EA documents disagree, the disagreement is noted.

| ID | Source | What it is |
|----|--------|------------|
| S1 | "Data Output from F1® 25 Game" v3, EA. Original: <https://forums.ea.com/blog/f1-games-game-info-hub-en/ea-sports-f1-25-udp-specification/12187347> (403). Verbatim text mirror: <https://github.com/MatMridul/APX-IQ/blob/main/docs/Data%20Output%20from%20F1%2025%20v3.txt> | The official specification: packet tables, the Restricted-data list, FAQ, change list and appendices. |
| S2 | "C++ structures for F1 25 UDP specification", © 2025 Electronic Arts. Mirror: <https://github.com/mantazzo/F1TV-HUD-Reworked/blob/main/documentation/UDP_Output/F1%2025%20Telemetry%20Output%20Structures.txt> | EA's official header file, with clean struct layouts, sizes and frequencies. Used for every struct and field name. |
| S3 | pits-n-giggles, local clone: `/Users/vankata/Desktop/Projects/pits-n-giggles/lib/f1_types/*.py` and `apps/backend/rust/crates/*` | An F1 25-aware parser that has shipped. Used only to corroborate. |
| S4 | MacManley/f1-26-udp README: <https://github.com/MacManley/f1-26-udp/blob/main/README.md> | A verbatim mirror of EA's "Data Output from F1® 26 Game", the `2026` UDP format used by the F1 25 2026 Season Pack. Official thread: <https://forums.ea.com/discussions/f1-25-general-discussion-en/discussion-f1%C2%AE25-2026-season-pack-udp-specification/13424444> (403). |
| S5 | EA / formula1.com: <https://www.formula1.com/en/latest/article/ea-sports-launch-f1-25-2026-season-pack-worldwide.14648iVI1giIQfh5y8v4uv>, <https://www.ea.com/games/f1/news/f1-25-2026-season-pack-launch> | Confirms the 2026 Season Pack is a paid expansion for F1 25, released 2026-06-03, and that EA is releasing no F1 26 game. |
| S6 | F1Laps settings FAQ: <https://www.f1laps.com/faqs/f1-26-udp-telemetry-settings/> | Third-party guidance on recommended settings and the consequences of choosing the 2025 format. |
| S7 | EA forum snippets seen in search results only; the pages were not fetched (403) | Unverified reports, flagged where they are used. |

## Verified facts

### Transport and settings (S1 FAQ)

- The game sends UDP. All values are little-endian and packed with no padding. The header is 29 bytes and starts every packet (S2).
- Players enable it in Options → Settings → UDP Telemetry Settings, at the bottom of the list. The settings there are on/off, IP address, port, **broadcast mode**, **send rate** and **UDP Format**. In broadcast mode the game sends across the subnet and no target IP is needed, only a port.
- The PC XML defaults are `<udp enabled="false" broadcast="false" ip="127.0.0.1" port="20777" sendRate="20" format="2025" yourTelemetry="restricted" onlineNames="off" />`. UDP is off by default, the send rate defaults to 20 Hz, and "Your Telemetry" defaults to Restricted.
- F1 25 can send the `2025`, `2024` or `2023` formats. pitwall needs `2025`. Packets in the `2024` format have no livery colours, no Lap Positions packet, no blister data and a different participant layout.
- The send rate goes up to 60 Hz (S2: "Rate as specified in menus (up to 60Hz)"). The specification does not list the menu's choices of rate.
- Packets sent at the menu rate (Motion, Lap Data, Car Telemetry, Car Status, Motion Ex) are always sent together on the same frame. Packets with fixed rates can arrive on any frame. The first frame carries Session, Participants, Car Setups, Lap Data, Motion, Car Telemetry, Car Status, Car Damage and Motion Ex, all with `sessionTime` 0.000.
- There are up to 22 cars. Use `m_numActiveCars` from Participants, and treat a car as valid only if its `m_resultStatus` in Lap Data is neither 0 (invalid) nor 1 (inactive). A car's vehicle index never changes during a session.
- Every wheel array is ordered RL, RR, FL, FR.

### Packet table (S2; sizes and rates match S1)

| ID | Packet | Rate | Size (B) | Scope | Main content |
|----|--------|------|----------|-------|--------------|
| — | Header | every packet | 29 | — | `m_packetFormat` (2025), `m_gameYear`, game version, `m_packetVersion`, `m_packetId`, `m_sessionUID` (u64), `m_sessionTime`, `m_frameIdentifier`, `m_overallFrameIdentifier` (does not go back after flashbacks), `m_playerCarIndex`, `m_secondaryPlayerCarIndex` |
| 0 | Motion | menu rate (≤60 Hz) | 1349 | all 22 cars | World position, velocity and direction, lateral/longitudinal/vertical G, yaw/pitch/roll. Sent only while the player is in control (S1). |
| 1 | Session | 2 Hz | 753 | session | Weather, track and air temperature, laps, track length/ID, session type, time left, pit speed limit, marshal zones with flags, safety car status, up to 64 forecast samples, forecast accuracy, pit window ideal/latest lap and rejoin position (**player only**), assists, rule settings, weekend structure, sector 2 and 3 start distances |
| 2 | Lap Data | menu rate | 1285 | all cars | Last and current lap time, sector 1 and 2 of the current lap, deltas to the car in front and to the leader, lap distance, total distance, position, lap number, pit status, pit stops, sector, lap invalid, penalties in seconds, warnings, corner-cutting warnings, unserved drive-through and stop-go penalties, grid position, driver status, result status, pit-lane timers, speed trap |
| 3 | Event | on event | 45 | — | 4-character code plus a union of details (codes below) |
| 4 | Participants | every 5 s | 1284 | all cars | AI flag, driver ID, network ID, **team ID**, **My Team flag**, race number, nationality, name (32-character UTF-8), **`m_yourTelemetry`**, `m_showOnlineNames`, tech level, platform, **`m_numColours` and `m_liveryColours[4]` (RGB)** |
| 5 | Car Setups | 2 Hz | 1133 | all cars | Full setup, plus `m_nextFrontWingValue` for the player. In multiplayer, other players' setups are blank whatever their setting. |
| 6 | Car Telemetry | menu rate | 1352 | all cars | Speed (km/h), throttle, steer, brake, clutch, gear, RPM, DRS on/off, rev lights, **brake temps[4]**, **tyre surface temps[4]**, **tyre inner temps[4]**, **engine temp**, **tyre pressures[4]**, surface type[4]. Also MFD panel index and suggested gear. |
| 7 | Car Status | menu rate | 1239 | all cars | TC, ABS, fuel mix, brake bias, pit limiter, fuel in tank and capacity, fuel remaining in laps, max/idle RPM, `m_drsAllowed`, `m_drsActivationDistance`, actual and visual tyre compound, tyre age in laps, FIA flag, ICE and MGU-K power, ERS store (J), ERS deploy mode, MGU-K and MGU-H harvest this lap, ERS deployed this lap, network paused |
| 8 | Final Classification | once at race end | 1042 | all cars | Position, laps, grid, points, stops, result status and reason, best lap, race time, penalties, up to 8 tyre stints (actual, visual, end lap) |
| 9 | Lobby Info | 2 Hz in lobby | 954 | lobby | Team, name, car number, `m_yourTelemetry`, ready status |
| 10 | Car Damage | 10 Hz | 1041 | all cars | Tyre wear (float %), tyre damage, brake damage, **tyre blisters**, front-left and front-right wing, rear wing, floor, diffuser, sidepod, DRS and ERS faults, gearbox, engine, wear on each PU component (MGU-H, ES, CE, ICE, MGU-K, TC), engine blown or seized |
| 11 | Session History | 20 Hz, one car per packet in rotation | 1460 | one car | Up to 100 laps of lap time, sectors 1–3 and validity bit flags; the laps of the best lap and best sectors; up to 8 tyre stints (end lap, actual and visual compound). About 1 Hz per car with 20 cars. |
| 12 | Tyre Sets | 20 Hz, one car per packet in rotation | 231 | one car | 20 sets (13 dry, 7 wet): compound, wear, available, recommended session, laps of life left, usable life, lap delta to the fitted set, fitted flag, `m_fittedIdx` |
| 13 | Motion Ex | menu rate | 273 | **player car only** | Suspension, wheel speed, slip, forces, local velocity, angular rates, aero heights, chassis yaw and pitch, wheel camber and camber gain |
| 14 | Time Trial | 1 Hz, Time Trial only | 101 | 3 data sets | Player session best, personal best and rival: lap and sector times, assists, valid |
| 15 | Lap Positions | 1 Hz | 1131 | all cars | Position of every car at the start of each lap. At most 50 laps per packet; longer races are split across two packets, distinguished by `m_lapStart`. |

Event codes (S1, S2): `SSTA` session started, `SEND` session ended, `FTLP` fastest lap (car, time), `RTMT` retirement (car, reason), `DRSE` DRS enabled, `DRSD` DRS disabled (reason: wet, safety car, red flag, minimum lap not reached), `TMPT` teammate in pits, `CHQF` chequered flag, `RCWN` race winner, `PENA` penalty (penalty type, infringement type, car, other car, time, lap, places gained), `SPTP` speed trap, `STLG` start lights (count), `LGOT` lights out, `DTSV` drive-through served, `SGSV` stop-go served (with stop time), `FLBK` flashback (frame ID and session time flashed back to), `BUTN` button status, `RDFL` red flag, `OVTK` overtake (overtaker, overtaken), `SCAR` safety car (type: full, virtual or formation; phase: deployed, returning, returned or resume), `COLL` collision (two cars).

### Tile → fields mapping

The column "Other humans (Restricted)" shows whether the tile still works when it follows another human whose "Your Telemetry" is Restricted. AI cars always show their data.

| Tile | Packet.field(s) | Rate | All cars? | Other humans (Restricted) |
|------|-----------------|------|-----------|---------------------------|
| Speed / RPM / gear | Telemetry `m_speed`, `m_engineRPM`, `m_gear`, `m_revLightsPercent`/`BitValue`; Status `m_maxRPM`, `m_idleRPM`, `m_maxGears`; Telemetry `m_suggestedGear` (player) | menu | yes | visible |
| Engine temp | Telemetry `m_engineTemperature` (°C, u16) | menu | yes | visible |
| G-force | Motion `m_gForceLateral`/`Longitudinal`/`Vertical` | menu | yes | visible. Motion stops when the player is not in control. |
| Track discipline | Lap Data `m_cornerCuttingWarnings`, `m_totalWarnings`, `m_currentLapInvalid`, `m_penalties`; `PENA` events with infringement types 7–9, 25–32 (corner cutting, running wide, wall riding, flashback); Session History `m_lapValidBitFlags`; Telemetry `m_surfaceType[4]` (grass, gravel and others); Session `m_cornerCuttingStringency` | menu / event | yes | visible |
| DRS | Telemetry `m_drs`; Status `m_drsAllowed`, `m_drsActivationDistance`; `DRSE`/`DRSD` events with reason; Damage `m_drsFault` | menu / event / 10 Hz | yes | DRS state visible; **`m_drsFault` zeroed** |
| ERS | Status `m_ersStoreEnergy` (J), `m_ersDeployMode` (none, medium, hotlap, overtake), `m_ersHarvestedThisLapMGUK`, `m_ersHarvestedThisLapMGUH`, `m_ersDeployedThisLap`, `m_enginePowerICE`, `m_enginePowerMGUK`; Damage `m_ersFault` | menu | yes | **all zeroed** except `m_ersFault` |
| Throttle/brake vs lap distance | Telemetry `m_throttle`, `m_brake`, `m_steer`, `m_speed` + Lap Data `m_lapDistance`, joined on `m_frameIdentifier` because both are sent on the same frame | menu | yes | visible |
| Lap state | Lap Data `m_currentLapNum`, `m_sector`, `m_driverStatus`, `m_pitStatus`, `m_currentLapInvalid`, `m_resultStatus`, `m_lapDistance`; Session `m_totalLaps`, `m_sessionTimeLeft`, `m_safetyCarStatus` | menu / 2 Hz | yes | visible |
| Weather + forecast | Session `m_weather`, `m_trackTemperature`, `m_airTemperature`, `m_weatherForecastSamples[≤64]` (session type, minutes ahead, weather, temperatures and trend, rain %), `m_forecastAccuracy` | 2 Hz | — | — |
| Track map | Motion `m_worldPositionX/Y/Z` for every car; Lap Data `m_lapDistance`; Session `m_trackLength`, `m_trackId`, marshal zones (`m_zoneStart` as a lap fraction and its flag), sector start distances | menu / 2 Hz | yes | visible |
| Race-control feed | Event packet (all codes above); Session `m_safetyCarStatus`, `m_marshalZones[].m_zoneFlag`, safety car, VSC and red flag counts; Status `m_vehicleFIAFlags` | event / 2 Hz | — | — |
| Focus-car tyres | Telemetry `m_tyresSurfaceTemperature[4]`, `m_tyresInnerTemperature[4]`, `m_tyresPressure[4]`; Damage `m_tyresWear[4]`, `m_tyresDamage[4]`, `m_tyreBlisters[4]`; Status `m_actualTyreCompound`, `m_visualTyreCompound`, `m_tyresAgeLaps` | menu / 10 Hz | yes | temperatures, pressures, compound and age visible; **wear and damage zeroed**; blisters not on the list |
| Focus-car brakes | Telemetry `m_brakesTemperature[4]`; Damage `m_brakesDamage[4]` | menu / 10 Hz | yes | temperatures visible; **damage zeroed** |
| Focus-car damage | Damage wing FL/FR, rear wing, floor, diffuser, sidepod, gearbox, engine, PU wear, engine blown or seized | 10 Hz | yes | **zeroed** (blown and seized are not on the list) |
| Lap timing, sectors, delta | Lap Data `m_lastLapTimeInMS`, `m_currentLapTimeInMS`, sector 1 and 2 of the current lap (ms + minutes), `m_deltaToCarInFront*`, `m_deltaToRaceLeader*`; Session History per-lap sectors 1–3, validity, laps of the best lap and sectors; `FTLP` event; Time Trial packet (TT only) | menu / about 1 Hz per car | yes | visible |
| Timing tower | Lap Data `m_carPosition`, `m_deltaToCarInFront*`, `m_deltaToRaceLeader*`, `m_pitStatus`, `m_numPitStops`, `m_penalties`, unserved DT/SG, `m_totalWarnings`, `m_resultStatus`; Status `m_visualTyreCompound`, `m_tyresAgeLaps`; Participants name, team, race number; Lap Positions for history | menu / 5 s / 1 Hz | yes | visible |
| Stint / strategy | Session `m_pitStopWindowIdealLap`, `m_pitStopWindowLatestLap`, `m_pitStopRejoinPosition` (**player only**); Tyre Sets (20 sets: wear, life left, usable life, available, lap delta, fitted); Session History tyre stints; Final Classification stints; Session `m_pitSpeedLimit` | 2 Hz / about 1 Hz per car | pit window: player only; others: yes | **Tyre Sets entirely zeroed** for Restricted humans |
| Fuel | Status `m_fuelInTank`, `m_fuelCapacity`, `m_fuelRemainingLaps`, `m_fuelMix`; Setups `m_fuelLoad` | menu / 2 Hz | yes | **zeroed** |

### Multiplayer: the "Your Telemetry" Restricted list (S1, "Restricted data")

When a human driver's "Your Telemetry" is Restricted, which is the default, the game sends these fields as zero for that driver's car. The player always sees their own car in full.

- **Car Status:** `m_fuelInTank`, `m_fuelCapacity`, `m_fuelMix`, `m_fuelRemainingLaps`, `m_frontBrakeBias`, `m_ersDeployMode`, `m_ersStoreEnergy`, `m_ersDeployedThisLap`, `m_ersHarvestedThisLapMGUK`, `m_ersHarvestedThisLapMGUH`, `m_enginePowerICE`, `m_enginePowerMGUK`.
- **Car Damage:** front-left and front-right wing, rear wing, floor, diffuser, sidepod, engine, gearbox; `m_tyresWear`, `m_tyresDamage` and `m_brakesDamage` on all four wheels; `m_drsFault`; all six PU wear fields.
- **Tyre Sets:** every field for that car.
- **Car Setups:** other players' setups are always blank in multiplayer, whatever their setting. Spectators see no setups at all.
- The flag is published for each car as `ParticipantData.m_yourTelemetry` (0 = restricted, 1 = public) and in Lobby Info. Names appear only if that player turned on "Show online ID / gamertags". Network humans have `m_driverId = 255`.

**Never restricted:** Motion, Lap Data, Car Telemetry (including tyre temperatures and pressures, brake temperatures and engine temperature), tyre compound and age, DRS state, Session History and Lap Positions.

### Teams and livery colours (S1, S2)

- `ParticipantData.m_teamId` is a u8. The F1 25 appendix lists 0–9 for the 2025 grid (Mercedes, Ferrari, Red Bull Racing, Williams, Aston Martin, Alpine, RB, Haas, McLaren, Sauber), 41 F1 Generic, **104 F1 Custom Team**, 129 Konnersport, 142/154 APXGP '24/'25, 155–168 the 2024 F2 teams and others, and 185–194 the 2024 F1 teams.
- `m_myTeam` is set to 1 for the My Team car. Game mode 27 is "My Team Career '25".
- **Livery colours are new in F1 25.** Each participant carries `m_numColours` and `m_liveryColours[4]` as `{u8 red, green, blue}`, described only as "Colours for the car". The change list says: "Added car colours to the participants packet (and reduced name size to 32 chars)".

### F1 25 compared with F1 24 (S1, "What has changed since last year?")

1. Stop-go penalty time added to the event packet (`SGSV.stopTime`).
2. Tyre blister percentage added to Car Damage.
3. Chassis pitch added to Motion Ex.
4. Car colours added to Participants; the name field cut from 48 to 32 characters.
5. Name field cut in Lobby Info as well.
6. Wheel camber and camber gain added to Motion Ex.
7. A more detailed reason for DRS being disabled (`DRSD.reason`).
8. A retirement reason added to `RTMT`.
9. A new Lap Positions packet (ID 15).
10. A result reason added to Final Classification.
11. The C6 compound documented (actual compound 22).

### Session restarts and flashbacks (S1, S2)

- **Flashback:** the game sends a `FLBK` event carrying `flashbackFrameIdentifier` and `flashbackSessionTime`, the frame and time it rewound to. Header `m_frameIdentifier` rewinds; `m_overallFrameIdentifier` "doesn't go back after flashbacks". A flashback can invalidate the lap: `PENA` infringement 31, "Lap invalidated flashback used". Session `m_recoveryMode` and `m_flashbackLimit` state whether flashbacks are allowed.
- **Session boundaries:** `SSTA` ("sent when the session starts") and `SEND`; `m_sessionUID` is a "unique identifier for the session". Session also carries `m_seasonLinkIdentifier`, `m_weekendLinkIdentifier` and `m_sessionLinkIdentifier`, which "persist across saves".
- **The specification does not say how a session restart is signalled.**

### The 2026 Season Pack (S4, S5, S6)

- EA released no F1 26 game. The 2026 season shipped as a paid expansion for F1 25 on 2026-06-03, also sold bundled as "F1 25: 2026 Season Edition" (S5). That expansion adds a **`2026` UDP format**. Its specification is titled "Data Output from F1® 26 Game", and it can also send `2025` and `2024` (S4).
- Changes in the `2026` format (S4): up to **24 cars**, so most packet sizes change. Engine temperature becomes u8. G-forces are quantised. Driver, network and team IDs become **u16**. A new **Car Telemetry 2** packet (ID 16) carries active aero mode and availability, overtake-mode availability, active and activation distance, a `m_2026Regulations` flag and a wrong-way flag. Session gains active-aero and DRS zones, start reaction time and new assist options. Car Status gains `m_ersHarvestLimitPerLap`, and ERS deploy mode 3 becomes "boost". Collision severity is added to the event. The Restricted list is unchanged.
- F1Laps states that the 2025 format "can file sessions under F1 25, label 2026 cars as 'F1 Generic,' and omit Boost/ERS data" (S6). An EA forum reply instead reports that 2026 teams use IDs 220–230 in the 2025 format (S7, unverified). These two claims conflict.

## Interpretation (not stated in the sources)

- **Rates for pitwall.** The 20 Hz default is enough for the timing and status tiles. Throttle and brake traces against lap distance look noticeably smoother at 60 Hz. With 22 cars at 60 Hz the five menu-rate packets total about 5.5 kB per frame (1349 + 1285 + 1352 + 1239 + 273 bytes), roughly 330 kB/s, which a LAN handles easily. Session History and Tyre Sets refresh each car only about once a second, so tiles fed by them lag by up to about 1 s.
- **Joining packets.** The menu-rate packets share a frame, so the collector can build one snapshot per `m_frameIdentifier` (or `m_overallFrameIdentifier`).
- **Track map.** There is no track outline or geometry. The collector has to build one from `m_worldPositionX/Z` as cars drive, or ship outlines keyed by `m_trackId`. Treating X and Z as the ground plane comes from S1's coordinate diagram and common practice; it should be confirmed against a capture. Motion is sent "only while player is in control", so the map may freeze while spectating, in menus or after the finish.
- **Restarts.** pits-n-giggles treats a change of `m_sessionUID` as a new session. It resets its state on the change and orders packets by `m_overallFrameIdentifier`, treating frame 0 as a fresh start (S3: `apps/backend/rust/crates/telemetry-core/src/frame_gate.rs`; test `runtime_ignores_first_session_packet_after_session_uid_change` in `crates/backend/tests/backend_runtime.rs`). The likeliest signal for a restart is therefore a new `m_sessionUID`, probably accompanied by `SEND`/`SSTA`. A capture should confirm this. Community reports from F1 22 also say `m_sessionTime` rewinds on flashback, which matches `m_frameIdentifier` rewinding.
- **Livery.** The colours are per participant, so the My Team car probably gets its own colours. The specification does not say so, and it does not say how many of the 4 slots each team fills.
- **Live delta to personal best.** The game sends no delta to the player's own best lap. pitwall has to compute it from `m_lapDistance` and `m_currentLapTimeInMS` against a stored reference lap, as pits-n-giggles does in `lib/delta/`. The game does send deltas to the car in front and to the leader.
- **"Track limits".** The game sends corner-cutting and total warning counts plus `PENA` infringement detail, but no separate track-limits counter.
- **ERS percentage.** Displaying ERS as a percentage needs the store's capacity, which the specification does not give (4 MJ is the common assumption). The format of the harvest and deploy fields is J per lap, implied by the store being in J.

## Gaps and caveats

1. The EA forum is behind a bot wall, so the official page could not be read. The mirrored document is "v3", while the EA post displayed "Version 6.0" in search results. That may be the forum's post-revision counter rather than the document version. A human should download the current attachment and compare it with S1.
2. EA's documents disagree on the Car Damage rate. S1 and S2 both say "10 per second", while the S1 FAQ example says Car Damage appears on frame 31 "since 2 updates per second". Plan for 10 Hz and measure.
3. Pit window and rejoin position cover the player car only. Session `m_gamePaused` applies to network games only; the specification does not say what is sent while a single-player game is paused.
4. A reported EA bug says `m_gameYear` reads 0 before the race starts (S7, unverified). Validate on `m_packetFormat == 2025`, not on `m_gameYear`.
5. The specification does not list the menu's send-rate options.
6. Tyre Sets restriction is worded as "All data within this packet for player car". Read it as: zeroed for any car whose human driver is Restricted.
7. The `2026` format is not compatible with `2025` (24 cars, u16 IDs). Season Pack players who leave the format at 2025 may lose 2026 teams and boost data (S6).

## Open questions (resolve with a capture)

1. When the player restarts a session from the pause menu, does `m_sessionUID` change, and do `SEND`/`SSTA` fire? Does the same apply when moving between sessions in a weekend? Do the link identifiers stay the same?
2. What `m_teamId` does the My Team car get: 104 (F1 Custom Team), something else, or 255? Only `m_myTeam = 1` is documented. pits-n-giggles maps 255 to `MY_TEAM` (`lib/f1_types/common.py`, `TeamID25`), but 255 does not appear in the F1 25 appendix and comes from F1 2020's numbering.
3. Are `m_liveryColours` filled in for My Team and for every team, how many slots does each use, and do special or alternative liveries change them?
4. Which send rates does the menu offer, and is Car Damage really 10 Hz?
5. Should pitwall's "F1 25 only" scope include the 2026 Season Pack's `2026` format? That means 24 cars, u16 IDs, Car Telemetry 2 and boost instead of overtake.
