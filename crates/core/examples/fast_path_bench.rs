//! Measures the UI read API (`pitwall_core::feed`): the seqlock fast path against a
//! `Mutex<FastFrame>` baseline, and the slow-state snapshot publish for scale.
//!
//! `cargo run --release -p pitwall-core --example fast_path_bench`
//! Needs Python for the assumed telemetry fixtures. Results: docs/native/fast-path.md.
use pitwall_core::{
    feed::{self, FastFrame},
    model::SessionModel,
    telemetry::parse_datagram,
};
use std::{
    hint::black_box,
    process::Command,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

const CARS: usize = 22;

fn packet(id: u8, fields: &[String]) -> Vec<u8> {
    let mut command = Command::new(if cfg!(windows) { "python" } else { "python3" });
    command.arg(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tools/telemetry/assumed_datagram.py"
    ));
    command.args(["2025", &id.to_string(), "--set", "header.session_uid=101"]);
    for field in fields {
        command.args(["--set", field]);
    }
    let output = command.output().expect("Python is required");
    assert!(output.status.success());
    output.stdout
}

/// A model with 22 valid cars, each with session, lap data and telemetry.
fn model() -> SessionModel {
    let mut model = SessionModel::default();
    let laps: Vec<String> = (0..CARS)
        .map(|car| format!("lap_data.{car}.result_status=2"))
        .collect();
    let inputs: Vec<String> = (0..CARS)
        .flat_map(|car| {
            [
                format!("car_telemetry_data.{car}.speed={}", 200 + car),
                format!("car_telemetry_data.{car}.throttle=0.8"),
                format!("car_telemetry_data.{car}.gear=6"),
                format!("car_telemetry_data.{car}.engine_rpm=11000"),
            ]
        })
        .collect();
    for bytes in [packet(1, &[]), packet(2, &laps), packet(6, &inputs)] {
        model.apply(Duration::ZERO, &parse_datagram(&bytes).unwrap());
    }
    model
}

/// Nanoseconds per call of `f`, the median of five runs of `n` calls.
fn per_call(n: u32, mut f: impl FnMut()) -> f64 {
    let mut runs: Vec<f64> = (0..5)
        .map(|_| {
            let start = Instant::now();
            for _ in 0..n {
                f();
            }
            start.elapsed().as_nanos() as f64 / f64::from(n)
        })
        .collect();
    runs.sort_by(f64::total_cmp);
    runs[2]
}

/// Run `readers` threads calling `read` while the caller writes for `window`. Returns
/// (ns per write, total reads per second).
fn contended(
    readers: usize,
    window: Duration,
    read: impl Fn() + Send + Sync + 'static,
    mut write: impl FnMut(),
) -> (f64, f64) {
    let read = Arc::new(read);
    let stop = Arc::new(AtomicBool::new(false));
    let count = Arc::new(AtomicU64::new(0));
    let threads: Vec<_> = (0..readers)
        .map(|_| {
            let (read, stop, count) = (read.clone(), stop.clone(), count.clone());
            thread::spawn(move || {
                let mut n = 0;
                while !stop.load(Ordering::Relaxed) {
                    read();
                    n += 1;
                }
                count.fetch_add(n, Ordering::Relaxed);
            })
        })
        .collect();
    let start = Instant::now();
    let mut writes = 0u64;
    while start.elapsed() < window {
        for _ in 0..100 {
            write();
        }
        writes += 100;
    }
    let elapsed = start.elapsed();
    stop.store(true, Ordering::Relaxed);
    for thread in threads {
        thread.join().unwrap();
    }
    (
        elapsed.as_nanos() as f64 / writes as f64,
        count.load(Ordering::Relaxed) as f64 / elapsed.as_secs_f64(),
    )
}

