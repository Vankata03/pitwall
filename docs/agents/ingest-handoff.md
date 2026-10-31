# Issue #29 integration handoff

Sources: [issue #29](https://github.com/Vankata03/pitwall/issues/29), `docs/spec/v1.md` sections 2–5. Tests named `assumed` use synthetic input; they do not qualify game captures, Windows, or performance.

## Recorder (#30)

Use `ingest::DatagramTap::datagram(arrival_time, bytes)` with `Ingest::ingest` or `udp::LiveUdp::poll`. The tap receives every original datagram before parsing and frame filtering, including duplicates and malformed input. Enqueue owned bytes there; never perform file I/O or wait on the receive thread. The recorder must reject unsupported formats, UID 0, and closed identities before writing. It owns its recording boundaries, retention, and shutdown.

The gate separately tracks closed `(session_uid, packet_format)` identities for model filtering and status counts. It closes an identity on replacement or Final Classification. `SEND` marks it ended; silence closes nothing. `Ingest::close_current` is available for collector shutdown.

## Session model (#31)

`Ingest::ingest(arrival_time_us, bytes, tap)` returns `(ParsedDatagram, Disposition)`. Apply only `Disposition::Apply` to the live model. The gate already handles UID eligibility, frame ordering, Events, and the ended-session exemptions. `LiveUdp::poll` returns each result with `arrival_time_us`; errors remain observable without reaching the model.

Use one monotonic microsecond origin for arrivals and `model.advance(time)`, including polls with no input. Keep replay on its own `Ingest` and model instances. Replay does not use `LiveUdp`, so it cannot forward packets. The parser is `telemetry::parse_datagram`.

## Host wiring

`config::Cli::load` reads TOML and reapplies CLI overrides. `PreparedUpdate::prepare(current, desired, forwarding)` resolves destinations and binds a replacement socket on the control thread. `LiveUdp::apply` swaps that prepared state on the receive thread. Failed preparation leaves active settings intact. Forward-only changes preserve the receive socket and queued datagrams.

`ingest::runtime::run` starts the UDP worker and checks config every 250 ms. Recorder and model attachment points are marked in that worker. They are placeholders until #30/#31 are integrated. HTTP settings are parsed here; HTTP serving, tray integration, and coordinated exit remain outside #29.

Status exposes cumulative UID 0, closed-identity, frame-drop, parse-error, and forwarding-error counts. Rates count valid datagrams per packet type over the preceding second, including duplicates. The displayed rate is the highest menu-rate packet type's rate, so aggregate packet traffic cannot hide a low game send rate.
