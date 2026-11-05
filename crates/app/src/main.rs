//! pitwall-native: the first native slice (#67). One gpui window running the core
//! in-process, fed by live UDP or a `.pitrec` replay, with the Speed/RPM/gear and Lap
//! timing tiles.
//!
//! Usage: `pitwall-native [--port N] [--replay FILE [--from SECONDS] [--loop]]
//! [--size WxH] [--stats]`. Without `--replay` it listens for live UDP (default 20777).

mod dashboard;
mod source;
mod theme;
mod tiles;

use std::{
    borrow::Cow,
    path::PathBuf,
    process::ExitCode,
    sync::{Arc, atomic::Ordering},
    thread,
    time::{Duration, Instant},
};

use gpui::{App, AppContext, Bounds, WindowBounds, WindowOptions, px, size};

use crate::{
    dashboard::{Dashboard, Stats},
    source::Source,
};

struct Args {
    source: Source,
    size: (f32, f32),
    stats: bool,
}

fn parse(mut args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut port = 20777;
    let mut replay = None;
    let mut from = Duration::ZERO;
    let mut looped = false;
    let mut size = (900.0, 480.0);
    let mut stats = false;
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--port" => port = value()?.parse().map_err(|e| format!("--port: {e}"))?,
            "--replay" => replay = Some(PathBuf::from(value()?)),
            "--from" => {
                from =
                    Duration::from_secs_f64(value()?.parse().map_err(|e| format!("--from: {e}"))?)
            }
            "--loop" => looped = true,
            "--stats" => stats = true,
            "--size" => {
                let text = value()?;
                let (w, h) = text.split_once('x').ok_or("--size takes WIDTHxHEIGHT")?;
                size = (
                    w.parse().map_err(|e| format!("--size: {e}"))?,
                    h.parse().map_err(|e| format!("--size: {e}"))?,
                );
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(Args {
        source: match replay {
            Some(path) => Source::Replay { path, from, looped },
            None => Source::Live { port },
        },
        size,
        stats,
    })
}

const FONTS: [&[u8]; 3] = [
    include_bytes!("../assets/fonts/Oxanium-Regular.ttf"),
    include_bytes!("../assets/fonts/Oxanium-SemiBold.ttf"),
    include_bytes!("../assets/fonts/Oxanium-Bold.ttf"),
];

fn main() -> ExitCode {
    let args = match parse(std::env::args().skip(1)) {
        Ok(args) => args,
        Err(error) => {
            eprintln!("pitwall-native: {error}");
            return ExitCode::from(2);
        }
    };
    let (publisher, reader) = pitwall_core::feed::channel();
    let label = args.source.label();
    let worker = match args.source.spawn(publisher) {
        Ok(worker) => worker,
        Err(error) => {
            eprintln!("pitwall-native: {error}");
            return ExitCode::FAILURE;
        }
    };
    let stats = Arc::new(Stats::default());
    if args.stats {
        print_stats(stats.clone());
    }
    thread::spawn(move || {
        if let Ok(Err(error)) = worker.join() {
            eprintln!("pitwall-native: source stopped: {error}");
        }
    });

    gpui_platform::application().run(move |cx: &mut App| {
        if let Err(error) = cx
            .text_system()
            .add_fonts(FONTS.iter().map(|f| Cow::Borrowed(*f)).collect())
        {
            eprintln!("pitwall-native: fonts: {error:#}");
        }
        let bounds = Bounds::centered(None, size(px(args.size.0), px(args.size.1)), cx);
        let opened = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                // A dashboard on a second screen is inactive while the game has focus:
                // never throttle it.
                inactive_frame_interval: None,
                low_power_frame_interval: None,
                thermal_frame_interval: None,
                ..Default::default()
            },
            |window, cx| cx.new(|cx| Dashboard::new(reader, label, stats, window, cx)),
        );
        if let Err(error) = opened {
            eprintln!("pitwall-native: window: {error:#}");
            cx.quit();
            return;
        }
        cx.on_window_closed(|cx, _| cx.quit()).detach();
        cx.activate(true);
    });
    ExitCode::SUCCESS
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
