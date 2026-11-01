//! Status-page HTTP boundary. All host controls use the shared LAN trust shell.
use crate::{
    http::{Access, HttpShell},
    ingest::config::{Cli, Config},
    paths::AppPaths,
    recorder::{self, Recorder},
    wire::CollectorStatus,
};
use axum::{
    Json,
    extract::{ConnectInfo, Path, State},
    http::{StatusCode, header},
    routing::{delete, get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs, io,
    net::{SocketAddr, UdpSocket},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Instant,
};

#[derive(Clone)]
pub struct Status(Arc<Mutex<Inner>>);
struct Inner {
    replay: Option<crate::replay::Controller>,
    reload_error: Option<String>,
    paths: AppPaths,
    config_path: PathBuf,
    cli: Cli,
    config: Config,
    recorder: Arc<Mutex<Recorder>>,
    collector: CollectorStatus,
    missing: BTreeMap<String, (String, Instant)>,
    track: Option<(i8, u16)>,
    cpu: Option<f32>,
    memory: Option<u64>,
    latency: Vec<f64>,
    system: sysinfo::System,
}
impl Status {
    pub fn new(
        paths: AppPaths,
        config_path: PathBuf,
        cli: Cli,
        config: Config,
        recorder: Arc<Mutex<Recorder>>,
    ) -> Self {
        Self(Arc::new(Mutex::new(Inner {
            replay: None,
            reload_error: None,
            paths,
            config_path,
            cli,
            config,
            recorder,
            collector: CollectorStatus::default(),
            missing: BTreeMap::new(),
            track: None,
            cpu: None,
            memory: None,
            latency: vec![],
            system: sysinfo::System::new(),
        })))
    }
    pub fn set_replay(&self, replay: crate::replay::Controller) {
        self.0.lock().unwrap().replay = Some(replay);
    }
    pub fn reload_error(&self, error: Option<String>) {
        self.0.lock().unwrap().reload_error = error;
    }
    /// Called on the control path, never on the UDP receive thread.
    pub fn update(
        &self,
        config: Config,
        collector: CollectorStatus,
        track: Option<(i8, u16)>,
        latency: Vec<f64>,
    ) {
        let mut i = self.0.lock().unwrap();
        i.config = config;
        i.collector = collector;
        i.track = track;
        i.latency = latency;
        if let Ok(pid) = sysinfo::get_current_pid() {
            i.system
                .refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
            if let Some(p) = i.system.process(pid) {
                let cpu = p.cpu_usage();
                let memory = p.memory();
                i.cpu = Some(cpu);
                i.memory = Some(memory);
            }
        }
    }
}
fn error(e: io::Error) -> StatusCode {
    if e.kind() == io::ErrorKind::ResourceBusy {
        return StatusCode::CONFLICT;
    }
    if e.kind() == io::ErrorKind::NotFound {
        StatusCode::NOT_FOUND
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}
fn urls(config: &Config) -> Vec<String> {
    let mut addresses = if config.http.bind.is_unspecified() {
        if_addrs::get_if_addrs()
            .unwrap_or_default()
            .into_iter()
            .map(|i| i.ip())
            .filter(|ip| !ip.is_unspecified() && (config.http.bind.is_ipv6() || ip.is_ipv4()))
            .collect::<Vec<_>>()
    } else {
        vec![config.http.bind]
    };
    addresses.sort();
    addresses.dedup();
    // Connect chooses the default route without sending any datagram.
    if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
        if socket.connect("192.0.2.1:9").is_ok() {
            if let Ok(local) = socket.local_addr() {
                if let Some(at) = addresses.iter().position(|ip| *ip == local.ip()) {
                    addresses.swap(0, at);
                }
            }
        }
    }
    addresses
        .into_iter()
        .map(|ip| format!("http://{}", SocketAddr::new(ip, config.http.port)))
        .collect()
}
async fn read(State(s): State<Status>, ConnectInfo(peer): ConnectInfo<SocketAddr>) -> Json<Value> {
    let mut i = s.0.lock().unwrap();
    i.missing.retain(|_, (_, at)| at.elapsed().as_secs() < 120);
    let mut latency = i.latency.clone();
    latency.sort_by(f64::total_cmp);
    let percentile = |p: f64| {
        latency
            .get(((latency.len() as f64 * p).ceil() as usize).saturating_sub(1))
            .copied()
    };
    Json(json!({
        "udpPort": i.config.udp.port,
        "reloadError": i.reload_error,
        "replay": i.replay.as_ref().and_then(|r| r.status()),
        "host": peer.ip().to_canonical().is_loopback(),
        "collector": i.collector,
        "urls": urls(&i.config),
        "missingStreamLayouts": i.missing.values().map(|v| &v.0).collect::<std::collections::BTreeSet<_>>(),
        "track": i.track,
        "cpuPercent": i.cpu,
        "memoryBytes": i.memory,
        "processingP95Ms": percentile(0.95),
        "processingP99Ms": percentile(0.99),
        "recorderDropped": i.recorder.lock().unwrap().dropped(),
    }))
}
async fn qr(
    State(s): State<Status>,
) -> Result<([(header::HeaderName, &'static str); 1], String), StatusCode> {
    let i = s.0.lock().unwrap();
    let url = urls(&i.config)
        .into_iter()
        .next()
        .ok_or(StatusCode::NOT_FOUND)?;
    let code = qrcode::QrCode::new(url).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok((
        [(header::CONTENT_TYPE, "image/svg+xml")],
        code.render::<qrcode::render::svg::Color>()
            .min_dimensions(192, 192)
            .build(),
    ))
}
async fn settings(State(s): State<Status>) -> Json<Config> {
    {
        let i = s.0.lock().unwrap();
        Json(
            i.cli
                .load(&i.config_path)
                .unwrap_or_else(|_| i.config.clone()),
        )
    }
}
async fn save(
    State(s): State<Status>,
    Json(config): Json<Config>,
) -> Result<Json<Value>, StatusCode> {
    config.validate().map_err(|_| StatusCode::BAD_REQUEST)?;
    // Validate forwarding targets off the receive path before persisting.
    if config.forward.enabled {
        use std::net::ToSocketAddrs;
        for target in &config.forward.targets {
            if !target
                .to_socket_addrs()
                .map_err(|_| StatusCode::BAD_REQUEST)?
                .any(|a| a.port() != 0)
            {
                return Err(StatusCode::BAD_REQUEST);
            }
        }
    }
    let i = s.0.lock().unwrap();
    let mut effective = config.clone();
    i.cli.apply(&mut effective);
    // Persist the file values; CLI overrides remain effective on every reload.
    let text = toml::to_string_pretty(&config).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if let Some(parent) = i.config_path.parent() {
        fs::create_dir_all(parent).map_err(error)?;
    }
    let tmp = i.config_path.with_extension("toml.tmp");
    fs::write(&tmp, text).map_err(error)?;
    fs::rename(tmp, &i.config_path).map_err(error)?;
    let changed = effective.http != i.config.http;
    let redirect = changed.then(|| {
        let bind = if effective.http.bind.is_unspecified() {
            if effective.http.bind.is_ipv4() {
                std::net::IpAddr::from([127, 0, 0, 1])
            } else {
                std::net::IpAddr::V6(std::net::Ipv6Addr::LOCALHOST)
            }
        } else {
            effective.http.bind
        };
        format!(
            "http://{}/status",
            SocketAddr::new(bind, effective.http.port)
        )
    });

    Ok(Json(json!({"redirect":redirect})))
}
#[derive(Deserialize)]
struct Missing {
    client: String,
    name: Option<String>,
}
async fn missing(
    State(s): State<Status>,
    Json(b): Json<Missing>,
) -> Result<StatusCode, StatusCode> {
    if b.client.len() > 100 || b.name.as_ref().is_some_and(|n| n.len() > 200) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let mut i = s.0.lock().unwrap();
    i.missing.retain(|_, (_, at)| at.elapsed().as_secs() < 120);
    match b.name {
        Some(name) => {
            if i.missing.len() >= 64 && !i.missing.contains_key(&b.client) {
                return Err(StatusCode::TOO_MANY_REQUESTS);
            }
            if i.missing.get(&b.client).is_none_or(|v| v.0 != name) {
                fs::create_dir_all(&i.paths.logs).map_err(error)?;
                use std::io::Write;
                writeln!(
                    fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(i.paths.logs.join("collector.log"))
                        .map_err(error)?,
                    "Missing Stream view layout: {name:?}"
                )
                .map_err(error)?;
                eprintln!("Missing Stream view layout: {name:?}");
            }
            i.missing.insert(b.client, (name, Instant::now()));
        }
        None => {
            i.missing.remove(&b.client);
        }
    }
    Ok(StatusCode::NO_CONTENT)
}
async fn recordings(State(s): State<Status>) -> Result<Json<Value>, StatusCode> {
    let i = s.0.lock().unwrap();
    Ok(Json(json!(
        recorder::list(&i.paths.recordings)
            .map_err(error)?
            .into_iter()
            .map(|e| json!({"name":e.file_name,"size":e.size,"sidecar":e.sidecar}))
            .collect::<Vec<_>>()
    )))
}
fn recording(i: &Inner, name: &str) -> Result<PathBuf, StatusCode> {
    if name.starts_with('.') || name.contains(['/', '\\', ':']) || !name.ends_with(".pitrec") {
        return Err(StatusCode::BAD_REQUEST);
    }
    let path = i.paths.recordings.join(name);
    let meta = fs::symlink_metadata(&path).map_err(error)?;
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(path)
}
#[derive(Deserialize)]
struct Pin {
    pinned: bool,
}
async fn pin(
    State(s): State<Status>,
    Path(name): Path<String>,
    Json(b): Json<Pin>,
) -> Result<StatusCode, StatusCode> {
    let i = s.0.lock().unwrap();
    recording(&i, &name)?;
    recorder::set_pinned(&i.paths.recordings, &name, b.pinned).map_err(error)?;
    Ok(StatusCode::NO_CONTENT)
}
async fn remove(
    State(s): State<Status>,
    Path(name): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let recorder = {
        let i = s.0.lock().unwrap();
        recording(&i, &name)?;
        i.recorder.clone()
    };
    let done = recorder.lock().unwrap().delete(&name).map_err(error)?;
    tokio::task::spawn_blocking(move || {
        done.recv()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .map_err(error)
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)??;
    Ok(StatusCode::NO_CONTENT)
}
async fn rescan(State(s): State<Status>) -> StatusCode {
    s.0.lock().unwrap().recorder.lock().unwrap().rescan();
    StatusCode::ACCEPTED
}
async fn open_folder(State(s): State<Status>) -> Result<StatusCode, StatusCode> {
    let path = s.0.lock().unwrap().paths.recordings.clone();
    #[cfg(target_os = "macos")]
    let command = "open";
    #[cfg(target_os = "windows")]
    let command = "explorer";
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let command = "xdg-open";
    std::process::Command::new(command)
        .arg(path)
        .spawn()
        .map_err(error)?;
    Ok(StatusCode::NO_CONTENT)
}
pub fn routes(shell: HttpShell, s: Status) -> HttpShell {
    shell
        .route("/api/status", get(read).with_state(s.clone()), Access::Any)
        .route("/api/status/qr", get(qr).with_state(s.clone()), Access::Any)
        .route(
            "/api/status/missing-stream",
            post(missing).with_state(s.clone()),
            Access::Any,
        )
        .route(
            "/api/settings",
            get(settings).put(save).with_state(s.clone()),
            Access::Host,
        )
        .route(
            "/api/recordings",
            get(recordings).with_state(s.clone()),
            Access::Any,
        )
        .route(
            "/api/recordings/rescan",
            post(rescan).with_state(s.clone()),
            Access::Host,
        )
        .route(
            "/api/recordings/open-folder",
            post(open_folder).with_state(s.clone()),
            Access::Host,
        )
        .route(
            "/api/recordings/{name}/pin",
            post(pin).with_state(s.clone()),
            Access::Host,
        )
        .route(
            "/api/recordings/{name}",
            delete(remove).with_state(s),
            Access::Host,
        )
}
