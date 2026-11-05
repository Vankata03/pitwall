//! Desktop lifecycle, shared by the tray and terminal entry points.
use crate::{
    ingest::config::{Cli, Config},
    paths::AppPaths,
};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

pub enum Launch {
    Primary(Instance),
    Existing(String),
}
pub struct Instance {
    _lock: File,
    endpoint: std::path::PathBuf,
}
impl Instance {
    pub fn acquire(path: &Path) -> io::Result<Launch> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        match file.try_lock() {
            Ok(()) => {
                let endpoint = path.with_extension("url");
                let _ = fs::remove_file(&endpoint);
                Ok(Launch::Primary(Self {
                    _lock: file,
                    endpoint,
                }))
            }
            Err(std::fs::TryLockError::WouldBlock) => {
                // The owner holds its lock before publishing. Never steal ownership.
                for _ in 0..50 {
                    // Windows exclusive locks also block reads: discovery is a separate file.
                    let url = fs::read_to_string(path.with_extension("url")).unwrap_or_default();
                    if url.starts_with("http://") && url.ends_with('\n') {
                        return Ok(Launch::Existing(url.trim().into()));
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(io::Error::other(
                    "existing pitwall is still starting; try again",
                ))
            }
            Err(std::fs::TryLockError::Error(e)) => Err(e),
        }
    }
    pub fn publish(&mut self, url: &str) -> io::Result<()> {
        let tmp = self.endpoint.with_extension("url.tmp");
        fs::write(&tmp, format!("{url}\n"))?;
        fs::rename(tmp, &self.endpoint)
    }
}
#[derive(Clone, Default)]
pub struct Control {
    pub stop: Arc<AtomicBool>,
    pub state: Arc<Mutex<DesktopState>>,
}
#[derive(Default)]
pub struct DesktopState {
    pub url: String,
    pub ready: bool,
    pub line: String,
}
impl Control {
    pub fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }
    pub fn request_stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}
pub fn dashboard_url(config: &Config) -> String {
    let ip = if config.http.bind.is_unspecified() {
        if config.http.bind.is_ipv4() {
            "127.0.0.1".parse().unwrap()
        } else {
            "::1".parse().unwrap()
        }
    } else {
        config.http.bind
    };
    format!("http://{}", std::net::SocketAddr::new(ip, config.http.port))
}
pub fn open(target: impl AsRef<std::ffi::OsStr>) -> io::Result<()> {
    #[cfg(windows)]
    let mut command = {
        let mut c = std::process::Command::new("rundll32.exe");
        c.arg("url.dll,FileProtocolHandler");
        c
    };
    #[cfg(target_os = "macos")]
    let mut command = std::process::Command::new("open");
    #[cfg(not(any(windows, target_os = "macos")))]
    let mut command = std::process::Command::new("xdg-open");
    command.arg(target).spawn().map(|_| ())
}
pub fn log(paths: &AppPaths, message: &str) {
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(paths.logs.join("collector.log"))
    {
        let _ = writeln!(file, "{:?} {message}", std::time::SystemTime::now());
    }
}
pub fn first_run(root: &Path) -> io::Result<bool> {
    match OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(root.join("browser-opened"))
    {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(false),
        Err(e) => Err(e),
    }
}
/// Startup-folder entry is opt-in and scoped to the current Windows user.
pub fn autostart(enabled: bool, cli: &Cli) -> io::Result<()> {
    #[cfg(windows)]
    {
        let root =
            std::env::var_os("APPDATA").ok_or_else(|| io::Error::other("APPDATA unavailable"))?;
        let path = std::path::PathBuf::from(root)
            .join("Microsoft/Windows/Start Menu/Programs/Startup/pitwall.vbs");
        if enabled {
            let exe = std::env::current_exe()?.display().to_string();
            let mut args = format!("\"{exe}\"");
            if let Some(path) = &cli.config_path {
                args.push_str(&format!(" --config \"{}\"", path.display()));
            }
            for (flag, value) in [
                ("--udp-port", cli.udp_port.map(|v| v.to_string())),
                ("--http-port", cli.http_port.map(|v| v.to_string())),
                ("--bind", cli.bind.map(|v| v.to_string())),
            ] {
                if let Some(value) = value {
                    args.push_str(&format!(" {flag} {value}"));
                }
            }
            fs::create_dir_all(path.parent().unwrap())?;
            fs::write(
                path,
                format!(
                    "CreateObject(\"WScript.Shell\").Run \"{}\", 0, False\r\n",
                    args.replace('"', "\"\"")
                ),
            )?;
        } else if path.exists() {
            fs::remove_file(path)?;
        }
    }
    #[cfg(not(windows))]
    let _ = (enabled, cli);
    Ok(())
}

