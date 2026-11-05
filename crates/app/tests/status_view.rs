//! The status window's content, derived from host status without a window.
use pitwall_core::ingest::{IngestStatus, udp::LiveStatus};
use pitwall_native::{host::Status, status_view::StatusView};

fn live(rate: f64, format: Option<u16>) -> LiveStatus {
    LiveStatus {
        ingest: IngestStatus {
            dropped_frames: 3,
            uid_zero: 4,
            closed_uid: 5,
            parse_errors: 6,
            last_format: format,
            packet_rate_hz: rate,
            ..IngestStatus::default()
        },
        forwarded: 0,
        forwarding_errors: 7,
    }
}

#[test]
fn connection_names_waiting_or_measured_rate_with_below_60_hz_guidance() {
    let waiting = StatusView::new(&Status::default());
    assert_eq!(waiting.connection, "Waiting for game");
    assert_eq!(waiting.rate_guidance, None);

    let status = Status {
        live: Some(live(30.0, Some(2025))),
        recorder_dropped: 2,
        ..Status::default()
    };
    let view = StatusView::new(&status);
    assert_eq!(view.connection, "Receiving 30 Hz");
    assert_eq!(
        view.rate_guidance.as_deref(),
        Some("Measured rate is below 60 Hz. Set UDP Send Rate to 60 Hz.")
    );
    assert_eq!(
        view.counters,
        [
            ("Dropped packets", "3".to_owned()),
            ("Recorder queue drops", "2".to_owned()),
            ("UID 0", "4".to_owned()),
            ("Closed UID", "5".to_owned()),
            ("Parse errors", "6".to_owned()),
            ("Forwarding errors", "7".to_owned()),
        ]
    );
    let full = StatusView::new(&Status {
        live: Some(live(60.0, Some(2025))),
        ..Status::default()
    });
    assert_eq!(full.connection, "Receiving 60 Hz");
    assert_eq!(full.rate_guidance, None);
}

#[test]
fn measures_read_unavailable_until_sampled_then_two_decimals() {
    let empty = StatusView::new(&Status::default());
    assert_eq!(
        empty.measures,
        [
            ("CPU (one logical core)", "Unavailable".to_owned()),
            ("Resident memory", "Unavailable".to_owned()),
            ("Processing p95", "Unavailable".to_owned()),
            ("Processing p99", "Unavailable".to_owned()),
        ]
    );
    let view = StatusView::new(&Status {
        cpu: Some(12.5),
        memory: Some(3 * 1_048_576 + 524_288),
        p95: Some(0.126),
        p99: Some(1.0),
        ..Status::default()
    });
    assert_eq!(
        view.measures,
        [
            ("CPU (one logical core)", "12.50 %".to_owned()),
            ("Resident memory", "3.50 MiB".to_owned()),
            ("Processing p95", "0.13 ms".to_owned()),
            ("Processing p99", "1.00 ms".to_owned()),
        ]
    );
    assert_eq!(
        pitwall_native::status_view::MEASURE_NOTE,
        "Processing measures each UDP poll and model update batch. Datagram-to-draw latency is unmeasured."
    );
}

#[test]
fn game_setup_checklist_names_the_effective_port_and_format_guidance() {
    let mut status = Status::default();
    status.config.udp.port = 20888;
    let view = StatusView::new(&status);
    assert_eq!(
        view.checklist,
        [
            "Enable UDP Telemetry in the F1® 25 game; it is off by default.",
            "Set UDP Format to 2025 or 2026.",
            "Set UDP Port to 20888, matching pitwall settings.",
            "Set UDP IP to 127.0.0.1 when the game runs on this PC, or to this PC's LAN IP when it runs on another device. Broadcast is an alternative to forwarding.",
            "Set UDP Send Rate to 60 Hz; 20–60 Hz is supported.",
        ]
    );
    assert_eq!(
        view.format,
        "Received format: none. Both 2025 and 2026 are accepted."
    );
    assert!(
        view.trust
            .iter()
            .any(|line| line.contains("Windows Firewall"))
    );
    assert!(view.trust.iter().any(|line| line.contains("unsigned")));
    assert!(
        !view
            .trust
            .iter()
            .any(|line| line.contains("iPad") || line.contains("HTTP")),
        "LAN-browser advice is obsolete natively"
    );

    status.live = Some(live(60.0, Some(2026)));
    assert_eq!(
        StatusView::new(&status).format,
        "Received format: 2026. Both 2025 and 2026 are accepted."
    );
    status.live = Some(live(60.0, Some(2024)));
    assert_eq!(
        StatusView::new(&status).format,
        "Unsupported telemetry format 2024. Set UDP Format to 2025 or 2026. Unsupported packets are neither parsed nor recorded."
    );
}

