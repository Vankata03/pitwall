# pitwall

A second-screen race-engineer dashboard for the F1 game: live car, timing and strategy data from the game, shown as customizable tiles.

## Data

**Collector**:
The process that receives the game's telemetry, keeps the live picture of the session, and serves it to the dashboard.
_Avoid_: Backend, server, ingester

**Host**:
The computer the collector runs on.
_Avoid_: Server, backend

**Status page**:
The dashboard page showing the collector's connection state and setup guidance, and, on the host, its settings.
_Avoid_: Admin page, diagnostics

**Telemetry format**:
The layout of the game's UDP data, chosen in the game's settings (for example `2025` or `2026`).
_Avoid_: Packet version, protocol

**Regulations**:
The ruleset a car runs under (pre-2026 or 2026), which decides what that car has, such as DRS or straight mode.
_Avoid_: Season, car year

**Session**:
One continuous game session (a practice, qualifying, race or time-trial run) as the game identifies it.
_Avoid_: Event, run

**Recording**:
A saved, replayable capture of the raw telemetry from one session.
_Avoid_: Log, dump, save

**Pinned recording**:
A recording kept out of automatic deletion.

**Replay**:
Playing a recording back through the collector as if the game were sending it live.
_Avoid_: Playback, simulation

**Stint**:
The laps a car completes on one set of tyres.

**Pit stop**:
A car's visit to the pit lane, with or without service.

**Reference lap**:
The earlier lap of the same car that the current lap is compared against: its best (fastest valid lap) or its previous (last completed lap).
_Avoid_: Target lap, ghost

**Live delta**:
How far ahead of or behind its reference lap a car is at its current point on the lap.
_Avoid_: Gap, split

**Race-control log**:
The ordered record of what happened in a session, from the game's events and from changes the collector notices. The race-control feed tile shows a filtered view of it.
_Avoid_: Event log

**Track map**:
The outline of one layout, with its pit lane once seen, identified by track ID and track length.
_Avoid_: Circuit map, track outline

**Track store**:
The maps the collector knows: those pitwall ships plus those learned on the host. It outlives sessions.
_Avoid_: Map cache, track database

## Cars

**Player car**:
The car the person at the controls is driving, or is watching while spectating.
_Avoid_: My car, own car

**Focus car**:
The car the dashboard is currently following; the player car by default, switchable to any car in the session.
_Avoid_: Selected car, hero car, target car

**Pinned car**:
A specific driver's car that one tile follows instead of the focus car.

**Livery**:
The visual appearance of a team's car (colours, sponsor marks) as drawn on the dashboard.
_Avoid_: Skin, theme

**Livery pack**:
A set of car images the user supplies and keeps on the host, shown in place of the art that ships with pitwall.
_Avoid_: Skin pack, asset pack

## Dashboard

**Tile**:
One self-contained visual panel on the dashboard showing one kind of data (for example the timing tower or the ERS panel). A layout can hold several tiles of the same type, each with its own settings.
_Avoid_: Widget, card, panel

**Layout**:
A named arrangement of tiles and their settings, saved on the host and chosen per device.
_Avoid_: Dashboard config, view, page

**Preset**:
A read-only layout that ships with pitwall.
_Avoid_: Template, default layout

**Notice tile**:
A tile showing pitwall's short unofficial line, so it can appear in stream view, which has no dashboard chrome.
_Avoid_: Disclaimer tile, watermark

**Edit mode**:
The dashboard state in which tiles can be added, removed, moved, resized and configured.

**Display menu**:
The per-device menu, on every screen outside stream view, where a viewer chooses units for that device.
_Avoid_: Preferences page, user settings

**Stream view**:
A screen showing a layout's tiles on a transparent page with no controls, for use as a broadcast source.
_Avoid_: Stream mode, overlay

**Temperature band**:
The cold, in-window and hot ranges pitwall ships for a tyre compound or for brakes.
_Avoid_: Operating limits, thresholds

**Restricted data**:
A value the game withholds from the telemetry for a car, as opposed to one that has not arrived yet.
_Avoid_: Hidden data, private data
