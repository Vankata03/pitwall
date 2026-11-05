//! Run the native overlay standalone against a `.pitrec` recording, and report what it
//! costs. The recording is opened read-only; `overlay.json` is only written with
//! `--settings`.
//!
//! ```text
//! pitwall-overlay-replay <recording.pitrec> [--start SECONDS] [--seconds SECONDS]
//!     [--speed 0.5|1|2|4] [--move] [--settings PATH] [--no-overlay]
//! ```

#[cfg(not(windows))]
fn main() {
    eprintln!("The native overlay is Windows only.");
    std::process::exit(2);
}

#[cfg(windows)]
fn main() {
    if let Err(error) = windows_main::run() {
        eprintln!("pitwall-overlay-replay: {error}");
        std::process::exit(1);
    }
}

#[cfg(windows)]
mod windows_main {
    use pitwall_core::{feed, replay::Replay};
    use pitwall_overlay::settings::{Loaded, OverlaySettings};
    use std::{
        fs::File,
        io,
        path::PathBuf,
        time::{Duration, Instant},
    };
    use windows::Win32::{
        Foundation::FILETIME,
        System::{
            ProcessStatus::{
                GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX,
            },
            Threading::{GetCurrentProcess, GetProcessTimes},
        },
    };

    struct Args {
        recording: PathBuf,
        start: f64,
        seconds: Option<f64>,
        speed: f64,
        moving: bool,
        settings: Option<PathBuf>,
        overlay: bool,
    }

