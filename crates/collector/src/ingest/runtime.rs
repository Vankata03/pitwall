//! Collector runtime composition. Config reads and DNS stay off the receive thread.
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
    sync::{Arc, Mutex, mpsc},
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
    let paths = AppPaths::resolve()?;
    let layouts = crate::layouts::LayoutLibrary::open(paths.layouts.clone())?;
    let tracks = crate::tracks::TrackStore::open(paths.tracks.clone());
    let stream = crate::wire::Stream::new(env!("PITWALL_BUILD_ID"));
    let recorder = Arc::new(Mutex::new(Recorder::start(
        RecorderConfig::new(
            paths.recordings.clone(),
            config.recording.size_cap_gb.saturating_mul(1_000_000_000),
        ),
        unix_seconds,
    )?));
    let replay = crate::replay::Controller::new(paths.recordings.clone(), recorder.clone());
    let status_page = crate::status::Status::new(
        paths.clone(),
        cli.config_path.clone().unwrap_or(default_path.clone()),
        cli.clone(),
        config.clone(),
        recorder.clone(),
    );
    status_page.set_replay(replay.clone());
    let recording_enabled = Arc::new(std::sync::atomic::AtomicBool::new(config.recording.enabled));
    let shell = crate::http::HttpShell::new(crate::http::TrustedHosts::discover()?);
    // Browsers request this even before the dashboard ships an icon.
    let shell = shell.route(
        "/favicon.ico",
        axum::routing::get(|| async { axum::http::StatusCode::NO_CONTENT }),
        crate::http::Access::Any,
    );
    let shell = crate::liveries::routes(shell, &paths);
    let shell = crate::layouts::routes(shell, layouts.clone());
    let shell = crate::tracks::routes(shell, tracks.clone());
    let shell = crate::replay::routes(shell, replay.clone());
    let shell = crate::status::routes(shell, status_page.clone());
    let router = crate::wire::routes(shell, stream.clone()).into_router();
    let http_runtime = tokio::runtime::Runtime::new()?;
    let http_address = |c: &Config| SocketAddr::new(c.http.bind, c.http.port);
    let mut http = http_runtime.block_on(crate::http::HttpListener::bind(
        http_address(&config),
        router,
    ))?;
    let (models, mut selected) = tokio::sync::watch::channel((
        crate::model::SessionModel::default(),
        crate::wire::CollectorStatus::default(),
    ));
    let publisher_tracks = tracks.clone();
    http_runtime.spawn(async move {
        let origin = tokio::time::Instant::now();
        let mut timer = tokio::time::interval(Duration::from_secs(1) / 60);
        timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! { _ = timer.tick() => {}, changed = selected.changed() => { if changed.is_err() { break; } } }
            let (model, collector) = selected.borrow_and_update().clone();
            replay.publish(origin.elapsed(), &model, &stream, crate::wire::Context {
                revisions: crate::wire::Revisions { layouts: layouts.revision() as u32, tracks: publisher_tracks.revision() as u32 },
                collector, replay: None,
            });
        }
    });
    let worker_recording = recording_enabled.clone();
    let worker_recorder = recorder.clone();
    let (updates, receiver) = mpsc::channel();
    let (status_sender, statuses) = mpsc::sync_channel(1);
    let worker =
        thread::Builder::new()
            .name("pitwall-udp".into())
            .spawn(move || -> io::Result<()> {
                let origin = Instant::now();
                let mut last_status = Instant::now();
                let mut model = crate::model::SessionModel::default();
                let mut tap = |at: u64, bytes: &[u8]| {
                    if worker_recording.load(std::sync::atomic::Ordering::Relaxed) {
                        worker_recorder.lock().unwrap().datagram(at, bytes);
                    }
                };
                let mut latency = Vec::new();
                loop {
                    loop {
                        match receiver.try_recv() {
                            Ok(update) => live.apply(update),
                            Err(mpsc::TryRecvError::Empty) => break,
                            Err(mpsc::TryRecvError::Disconnected) => return Ok(()),
                        }
                    }
                    let now = || origin.elapsed().as_micros().min(u64::MAX as u128) as u64;
                    let processing_start = Instant::now();
                    let received = live.poll(256, now, &mut tap)?;
                    let had_input = !received.is_empty();
                    for received in received {
                        if let Ok((parsed, Disposition::Apply)) = received.result {
                            model.apply(Duration::from_micros(received.arrival_time_us), &parsed);
                            tracks.observe(&model);
                        }
                    }
                    if had_input && latency.len() < 4096 {
                        latency.push(processing_start.elapsed().as_secs_f64() * 1000.0);
                    }
                    model.advance(Duration::from_micros(now()));
                    let status = live.status(now());
                    models.send_replace((
                        model.clone(),
                        crate::wire::CollectorStatus {
                            packet_rate_hz: status.ingest.packet_rate_hz.round() as u32,
                            dropped: status.ingest.dropped_frames as u32,
                            parse_errors: status.ingest.parse_errors as u32,
                            uid_zero: status.ingest.uid_zero as u32,
                            closed_uid: status.ingest.closed_uid as u32,
                            forwarding_errors: status.forwarding_errors as u32,
                            last_format: status.ingest.last_format,
                        },
                    ));
                    if last_status.elapsed() >= Duration::from_secs(1) {
                        let track = match model.session() {
                            crate::model::Readout::Live(v) | crate::model::Readout::Stale(v) => {
                                Some((v.track_id, v.track_length))
                            }
                            _ => None,
                        };
                        let _ = status_sender.try_send((
                            live.status(now()),
                            track,
                            std::mem::take(&mut latency),
                        ));
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
            let update = if next.udp != config.udp || next.forward != config.forward {
                Some(PreparedUpdate::prepare(
                    address(&config),
                    address(&next),
                    &next.forward,
                )?)
            } else {
                None
            };
            if next.http != config.http {
                http_runtime.block_on(http.restart(http_address(&next)))?;
            }
            if let Some(update) = update {
                updates
                    .send(update)
                    .map_err(|_| io::Error::other("UDP receive thread stopped"))?;
                if cli.console {
                    eprintln!("Applied UDP/forwarding settings: {}", address(&next));
                }
            }
            if next.recording.size_cap_gb != config.recording.size_cap_gb {
                recorder
                    .lock()
                    .unwrap()
                    .set_size_cap(next.recording.size_cap_gb.saturating_mul(1_000_000_000));
            }
            recording_enabled.store(next.recording.enabled, std::sync::atomic::Ordering::Relaxed);
            config = next;
            Ok(())
        })();
        match reload {
            Ok(()) => {
                last_error = None;
                status_page.reload_error(None);
            }
            Err(error) => {
                let message = error.to_string();
                status_page.reload_error(Some(message.clone()));
                if last_error.as_ref() != Some(&message) {
                    eprintln!("Config reload rejected; keeping active settings: {message}");
                    last_error = Some(message);
                }
            }
        }
        while let Ok((status, track, latency)) = statuses.try_recv() {
            status_page.update(
                config.clone(),
                crate::wire::CollectorStatus {
                    packet_rate_hz: status.ingest.packet_rate_hz.round() as u32,
                    dropped: status.ingest.dropped_frames as u32,
                    parse_errors: status.ingest.parse_errors as u32,
                    uid_zero: status.ingest.uid_zero as u32,
                    closed_uid: status.ingest.closed_uid as u32,
                    forwarding_errors: status.forwarding_errors as u32,
                    last_format: status.ingest.last_format,
                },
                track,
                latency,
            );
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
