# Issue #35 integration handoff

## Dashboard (#36)

- Import `TextMessage` and `Trace` from `dashboard/src/generated/wire/`. The `Frame` class in `frame.ts` reads a 976-byte `ArrayBuffer` through `DataView` without per-frame car objects.
- Connect to `/ws`. Expect `hello`, `snapshot`, then binary Frames and JSON updates. Discard older epochs. A snapshot replaces all stored state and history. Replace History entries by `id`; preserve race-control arrival order.
- JSON `Readout<T>` uses `{state: "live" | "stale", value: T}` or `{state: "restricted" | "absent"}`. Missing trace channels are `null`.
- State includes `context.revisions.layouts`, `context.revisions.tracks`, collector status and optional replay status. Refetch libraries when revisions change.
- Fetch `/api/cars/{index}/laps/{lap}/trace?epoch={epoch}`. A stale epoch returns 409; an unavailable car or retained lap returns 404. Columns are distance (metres), time (milliseconds), speed (km/h), throttle and brake (fractions).

## Replay (#45)

- `wire::Stream::tick(transport_time, Source, &SessionModel, Context)` publishes the selected model. Transport time is monotonic and independent of replay position or speed.
- Supply the separate replay instance with `Source::Replay`, while live ingest and recording continue. Switching sources bumps the wire epoch. Call `Stream::reset()` before the next tick after a seek, even if the restored model epoch matches.
- Drive `tick` after input changes for History, and at least 60 times per second for Frame/State scheduling. The stream limits Frame to 60 Hz and State to 5 Hz. It does no socket or disk I/O.
- `wire::routes(HttpShell, Stream)` registers WebSocket and trace routes. Each screen has latest-value Frame/State slots and reliable History/snapshots. More than 4096 pending History entries closes that screen.

## Runtime and contracts

`ingest::runtime::run` composes live UDP, recorder, model, track observation, layout/track routes, stream publication and the configured HTTP listener. UDP publishes model checkpoints through a latest-value channel; network writes run on the HTTP runtime. HTTP bind changes follow config reloads. Tray integration and replay transport belong to later tickets.

Regenerate with `cargo run --example generate_wire`. `cargo test --test wire_contract` regenerates into a temporary directory and compares every checked-in file; normal CI runs this check. Telemetry contract derives originate in `tools/telemetry/generate_parser.py`.

All new synthetic wire/runtime tests are labelled `assumed`. Loopback datagram-to-WebSocket checks do not qualify live-game telemetry, Windows or performance.
