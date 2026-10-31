# Recording tools

Build the dashboard as described in `docs/development.md` before running Cargo.
The offline `pitrec` command uses the collector's recording codec. It requires
no Python packages and runs only when invoked.

## Convert a capture

```sh
cargo run --locked --offline --bin pitrec -- convert capture.pwcap session.pitrec
```

The input layout comes from `tools/capture/capture.py`. Conversion preserves
raw datagrams, including unknown packet IDs. It skips UID 0. Without a filter,
it requires one non-zero `(UID, telemetry format)` identity; a mixed capture
fails. To extract one identity from a capture containing several sessions:

```sh
cargo run --locked --offline --bin pitrec -- convert capture.pwcap session.pitrec --uid 0x2a --format 2026
```

Supply both selection flags. UID accepts decimal or hexadecimal with `0x`;
format accepts `2025` or `2026`. Filtering includes every datagram of that
identity. Recorder close rules, retention, sidecars and minimum session duration
belong to ticket #30 and are not applied by this tool.

The first selected datagram sets arrival time zero. Conversion subtracts its
nanosecond timestamp before converting the remaining offsets to microseconds,
rounding down. Equal microsecond times retain their arrival order. The header's
UTC start is the capture start plus that first datagram's nanosecond offset.
Game version is `gameYear.gameMajorVersion.gameMinorVersion` from its header.
An incomplete final capture record is ignored; malformed metadata or complete
records fail conversion. A selection with no matching datagrams also fails.

## Cut a clip

```sh
cargo run --locked --offline --bin pitrec -- clip session.pitrec clip.pitrec 10000000 20000000
```

Bounds are integer microseconds in the source recording. The interval includes
its start and excludes its end. The cutter preserves datagrams, assigns new
sequences from zero, and rebases arrival time and UTC start to the first selected
record. Other header fields are retained. Empty or reversed intervals fail.
An incomplete tail contributes no records. A clip contains no synthesized
session initialization; choose a range containing the inputs a model test needs.

Both commands create a new output file and refuse to overwrite any existing
path, including the input. On failure they remove the output they created.

## Collector interface

`pitwall_collector::pitrec` exposes `Header`, `Record`, `Writer<W: Write>` and
`Reader<R: Read>`. The JSON header fields are `packet_format`, `game_version`,
`pitwall_version` and `started_utc`; the caller supplies the UTC time.

Use `Writer::new`, then `write(arrival_us, datagram)` in monotonic arrival order.
The writer normalizes its first timestamp to zero and assigns global `u32`
sequences. It writes roughly one second per zstd frame, grouped by packet ID.
`flush_block` commits a shorter block; `finish` commits the last block and
returns the sink. Dropping a writer discards its buffered records. After an I/O
error, abandon the writer. Compression uses zstd level 1; final recompression
policy remains with the recorder. The sink's `flush` is not an OS durability sync.

`Reader::new` reads the header. Iteration yields `io::Result<Record>` in sequence
order with `seq`, `arrival_us` and unchanged `datagram` bytes. A partial block
prefix or payload stops iteration; `has_incomplete_tail` distinguishes it from
clean EOF. A malformed complete block yields one error and ends iteration,
without yielding any records from that block. Headers are capped at 1 MiB,
decoded blocks at 64 MiB, and compressed blocks at 65 MiB.

## Verification boundary

All ticket #28 fixtures are synthetic and labelled `assumed`. No real capture,
Windows execution, recording throughput or compression ratio was verified.
