//! Offline benchmark: real pitrec reader, ingest gate and session model; no sockets or drawing.
use pitwall_collector::{
    ingest::{Disposition, Ingest},
    model::SessionModel,
    pitrec::Reader,
};
use std::{
    io::Cursor,
    time::{Duration, Instant},
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bytes = std::fs::read(std::env::args_os().nth(1).ok_or("recording required")?)?;
    let start = Instant::now();
    let mut ingest = Ingest::default();
    let mut model = SessionModel::default();
    let mut count = 0_u64;
    for record in Reader::new(Cursor::new(&bytes))? {
        let record = record?;
        let (packet, disposition) =
            ingest.ingest(record.arrival_us, &record.datagram, &mut |_, _: &[u8]| {})?;
        if disposition == Disposition::Apply {
            model.apply(Duration::from_micros(record.arrival_us), &packet);
        }
        model.advance(Duration::from_micros(record.arrival_us));
        count += 1;
    }
    std::hint::black_box(&model);
    if count == 0 {
        return Err("empty recording".into());
    }
    println!(
        "{}",
        serde_json::json!({"milliseconds": start.elapsed().as_secs_f64()*1000.0, "records": count})
    );
    Ok(())
}
