# 0003. Existing recordings, layouts and config keep loading

Date: 2026-11-05
Status: Accepted

## Context

Users of the web build have data in `%APPDATA%\pitwall`: `.pitrec` recordings, `layouts.json` (versioned by `schemaVersion`), `config.toml`, cached track maps, an optional livery pack and an overlay position file. The native app (ADR 0001, ADR 0002) removes the HTTP server and the browser, so some of that data no longer has a meaning, and some of it was never on disk.

## Decision

The native app reads the existing data folder. It migrates when needed and never breaks silently.

- **Recordings:** `.pitrec` keeps its format. Every existing recording opens and replays unchanged.
- **Layouts:** `layouts.json` loads as it is. The first write after a change that needs a new shape bumps `schemaVersion` and first copies the old file to `layouts.json.bak`. The wide, tablet and phone arrangements become window-size classes: a window picks the arrangement that fits its size.
- **Config:** `config.toml` loads as it is. The obsolete `[http]` keys (`port`, `bind`) and the `--http-port` and `--bind` flags are accepted, logged once as ignored, and dropped from the file on the next save. A flag that has no meaning never stops the app from starting.
- **Track maps, livery pack, overlay position:** keep their paths and formats.
- **Not migrated:** display-menu choices (units, focus car) that each browser kept in its own `localStorage`. A native app cannot read them. Each native window starts with the defaults and remembers its own choices from then on.

## Consequences

- Loading code keeps reading old shapes, with tests built from real files of each shape.
- A file the app cannot parse is kept, renamed with a `.bad` suffix, reported in the log and in the status UI, and replaced by defaults. It is never overwritten in place.
- After the first native save, the web build may no longer read `layouts.json`; `layouts.json.bak` holds the copy it can read.
