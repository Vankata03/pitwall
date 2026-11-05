use pitwall_core::ingest::config::Cli;
use pitwall_core::{data::DataFolder, paths::AppPaths};
use pitwall_native::desktop::startup_script;
use pitwall_native::host::{Command, Host};
use std::path::Path;

fn host() -> Host {
    let dir = std::env::temp_dir().join(format!(
        "pitwall-host-test-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let paths = AppPaths::from_data_dir(&dir);
    std::fs::create_dir_all(&paths.recordings).unwrap();
    for name in [
        "2026-11-01_1338_Melbourne_Race.pitrec",
        "2026-11-01_1338_Melbourne_Race.json",
    ] {
        std::fs::copy(
            Path::new("../core/tests/fixtures/data/recordings").join(name),
            paths.recordings.join(name),
        )
        .unwrap();
    }
    Host::open(
        DataFolder::open(paths, &Cli::default()).unwrap(),
        Cli::default(),
    )
    .unwrap()
}

#[test]
fn recordings_pin_replay_transport_and_loaded_delete_protection() {
    let mut host = host();
    let name = host.status().recordings[0].file_name.clone();
    host.command(Command::Pin(name.clone(), true)).unwrap();
    assert!(host.status().recordings[0].sidecar.pinned);
    host.command(Command::Start(name.clone())).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while host.status().loading.is_some() {
        host.tick(std::time::Duration::ZERO).unwrap();
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
    host.command(Command::Pause(true)).unwrap();
    host.command(Command::Seek(1.0)).unwrap();
    host.command(Command::Speed(2.0)).unwrap();
    host.tick(std::time::Duration::ZERO).unwrap();
    let replay = host.status().replay.unwrap();
    assert!(replay.paused);
    assert_eq!(replay.position, 1.0);
    assert_eq!(replay.speed, 2.0);
    assert!(host.command(Command::Delete(name.clone())).is_err());
    assert!(host.command(Command::Speed(3.0)).is_err());
    assert!(host.command(Command::Seek(f64::NAN)).is_err());
    host.command(Command::Stop).unwrap();
    host.command(Command::Delete(name)).unwrap();
    assert!(host.status().recordings.is_empty());
}

#[test]
fn stopped_load_never_selects_replay_and_settings_errors_keep_effective_values() {
    let mut host = host();
    let initial = host.status().config;
    let mut invalid = initial.clone();
    invalid.udp.port = 0;
    assert!(host.command(Command::Save(invalid)).is_err());
    assert_eq!(host.status().config, initial);
    assert!(
        host.command(Command::Start("../escape.pitrec".into()))
            .is_err()
    );
    let name = host.status().recordings[0].file_name.clone();
    host.command(Command::Start(name)).unwrap();
    host.command(Command::Stop).unwrap();
    host.tick(std::time::Duration::from_secs(1)).unwrap();
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
