//! What the status window shows, derived from host [`Status`] without a window.
use crate::host::Status;

pub struct StatusView {
    /// Current error, load notices and unreadable files kept as `.bad`.
    pub alerts: Vec<String>,
    pub connection: String,
    pub rate_guidance: Option<String>,
    pub counters: Vec<(&'static str, String)>,
    pub measures: Vec<(&'static str, String)>,
    pub checklist: Vec<String>,
    pub format: String,
    /// Windows Firewall and unsigned-build guidance.
    pub trust: Vec<&'static str>,
    pub recordings: Vec<RecordingRow>,
    /// Replay source, load progress, position and state, or "Live source".
    pub replay: String,
    /// Relearn map needs the current track ID and length.
    pub relearn: bool,
}

pub struct RecordingRow {
    pub name: String,
    pub summary: String,
    pub pinned: bool,
}

pub const NO_RECORDINGS: &str = "No recordings.";

fn replay_line(status: &Status) -> String {
    let name = status.replay_name.as_deref().unwrap_or("Replay");
    if let Some(replay) = &status.replay {
        let mut line = format!(
            "{name} · {} · {:.0}% loaded · {:.1} s / {:.1} s · {} · {}x",
            replay.banner,
            replay.load_progress * 100.0,
            replay.position,
            replay.duration,
            if replay.paused { "Paused" } else { "Playing" },
            replay.speed,
        );
        if let Some(background) = &replay.background_live {
            line.push_str(" · ");
            line.push_str(background);
        }
        line
    } else if let Some(progress) = status.loading {
        format!("{name} · Loading replay · {:.0}% loaded", progress * 100.0)
    } else {
        "Live source".into()
    }
}

const TRUST: [&str; 2] = [
    "Allow pitwall through the first-launch Windows Firewall prompt on Private networks. Public-network blocking can stop UDP telemetry sent from another device.",
    "The portable binary is unsigned. Windows SmartScreen may warn; use More info → Run anyway only for a build you trust.",
];

fn format_line(format: Option<u16>) -> String {
    match format {
        Some(format) if !matches!(format, 2025 | 2026) => format!(
            "Unsupported telemetry format {format}. Set UDP Format to 2025 or 2026. Unsupported packets are neither parsed nor recorded."
        ),
        _ => format!(
            "Received format: {}. Both 2025 and 2026 are accepted.",
            format.map_or_else(|| "none".into(), |f| f.to_string())
        ),
    }
}

pub const MEASURE_NOTE: &str = "Processing measures each UDP poll and model update batch. Datagram-to-draw latency is unmeasured.";

fn measure(value: Option<f64>, unit: &str) -> String {
    value.map_or_else(|| "Unavailable".into(), |v| format!("{v:.2} {unit}"))
}

/// "Waiting for game" until a packet rate is measured, then "Receiving N Hz".
pub fn connection_line(status: &Status) -> String {
    match &status.live {
        Some(live) if live.ingest.packet_rate_hz > 0.0 => {
            format!("Receiving {:.0} Hz", live.ingest.packet_rate_hz)
        }
        _ => "Waiting for game".into(),
    }
}

impl StatusView {
    pub fn new(status: &Status) -> Self {
        let rate = status
            .live
            .as_ref()
            .map_or(0.0, |l| l.ingest.packet_rate_hz);
        let count = |value: Option<u64>| value.map_or_else(|| "0".into(), |v| v.to_string());
        let live = status.live.as_ref();
        let alerts = status
            .error
            .iter()
            .cloned()
            .chain(status.notices.iter().cloned())
            .chain(
                status
                    .bad_files
                    .iter()
                    .map(|path| format!("Unreadable file kept aside: {}", path.display())),
            )
            .chain(
                status
                    .windows
                    .iter()
                    .map(|(window, problem)| format!("Window {window}: {problem}")),
            )
            .collect();
        Self {
            alerts,
            connection: connection_line(status),
            rate_guidance: (rate > 0.0 && rate < 60.0)
                .then(|| "Measured rate is below 60 Hz. Set UDP Send Rate to 60 Hz.".into()),
            counters: vec![
                ("Dropped packets", count(live.map(|l| l.ingest.dropped_frames))),
                ("Recorder queue drops", status.recorder_dropped.to_string()),
                ("UID 0", count(live.map(|l| l.ingest.uid_zero))),
                ("Closed UID", count(live.map(|l| l.ingest.closed_uid))),
                ("Parse errors", count(live.map(|l| l.ingest.parse_errors))),
                ("Forwarding errors", count(live.map(|l| l.forwarding_errors))),
            ],
            measures: vec![
                ("CPU (one logical core)", measure(status.cpu.map(f64::from), "%")),
                (
                    "Resident memory",
                    measure(status.memory.map(|b| b as f64 / 1_048_576.0), "MiB"),
                ),
                ("Processing p95", measure(status.p95, "ms")),
                ("Processing p99", measure(status.p99, "ms")),
            ],
            checklist: vec![
                "Enable UDP Telemetry in the F1® 25 game; it is off by default.".into(),
                "Set UDP Format to 2025 or 2026.".into(),
                format!(
                    "Set UDP Port to {}, matching pitwall settings.",
                    status.config.udp.port
                ),
                "Set UDP IP to 127.0.0.1 when the game runs on this PC, or to this PC's LAN IP when it runs on another device. Broadcast is an alternative to forwarding.".into(),
                "Set UDP Send Rate to 60 Hz; 20–60 Hz is supported.".into(),
            ],
            format: format_line(live.and_then(|l| l.ingest.last_format)),
            trust: TRUST.to_vec(),
            recordings: status
                .recordings
                .iter()
                .map(|item| RecordingRow {
                    name: item.file_name.clone(),
                    summary: format!(
                        "{} · {:.0} s · {} laps · {:.1} MiB · {}",
                        item.sidecar.track.as_deref().unwrap_or("Unknown track"),
                        item.sidecar.duration_us as f64 / 1_000_000.0,
                        item.sidecar.laps,
                        item.size as f64 / 1_048_576.0,
                        if item.sidecar.pinned { "Pinned" } else { "Unpinned" },
                    ),
                    pinned: item.sidecar.pinned,
                })
                .collect(),
            replay: replay_line(status),
            relearn: status.track.is_some(),
        }
    }
}

/// Spec section 15 short line.
pub const SHORT_LINE: &str = "Unofficial. Not associated with the Formula 1 companies or EA.";

/// Spec section 15 full notice, one paragraph per entry.
pub const FULL_NOTICE: [&str; 3] = [
    "pitwall is unofficial and is not associated in any way with the Formula 1 companies. F1, FORMULA ONE, FORMULA 1, FIA FORMULA ONE WORLD CHAMPIONSHIP, GRAND PRIX and related marks are trade marks of Formula One Licensing B.V.",
    "This project is not endorsed by or affiliated with EA or its licensors.",
    "Team names, driver names and other third-party names and marks belong to their respective owners and are used only to identify them.",
];
