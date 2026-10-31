# F1 25 UDP format `2026`: packet layout

This note records how UDP Format `2026` differs from format `2025`, checked against a real capture. It extends `docs/research/f1-25-udp-telemetry.md` on the `research/f1-25-udp-telemetry` branch, which covers format `2025`.

## Provenance

EA's 2026 specification was not read. The layouts come from the struct definitions in [pits-n-giggles](https://github.com/ashwin-nat/pits-n-giggles) (MIT), `lib/f1_types/`, at commit `3490db9` (2026-09-21). Only the layouts were taken, which is what the map allows; no code is reused. The older local clone at `/Users/vankata/Desktop/Projects/pits-n-giggles` predates this support and has none of it.

The check used the capture `2026-spa-practice` (Spa, 2026 grid, UDP Format `2026`, game v1.26, one AI field, 22 active cars).

## Packet sizes

All values are little-endian and packed. The header stays 29 bytes. Every per-car array has 24 entries.

| ID | Packet | Bytes | Layout | Per car | Trailer | Capture |
| --- | --- | --- | --- | --- | --- | --- |
| 0 | Motion | 1325 | changed | 54 | none | size and values match |
| 1 | Session | 926 | extended | n/a | n/a | size matches |
| 2 | Lap Data | 1399 | same fields | 57 | 2 bytes | size and values match |
| 3 | Event | 45 | some events changed | n/a | n/a | size and codes match |
| 4 | Participants | 1470 | changed | 60 | 1-byte count first | size and values match |
| 5 | Car Setups | 1233 | same fields | 50 | 4 bytes | size matches |
| 6 | Car Telemetry | 1448 | changed | 59 | 3 bytes | size and values match |
| 7 | Car Status | 1445 | changed | 59 | none | size matches |
| 8 | Final Classification | 1134 | same fields | 46 | 1-byte count first | size matches |
| 10 | Car Damage | 1133 | same fields | 46 | none | size matches |
| 11 | Session History | 1460 | same | n/a | n/a | size matches |
| 12 | Tyre Sets | 231 | same | n/a | n/a | size matches |
| 13 | Motion Ex | 273 | same | n/a | n/a | size matches |
| 15 | Lap Positions | 1231 | changed | n/a | n/a | size matches |
| 16 | Car Telemetry 2 | 269 | new | 10 | none | size and values match |

Lobby Info (9) and Time Trial (14) did not appear in the capture, so their layouts are unchecked. The reference gives 43 bytes per Lobby Info entry and 25 per Time Trial data set, both with a u16 `m_teamId`.

## Changed layouts

**Motion, car entry (54 bytes).** The forward and right direction vectors are i16 as before. Lateral, longitudinal and vertical G-force become i16 scaled by 1000 instead of floats, which is where the 6 bytes per car go.

| Offset | Field | Type |
| --- | --- | --- |
| 0 | world position X, Y, Z | 3 × f32 |
| 12 | world velocity X, Y, Z | 3 × f32 |
| 24 | forward direction X, Y, Z | 3 × i16 |
| 30 | right direction X, Y, Z | 3 × i16 |
| 36 | G-force lateral, longitudinal, vertical (÷1000) | 3 × i16 |
| 42 | yaw, pitch, roll | 3 × f32 |

**Participants, car entry (60 bytes).** `m_driverId`, `m_networkId` and `m_teamId` widen to u16, and an empty slot reports 65535 for driver and team. The offsets are: `m_aiControlled` 0, `m_driverId` 1, `m_networkId` 3, `m_teamId` 5, `m_myTeam` 7, `m_raceNumber` 8, `m_nationality` 9, `m_name` 10 (32 bytes), `m_yourTelemetry` 42, `m_showOnlineNames` 43, `m_techLevel` 44 (u16), `m_platform` 46, `m_numColours` 47, then four RGB livery colours.

**Car Telemetry, car entry (59 bytes).** `m_engineTemperature` narrows from u16 to u8. Everything else keeps its order.

**Car Status, car entry (59 bytes).** A float `m_ersHarvestedLimitPerLap` joins the ERS fields, between `m_ersHarvestedThisLapMGUH` and `m_ersDeployedThisLap`.

**Car Telemetry 2, car entry (10 bytes).** The fields are `m_activeAeroMode` (u8), `m_activeAeroAvailable` (u8), `m_activeAeroActivationDistance` (u16), `m_overtakeAvailable` (u8), `m_overtakeActive` (u8), `m_overtakeActivationDistance` (u16), `m_2026Regulations` (u8) and `m_drivingWrongWay` (u8).

**Session, tail.** After the 2025 fields come `m_activeAeroTrackStatus` (0 full, 1 partial), then a count and 8 full-mode active aero zones, a count and 8 partial-mode zones, a count and 4 DRS zones (each zone is a start and end f32), then `m_startReactionTime` (f32) and five u8 fields this note does not name.

**Events.** `COLL` gains a severity byte. The reference adds `PMEN` and `PMDI` (partial aero mode) and `OVEN` and `OVDI` (overtake mode). `RTMT` carries 2 bytes, and `SGSV` 5, as in format `2025`.

**Lap Positions.** The reference sizes each lap at 24 cars, which matches 1231 bytes (29 + 2 + 50 × 24).

## Values checked in the capture

- Team IDs 476 to 486 appear as expected, including Audi (485) and Cadillac (486).
- 22 of 24 slots are active. Slots 22 and 23 hold team 65535 and an empty name.
- `m_2026Regulations` is 1 for all 22 active cars and 0 for the two empty slots, in all 17,582 packets.
- `m_lapDistance` ran from 0.3 to 7007.4 m, about the length of Spa.
- World Y ranged from -34.2 to 68.2 m, or 102 m, while X and Z spanned about 1,270 and 2,030 m. World Y is the vertical axis.
- Top speed was 335 km/h. Lap times were 114.618 s and 130.185 s.
- G-force samples look right (for example -1.90 lateral), but the maximum was 10.35, which may be a kerb strike or collision and has not been examined.

## Further facts from the capture

- **Pit lane and start line:** `m_lapDistance` keeps counting in the pit lane and wraps at the line there, as on track: 7007.4 to 0.3, with `m_currentLapNum` advancing. `m_pitStatus` went to 1 at 6922.5 m and to 2 at 49.1 m, where the car held for 2.5 seconds, and back to 0 at 300.6 m. It was never negative on this run.
- **Compression:** the 154.2 MB payload gzips to 41.5 MB in arrival order and to 37.8 MB grouped by packet type, about 9% better. gzip stands in for zstd, which could not be run on the PC, so these figures are only a floor for what grouping gives.

## From the reference, not the capture

- **Boost:** there is no boost field. pits-n-giggles shows boost when a 2026 car's ERS deploy mode reads overtake while `m_overtakeActive` is 0.
- **My Team:** the reference gives team 65535 for My Team in format `2026` and 255 in earlier formats. The capture shows 65535 on empty slots too, so a team ID cannot identify My Team. Use `m_myTeam`.
- **Spectating:** `m_isSpectating` and `m_spectatorCarIndex` are in the Session packet, with 255 meaning no car.
- **Later 2026 IDs:** the reference continues to 499 for 2026 feeder series teams.

## Open

- How 2026 cars report on format `2025`, and what 2025-grid cars report on format `2026`, are unobserved. The two sources for the first still conflict.
- Car Status, Car Damage, Session and Final Classification match by size only; their values have not been read.
- World-coordinate stability, flashback behaviour, the My Team team ID, delta fields for lapped cars, real zstd levels and model load time all need further captures or a working parser.
