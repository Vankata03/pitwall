//! Host commands and status, driven without a window or live UDP.
use pitwall_core::{data::DataFolder, ingest::config::Cli, paths::AppPaths};
use pitwall_native::{
    desktop::{Startup, startup_script},
    host::{Command, Desktop, Host, Runtime},
};
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicU32, Ordering},
    time::{Duration, Instant},
};

/// Start with Windows in the temp folder, no notifications.
fn quiet(script: PathBuf) -> Desktop {
    Desktop {
        startup: Startup::at(script),
        notifications: false,
    }
}

const FIXTURE: &str = "2026-11-01_1338_Melbourne_Race";

/// A temporary data directory, removed on drop. Never the real `%APPDATA%\pitwall`.
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "pitwall-native-host-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
    fn paths(&self) -> AppPaths {
        AppPaths::from_data_dir(&self.0)
    }
    fn startup(&self) -> PathBuf {
        self.0.join("Startup").join("pitwall.vbs")
    }
    fn with_recording(self) -> Self {
        let recordings = self.paths().recordings;
        std::fs::create_dir_all(&recordings).unwrap();
        for ext in ["pitrec", "json"] {
            let name = format!("{FIXTURE}.{ext}");
            std::fs::copy(
                Path::new("../core/tests/fixtures/data/recordings").join(&name),
                recordings.join(name),
            )
            .unwrap();
        }
        self
    }
    fn cli(&self) -> Cli {
        Cli {
            data_dir: Some(self.0.clone()),
            ..Cli::default()
        }
    }
    fn open_with(&self, cli: Cli) -> Host {
        let folder = DataFolder::open(self.paths(), &cli).unwrap();
        Host::open(folder, cli, quiet(self.startup())).unwrap()
    }
    fn open(&self) -> Host {
        self.open_with(self.cli())
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn wait_loaded(host: &mut Host) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while host.status().loading.is_some() {
        host.tick(Duration::ZERO).unwrap();
        assert!(Instant::now() < deadline, "replay never loaded");
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn recordings_pin_replay_transport_and_loaded_delete_protection() {
    let temp = Temp::new().with_recording();
    let mut host = temp.open();
    let name = host.status().recordings[0].file_name.clone();
    host.command(Command::Pin(name.clone(), true)).unwrap();
    assert!(host.status().recordings[0].sidecar.pinned);
    host.command(Command::Start(name.clone())).unwrap();
    wait_loaded(&mut host);
    host.command(Command::Pause(true)).unwrap();
    host.command(Command::Seek(1.0)).unwrap();
    host.command(Command::Speed(2.0)).unwrap();
    host.tick(Duration::ZERO).unwrap();
    let replay = host.status().replay.unwrap();
    assert!(replay.paused);
    assert_eq!(replay.position, 1.0);
    assert_eq!(replay.speed, 2.0);
    assert!(host.command(Command::Delete(name.clone())).is_err());
    assert!(host.command(Command::Speed(3.0)).is_err());
    assert!(host.command(Command::Seek(f64::NAN)).is_err());
    host.command(Command::Stop).unwrap();
    // Pinned protects automatic retention, not an explicit delete.
    host.command(Command::Delete(name)).unwrap();
    assert!(host.status().recordings.is_empty());
}

#[test]
fn stopped_load_never_selects_replay_and_settings_errors_keep_effective_values() {
    let temp = Temp::new().with_recording();
    let mut host = temp.open();
    let initial = host.status().config;
    let mut invalid = initial.clone();
    invalid.udp.port = 0;
    assert!(host.command(Command::Save(invalid)).is_err());
    let mut invalid = initial.clone();
    invalid.recording.size_cap_gb = 0;
    assert!(host.command(Command::Save(invalid)).is_err());
    let mut invalid = initial.clone();
    invalid.forward.enabled = true;
    invalid.forward.targets = vec!["not a target".into()];
    assert!(host.command(Command::Save(invalid)).is_err());
    assert_eq!(host.status().config, initial);
    assert!(
        !temp.paths().config.exists(),
        "rejected settings are not saved"
    );
    assert!(
        host.command(Command::Start("../escape.pitrec".into()))
            .is_err()
    );
    let name = host.status().recordings[0].file_name.clone();
    host.command(Command::Start(name)).unwrap();
    host.command(Command::Stop).unwrap();
    host.tick(Duration::from_secs(1)).unwrap();
    assert!(host.status().replay.is_none());
    assert!(host.status().loading.is_none());
    assert!(host.command(Command::Pause(true)).is_err());
}

#[test]
fn autostart_keeps_isolated_data_and_explicit_config() {
    let cli = Cli {
        data_dir: Some("C:/isolated data".into()),
        config_path: Some("C:/my config.toml".into()),
        udp_port: Some(20888),
        ..Cli::default()
    };
    let script = startup_script(Path::new("C:/apps/pitwall.exe"), &cli);
    assert!(script.contains(r#"--data-dir ""C:/isolated data"""#));
    assert!(script.contains(r#"--config ""C:/my config.toml"""#));
    assert!(script.contains("--udp-port 20888"));
    assert!(!script.contains("--http"));
}

#[test]
fn start_with_windows_creates_and_removes_the_startup_script_with_data_dir() {
    let temp = Temp::new();
    let mut host = temp.open();
    assert!(!host.status().config.app.start_with_windows, "defaults off");
    let mut config = host.status().file_config;
    config.app.start_with_windows = true;
    host.command(Command::Save(config.clone())).unwrap();
    let script = std::fs::read_to_string(temp.startup()).unwrap();
    assert!(script.contains("--data-dir"), "{script}");
    assert!(script.contains(&temp.0.display().to_string()), "{script}");
    assert!(host.status().config.app.start_with_windows);

    config.app.start_with_windows = false;
    host.command(Command::Save(config)).unwrap();
    assert!(!temp.startup().exists());
}

#[test]
fn failed_start_with_windows_reports_and_keeps_settings() {
    let temp = Temp::new();
    // The Startup folder path is a file, so the script cannot be written.
    std::fs::write(temp.0.join("Startup"), b"not a folder").unwrap();
    let mut host = temp.open();
    let before = host.status();
    let mut config = before.file_config.clone();
    config.app.start_with_windows = true;
    assert!(host.command(Command::Save(config)).is_err());
    assert_eq!(host.status().config, before.config);
    assert_eq!(host.status().file_config, before.file_config);
    assert!(
        !temp.paths().config.exists(),
        "failed save must not persist"
    );
}

#[test]
fn save_persists_file_values_drops_obsolete_http_and_keeps_cli_overrides() {
    let temp = Temp::new();
    std::fs::create_dir_all(temp.paths().root).unwrap();
    std::fs::write(
        temp.paths().config,
        "[http]\nport = 8777\nbind = \"0.0.0.0\"\n[udp]\nport = 20777\n",
    )
    .unwrap();
    let mut host = temp.open_with(Cli {
        udp_port: Some(20999),
        ..temp.cli()
    });
    assert_eq!(
        host.status().config.udp.port,
        20999,
        "CLI override effective"
    );
    let mut config = host.status().file_config;
    assert_eq!(config.udp.port, 20777, "file value distinct from CLI");
    config.udp.port = 20800;
    config.forward.enabled = true;
    config.forward.targets = vec!["127.0.0.1:20900".into()];
    config.recording.size_cap_gb = 7;
    host.command(Command::Save(config)).unwrap();

    let saved = std::fs::read_to_string(temp.paths().config).unwrap();
    assert!(!saved.contains("[http]"), "{saved}");
    assert!(saved.contains("port = 20800"), "{saved}");
    let status = host.status();
    assert_eq!(status.file_config.udp.port, 20800);
    assert_eq!(
        status.config.udp.port, 20999,
        "CLI override survives a save"
    );
    assert_eq!(status.config.recording.size_cap_gb, 7);
    assert_eq!(status.config.forward.targets, ["127.0.0.1:20900"]);
    assert_eq!(
        status.feedback,
        "Settings saved. Live reload applies valid changes; command-line overrides remain effective."
    );
}

#[test]
fn reload_applies_valid_external_edits_and_retains_settings_on_invalid_ones() {
    let temp = Temp::new();
    let mut host = temp.open();
    std::fs::create_dir_all(temp.paths().root).unwrap();
    std::fs::write(temp.paths().config, "[recording]\nsize_cap_gb = 3\n").unwrap();
    host.reload_settings().unwrap();
    assert_eq!(host.status().config.recording.size_cap_gb, 3);

    for invalid in [
        "[recording]\nsize_cap_gb = 0\n",
        "[recording\n",
        "[nonsense]\nx = 1\n",
    ] {
        std::fs::write(temp.paths().config, invalid).unwrap();
        assert!(host.reload_settings().is_err(), "{invalid}");
        assert_eq!(host.status().config.recording.size_cap_gb, 3);
        assert_eq!(host.status().file_config.recording.size_cap_gb, 3);
        assert_eq!(
            std::fs::read_to_string(temp.paths().config).unwrap(),
            invalid,
            "a rejected reload never rewrites the file"
        );
    }
}

#[test]
fn malformed_data_is_kept_as_bad_and_reported_with_earlier_bad_files() {
    let temp = Temp::new();
    let root = temp.paths().root;
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(temp.paths().config, "[udp\nport = ").unwrap();
    std::fs::write(root.join("layouts.json.bad"), "kept from an earlier run").unwrap();
    let host = temp.open();
    let status = host.status();
    assert!(
        status.notices.iter().any(|n| n.contains("config.toml")),
        "{:?}",
        status.notices
    );
    assert_eq!(
        std::fs::read_to_string(root.join("config.toml.bad")).unwrap(),
        "[udp\nport = ",
        "the unreadable original is kept unchanged"
    );
    assert_eq!(
        status.bad_files,
        [root.join("config.toml.bad"), root.join("layouts.json.bad")]
    );
    assert_eq!(status.config.udp.port, 20777, "defaults loaded");
}

#[test]
fn replay_jump_restart_end_and_relearn_of_the_replayed_track() {
    let temp = Temp::new().with_recording();
    let mut host = temp.open();
    let name = host.status().recordings[0].file_name.clone();
    host.command(Command::Start(name.clone())).unwrap();
    wait_loaded(&mut host);
    let replay = host.status().replay.unwrap();
    assert_eq!(
        (replay.position, replay.speed, replay.paused),
        (0.0, 1.0, false)
    );
    assert_eq!(host.status().replay_name.as_deref(), Some(name.as_str()));

    assert!(
        host.command(Command::JumpLap(0, 200)).is_err(),
        "unavailable lap"
    );
    assert!(
        host.command(Command::Seek(1e6)).is_err(),
        "outside the recording"
    );

    let duration = replay.duration;
    host.command(Command::Seek(duration)).unwrap();
    host.tick(Duration::ZERO).unwrap();
    let ended = host.status().replay.unwrap();
    assert_eq!(ended.banner, "Replay ended — restart or exit");
    host.command(Command::Restart).unwrap();
    host.tick(Duration::ZERO).unwrap();
    let restarted = host.status().replay.unwrap();
    assert_eq!((restarted.position, restarted.paused), (0.0, false));

    host.command(Command::Seek(duration)).unwrap();
    host.tick(Duration::ZERO).unwrap();
    let track = host
        .status()
        .track
        .expect("replayed session names its track");
    host.command(Command::Relearn).unwrap();
    assert_eq!(host.status().track, Some(track));
}

#[test]
fn missing_or_invalid_recordings_report_errors_and_stay_live() {
    let temp = Temp::new().with_recording();
    let mut host = temp.open();
    std::fs::write(temp.paths().recordings.join("empty.pitrec"), b"").unwrap();
    for name in ["missing.pitrec", "empty.pitrec"] {
        if host.command(Command::Start(name.into())).is_err() {
            assert!(host.status().loading.is_none() && host.status().replay.is_none());
            continue;
        }
        let deadline = Instant::now() + Duration::from_secs(10);
        let error = loop {
            if let Err(e) = host.tick(Duration::ZERO) {
                break e;
            }
            assert!(Instant::now() < deadline, "{name} never failed");
            std::thread::sleep(Duration::from_millis(1));
        };
        assert!(!error.to_string().is_empty());
        let status = host.status();
        assert!(status.replay.is_none() && status.loading.is_none());
        assert_eq!(status.replay_name, None);
    }
    assert!(host.command(Command::Relearn).is_err(), "no current track");
    assert!(
        host.command(Command::Pin("missing.pitrec".into(), true))
            .is_err()
    );
    assert!(
        host.command(Command::Delete("../config.toml".into()))
            .is_err()
    );
}

#[test]
fn command_line_replay_plays_an_outside_file_without_copying_it_into_the_library() {
    let temp = Temp::new();
    let mut host = temp.open();
    let outside =
        Path::new("../core/tests/fixtures/data/recordings").join(format!("{FIXTURE}.pitrec"));
    let before = std::fs::read(&outside).unwrap();
    host.command(Command::StartFile(outside.clone())).unwrap();
    wait_loaded(&mut host);
    assert!(host.status().replay.is_some());
    assert_eq!(
        host.status().replay_name.as_deref(),
        Some(format!("{FIXTURE}.pitrec").as_str())
    );
    assert!(host.status().recordings.is_empty(), "library untouched");
    assert_eq!(std::fs::read(&outside).unwrap(), before);
}

#[test]
fn a_busy_udp_port_reports_keeps_running_and_retries_until_free() {
    let temp = Temp::new();
    let blocker = std::net::UdpSocket::bind("0.0.0.0:0").unwrap();
    let port = blocker.local_addr().unwrap().port();
    let cli = Cli {
        udp_port: Some(port),
        ..temp.cli()
    };
    let folder = DataFolder::open(temp.paths(), &cli).unwrap();
    let runtime = Runtime::start(folder, cli, quiet(temp.startup())).unwrap();
    let wait = |what: &str, done: &dyn Fn(&pitwall_native::host::Status) -> bool| {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let status = runtime.control.status();
            if done(&status) {
                return status;
            }
            assert!(Instant::now() < deadline, "{what}: {:?}", status.error);
            std::thread::sleep(Duration::from_millis(20));
        }
    };
    let failed = wait("port error", &|s| s.error.is_some());
    let error = failed.error.unwrap();
    assert!(error.contains(&format!("UDP port {port}")), "{error}");
    assert!(!failed.ready);

    drop(blocker);
    let ready = wait("retry binds", &|s| s.ready && s.error.is_none());
    assert_eq!(ready.line, "Waiting for game");
    let sampled = wait("process sampled", &|s| {
        s.cpu.is_some() && s.memory.is_some()
    });
    assert!(sampled.memory.unwrap() > 0);
    runtime.shutdown().unwrap();
    let log = std::fs::read_to_string(temp.paths().logs.join("collector.log")).unwrap();
    assert!(log.contains(&format!("Listening: UDP {port}")), "{log}");
    assert!(log.contains("Native host stopped"), "{log}");
}
