//! Assumed configuration and CLI scenarios, isolated from the user's config.
use pitwall_collector::ingest::config::{Cli, Config};
use std::path::PathBuf;

#[test]
fn assumed_toml_defaults_and_cli_overrides_match_spec() {
    let defaults = Config::from_toml("").unwrap();
    assert_eq!(defaults.udp.port, 20777);
    assert_eq!(defaults.http.port, 8777);
    assert_eq!(defaults.http.bind.to_string(), "0.0.0.0");
    assert!(!defaults.forward.enabled);
    assert!(defaults.forward.targets.is_empty());
    assert!(defaults.recording.enabled);
    assert_eq!(defaults.recording.size_cap_gb, 20);
    assert!(!defaults.app.start_with_windows);
    let mut config = Config::from_toml(
        r#"
        [udp]
        port = 20000
        [forward]
        enabled = true
        targets = ["localhost:22000", "[::1]:23000"]
        [recording]
        enabled = false
        size_cap_gb = 12
    "#,
    )
    .unwrap();
    let cli = Cli::parse([
        "--console",
        "--udp-port",
        "21000",
        "--http-port=8800",
        "--bind",
        "127.0.0.1",
        "--config",
        "custom.toml",
        "--data-dir",
        "isolated",
    ])
    .unwrap();
    cli.apply(&mut config);
    assert!(cli.console);
    assert_eq!(cli.config_path, Some(PathBuf::from("custom.toml")));
    assert_eq!(cli.data_dir, Some(PathBuf::from("isolated")));
    assert_eq!((config.udp.port, config.http.port), (21000, 8800));
    assert_eq!(config.http.bind.to_string(), "127.0.0.1");
    assert_eq!(config.forward.targets, ["localhost:22000", "[::1]:23000"]);
    assert!(!config.recording.enabled);
    assert_eq!(config.recording.size_cap_gb, 12);
}

#[test]
fn assumed_bad_config_and_flags_are_rejected_without_silent_defaults() {
    for text in [
        "[udp]\nport = 0",
        "[udp]\nport = 65536",
        "[udp]\nport = '20777'",
        "[forward]\nenabled = 1",
        "[udp]\nprot = 20777",
        "[recording]\nsize_cap_gb = 0",
        "[http]\nbind = 'elsewhere'",
    ] {
        assert!(Config::from_toml(text).is_err(), "{text}");
    }
    for args in [
        vec!["--unknown"],
        vec!["--udp-port"],
        vec!["--udp-port", "0"],
        vec!["--http-port=65536"],
        vec!["--bind=not-an-ip"],
        vec!["--console=yes"],
        vec!["--config="],
        vec!["--data-dir="],
        vec!["--data-dir"],
    ] {
        assert!(Cli::parse(args.clone()).is_err(), "{args:?}");
    }
}

#[test]
fn assumed_reload_keeps_cli_overrides_and_explicit_missing_file_is_an_error() {
    let root = std::env::temp_dir().join(format!("pitwall-ingest-config-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("config.toml");
    assert_eq!(Cli::default().load(&path).unwrap(), Config::default());
    let cli = Cli::parse([
        std::ffi::OsString::from("--config"),
        path.clone().into_os_string(),
        "--udp-port=24000".into(),
    ])
    .unwrap();
    assert!(cli.load(&path).is_err());
    std::fs::write(
        &path,
        "[udp]\nport=24001\n[forward]\nenabled=true\ntargets=['localhost:22000']",
    )
    .unwrap();
    let first = cli.load(&path).unwrap();
    assert_eq!(first.udp.port, 24000);
    assert!(first.forward.enabled);
    std::fs::write(&path, "[udp]\nport=24002").unwrap();
    let second = cli.load(&path).unwrap();
    assert_eq!(second.udp.port, 24000);
    assert!(!second.forward.enabled);
    std::fs::write(&path, "malformed").unwrap();
    assert!(cli.load(&path).is_err());
    std::fs::remove_dir_all(&root).unwrap();
}
