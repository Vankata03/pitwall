use pitwall_collector::app::{Instance, Launch};

#[test]
fn second_launch_finds_owner_and_stale_file_can_be_reclaimed() {
    let dir = std::env::temp_dir().join(format!("pitwall-instance-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("instance");
    let Launch::Primary(mut owner) = Instance::acquire(&path).unwrap() else {
        panic!("first launch")
    };
    owner.publish("http://127.0.0.1:8777").unwrap();
    let Launch::Existing(url) = Instance::acquire(&path).unwrap() else {
        panic!("duplicate owner")
    };
    assert_eq!(url, "http://127.0.0.1:8777");
    owner.publish("http://127.0.0.1:9000").unwrap();
    let Launch::Existing(url) = Instance::acquire(&path).unwrap() else {
        panic!("duplicate owner")
    };
    assert_eq!(url, "http://127.0.0.1:9000");
    drop(owner);
    assert!(matches!(
        Instance::acquire(&path).unwrap(),
        Launch::Primary(_)
    ));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn browser_opens_only_on_first_ready_launch() {
    let dir = std::env::temp_dir().join(format!("pitwall-first-run-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    assert!(pitwall_collector::app::first_run(&dir).unwrap());
    assert!(!pitwall_collector::app::first_run(&dir).unwrap());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn wildcard_listener_uses_loopback_for_host_actions() {
    use pitwall_collector::{app::dashboard_url, ingest::config::Config};
    let mut config = Config::default();
    assert_eq!(dashboard_url(&config), "http://127.0.0.1:8777");
    config.http.bind = "::".parse().unwrap();
    config.http.port = 9000;
    assert_eq!(dashboard_url(&config), "http://[::1]:9000");
}

#[test]
fn port_conflict_names_port_without_losing_instance_and_runtime_can_stop() {
    use pitwall_collector::{
        app::Control,
        ingest::{
            config::{Cli, Config},
            runtime,
        },
        paths::AppPaths,
    };
    let dir = std::env::temp_dir().join(format!("pitwall-runtime-{}", std::process::id()));
    let paths = AppPaths::from_data_dir(&dir);
    std::fs::create_dir_all(&paths.root).unwrap();
    let occupied = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let port = occupied.local_addr().unwrap().port();
    let http = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let mut config = Config::default();
    config.udp.port = port;
    config.http.port = http.local_addr().unwrap().port();
    config.http.bind = "127.0.0.1".parse().unwrap();
    std::fs::write(&paths.config, toml::to_string(&config).unwrap()).unwrap();
    let control = Control::default();
    let err = runtime::run_at(
        Cli::default(),
        paths.config.clone(),
        paths.clone(),
        control.clone(),
    )
    .unwrap_err();
    assert!(
        err.to_string().contains(&format!("UDP port {port}")),
        "{err}"
    );
    drop(occupied);
    drop(http);
    let worker_paths = paths.clone();
    let c = control.clone();
    let worker = std::thread::spawn(move || {
        runtime::run_at(Cli::default(), worker_paths.config.clone(), worker_paths, c)
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !control.state.lock().unwrap().ready && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let ready = control.state.lock().unwrap().ready;
    control.request_stop();
    worker.join().unwrap().unwrap();
    assert!(ready, "runtime never became ready");
    assert!(std::net::UdpSocket::bind(("127.0.0.1", port)).is_ok());
    std::fs::remove_dir_all(dir).unwrap();
}
