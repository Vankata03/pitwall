//! Synthetic publication cost; not a game or device performance qualification.
//! `cargo run --release --example wire_bench [completed_laps]`
use pitwall_collector::{
    model::SessionModel,
    telemetry::{Packet, parse_datagram},
    wire::{Context, Source, Stream},
};
use std::{
    process::Command,
    time::{Duration, Instant},
};
fn main() {
    let bytes = Command::new(if cfg!(windows) { "python" } else { "python3" })
        .args([
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tools/telemetry/assumed_datagram.py"
            ),
            "2026",
            "2",
            "--set",
            "header.session_uid=101",
        ])
        .output()
        .unwrap();
    let mut packet = parse_datagram(&bytes.stdout).unwrap();
    let mut model = SessionModel::default();
    let laps: u8 = std::env::args()
        .nth(1)
        .map_or(100, |v| v.parse().expect("completed laps, 1 to 100"));
    for n in 1..=laps {
        packet.header.session_time = n as f32 * 90.0;
        if let Packet::Lap(d) = &mut packet.packet {
            for l in d.lap_data.iter_mut().flatten() {
                l.result_status = 2;
                l.current_lap_num = n;
                l.last_lap_time_in_ms = 90000;
                l.sector = 2;
                l.sector1_time_ms_part = 30000;
                l.sector2_time_ms_part = 30000;
            }
        }
        model.apply(Duration::from_secs(n as u64 * 90), &packet);
    }
    let mut telemetry = parse_datagram(
        &Command::new(if cfg!(windows) { "python" } else { "python3" })
            .args([
                concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../tools/telemetry/assumed_datagram.py"
                ),
                "2026",
                "6",
                "--set",
                "header.session_uid=101",
            ])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    let stream = Stream::new("bench");
    stream.tick(Duration::ZERO, Source::Live, &model, Context::default());
    // Static: nothing arrives, so only the 60 Hz slot and the clone cost remain.
    let start = Instant::now();
    for n in 1..=600 {
        stream.tick(
            Duration::from_millis(n * 17),
            Source::Live,
            &model,
            Context::default(),
        );
    }
    println!(
        "static model:    {:.3} ms/tick",
        start.elapsed().as_secs_f64() * 1000.0 / 600.0
    );
    // Live: one Lap Data and one Car Telemetry packet per tick on lap 101, then the
    // runtime's model.clone() for the publisher, so copy-on-write is paid as in the collector.
    let (mut apply_us, mut clone_us, mut tick_us) = (0.0, 0.0, 0.0);
    for n in 1..=600u32 {
        let ms = n * 17;
        let at = f32::from(laps) * 90.0 + ms as f32 / 1000.0;
        packet.header.session_time = at;
        telemetry.header.session_time = at;
        if let Packet::Lap(d) = &mut packet.packet {
            for l in d.lap_data.iter_mut().flatten() {
                l.current_lap_num = laps + 1;
                l.current_lap_time_in_ms = ms;
                l.last_lap_time_in_ms = 90000;
                l.lap_distance = ms as f32 / 90000.0 * 7000.0;
                l.sector = if ms < 30000 { 0 } else { 1 };
                l.sector1_time_ms_part = if ms < 30000 { 0 } else { 30000 };
                l.sector2_time_ms_part = 0;
            }
        }
        let t = Instant::now();
        let at = Duration::from_secs_f32(at);
        model.apply(at, &packet);
        model.apply(at, &telemetry);
        apply_us += t.elapsed().as_secs_f64() * 1e6;
        let t = Instant::now();
        let held = model.clone();
        clone_us += t.elapsed().as_secs_f64() * 1e6;
        let t = Instant::now();
        stream.tick(
            Duration::from_millis(100_000 + n as u64 * 17),
            Source::Live,
            &held,
            Context::default(),
        );
        tick_us += t.elapsed().as_secs_f64() * 1e6;
    }
    println!(
        "live model:      apply {:.0} us, clone {:.0} us, tick {:.3} ms (24 cars, {laps} completed laps)",
        apply_us / 600.0,
        clone_us / 600.0,
        tick_us / 600.0 / 1000.0
    );
}