fn listing(name: &str, track: Option<&str>, pinned: bool) -> pitwall_core::recorder::Listing {
    pitwall_core::recorder::Listing {
        file_name: name.into(),
        size: 5 * 1_048_576 / 2,
        sidecar: pitwall_core::recorder::Sidecar {
            session_uid: "1".into(),
            packet_format: 2025,
            started_utc: "2026-11-01T13:38:00Z".into(),
            track: track.map(str::to_owned),
            session_type: None,
            duration_us: 61_400_000,
            laps: 3,
            pinned,
            recompressed: true,
        },
    }
}

#[test]
fn recordings_list_track_seconds_laps_size_and_pinned_state() {
    assert!(StatusView::new(&Status::default()).recordings.is_empty());
    assert_eq!(pitwall_native::status_view::NO_RECORDINGS, "No recordings.");
    let status = Status {
        recordings: vec![
            listing("a.pitrec", Some("Melbourne"), true),
            listing("b.pitrec", None, false),
        ],
        ..Status::default()
    };
    let rows = StatusView::new(&status).recordings;
    assert_eq!(rows[0].name, "a.pitrec");
    assert_eq!(
        rows[0].summary,
        "Melbourne · 61 s · 3 laps · 2.5 MiB · Pinned"
    );
    assert!(rows[0].pinned);
    assert_eq!(
        rows[1].summary,
        "Unknown track · 61 s · 3 laps · 2.5 MiB · Unpinned"
    );
}

#[test]
fn replay_line_shows_source_progress_position_state_and_background_live() {
    assert_eq!(StatusView::new(&Status::default()).replay, "Live source");
    let loading = Status {
        loading: Some(0.42),
        replay_name: Some("a.pitrec".into()),
        ..Status::default()
    };
    assert_eq!(
        StatusView::new(&loading).replay,
        "a.pitrec · Loading replay · 42% loaded"
    );
    let playing = Status {
        replay_name: Some("a.pitrec".into()),
        replay: Some(pitwall_core::replay::ReplayState {
            duration: 90.0,
            position: 12.34,
            speed: 2.0,
            paused: false,
            banner: "Replay".into(),
            load_progress: 1.0,
            background_live: Some("A live session is recording in the background".into()),
        }),
        ..Status::default()
    };
    assert_eq!(
        StatusView::new(&playing).replay,
        "a.pitrec · Replay · 100% loaded · 12.3 s / 90.0 s · Playing · 2x · A live session is recording in the background"
    );
}

#[test]
fn relearn_needs_the_current_track_id_and_length() {
    assert!(!StatusView::new(&Status::default()).relearn);
    let status = Status {
        track: Some(pitwall_core::tracks::TrackKey::new(0, 5278)),
        ..Status::default()
    };
    assert!(StatusView::new(&status).relearn);
}

#[test]
fn notice_matches_the_spec_full_notice_and_short_line_exactly() {
    use pitwall_native::status_view::{FULL_NOTICE, SHORT_LINE};
    let spec = std::fs::read_to_string("../../docs/spec/v1.md").unwrap();
    let section = &spec[spec.find("## 15. Notices and marks").unwrap()..];
    let quoted = |after: &str| -> Vec<String> {
        section[section.find(after).unwrap()..]
            .lines()
            .skip_while(|l| !l.starts_with('>'))
            .take_while(|l| l.starts_with('>'))
            .map(|l| l.trim_start_matches('>').trim().to_owned())
            .filter(|l| !l.is_empty())
            .collect()
    };
    assert_eq!(FULL_NOTICE.to_vec(), quoted("**Full notice.**"));
    assert_eq!(vec![SHORT_LINE.to_owned()], quoted("**Short line.**"));
}

#[test]
fn alerts_show_the_current_error_load_notices_and_kept_bad_files() {
    assert!(StatusView::new(&Status::default()).alerts.is_empty());
    let status = Status {
        error: Some("UDP port 20777: in use".into()),
        notices: vec!["Settings (config.toml) could not be read".into()],
        bad_files: vec!["C:/data/pitwall/config.toml.bad".into()],
        ..Status::default()
    };
    assert_eq!(
        StatusView::new(&status).alerts,
        [
            "UDP port 20777: in use",
            "Settings (config.toml) could not be read",
            "Unreadable file kept aside: C:/data/pitwall/config.toml.bad",
        ]
    );
}
