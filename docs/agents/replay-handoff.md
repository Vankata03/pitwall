# Issue #45 integration handoff

- `replay::Replay::load(File, progress)` builds a separate model, ingest gate, 30-second checkpoints and a per-car lap index. `seek(Duration)`, `jump_lap(car, lap)` and `tick(elapsed)` use recording arrival time. Lap jump selects the last attempt. Raw records remain on disk; checkpoints retain model, ingest and reader continuations.
- `replay::Controller::new(recordings_dir, Arc<Mutex<Recorder>>)` owns the selected replay. `publish(transport_time, live_model, stream, context)` advances transport, publishes replay State and forces wire resets after seeks and source changes. The runtime calls it on its existing 60 Hz publication schedule.
- Loading runs on a blocking worker. State reports `loadProgress` from 0 to 1. A failed load stays in replay with an error banner until another start or stop. Stop invalidates an in-flight load; its worker releases the retention guard when it returns.
- A `Recorder::mark_loaded` guard covers loading and the loaded replay. Live UDP, its model and recording continue independently. Replay uses an empty datagram tap and has no UDP sender.

## HTTP interface

All bodies are JSON. Every mutation retains the shell's same-origin, known-Host checks.

| POST route | Access | Body | Success |
| --- | --- | --- | --- |
| `/api/replay/start` | Host | `{"name":"recording.pitrec"}` | 202; load continues asynchronously |
| `/api/replay/stop` | Host | `{}` | 204 |
| `/api/replay/pause` | Any screen | `{"paused":true}` | 204 |
| `/api/replay/speed` | Any screen | `{"speed":2}` | 204 |
| `/api/replay/seek` | Any screen | `{"position":30.5}` | 204 |
| `/api/replay/lap` | Any screen | `{"car":0,"lap":2}` | 204 |

- Position is seconds from recording start; car is a zero-based telemetry index.
- Speeds: 0.5, 1, 2, 4. Transport returns 409 before a replay is loaded, 400 for unsupported values or an out-of-range seek, and 404 for a missing lap/file. Axum rejects malformed JSON bodies.
- Seeking preserves pause state. At EOF replay pauses; restart with seek to zero followed by `pause` with `false`, or exit with stop.
- State uses the existing generated `ReplayState`: position, speed, paused, banner, load progress and background live notice. No wire-contract change is required.

## Verification boundary

- Synthetic `assumed` tests cover seek-equals-straight-play across silence, checkpoints and flashback, last-attempt lap jump, transport, route access, wire epochs and loaded-recording retention.
- No status-page UI is included (#48).
- Real captures, Windows, live-game forwarding/recording integration and performance remain unqualified. The section 16 soft targets (seek ≤1 second, two-hour load ≤10 seconds, collector RSS ≤256 MB including checkpoints) require measurement.
