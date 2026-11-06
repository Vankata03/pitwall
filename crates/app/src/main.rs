//! pitwall-native: the native app. One process owns the data folder and runs the host
//! (live UDP, recorder, replay) in-process; gpui windows show the dashboard and the
//! status/settings page, the tray stays resident and a Win32 overlay draws in game.
//!
//! Startup goes through `pitwall_core::data::DataFolder`, so existing data folders load
//! unchanged and obsolete `--http-port`/`--bind` flags never stop startup.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod dashboard;
mod source;
mod status;
mod theme;
mod tiles;
mod tray;
mod windows;

use crate::{dashboard::Stats, source::Args, status::StatusWindow, windows::Windows};
use gpui::{App, AppContext, Bounds, WindowBounds, WindowOptions, px, size};
use pitwall_core::{data::DataFolder, paths::AppPaths};
use pitwall_native::{
    desktop::{self, FirstRun, Instance, Launch},
    host::{Command, Control, Desktop, Runtime},
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

fn open_status(cx: &mut App, control: Control, dashboard: Arc<dyn Fn(&mut App)>) {
    let bounds = Bounds::centered(None, size(px(860.0), px(860.0)), cx);
    if let Err(e) = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..Default::default()
        },
        |window, cx| cx.new(|cx| StatusWindow::new(control, dashboard, window, cx)),
    ) {
        eprintln!("pitwall-native: status window: {e:#}");
    }
}

/// Report a failure that stops startup: stderr, the log and a notification, since a
/// release build has no console.
fn fatal(paths: Option<&AppPaths>, message: &str) -> ExitCode {
    eprintln!("pitwall-native: {message}");
    if let Some(paths) = paths {
        desktop::log(paths, &format!("Startup failed: {message}"));
    }
    desktop::notify("pitwall could not start", message);
    ExitCode::FAILURE
}

