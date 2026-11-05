//! Native pitwall application: in-process host, gpui windows and a Win32 overlay.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod dashboard;
mod source;
mod status;
mod theme;
mod tiles;
mod tray;

use crate::{
    dashboard::{Dashboard, Stats},
    source::Args,
    status::StatusWindow,
};
use gpui::{App, AppContext, Bounds, WindowBounds, WindowOptions, px, size};
use pitwall_core::{data::DataFolder, paths::AppPaths};
use pitwall_native::{
    desktop::{self, Instance},
    host::{Command, Control, Runtime},
};
use std::{
    borrow::Cow,
    process::ExitCode,
    sync::{Arc, atomic::Ordering},
    thread,
    time::{Duration, Instant},
};
const FONTS: [&[u8]; 3] = [
    include_bytes!("../assets/fonts/Oxanium-Regular.ttf"),
    include_bytes!("../assets/fonts/Oxanium-SemiBold.ttf"),
    include_bytes!("../assets/fonts/Oxanium-Bold.ttf"),
];

fn dashboard(
    cx: &mut App,
    reader: pitwall_core::feed::Reader,
    stats: Arc<Stats>,
    dimensions: (f32, f32),
) -> Result<(), String> {
    let bounds = Bounds::centered(None, size(px(dimensions.0), px(dimensions.1)), cx);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            inactive_frame_interval: None,
            low_power_frame_interval: None,
            thermal_frame_interval: None,
            ..Default::default()
        },
        |window, cx| {
            cx.new(|cx| {
                Dashboard::new(
                    reader,
                    "Native host (status and settings in tray)".into(),
                    stats,
                    window,
                    cx,
                )
            })
        },
    )
    .map(|_| ())
    .map_err(|e| format!("Dashboard: {e:#}"))
}
fn settings(cx: &mut App, control: Control, open_dashboard: Arc<dyn Fn(&mut App)>) {
    let bounds = Bounds::centered(None, size(px(850.0), px(850.0)), cx);
    if let Err(e) = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..Default::default()
        },
        |window, cx| cx.new(|cx| StatusWindow::new(control, open_dashboard, window, cx)),
    ) {
        eprintln!("Status window: {e:#}");
    }
}
fn main() -> ExitCode {
    let args = match Args::parse(std::env::args().skip(1)) {
        Ok(args) => args,
        Err(e) => {
            eprintln!("pitwall-native: {e}");
            return ExitCode::from(2);
        }
    };
    if args.cli.help {
        println!(
            "pitwall-native [--console] [--udp-port PORT] [--config PATH] [--data-dir DIR] [--replay FILE] [--from SECONDS] [--loop] [--size WxH] [--stats]"
        );
        return ExitCode::SUCCESS;
    }
    let paths = match AppPaths::resolve_or(args.cli.data_dir.as_deref()) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let _instance = match Instance::acquire(&paths) {
        Ok(Some(instance)) => instance,
        Ok(None) => return ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            return if e.kind() == std::io::ErrorKind::AlreadyExists {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            };
        }
    };
    let mut folder = match DataFolder::open(paths.clone(), &args.cli) {
        Ok(folder) => folder,
        Err(e) => {
            eprintln!("Data folder: {e}");
            desktop::log(&paths, &format!("Startup: {e}"));
            return ExitCode::FAILURE;
        }
    };
    folder.obsolete.extend(args.obsolete);
    for notice in folder.notices() {
        desktop::log(&paths, &notice);
    }
    let overlay = folder.overlay;
    let runtime = match Runtime::start(folder, args.cli.clone()) {
        Ok(runtime) => runtime,
        Err(e) => {
            eprintln!("Host: {e}");
            return ExitCode::FAILURE;
        }
    };
    let stats = Arc::new(Stats::default());
    if args.stats {
        print_stats(stats.clone());
    }
    let reader = runtime.reader.clone();
    let control = runtime.control.clone();
    let app_paths = paths.clone();
    let tray_reader = runtime.reader.clone();
    gpui_platform::application().run(move |cx: &mut App| {
        if let Err(e) = cx
            .text_system()
            .add_fonts(FONTS.iter().map(|f| Cow::Borrowed(*f)).collect())
        {
            desktop::log(&app_paths, &format!("Fonts: {e:#}"));
        }
        let first_run = !app_paths.root.join("native-opened").exists();
        let dashboard_paths = app_paths.clone();
        let dashboard_control = control.clone();
        let open_dashboard: Arc<dyn Fn(&mut App)> = Arc::new(move |cx| {
            match dashboard(cx, reader.clone(), stats.clone(), args.size) {
                Ok(()) => {
                    let _ = std::fs::write(dashboard_paths.root.join("native-opened"), b"opened");
                    cx.activate(true);
                }
                Err(e) => {
                    desktop::log(&dashboard_paths, &e);
                    let _ = notify_rust::Notification::new()
                        .summary("pitwall window failed")
                        .body(&e)
                        .show();
                }
            }
            let _ = &dashboard_control;
        });
        // First-run and debugging invocations open a dashboard; ordinary starts stay in the tray.
        if first_run || args.cli.console || args.stats || args.replay.is_some() {
            open_dashboard(cx);
        }
        if first_run {
            settings(cx, control.clone(), open_dashboard.clone());
        }
        let mut tray = match tray::Tray::new(tray_reader, app_paths.clone(), overlay) {
            Ok(tray) => Some(tray),
            Err(e) => {
                desktop::log(&app_paths, &format!("Tray: {e}"));
                settings(cx, control.clone(), open_dashboard.clone());
                None
            }
        };
        let executor = cx.background_executor().clone();
        cx.spawn(async move |cx| {
            let mut initial_replay = args.replay;
            let mut initial_seek = Some(args.from);
            loop {
                executor.timer(Duration::from_millis(100)).await;
                let state = control.status();
                if let Some(path) = initial_replay.take() {
                    // CLI replay may name an external recording. Copy it into this host's library;
                    // never modify the supplied recording.
                    let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
                    if let Some(name) = name {
                        let target = app_paths.recordings.join(&name);
                        let copied = if path == target || target.exists() {
                            Ok(())
                        } else {
                            std::fs::copy(&path, &target).map(|_| ())
                        };
                        if let Err(e) = copied {
                            desktop::log(&app_paths, &format!("Replay: {e}"));
                        } else {
                            let _ = control.send(Command::Start(name));
                        }
                    }
                }
                if state.replay.is_some()
                    && let Some(from) = initial_seek.take()
                {
                    let _ = control.send(Command::Seek(from.as_secs_f64()));
                }
                if args.looped
                    && state
                        .replay
                        .as_ref()
                        .is_some_and(|r| r.position >= r.duration)
                {
                    let _ = control.send(Command::Restart);
                }
                let requested = app_paths.root.join("native-open");
                let second_launch = requested.exists();
                if second_launch {
                    let _ = std::fs::remove_file(requested);
                }
                let actions = tray
                    .as_mut()
                    .map_or_else(Vec::new, |tray| tray.tick(&control));
                cx.update(|cx| {
                    if second_launch {
                        open_dashboard(cx);
                    }
                    for action in actions {
                        match action {
                            tray::Action::Dashboard => open_dashboard(cx),
                            tray::Action::Settings => {
                                settings(cx, control.clone(), open_dashboard.clone())
                            }
                            tray::Action::Quit => {
                                cx.quit();
                            }
                        }
                    }
                });
            }
        })
        .detach();
    });
    match runtime.shutdown() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            desktop::log(&paths, &format!("Shutdown: {e}"));
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn print_stats(stats: Arc<Stats>) {
    thread::spawn(move || {
        let start = Instant::now();
        let (mut renders, mut frames) = (0, 0);
        loop {
            thread::sleep(Duration::from_secs(1));
            let r = stats.renders.load(Ordering::Relaxed);
            let f = stats.frames.load(Ordering::Relaxed);
            println!(
                "t={:.0}s renders/s={} frames/s={}",
                start.elapsed().as_secs_f64(),
                r - renders,
                f - frames
            );
            (renders, frames) = (r, f);
        }
    });
}
