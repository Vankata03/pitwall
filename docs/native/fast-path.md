# UI read API and fast-path measurements

Issue #66 splits the UI-free core into `crates/core` (`pitwall-core`) and gives the UI two
ways to read it, both in-process, with no serialisation: a slow-state snapshot and a
separate fast path for frame-rate data. This note records the choice of fast-path
mechanism and the numbers behind it.

## The API

`pitwall_core::feed::channel()` returns a `Publisher` for the ingest thread and a
`Reader` that UI threads clone.

| Path | Publish | Read | Content |
| --- | --- | --- | --- |
| Slow state | `publish_snapshot(&model)` | `snapshot() -> Snapshot` | The whole `SessionModel` behind an `Arc`: session, cars, laps, stints, race-control log. `Snapshot` dereferences to `SessionModel`. |
| Fast path | `publish_fast(&model)` | `fast() -> FastFrame`, `fast_generation() -> u64` | Per car slot: speed, throttle, brake, steer, clutch, gear, RPM, DRS and rev lights, each `Live`, `Stale` or `Absent`. Also the session time and the player car index. |

Each path carries a generation that advances on every publish. A UI keeps the generation
it last drew and repaints only when it changes; `fast_generation()` is one atomic load, so
that check costs nothing.

The ingest thread should call `publish_fast` after each batch of applied datagrams and
`publish_snapshot` at a lower rate (the current web runtime clones the model on every
batch with input and every 50 ms when idle).

## Mechanism: a seqlock over atomic words

Three mechanisms were candidates: a lock-free ring, a seqlock and a triple buffer.

- A readout needs only the latest frame. Traces also need intervening samples, so #68
  adds a bounded ring of independently seqlocked frames alongside the latest frame.
- A triple buffer has one reader. Several native windows and the in-game overlay may read
  at once.
- A seqlock has one writer, any number of readers, and a writer that never waits.

The fast frame is a fixed size, so the seqlock stores it as 98 `AtomicU64` words: one
header word, three per car slot for the inputs, one per car slot for lap distance and
one model epoch word. The optional catch-up read (`fast_since`) retains 256 publications
in approximately 198 KiB of atomic storage per channel. It reports overruns; the overlay
clears its old history before consuming the retained tail. Epoch tags prevent a new
snapshot from displaying old frames while the two feed paths catch up.
The measurements below were taken before the lap-distance word was added (73 words, 608
bytes as a struct); #68 added it so the overlay can align a live sample with the
reference lap at the same circuit position. The writer marks the sequence
odd, stores the words, then marks it even. A reader copies the words and retries if the
sequence moved. Because every word is an atomic, a racing read is never a data race, and
there is no `unsafe`. The `Publisher` is not `Clone` and takes `&mut self`, which enforces
the single writer.

The slow path is a `Mutex<Snapshot>` held only to swap or clone an `Arc`. The model's own
`Arc`s keep the clone cheap; readers keep an old snapshot as long as they hold it.

## Measurements

Run with `cargo run --release -p pitwall-core --example fast_path_bench`. Machine: AMD
Ryzen 7 7800X3D (8 cores, 16 threads), Windows 11, rustc 1.96.0, release profile. The
model holds 22 valid cars with telemetry, built from the assumed packet fixtures. The
mutex baseline is a `Mutex<FastFrame>` holding the same frame; its write is only the copy,
without the model extraction `publish_fast` does, so it flatters the mutex.

Uncontended, nanoseconds per call (median of five runs):

| Operation | ns |
| --- | ---: |
| `publish_fast` (extract 24 slots from the model, encode, write) | 151 |
| `fast()` (copy and decode) | 42 |
| `fast_generation()` | 1 |
| Mutex write (frame copy only) | 16 |
| Mutex read (frame copy) | 7 |
| `publish_snapshot` (model clone into an `Arc`) | 149 |
| `snapshot()` (clone the `Arc`) | 7 |

The writer running flat out, with three reader threads spinning on reads:

| Mechanism | ns per write | Reads per second |
| --- | ---: | ---: |
| Seqlock | 257–267 | 25–27 million |
| Mutex | 516–565 | 8–10 million |

The writer at a game-like 120 Hz, with three reader threads spinning, over 120 writes
(three runs):

| Mechanism | Mean write | Worst write |
| --- | ---: | ---: |
| Seqlock | 0.84–0.88 µs | 1.5–5.7 µs |
| Mutex | 1.04–1.25 µs | 25–41 µs |

## Conclusion

The seqlock is the fast path. Uncontended, the mutex is faster per call, but every number
is far below a frame at 240 fps (4.2 ms). What matters is the ingest thread: under reader
contention the mutex stalls it for up to about 40 µs per write, and it slows further as
readers are added, while the seqlock writer stays in single-digit microseconds and readers
scale. A reader of the seqlock can retry while a write is in progress, but a write takes
well under a microsecond, so a reader never waits long.

`tests/feed.rs` in `crates/core` covers the API: the empty start, the inputs and their
staleness, the independence of the two paths, and three concurrent readers that never
see a torn frame across 200,000 writes. Removing the sequence recheck from the reader
makes that last test fail with torn frames.

## #68 catch-up history measurement

The preceding tables describe the original latest-frame path before lap distance,
epoch and catch-up history were added. A rerun on 2026-11-05 with
`cargo run --release -p pitwall-core --example fast_path_bench` reports an 808-byte
`FastFrame`, 265.8 ns per uncontended publish (including extraction and both seqlock
writes), 71.7 ns per latest-frame read and 1.0 ns per generation read. These figures
do not measure `fast_since` copying a batch. The concurrency test now also checks
retained frames against their publication generations while three readers race the
200,000 writes. The bounded-history and publication-order cases have separate tests.
