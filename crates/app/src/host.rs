//! In-process native host controller. Disk operations and replay loading never run on
//! the UDP receiver or gpui thread.
use crate::desktop;
use pitwall_core::{
    data::DataFolder,
    feed::{self, FastPublisher, Publisher, Reader},
    ingest::{
        DatagramTap, Disposition,
        config::{Cli, Config},
        udp::{LiveStatus, LiveUdp, PreparedUpdate},
    },
    model::{Lifecycle, Readout, SessionModel},
    recorder::{self, Listing, LoadedGuard, Recorder, RecorderConfig},
    replay::{Replay, ReplayState},
    tracks::{TrackKey, TrackStore},
};
use std::{
    fs::{self, File},
    io,
    net::{Ipv4Addr, SocketAddr},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug)]
pub enum Command {
    Save(Config),
    Rescan,
    Pin(String, bool),
    Delete(String),
    Start(String),
    /// A replay outside the library, from the command line. Never copied into it.
    StartFile(PathBuf),
    Stop,
    Pause(bool),
    Seek(f64),
    Speed(f64),
    JumpLap(usize, u8),
    Restart,
    Relearn,
    OpenRecordings,
    OpenLogs,
    Shutdown,
    /// A window reports why it cannot show what it should (`Some`), or that it recovered
    /// or closed (`None`). Shown in status and logged once per change.
    WindowProblem(String, Option<String>),
}

#[derive(Clone, Default)]
pub struct Status {
    pub config: Config,
    pub file_config: Config,
    pub live: Option<LiveStatus>,
    pub ready: bool,
    pub line: String,
    pub error: Option<String>,
    pub feedback: String,
    pub notices: Vec<String>,
    /// Unreadable data files kept aside as `*.bad` in the data folder, from any run.
    pub bad_files: Vec<std::path::PathBuf>,
    pub recordings: Vec<Listing>,
    pub replay: Option<ReplayState>,
    pub replay_name: Option<String>,
    pub loading: Option<f64>,
    pub recorder_dropped: u64,
    pub cpu: Option<f32>,
    pub memory: Option<u64>,
    pub p95: Option<f64>,
    pub p99: Option<f64>,
    pub track: Option<TrackKey>,
    pub revision: u64,
    pub busy: bool,
    /// Problems windows report, by window id, such as a stream window's missing layout.
    pub windows: std::collections::BTreeMap<String, String>,
}

#[derive(Clone)]
pub struct Control {
    sender: mpsc::Sender<Command>,
    state: Arc<Mutex<Status>>,
}
impl Control {
    pub fn send(&self, command: Command) -> io::Result<()> {
        self.sender
            .send(command)
            .map_err(|_| io::Error::other("native host stopped"))
    }
    pub fn status(&self) -> Status {
        self.state.lock().unwrap().clone()
    }
}

/// Where the host touches the desktop: the Startup-folder entry and notifications.
/// Tests point both away from the user's session.
pub struct Desktop {
    pub startup: desktop::Startup,
    pub notifications: bool,
}

impl Desktop {
    /// The current user's Startup folder, with notifications.
    pub fn user() -> Self {
        Self {
            startup: desktop::Startup::user(),
            notifications: true,
        }
    }
}

pub struct Runtime {
    pub control: Control,
    pub reader: Reader,
    /// The host's track store (shared): the Track map tile reads maps from it.
    pub tracks: TrackStore,
    worker: Option<JoinHandle<io::Result<()>>>,
}
impl Runtime {
    pub fn start(folder: DataFolder, cli: Cli, desktop: Desktop) -> io::Result<Self> {
        let (publisher, reader) = feed::channel();
        let (sender, receiver) = mpsc::channel();
        let host = Host::open(folder, cli, desktop)?;
        let tracks = host.tracks.clone();
        let state = Arc::new(Mutex::new(host.status.clone()));
        let control = Control {
            sender,
            state: state.clone(),
        };
        let worker = thread::Builder::new()
            .name("pitwall-host".into())
            .spawn(move || host.run(receiver, state, publisher))?;
        Ok(Self {
            control,
            reader,
            tracks,
            worker: Some(worker),
        })
    }
    pub fn shutdown(mut self) -> io::Result<()> {
        let _ = self.control.send(Command::Shutdown);
        self.worker
            .take()
            .unwrap()
            .join()
            .map_err(|_| io::Error::other("native host panicked"))?
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        let _ = self.control.send(Command::Shutdown);
    }
}