fn main() -> ExitCode {
    let args = match Args::parse(std::env::args().skip(1)) {
        Ok(args) => args,
        Err(error) => {
            desktop::attach_console();
            eprintln!("pitwall-native: {error}");
            return ExitCode::from(2);
        }
    };
    if args.cli.console || args.cli.help {
        desktop::attach_console();
    }
    if args.cli.help {
        println!("{}", source::HELP);
        return ExitCode::SUCCESS;
    }
    let paths = match AppPaths::resolve_or(args.cli.data_dir.as_deref()) {
        Ok(paths) => paths,
        Err(error) => return fatal(None, &error.to_string()),
    };
    let _ = tiles::car_status::LIVERY_FOLDER.set(paths.liveries.clone());
    let instance = match Instance::acquire(&paths) {
        Ok(Launch::Primary(instance)) => instance,
        Ok(Launch::Existing) => {
            eprintln!(
                "pitwall-native: pitwall is already running for {}; asked it to open its dashboard (it may still be starting)",
                paths.root.display()
            );
            return ExitCode::SUCCESS;
        }
        Err(error) => return fatal(Some(&paths), &format!("instance lock: {error}")),
    };
    let mut folder = match DataFolder::open(paths.clone(), &args.cli) {
        Ok(folder) => folder,
        Err(error) => return fatal(Some(&paths), &format!("data folder: {error}")),
    };
    if !args.obsolete.is_empty() {
        // DataFolder logged the file's obsolete keys; the native parser kept the flags.
        desktop::log(
            &paths,
            &format!("Ignored obsolete flags: {}", args.obsolete.join(", ")),
        );
        folder.obsolete.extend(args.obsolete.iter().cloned());
    }
    let overlay = folder.overlay;
    // Every dashboard window shares this one library.
    let layouts = folder.layouts.clone();
    let runtime = match Runtime::start(folder, args.cli.clone(), Desktop::user()) {
        Ok(runtime) => runtime,
        Err(error) => return fatal(Some(&paths), &format!("host: {error}")),
    };
    let stats = Arc::new(Stats::default());
    if args.stats {
        print_stats(stats.clone());
    }
    if let Some(path) = args.replay.clone() {
        let _ = runtime.control.send(Command::StartFile(path));
    }

    let reader = runtime.reader.clone();
    let control = runtime.control.clone();
    let app_paths = paths.clone();
    gpui_platform::application().run(move |cx: &mut App| {
        if let Err(error) = cx
            .text_system()
            .add_fonts(FONTS.iter().map(|f| Cow::Borrowed(*f)).collect())
        {
            desktop::log(&app_paths, &format!("Fonts: {error:#}"));
        }
        let first_run = FirstRun::new(&app_paths);
        // Every dashboard and stream window shares one layout library and one window list.
        let windows = Windows::load(
            reader.clone(),
            stats.clone(),
            layouts.clone(),
            app_paths.clone(),
            control.clone(),
            args.size,
        );
        let dashboard: Arc<dyn Fn(&mut App)> = {
            let (windows, paths) = (windows.clone(), app_paths.clone());
            let first_run = FirstRun::new(&app_paths);
            Arc::new(move |cx| {
                // A failure is logged and notified where the window is opened.
                if windows.open_dashboard(cx).is_ok() {
                    if first_run.pending()
                        && let Err(error) = first_run.opened()
                    {
                        desktop::log(&paths, &format!("First run: {error}"));
                    }
                    cx.activate(true);
                }
            })
        };
        // The windows open at the last quit come back, each on its monitor. Otherwise only
        // the first successful run and debugging starts open a window; ordinary starts
        // stay in the tray.
        let debugging = args.cli.console || args.stats || args.replay.is_some();
        if !windows.restore(cx) && (first_run.pending() || debugging) {
            dashboard(cx);
        }
        let mut tray = match tray::Tray::new(reader.clone(), app_paths.clone(), overlay) {
            Ok(tray) => Some(tray),
            Err(error) => {
                // Without a tray the status window is the recovery path.
                desktop::log(&app_paths, &format!("Tray: {error}"));
                open_status(cx, control.clone(), dashboard.clone());
                None
            }
        };
        if tray.is_none() {
            let windows = windows.clone();
            cx.on_window_closed(move |cx, _| {
                if cx.windows().is_empty() {
                    windows.quit();
                    cx.quit();
                }
            })
            .detach();
        }
        let executor = cx.background_executor().clone();
        let control = control.clone();
        let looped = args.looped.then_some(args.from);
        let mut seek = (!args.from.is_zero()).then_some(args.from);
        cx.spawn(async move |cx| {
            loop {
                executor.timer(Duration::from_millis(100)).await;
                let state = control.status();
                if let Some(replay) = &state.replay {
                    if let Some(from) = seek.take() {
                        let _ =
                            control.send(Command::Seek(from.as_secs_f64().min(replay.duration)));
                    } else if let Some(from) = looped
                        && replay.position >= replay.duration
                    {
                        let _ =
                            control.send(Command::Seek(from.as_secs_f64().min(replay.duration)));
                        let _ = control.send(Command::Pause(false));
                    }
                }
                let second_launch = instance.take_open_request();
                let actions = tray
                    .as_mut()
                    .map_or_else(Vec::new, |tray| tray.tick(&control));
                let quit = actions.iter().any(|a| matches!(a, tray::Action::Quit));
                windows.save_if_due();
                if quit {
                    // The windows open now come back on the next start.
                    windows.quit();
                    // Dropping the tray closes the overlay and saves its geometry.
                    drop(tray.take());
                }
                cx.update(|cx| {
                    if second_launch {
                        dashboard(cx);
                    }
                    for action in actions {
                        match action {
                            tray::Action::Dashboard => dashboard(cx),
                            tray::Action::Settings => {
                                open_status(cx, control.clone(), dashboard.clone())
                            }
                            tray::Action::Quit => cx.quit(),
                        }
                    }
                });
                if quit {
                    break;
                }
            }
        })
        .detach();
    });
    match runtime.shutdown() {
        Ok(()) => {
            desktop::log(&paths, "Shut down");
            ExitCode::SUCCESS
        }
        Err(error) => {
            desktop::log(&paths, &format!("Shutdown: {error}"));
            eprintln!("pitwall-native: {error}");
            ExitCode::FAILURE
        }
    }
}

/// One line per second: renders (frames with new content) and display frames checked.
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