pub fn run(cli: Cli, paths: AppPaths) -> io::Result<()> {
    fs::create_dir_all(&paths.logs)?;
    let mut instance = match Instance::acquire(&paths.root.join("instance"))? {
        Launch::Primary(i) => i,
        Launch::Existing(url) => return open(url),
    };
    let config = cli.load(&paths.config)?;
    if cli.config_path.is_none() && !paths.config.exists() {
        fs::write(
            &paths.config,
            toml::to_string_pretty(&config).map_err(io::Error::other)?,
        )?;
    }
    instance.publish(&dashboard_url(&config))?;
    let control = Control::default();
    let signal_control = control.clone();
    ctrlc::set_handler(move || signal_control.request_stop()).map_err(io::Error::other)?;
    let worker_control = control.clone();
    let worker_paths = paths.clone();
    let worker_cli = cli.clone();
    let worker = std::thread::spawn(move || {
        let mut last_error = String::new();
        while !worker_control.stopped() {
            if let Err(e) = crate::ingest::runtime::run_controlled(
                worker_cli.clone(),
                worker_paths.config.clone(),
                worker_control.clone(),
            ) {
                let message = e.to_string();
                {
                    let mut s = worker_control.state.lock().unwrap();
                    s.ready = false;
                    s.line = message.clone();
                }
                if message != last_error {
                    log(&worker_paths, &message);
                    if worker_cli.console {
                        eprintln!("{message}");
                    }
                    #[cfg(any(windows, target_os = "macos"))]
                    if !worker_cli.console {
                        let _ = notify_rust::Notification::new()
                            .summary("pitwall needs attention")
                            .body(&message)
                            .show();
                    }
                    last_error = message;
                }
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    });
    log(
        &paths,
        &format!("Starting build {}", env!("PITWALL_BUILD_ID")),
    );
    if cli.console {
        while !control.stopped() && !worker.is_finished() {
            tick(&control, &mut instance, &paths);
            std::thread::sleep(Duration::from_millis(250));
        }
    } else {
        #[cfg(any(windows, target_os = "macos"))]
        {
            let result = tray(&control, &mut instance, &paths, &cli);
            control.request_stop();
            if let Err(e) = result {
                let _ = worker.join();
                return Err(e);
            }
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        while !control.stopped() && !worker.is_finished() {
            tick(&control, &mut instance, &paths);
            std::thread::sleep(Duration::from_millis(250));
        }
    }
    control.request_stop();
    worker
        .join()
        .map_err(|_| io::Error::other("collector thread panicked"))?;
    Ok(())
}
fn tick(control: &Control, instance: &mut Instance, paths: &AppPaths) {
    let state = control.state.lock().unwrap();
    if !state.url.is_empty() {
        let existing = fs::read_to_string(paths.root.join("instance.url")).unwrap_or_default();
        if existing.trim() != state.url {
            let _ = instance.publish(&state.url);
        }
    }
    if state.ready {
        match first_run(&paths.root) {
            Ok(true) => {
                if let Err(e) = open(&state.url) {
                    log(paths, &format!("Browser: {e}"));
                    let _ = fs::remove_file(paths.root.join("browser-opened"));
                }
            }
            Err(e) => log(paths, &format!("First run: {e}")),
            _ => {}
        }
    }
}
#[cfg(any(windows, target_os = "macos"))]
fn tray(control: &Control, instance: &mut Instance, paths: &AppPaths, cli: &Cli) -> io::Result<()> {
    use tao::{
        event::Event,
        event_loop::{ControlFlow, EventLoopBuilder},
        platform::run_return::EventLoopExtRunReturn,
    };
    use tray_icon::{
        Icon, TrayIconBuilder,
        menu::{Menu, MenuEvent, MenuItem},
    };
    #[cfg(windows)]
    let mut event_loop =
        EventLoopBuilder::<crate::overlay::OverlayEvent>::with_user_event().build();
    #[cfg(not(windows))]
    let mut event_loop = EventLoopBuilder::new().build();
    #[cfg(target_os = "macos")]
    {
        use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
        event_loop.set_activation_policy(ActivationPolicy::Accessory);
    }
    let menu = Menu::new();
    let state = MenuItem::new("Waiting for game", false, None);
    let dashboard = MenuItem::new("Open dashboard", true, None);
    let qr = MenuItem::new("Show QR", true, None);
    let settings = MenuItem::new("Settings", true, None);
    let logs = MenuItem::new("Open log folder", true, None);
    let quit = MenuItem::new("Quit", true, None);
    menu.append_items(&[&state, &dashboard, &qr, &settings, &logs])
        .map_err(io::Error::other)?;
    #[cfg(windows)]
    let mut overlay = OverlayMenu::new(&menu, paths, event_loop.create_proxy())?;
    menu.append(&quit).map_err(io::Error::other)?;
    // Original geometric icon: two vertical timing bars, no third-party marks.
    let mut pixels = vec![0; 16 * 16 * 4];
    for y in 2..14 {
        for x in [4, 5, 10, 11] {
            let at = (y * 16 + x) * 4;
            pixels[at..at + 4].copy_from_slice(&[210, 210, 210, 255]);
        }
    }
    let icon = Icon::from_rgba(pixels, 16, 16).map_err(io::Error::other)?;
    let mut tray = None;
    let mut failure = None;
    event_loop.run_return(|event, _target, flow| {
        #[cfg(windows)]
        overlay.handle(&event);
        *flow = ControlFlow::WaitUntil(std::time::Instant::now() + Duration::from_millis(250));
        if matches!(event, Event::NewEvents(tao::event::StartCause::Init)) {
            match TrayIconBuilder::new()
                .with_menu(Box::new(menu.clone()))
                .with_icon(icon.clone())
                .with_tooltip("pitwall")
                .build()
            {
                Ok(value) => tray = Some(value),
                Err(e) => {
                    failure = Some(io::Error::other(e));
                    *flow = ControlFlow::Exit;
                    return;
                }
            }
        }
        if matches!(event, Event::MainEventsCleared) {
            if control.stopped() {
                *flow = ControlFlow::Exit;
                return;
            }
            tick(control, instance, paths);
            let s = control.state.lock().unwrap();
            #[cfg(windows)]
            overlay.sync(_target, &s, paths);
            state.set_text(if s.line.is_empty() {
                "Waiting for game"
            } else {
                &s.line
            });
            while let Ok(event) = MenuEvent::receiver().try_recv() {
                #[cfg(windows)]
                if overlay.menu_event(&event.id, _target, &s, paths) {
                    continue;
                }
                if event.id == quit.id() {
                    control.request_stop();
                    *flow = ControlFlow::Exit;
                } else {
                    let target = if event.id == logs.id() {
                        paths.logs.display().to_string()
                    } else if event.id == settings.id() && !s.ready {
                        cli.config_path
                            .as_ref()
                            .unwrap_or(&paths.config)
                            .display()
                            .to_string()
                    } else if event.id == settings.id() {
                        format!("{}/status", s.url)
                    } else if event.id == qr.id() {
                        format!("{}/status#discovery", s.url)
                    } else if event.id == dashboard.id() {
                        s.url.clone()
                    } else {
                        continue;
                    };
                    if let Err(e) = open(target) {
                        log(paths, &format!("Open: {e}"));
                    }
                }
            }
        }
    });
    #[cfg(windows)]
    overlay.close(paths);
    drop(tray);
    if let Some(e) = failure {
        Err(e)
    } else {
        Ok(())
    }
}

/// Tray entries and window of the in-game overlay. Windows only: WebView2 hosts the page.
#[cfg(windows)]
struct OverlayMenu {
    show: tray_icon::menu::CheckMenuItem,
    moving: tray_icon::menu::CheckMenuItem,
    settings: crate::overlay::OverlaySettings,
    window: Option<crate::overlay::Overlay>,
    proxy: tao::event_loop::EventLoopProxy<crate::overlay::OverlayEvent>,
    failed: bool,
}

#[cfg(windows)]
impl OverlayMenu {
    fn new(
        menu: &tray_icon::menu::Menu,
        paths: &AppPaths,
        proxy: tao::event_loop::EventLoopProxy<crate::overlay::OverlayEvent>,
    ) -> io::Result<Self> {
        use tray_icon::menu::{CheckMenuItem, PredefinedMenuItem};
        let settings = crate::overlay::OverlaySettings::load(&Self::file(paths));
        let show = CheckMenuItem::new("Throttle/brake overlay", true, settings.visible, None);
        let moving = CheckMenuItem::new("Move overlay", settings.visible, false, None);
        menu.append_items(&[
            &PredefinedMenuItem::separator(),
            &show,
            &moving,
            &PredefinedMenuItem::separator(),
        ])
        .map_err(io::Error::other)?;
        Ok(Self {
            show,
            moving,
            settings,
            window: None,
            proxy,
            failed: false,
        })
    }

    fn file(paths: &AppPaths) -> std::path::PathBuf {
        paths.root.join("overlay.json")
    }

    fn save(&mut self, paths: &AppPaths) {
        if let Some(window) = &self.window {
            self.settings.position = window.position().or(self.settings.position);
            self.settings.size = Some(window.size());
        }
        if let Err(e) = self.settings.save(&Self::file(paths)) {
            log(paths, &format!("Overlay settings: {e}"));
        }
    }

    /// Open the window once the dashboard is being served, and follow address changes.
    fn sync(
        &mut self,
        target: &tao::event_loop::EventLoopWindowTarget<crate::overlay::OverlayEvent>,
        state: &DesktopState,
        paths: &AppPaths,
    ) {
        if !self.settings.visible || !state.ready || state.url.is_empty() {
            return;
        }
        if let Some(window) = &mut self.window {
            if let Err(e) = window.set_base(&state.url) {
                log(paths, &format!("Overlay: {e}"));
            }
            return;
        }
        if self.failed {
            return;
        }
        match crate::overlay::Overlay::open(
            target,
            self.proxy.clone(),
            &state.url,
            self.settings,
            paths.root.join("webview"),
        ) {
            Ok(window) => self.window = Some(window),
            Err(e) => {
                // Usually a missing WebView2 runtime. Say so once instead of retrying every tick.
                self.failed = true;
                log(paths, &format!("Overlay: {e}"));
                let _ = notify_rust::Notification::new()
                    .summary("pitwall overlay could not open")
                    .body(&e.to_string())
                    .show();
            }
        }
    }

    /// Handle a tray click on an overlay entry. Returns false for other entries.
    fn menu_event(
        &mut self,
        id: &tray_icon::menu::MenuId,
        target: &tao::event_loop::EventLoopWindowTarget<crate::overlay::OverlayEvent>,
        state: &DesktopState,
        paths: &AppPaths,
    ) -> bool {
        if id == self.show.id() {
            let visible = self.show.is_checked();
            if !visible {
                self.save(paths);
                self.window = None;
                self.moving.set_checked(false);
            }
            self.settings.visible = visible;
            self.failed = false;
            self.moving.set_enabled(visible);
            self.save(paths);
            self.sync(target, state, paths);
            true
        } else if id == self.moving.id() {
            let moving = self.moving.is_checked();
            if let Some(Err(e)) = self.window.as_mut().map(|w| w.set_moving(moving)) {
                log(paths, &format!("Overlay: {e}"));
            }
            if !moving {
                self.save(paths);
            }
            true
        } else {
            false
        }
    }

    fn handle(&self, event: &tao::event::Event<crate::overlay::OverlayEvent>) {
        if let (tao::event::Event::UserEvent(event), Some(window)) = (event, &self.window) {
            window.handle(*event);
        }
    }

    fn close(&mut self, paths: &AppPaths) {
        if self.window.is_some() {
            self.save(paths);
        }
        self.window = None;
    }
}
