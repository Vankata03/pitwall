# pitwall

A second-screen race-engineer dashboard for the F1 game: live car, timing and strategy data from the game, shown as customizable tiles.

## Data

**Collector**:
The process that receives the game's telemetry, keeps the live picture of the session, and serves it to the dashboard.
_Avoid_: Backend, server, ingester

**Session**:
One continuous game session (a practice, qualifying, race or time-trial run) as the game identifies it.
_Avoid_: Event, run

**Recording**:
A saved, replayable capture of the raw telemetry from one session.
_Avoid_: Log, dump, save

**Replay**:
Playing a recording back through the collector as if the game were sending it live.
_Avoid_: Playback, simulation

**Stint**:
The laps a car completes on one set of tyres, between pit stops.

## Cars

**Player car**:
The car the person at the controls is driving.
_Avoid_: My car, own car

**Focus car**:
The car the dashboard is currently following; the player car by default, switchable to any car in the session.
_Avoid_: Selected car, hero car, target car

**Livery**:
The visual appearance of a team's car (colours, sponsor marks) as drawn on the dashboard.
_Avoid_: Skin, theme

## Dashboard

**Tile**:
One self-contained visual panel on the dashboard showing one kind of data (for example the timing tower or the ERS panel).
_Avoid_: Widget, card, panel

**Layout**:
A named arrangement of tiles and their settings, saved per device.
_Avoid_: Dashboard config, view, page

**Preset**:
A layout that ships with pitwall.
_Avoid_: Template, default layout