fn main() {
    let model = model();
    let window = Duration::from_millis(500);
    let (mut publisher, reader) = feed::channel();
    publisher.publish_fast(&model);
    let frame = reader.fast();
    println!("fast frame: {} bytes as a struct", size_of::<FastFrame>());
    assert!(
        frame.cars[..CARS]
            .iter()
            .all(|car| car.map(|c| c.gear) == pitwall_core::model::Readout::Live(6))
    );

    println!("\n# uncontended, ns per call");
    println!(
        "seqlock publish_fast (extract 24 slots + write)  {:8.1}",
        per_call(100_000, || publisher.publish_fast(black_box(&model)))
    );
    println!(
        "seqlock fast() read                               {:8.1}",
        per_call(100_000, || {
            black_box(reader.fast());
        })
    );
    println!(
        "seqlock fast_generation()                         {:8.1}",
        per_call(1_000_000, || {
            black_box(reader.fast_generation());
        })
    );
    let mutex = Arc::new(Mutex::new(frame));
    println!(
        "mutex write (frame copy under lock)               {:8.1}",
        per_call(100_000, || *mutex.lock().unwrap() = black_box(frame))
    );
    println!(
        "mutex read (frame copy under lock)                {:8.1}",
        per_call(100_000, || {
            black_box(*mutex.lock().unwrap());
        })
    );
    println!(
        "publish_snapshot (model clone into Arc)           {:8.1}",
        per_call(10_000, || publisher.publish_snapshot(black_box(&model)))
    );
    println!(
        "snapshot() read (Arc clone under lock)            {:8.1}",
        per_call(100_000, || {
            black_box(reader.snapshot());
        })
    );

    println!("\n# channel construction and catch-up reads, ns per call (median of five)");
    println!(
        "channel() allocate/initialize/drop {:10.1}",
        per_call(100, || {
            black_box(feed::channel());
        })
    );
    for _ in 0..feed::FAST_HISTORY_CAPACITY {
        publisher.publish_fast(&model);
    }
    let latest = reader.fast_generation();
    for count in [0, 1, 4, feed::FAST_HISTORY_CAPACITY] {
        let since = latest - count as u64;
        let batch = reader.fast_since(since);
        assert_eq!(batch.frames.len(), count);
        assert!(!batch.overrun);
        println!(
            "fast_since {count:4} frames {:10.1}",
            per_call(100, || {
                black_box(reader.fast_since(since));
            })
        );
    }

    for readers in [1, 3] {
        println!("\n# writer flat out, {readers} reader thread(s) spinning");
        let r = reader.clone();
        let (write, reads) = contended(
            readers,
            window,
            move || {
                black_box(r.fast());
            },
            || publisher.publish_fast(black_box(&model)),
        );
        println!(
            "seqlock: {write:8.1} ns/write, {:6.2} M reads/s",
            reads / 1e6
        );
        let m = mutex.clone();
        let (write, reads) = contended(
            readers,
            window,
            move || {
                black_box(*m.lock().unwrap());
            },
            || *mutex.lock().unwrap() = black_box(frame),
        );
        println!(
            "mutex:   {write:8.1} ns/write, {:6.2} M reads/s",
            reads / 1e6
        );
    }

    println!("\n# writer at a game-like 120 Hz, 3 readers spinning: worst write latency");
    for (name, use_mutex) in [("seqlock", false), ("mutex", true)] {
        let stop = Arc::new(AtomicBool::new(false));
        let threads: Vec<_> = (0..3)
            .map(|_| {
                let (r, m, stop) = (reader.clone(), mutex.clone(), stop.clone());
                thread::spawn(move || {
                    while !stop.load(Ordering::Relaxed) {
                        if use_mutex {
                            black_box(*m.lock().unwrap());
                        } else {
                            black_box(r.fast());
                        }
                    }
                })
            })
            .collect();
        let mut worst = Duration::ZERO;
        let mut total = Duration::ZERO;
        for _ in 0..120 {
            let start = Instant::now();
            if use_mutex {
                *mutex.lock().unwrap() = black_box(frame);
            } else {
                publisher.publish_fast(black_box(&model));
            }
            let took = start.elapsed();
            worst = worst.max(took);
            total += took;
            thread::sleep(Duration::from_micros(8_333));
        }
        stop.store(true, Ordering::Relaxed);
        for thread in threads {
            thread.join().unwrap();
        }
        println!(
            "{name}: mean {:6.2} us, worst {:6.2} us",
            total.as_secs_f64() * 1e6 / 120.0,
            worst.as_secs_f64() * 1e6
        );
    }
}
