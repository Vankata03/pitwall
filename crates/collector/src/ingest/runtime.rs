//! Executable wiring until the recorder (#30), session model (#31), and host
//! application are attached. Config reads and DNS stay off the receive thread.
use super::{
    DatagramTap, Disposition,
    config::{Cli, Config},
    udp::{LiveUdp, PreparedUpdate},
};
use crate::{
    paths::AppPaths,
    recorder::{Recorder, RecorderConfig},
};
use std::{
    io,
    net::{Ipv4Addr, SocketAddr},
    path::PathBuf,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

fn unix_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

fn address(config: &Config) -> SocketAddr {
    SocketAddr::from((Ipv4Addr::UNSPECIFIED, config.udp.port))
}

pub fn run(cli: Cli, default_path: PathBuf) -> io::Result<()> {
    let mut config = cli.load(&default_path)?;
    let mut live = LiveUdp::bind(address(&config), &config.forward)?;
    let recording = config.recording.clone();
    let (updates, receiver) = mpsc::channel();
    let (status_sender, statuses) = mpsc::sync_channel(1);
    let worker =
        thread::Builder::new()
            .name("pitwall-udp".into())
            .spawn(move || -> io::Result<()> {
                let origin = Instant::now();
                let mut last_status = Instant::now();
                // The recorder only enqueues; dropping it at exit closes the open recording.
                let mut recorder = if recording.enabled {
                    Some(Recorder::start(
                        RecorderConfig::new(
                            AppPaths::resolve()?.recordings,
                            recording.size_cap_gb.saturating_mul(1_000_000_000),
                        ),
                        unix_seconds,
                    )?)
                } else {
                    None
                };
                let mut tap = |at: u64, bytes: &[u8]| {
                    if let Some(recorder) = recorder.as_mut() {
                        recorder.datagram(at, bytes);
                    }
                };
                loop {
                    loop {
                        match receiver.try_recv() {
                            Ok(update) => live.apply(update),
                            Err(mpsc::TryRecvError::Empty) => break,
                            Err(mpsc::TryRecvError::Disconnected) => return Ok(()),
                        }
                    }
                    let now = || origin.elapsed().as_micros().min(u64::MAX as u128) as u64;
                    for received in live.poll(256, now, &mut tap)? {
                        if let Ok((parsed, Disposition::Apply)) = received.result {
                            // #31: model.apply(Duration::from_micros(received.arrival_time_us), &parsed).
                            // Parsing and lifecycle filtering already happened once.
                            let _ = parsed;
                        }
                    }
                    // #31: model.advance(Duration::from_micros(now())) also carries silence.
                    if last_status.elapsed() >= Duration::from_secs(1) {
                        let _ = status_sender.try_send(live.status(now()));
                        last_status = Instant::now();
                    }
                    thread::sleep(Duration::from_millis(1));
                }
            })?;

    if cli.console {
        eprintln!(
            "Receiving live UDP on {}; config: {}",
            address(&config),
            cli.config_path.as_ref().unwrap_or(&default_path).display()
        );
    }
    let mut last_error = None;
    loop {
        if worker.is_finished() {
            return worker
                .join()
                .map_err(|_| io::Error::other("UDP receive thread panicked"))?;
        }
        // Reading a partially saved or invalid file cannot replace active state.
        let reload = (|| -> io::Result<()> {
            let next = cli.load(&default_path)?;
            if next.udp != config.udp || next.forward != config.forward {
                let update =
                    PreparedUpdate::prepare(address(&config), address(&next), &next.forward)?;
                updates
                    .send(update)
                    .map_err(|_| io::Error::other("UDP receive thread stopped"))?;
                if cli.console {
                    eprintln!("Applied UDP/forwarding settings: {}", address(&next));
                }
            }
            config = next;
            Ok(())
        })();
        match reload {
            Ok(()) => last_error = None,
            Err(error) => {
                let message = error.to_string();
                if last_error.as_ref() != Some(&message) {
                    eprintln!("Config reload rejected; keeping active settings: {message}");
                    last_error = Some(message);
                }
            }
        }
        while let Ok(status) = statuses.try_recv() {
            if cli.console {
                eprintln!(
                    "Receiving {} Hz; dropped frames: {}; UID 0: {}; closed UID: {}; forwarding errors: {}",
                    status.ingest.packet_rate_hz,
                    status.ingest.dropped_frames,
                    status.ingest.uid_zero,
                    status.ingest.closed_uid,
                    status.forwarding_errors
                );
                if let Some(format) = status
                    .ingest
                    .last_format
                    .filter(|format| !matches!(format, 2025 | 2026))
                {
                    eprintln!(
                        "Unsupported telemetry format {format}; set the game's UDP Format to 2025 or 2026"
                    );
                }
            }
        }
        thread::sleep(Duration::from_millis(250));
    }
}
