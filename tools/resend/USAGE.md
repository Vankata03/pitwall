# UDP re-send tool

Build the dashboard before Cargo:

```sh
pnpm install --frozen-lockfile && pnpm build
cargo run --release --locked --offline --bin pitrec-resend -- session.pitrec 127.0.0.1:20777
```

Start the collector separately. The destination is a host and UDP port;
IPv6 addresses use brackets, for example `[::1]:20777`. The command sends once
at 1x, then exits. Stop it early with Ctrl-C.

`pitrec::Reader` restores `seq` arrival order from the grouped compressed
blocks. Datagrams are sent unchanged, including unknown packet IDs. The first
datagram sends immediately after its block is decoded. Later sends use
absolute monotonic deadlines from its send time and recorded `arrival_us`.
Equal timestamps retain sequence order. If decoding or scheduling runs late,
overdue datagrams send immediately; later deadlines remain unchanged.

The tool streams one decoded block at a time. Malformed input, file errors,
address errors and socket errors exit nonzero. An incomplete tail also exits
nonzero after sending complete records. Already sent datagrams cannot be
retracted. An empty recording reports zero sends. The final count reports
socket sends, not confirmed delivery: UDP has no acknowledgement.

The tool does not run during builds or in CI. Run its ignored tests explicitly
on a machine with IPv4 and IPv6 loopback:

```sh
cargo test --locked --offline -p pitwall-collector --test pitrec_resend -- --ignored
```

Tests use synthetic recordings and local UDP receivers. They check byte
preservation, sequence order across blocks, recorded spacing, IPv6 and failure
reporting. Scheduler tolerances are broad. These tests do not qualify a running
collector, Windows, game captures, four-screen latency or performance budgets.