/// A source switch and fast write share only this short critical section. Slow model
/// cloning, disk access and snapshot publication never hold it. Readers use the seqlock.
struct FastSource {
    writer: FastPublisher,
    replay: bool,
    restore_live: bool,
}
type FastOutput = Arc<Mutex<FastSource>>;

struct LiveView {
    model: SessionModel,
    status: Option<LiveStatus>,
    error: Option<String>,
    latency: Vec<f64>,
    generation: u64,
}
struct ReceiverWorker {
    updates: mpsc::Sender<PreparedUpdate>,
    stop: Arc<AtomicBool>,
    state: Arc<Mutex<LiveView>>,
    worker: Option<JoinHandle<io::Result<()>>>,
}
impl ReceiverWorker {
    fn start(
        live: LiveUdp,
        recorder: Arc<Mutex<Recorder>>,
        enabled: Arc<AtomicBool>,
        tracks: TrackStore,
        fast: FastOutput,
    ) -> io::Result<Self> {
        let (updates, receiver) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let state = Arc::new(Mutex::new(LiveView {
            model: SessionModel::default(),
            status: None,
            error: None,
            latency: vec![],
            generation: 0,
        }));
        let worker_state = state.clone();
        let worker_stop = stop.clone();
        let worker = thread::Builder::new()
            .name("pitwall-udp".into())
            .spawn(move || {
                let result = receive(
                    live,
                    recorder,
                    enabled,
                    tracks,
                    receiver,
                    &worker_stop,
                    &worker_state,
                    fast,
                );
                if let Err(error) = &result {
                    worker_state.lock().unwrap().error = Some(error.to_string());
                }
                result
            })?;
        Ok(Self {
            updates,
            stop,
            state,
            worker: Some(worker),
        })
    }
}
impl Drop for ReceiverWorker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn receive(
    mut live: LiveUdp,
    recorder: Arc<Mutex<Recorder>>,
    enabled: Arc<AtomicBool>,
    tracks: TrackStore,
    updates: mpsc::Receiver<PreparedUpdate>,
    stop: &AtomicBool,
    state: &Mutex<LiveView>,
    fast: FastOutput,
) -> io::Result<()> {
    let origin = Instant::now();
    let mut last_publish = Instant::now() - Duration::from_secs(1);
    let mut last_status = last_publish;
    let mut dirty = false;
    let mut model = SessionModel::default();
    let mut samples = Vec::new();
    let mut tap = |at, bytes: &[u8]| {
        if enabled.load(Ordering::Relaxed) {
            recorder.lock().unwrap().datagram(at, bytes);
        }
    };
    while !stop.load(Ordering::Relaxed) {
        while let Ok(update) = updates.try_recv() {
            live.apply(update);
        }
        let now = || origin.elapsed().as_micros().min(u128::from(u64::MAX)) as u64;
        let started = Instant::now();
        let received = live.poll(256, now, &mut tap)?;
        let had_input = !received.is_empty();
        for received in received {
            if let Ok((parsed, Disposition::Apply)) = received.result {
                model.apply(Duration::from_micros(received.arrival_time_us), &parsed);
                tracks.observe(&model);
                dirty = true;
                let mut output = fast.lock().unwrap();
                if !output.replay {
                    output.writer.publish_fast(&model);
                }
            }
        }
        if had_input && samples.len() < 4096 {
            samples.push(started.elapsed().as_secs_f64() * 1000.0);
        }
        let stale_changed = model.advance_readouts(Duration::from_micros(now()));
        dirty |= stale_changed;
        {
            let mut output = fast.lock().unwrap();
            if !output.replay && (stale_changed || output.restore_live) {
                output.writer.publish_fast(&model);
                output.restore_live = false;
            }
        }
        let publish = dirty && last_publish.elapsed() >= Duration::from_millis(200);
        if publish || last_status.elapsed() >= Duration::from_millis(50) {
            let mut out = state.lock().unwrap();
            if publish {
                out.model = model.clone();
                out.generation += 1;
                last_publish = Instant::now();
                dirty = false;
            }
            out.status = Some(live.status(now()));
            out.latency.append(&mut samples);
            if out.latency.len() > 4096 {
                let excess = out.latency.len() - 4096;
                out.latency.drain(..excess);
            }
            last_status = Instant::now();
        }
        live.wait(Duration::from_millis(5))?;
    }
    Ok(())
}

