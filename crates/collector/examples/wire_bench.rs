//! Synthetic publication cost; not a game or device performance qualification.
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
    let bytes = Command::new("python3")
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
    for n in 1..=100 {
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
    let stream = Stream::new("bench");
    stream.tick(Duration::ZERO, Source::Live, &model, Context::default());
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
        "600 ticks, 24 cars, 99 completed laps: {:.3} ms/tick",
        start.elapsed().as_secs_f64() * 1000.0 / 600.0
    );
}