    fn args() -> io::Result<Args> {
        let usage = || {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "usage: pitwall-overlay-replay <recording.pitrec> [--start S] [--seconds S] \
                 [--speed X] [--move] [--settings PATH] [--no-overlay]",
            )
        };
        let mut parsed = Args {
            recording: PathBuf::new(),
            start: 0.0,
            seconds: None,
            speed: 1.0,
            moving: false,
            settings: None,
            overlay: true,
        };
        let mut it = std::env::args().skip(1);
        let number = |value: Option<String>| -> io::Result<f64> {
            value.and_then(|v| v.parse().ok()).ok_or_else(usage)
        };
        while let Some(arg) = it.next() {
            match arg.as_str() {
                "--start" => parsed.start = number(it.next())?,
                "--seconds" => parsed.seconds = Some(number(it.next())?),
                "--speed" => parsed.speed = number(it.next())?,
                "--move" => parsed.moving = true,
                "--no-overlay" => parsed.overlay = false,
                "--settings" => parsed.settings = Some(it.next().ok_or_else(usage)?.into()),
                other if !other.starts_with("--") && parsed.recording.as_os_str().is_empty() => {
                    parsed.recording = other.into();
                }
                _ => return Err(usage()),
            }
        }
        if parsed.recording.as_os_str().is_empty() {
            return Err(usage());
        }
        if !parsed.start.is_finite()
            || parsed.start < 0.0
            || parsed.seconds.is_some_and(|s| !s.is_finite() || s <= 0.0)
        {
            return Err(usage());
        }
        Ok(parsed)
    }

    fn process_cpu() -> Duration {
        let (mut created, mut exited, mut kernel, mut user) = Default::default();
        // SAFETY: the current-process pseudo handle and local out-pointers.
        let _ = unsafe {
            GetProcessTimes(
                GetCurrentProcess(),
                &mut created,
                &mut exited,
                &mut kernel,
                &mut user,
            )
        };
        let ticks = |t: FILETIME| u64::from(t.dwHighDateTime) << 32 | u64::from(t.dwLowDateTime);
        Duration::from_nanos((ticks(kernel) + ticks(user)) * 100)
    }

    /// Private bytes and working set of this process.
    fn memory() -> (usize, usize) {
        let mut counters = PROCESS_MEMORY_COUNTERS_EX {
            cb: size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
            ..Default::default()
        };
        // SAFETY: a sized counters struct for the current process.
        let _ = unsafe {
            GetProcessMemoryInfo(
                GetCurrentProcess(),
                &mut counters as *mut _ as *mut PROCESS_MEMORY_COUNTERS,
                counters.cb,
            )
        };
        (counters.PrivateUsage, counters.WorkingSetSize)
    }

    const MB: f64 = 1024.0 * 1024.0;

    pub fn run() -> io::Result<()> {
        let args = args()?;
        let loaded = Instant::now();
        let mut replay = Replay::load(File::open(&args.recording)?, |_| {})?;
        replay.set_speed(args.speed)?;
        replay.seek(Duration::from_secs_f64(args.start))?;
        eprintln!(
            "Loaded {} in {:.1} s",
            args.recording.display(),
            loaded.elapsed().as_secs_f64()
        );
        let settings = match &args.settings {
            Some(path) => match OverlaySettings::load(path) {
                Loaded::Malformed { kept, error, .. } => {
                    eprintln!(
                        "overlay.json was unreadable ({error}); kept as {}",
                        kept.display()
                    );
                    OverlaySettings::default()
                }
                loaded => loaded.settings(),
            },
            None => OverlaySettings::default(),
        };

        let (mut publisher, reader) = feed::channel();
        publisher.publish_snapshot(replay.model());
        publisher.publish_fast(replay.model());
        let (before_private, _) = memory();
        let overlay = if args.overlay {
            let overlay = pitwall_overlay::spawn(reader, settings, args.settings.clone())?;
            overlay.set_moving(args.moving);
            Some(overlay)
        } else {
            None
        };

        // The ingest side: advance the replay at 60 Hz, publish every tick, and publish
        // the slow snapshot at 10 Hz.
        let tick = Duration::from_secs_f64(1.0 / 60.0);
        let started = Instant::now();
        let cpu_start = process_cpu();
        let overlay_cpu_start = overlay.as_ref().map_or(Duration::ZERO, |o| o.cpu_time());
        let mut last = started;
        let mut next = started;
        let mut ticks = 0u64;
        let mut report = started + Duration::from_secs(10);
        let mut peak_private = 0;
        loop {
            next += tick;
            if let Some(wait) = next.checked_duration_since(Instant::now()) {
                std::thread::sleep(wait);
            }
            let now = Instant::now();
            replay.tick(now - last)?;
            last = now;
            publisher.publish_fast(replay.model());
            if ticks.is_multiple_of(6) {
                publisher.publish_snapshot(replay.model());
            }
            ticks += 1;
            peak_private = peak_private.max(memory().0);
            if now >= report {
                report += Duration::from_secs(10);
                print_stats(
                    &overlay,
                    started,
                    cpu_start,
                    overlay_cpu_start,
                    before_private,
                    peak_private,
                );
            }
            let done = replay.state(None).paused;
            if done
                || args
                    .seconds
                    .is_some_and(|s| started.elapsed().as_secs_f64() >= s)
            {
                break;
            }
        }
        print_stats(
            &overlay,
            started,
            cpu_start,
            overlay_cpu_start,
            before_private,
            peak_private,
        );
        if let Some(overlay) = overlay {
            let saved = overlay.close();
            eprintln!("Final geometry: {saved:?}");
        }
        Ok(())
    }

    fn print_stats(
        overlay: &Option<pitwall_overlay::Overlay>,
        started: Instant,
        cpu_start: Duration,
        overlay_cpu_start: Duration,
        before_private: usize,
        peak_private: usize,
    ) {
        let wall = started.elapsed().as_secs_f64();
        let process = (process_cpu() - cpu_start).as_secs_f64() / wall * 100.0;
        let (private, working) = memory();
        let thread = overlay
            .as_ref()
            .map(|o| {
                format!(
                    "{:.2}",
                    o.cpu_time().saturating_sub(overlay_cpu_start).as_secs_f64() / wall * 100.0
                )
            })
            .unwrap_or_else(|| "-".into());
        println!(
            "{wall:.0} s: overlay thread {thread} % of one core, process {process:.2} %; \
             private {:.1} MiB (peak {:.1}, {:+.1} since the overlay opened), working set {:.1} MiB",
            private as f64 / MB,
            peak_private as f64 / MB,
            (private as f64 - before_private as f64) / MB,
            working as f64 / MB,
        );
        if let Some(overlay) = overlay {
            let metrics = overlay.metrics();
            println!(
                "{} presented frames ({:.1} fps), {} render failures",
                metrics.frames,
                metrics.frames as f64 / wall,
                metrics.failures
            );
        }
    }
}