type LoadResult = (u64, String, io::Result<(Replay, Option<LoadedGuard>)>);
pub struct Host {
    folder: DataFolder,
    cli: Cli,
    startup: desktop::Startup,
    notifications: bool,
    status: Status,
    recorder: Arc<Mutex<Recorder>>,
    enabled: Arc<AtomicBool>,
    live: Option<ReceiverWorker>,
    tracks: TrackStore,
    replay: Option<(Replay, Option<LoadedGuard>)>,
    load_generation: u64,
    loaded_tx: mpsc::Sender<LoadResult>,
    loaded_rx: mpsc::Receiver<LoadResult>,
    progress: Arc<Mutex<(u64, f64)>>,
    /// The live model is in a session while a replay plays.
    live_session: bool,
}
impl Host {
    pub fn open(folder: DataFolder, cli: Cli, desktop: Desktop) -> io::Result<Self> {
        fs::create_dir_all(&folder.paths.root)?;
        let recorder = Recorder::start(
            RecorderConfig::new(
                folder.paths.recordings.clone(),
                folder
                    .config
                    .recording
                    .size_cap_gb
                    .saturating_mul(1_000_000_000),
            ),
            || {
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_or(0, |d| d.as_secs() as i64)
            },
        )?;
        let status = Status {
            config: folder.config.clone(),
            file_config: folder.config.clone(),
            live: None,
            ready: false,
            line: "Starting".into(),
            error: None,
            feedback: String::new(),
            notices: folder.notices(),
            bad_files: desktop::bad_files(&folder.paths.root),
            recordings: recorder::list(&folder.paths.recordings)?,
            replay: None,
            replay_name: None,
            loading: None,
            recorder_dropped: 0,
            cpu: None,
            memory: None,
            p95: None,
            p99: None,
            track: None,
            revision: 0,
            busy: false,
            windows: Default::default(),
        };
        let enabled = Arc::new(AtomicBool::new(folder.config.recording.enabled));
        let tracks = TrackStore::open(folder.paths.tracks.clone());
        let (loaded_tx, loaded_rx) = mpsc::channel();
        let mut host = Self {
            folder,
            cli,
            startup: desktop.startup,
            notifications: desktop.notifications,
            status,
            recorder: Arc::new(Mutex::new(recorder)),
            enabled,
            live: None,
            tracks,
            replay: None,
            load_generation: 0,
            loaded_tx,
            loaded_rx,
            progress: Arc::new(Mutex::new((0, 0.0))),
            live_session: false,
        };
        host.load_file_settings()?;
        Ok(host)
    }
    pub fn status(&self) -> Status {
        let mut status = self.status.clone();
        status.bad_files = desktop::bad_files(&self.folder.paths.root);
        status
    }
    fn load_file_settings(&mut self) -> io::Result<()> {
        if let Some(config) = self.read_file_settings()? {
            self.status.file_config = config;
        }
        Ok(())
    }
    /// The config file's values, without obsolete `[http]`; `None` when it is missing.
    fn read_file_settings(&self) -> io::Result<Option<Config>> {
        let text = match fs::read_to_string(&self.folder.config_path) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        let mut table: toml::Table = toml::from_str(&text).map_err(io::Error::other)?;
        table.remove("http");
        Ok(Some(table.try_into().map_err(io::Error::other)?))
    }
    /// Apply external edits to the config file through the same validate/prepare/swap path
    /// as a save. A rejected edit keeps the active settings and never rewrites the file.
    pub fn reload_settings(&mut self) -> io::Result<()> {
        match self.read_file_settings()? {
            Some(next) if next != self.status.file_config => {
                self.apply_settings(next, false)?;
                desktop::log(&self.folder.paths, "Settings reloaded");
                Ok(())
            }
            _ => Ok(()),
        }
    }
    fn effective(&self, config: &Config) -> Config {
        let mut config = config.clone();
        if let Some(port) = self.cli.udp_port {
            config.udp.port = port;
        }
        config
    }
    fn apply_settings(&mut self, config: Config, save: bool) -> io::Result<()> {
        config.validate().map_err(io::Error::other)?;
        let next = self.effective(&config);
        let update = if self.live.is_some()
            && (next.udp != self.status.config.udp || next.forward != self.status.config.forward)
        {
            Some(PreparedUpdate::prepare(
                address(&self.status.config),
                address(&next),
                &next.forward,
            )?)
        } else {
            None
        };
        // Even before bind succeeds, validate forwarding without acquiring the configured port.
        if self.live.is_none() {
            let _ = PreparedUpdate::prepare(address(&next), address(&next), &next.forward)?;
        }
        let changed_startup = next.app != self.status.config.app;
        if changed_startup {
            self.startup.set(next.app.start_with_windows, &self.cli)?;
        }
        if save && let Err(error) = self.folder.save_config(&config) {
            if changed_startup {
                let _ = self
                    .startup
                    .set(self.status.config.app.start_with_windows, &self.cli);
            }
            return Err(error);
        }
        if let Some(update) = update {
            self.live
                .as_ref()
                .unwrap()
                .updates
                .send(update)
                .map_err(|_| io::Error::other("UDP receiver stopped"))?;
        }
        self.recorder
            .lock()
            .unwrap()
            .set_size_cap(next.recording.size_cap_gb.saturating_mul(1_000_000_000));
        self.enabled
            .store(next.recording.enabled, Ordering::Relaxed);
        self.status.file_config = config;
        self.status.config = next;
        desktop::log(&self.folder.paths, "Settings applied");
        Ok(())
    }
    pub fn command(&mut self, command: Command) -> io::Result<()> {
        if let Command::WindowProblem(window, problem) = command {
            self.window_problem(window, problem);
            return Ok(());
        }
        let feedback = match &command {
            Command::Save(_) => {
                "Settings saved. Live reload applies valid changes; command-line overrides remain effective."
            }
            Command::Delete(_) => "Recording deleted.",
            Command::Relearn => "Map relearning.",
            _ => "Done.",
        };
        self.status.feedback.clear();
        match command {
            Command::Save(config) => self.apply_settings(config, true)?,
            Command::Rescan => {
                self.recorder.lock().unwrap().rescan();
                self.recorder.lock().unwrap().sync()?;
            }
            Command::Pin(name, pinned) => {
                recorder::set_pinned(&self.folder.paths.recordings, &name, pinned)?
            }
            Command::Delete(name) => self
                .recorder
                .lock()
                .unwrap()
                .delete(&name)?
                .recv()
                .map_err(io::Error::other)??,
            Command::Start(name) => {
                let path = recorder::recording_path(&self.folder.paths.recordings, &name)?;
                let guard = self.recorder.lock().unwrap().mark_loaded(&name);
                self.load(path, name, Some(guard))?;
            }
            Command::StartFile(path) => {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .ok_or_else(|| io::Error::other("replay path names no file"))?;
                let library = fs::canonicalize(&self.folder.paths.recordings).ok();
                let parent = path.parent().and_then(|p| fs::canonicalize(p).ok());
                if library.is_some() && library == parent {
                    return self.command(Command::Start(name));
                }
                self.load(path, name, None)?;
            }
            Command::Stop => {
                self.load_generation += 1;
                self.replay = None;
                self.status.replay = None;
                self.status.loading = None;
                self.status.replay_name = None;
            }
            Command::Pause(paused) => self.active()?.pause(paused),
            Command::Seek(seconds) => {
                if !seconds.is_finite() || seconds < 0.0 {
                    return Err(io::Error::other("invalid seek position"));
                }
                self.active()?
                    .seek(Duration::try_from_secs_f64(seconds).map_err(io::Error::other)?)?;
                self.load_generation += 1;
            }
            Command::Speed(speed) => self.active()?.set_speed(speed)?,
            Command::JumpLap(car, lap) => {
                self.active()?.jump_lap(car, lap)?;
                self.load_generation += 1;
            }
            Command::Restart => {
                self.active()?.seek(Duration::ZERO)?;
                self.active()?.pause(false);
                self.load_generation += 1;
            }
            Command::Relearn => {
                let key = self
                    .status
                    .track
                    .ok_or_else(|| io::Error::other("no current track"))?;
                self.tracks
                    .relearn(key)
                    .map_err(|e| io::Error::other(format!("{e:?}")))?;
            }
            Command::OpenRecordings => desktop::open(&self.folder.paths.recordings)?,
            Command::OpenLogs => desktop::open(&self.folder.paths.logs)?,
            Command::Shutdown | Command::WindowProblem(..) => {}
        }
        self.status.recordings = recorder::list(&self.folder.paths.recordings)?;
        self.status.feedback = feedback.into();
        self.status.error = None;
        self.status.revision += 1;
        Ok(())
    }
    fn window_problem(&mut self, window: String, problem: Option<String>) {
        let paths = &self.folder.paths;
        match problem {
            Some(problem) => {
                if self.status.windows.get(&window) != Some(&problem) {
                    desktop::log(paths, &format!("Window {window}: {problem}"));
                    self.status.windows.insert(window, problem);
                    self.status.revision += 1;
                }
            }
            None => {
                if self.status.windows.remove(&window).is_some() {
                    desktop::log(
                        paths,
                        &format!("Window {window}: problem cleared (recovered or closed)"),
                    );
                    self.status.revision += 1;
                }
            }
        }
    }
    /// Load off the host thread; a later Start or Stop makes this load's result stale.
    fn load(&mut self, path: PathBuf, name: String, guard: Option<LoadedGuard>) -> io::Result<()> {
        self.load_generation += 1;
        let generation = self.load_generation;
        let tx = self.loaded_tx.clone();
        let progress = self.progress.clone();
        let tracks = self.tracks.clone();
        self.status.loading = Some(0.0);
        self.status.replay_name = Some(name.clone());
        self.replay = None;
        self.status.replay = None;
        thread::Builder::new()
            .name("pitwall-replay-load".into())
            .spawn(move || {
                let result = File::open(path)
                    .and_then(|file| {
                        Replay::load(file, |value| {
                            *progress.lock().unwrap() = (generation, value);
                        })
                    })
                    .map(|replay| {
                        tracks.observe(replay.model());
                        (replay, guard)
                    });
                let _ = tx.send((generation, name, result));
            })?;
        Ok(())
    }
    fn active(&mut self) -> io::Result<&mut Replay> {
        self.replay
            .as_mut()
            .map(|v| &mut v.0)
            .ok_or_else(|| io::Error::other("no active replay"))
    }
    pub fn tick(&mut self, elapsed: Duration) -> io::Result<()> {
        while let Ok((generation, name, result)) = self.loaded_rx.try_recv() {
            if generation != self.load_generation {
                continue;
            }
            self.status.loading = None;
            match result {
                Ok(replay) => {
                    self.replay = Some(replay);
                    self.status.replay_name = Some(name);
                }
                Err(e) => {
                    self.status.replay_name = None;
                    return Err(e);
                }
            }
        }
        if self.status.loading.is_some() {
            let (generation, progress) = *self.progress.lock().unwrap();
            if generation == self.load_generation {
                self.status.loading = Some(progress);
            }
        }
        if let Some((replay, _)) = &mut self.replay {
            replay.tick(elapsed)?;
            self.tracks.observe(replay.model());
            let background = self
                .live_session
                .then(|| "A live session is recording in the background".to_owned());
            self.status.replay = Some(replay.state(background));
            self.status.track = track_of(replay.model());
        }
        Ok(())
    }
    fn refresh_bad_files(&mut self) {
        let files = desktop::bad_files(&self.folder.paths.root);
        if files != self.status.bad_files {
            for path in files
                .iter()
                .filter(|path| !self.status.bad_files.contains(path))
            {
                desktop::log(
                    &self.folder.paths,
                    &format!("Unreadable file kept as {}", path.display()),
                );
            }
            self.status.bad_files = files;
            self.status.revision += 1;
        }
    }
    fn report(&mut self, error: io::Error) {
        let text = error.to_string();
        if self.status.error.as_ref() != Some(&text) {
            desktop::log(&self.folder.paths, &text);
            if self.notifications {
                desktop::notify("pitwall needs attention", &text);
            }
        }
        self.status.error = Some(text);
        self.status.revision += 1;
    }
    fn run(
        mut self,
        receiver: mpsc::Receiver<Command>,
        state: Arc<Mutex<Status>>,
        publisher: Publisher,
    ) -> io::Result<()> {
        desktop::log(
            &self.folder.paths,
            &format!("Starting native build {}", env!("CARGO_PKG_VERSION")),
        );
        if let Err(e) = self
            .startup
            .set(self.status.config.app.start_with_windows, &self.cli)
        {
            self.report(e);
        }
        let mut last = Instant::now();
        let mut housekeeping = Instant::now() - Duration::from_secs(2);
        let mut system = sysinfo::System::new();
        let (mut publisher, fast_writer) = publisher.split();
        let fast = Arc::new(Mutex::new(FastSource {
            writer: fast_writer,
            replay: false,
            restore_live: false,
        }));
        let mut published = None;
        let mut snapshotted = None;
        let mut last_snapshot = Instant::now() - Duration::from_secs(1);
        let mut source = None;
        loop {
            match receiver.recv_timeout(Duration::from_millis(16)) {
                Ok(Command::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Ok(command) => {
                    self.status.busy = true;
                    *state.lock().unwrap() = self.status.clone();
                    if let Err(e) = self.command(command) {
                        self.report(e);
                    }
                    self.status.busy = false;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
            if let Err(e) = self.tick(last.elapsed()) {
                self.report(e);
            }
            last = Instant::now();
            if housekeeping.elapsed() >= Duration::from_secs(1) {
                self.refresh_bad_files();
                if let Some(live) = &self.live
                    && live.worker.as_ref().is_some_and(JoinHandle::is_finished)
                {
                    let error = live
                        .state
                        .lock()
                        .unwrap()
                        .error
                        .clone()
                        .unwrap_or_else(|| "UDP receiver stopped".into());
                    self.live = None;
                    self.report(io::Error::other(error));
                }
                if self.live.is_none() {
                    match LiveUdp::bind(address(&self.status.config), &self.status.config.forward)
                        .and_then(|live| {
                            ReceiverWorker::start(
                                live,
                                self.recorder.clone(),
                                self.enabled.clone(),
                                self.tracks.clone(),
                                fast.clone(),
                            )
                        }) {
                        Ok(live) => {
                            self.live = Some(live);
                            self.status.ready = true;
                            self.status.error = None;
                            desktop::log(
                                &self.folder.paths,
                                &format!("Listening: UDP {}", self.status.config.udp.port),
                            );
                        }
                        Err(e) => {
                            self.status.ready = false;
                            self.report(io::Error::other(format!("UDP port {}: {e}. Start pitwall before the game or choose another port.", self.status.config.udp.port)));
                        }
                    }
                }
                if let Err(e) = self.reload_settings() {
                    self.report(io::Error::other(format!(
                        "Settings reload rejected; active settings retained: {e}"
                    )));
                }
                if let Ok(list) = recorder::list(&self.folder.paths.recordings) {
                    self.status.recordings = list;
                }
                self.status.recorder_dropped = self.recorder.lock().unwrap().dropped();
                if let Ok(pid) = sysinfo::get_current_pid() {
                    system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
                    if let Some(p) = system.process(pid) {
                        self.status.cpu = Some(p.cpu_usage());
                        self.status.memory = Some(p.memory());
                    }
                }
                if let Some(live) = &self.live {
                    let mut live = live.state.lock().unwrap();
                    let mut values = std::mem::take(&mut live.latency);
                    values.sort_by(f64::total_cmp);
                    let percentile = |p: f64| {
                        values
                            .get(((values.len() as f64 * p).ceil() as usize).saturating_sub(1))
                            .copied()
                    };
                    self.status.p95 = percentile(0.95);
                    self.status.p99 = percentile(0.99);
                }
                self.status.revision += 1;
                housekeeping = Instant::now();
            }
            let replay_active = self.replay.is_some() || self.status.loading.is_some();
            {
                let mut output = fast.lock().unwrap();
                if output.replay != replay_active {
                    output.replay = replay_active;
                    output.restore_live = !replay_active;
                }
            }
            if let Some(live) = &self.live {
                let live = live.state.lock().unwrap();
                self.status.live = live.status.clone();
                self.live_session = matches!(live.model.lifecycle(), Lifecycle::Live);
            }
            self.status.line = crate::status_view::connection_line(&self.status);
            if self.status.loading.is_none() {
                if let Some((replay, _)) = &self.replay {
                    let key = (
                        self.load_generation,
                        replay.position(),
                        replay.model().epoch(),
                    );
                    let new_source = source != Some(true);
                    let changed = published != Some(key);
                    if changed {
                        fast.lock().unwrap().writer.publish_fast(replay.model());
                        published = Some(key);
                    }
                    // A load or seek changes the model timeline immediately. Pending
                    // running data also flushes once after pause or end, then stays idle.
                    let timeline = new_source
                        || snapshotted.is_none_or(|old: (u64, Duration, u64)| {
                            old.0 != key.0 || old.2 != key.2
                        });
                    if snapshotted != Some(key)
                        && (timeline || last_snapshot.elapsed() >= Duration::from_millis(200))
                    {
                        publisher.publish_snapshot(replay.model());
                        last_snapshot = Instant::now();
                        snapshotted = Some(key);
                    }
                    self.status.track = track_of(replay.model());
                    source = Some(true);
                } else if let Some(live) = &self.live {
                    let live = live.state.lock().unwrap();
                    let key = (live.generation, Duration::ZERO, 0);
                    if source != Some(false) || published != Some(key) {
                        publisher.publish_snapshot(&live.model);
                        published = Some(key);
                    }
                    self.status.track = track_of(&live.model);
                    source = Some(false);
                } else if source == Some(true) {
                    let model = SessionModel::default();
                    fast.lock().unwrap().writer.publish_fast(&model);
                    publisher.publish_snapshot(&model);
                    source = None;
                    published = None;
                }
            }
            *state.lock().unwrap() = self.status.clone();
        }
        drop(self.live.take());
        self.replay = None;
        self.recorder.lock().unwrap().sync()?;
        desktop::log(&self.folder.paths, "Native host stopped");
        Ok(())
    }
}
/// The current track's ID and length, for Relearn map.
fn track_of(model: &SessionModel) -> Option<TrackKey> {
    match model.session() {
        Readout::Live(s) | Readout::Stale(s) if s.track_id >= 0 && s.track_length > 0 => {
            Some(TrackKey::new(s.track_id, s.track_length))
        }
        _ => None,
    }
}

fn address(config: &Config) -> SocketAddr {
    SocketAddr::from((Ipv4Addr::UNSPECIFIED, config.udp.port))
}
