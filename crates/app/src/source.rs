//! Where the session model comes from: live UDP or a `.pitrec` replay, each driven on its
//! own thread and published to the UI through `pitwall_core::feed`.

use std::{
    fs::File,
    io,
    net::{Ipv4Addr, SocketAddr},
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};

use pitwall_core::{
    feed::Publisher,
    ingest::{Disposition, config::ForwardConfig, udp::LiveUdp},
    model::SessionModel,
    replay::Replay,
};

/// Slow-state publishes at most this often. The Lap timing clock reads the snapshot,
/// so this is also its refresh ceiling.
const SNAPSHOT_INTERVAL: Duration = Duration::from_nanos(16_666_667);
/// Without input, publish anyway this often so staleness (time-driven) shows.
const IDLE_PUBLISH: Duration = Duration::from_millis(250);
const WAKE_TIMEOUT: Duration = Duration::from_millis(100);
/// Replay ticks at the game's frame rate, so the UI redraws at most 60 times a second.
const REPLAY_TICK: Duration = Duration::from_nanos(16_666_667);

#[derive(Debug, Clone)]
pub enum Source {
    Live {
        port: u16,
    },
    Replay {
        path: PathBuf,
        from: Duration,
        looped: bool,
    },
}

impl Source {
    pub fn label(&self) -> String {
        match self {
            Self::Live { port } => format!("Live UDP :{port}"),
            Self::Replay { path, .. } => format!(
                "Replay {}",
                path.file_name().map_or_else(
                    || path.display().to_string(),
                    |n| n.to_string_lossy().into_owned()
                )
            ),
        }
    }

    /// Start the source thread. Errors before the first publish (bind, open) return here.
    pub fn spawn(self, publisher: Publisher) -> io::Result<thread::JoinHandle<io::Result<()>>> {
        match self {
            Self::Live { port } => {
                let live = LiveUdp::bind(
                    SocketAddr::from((Ipv4Addr::UNSPECIFIED, port)),
                    &ForwardConfig::default(),
                )?;
                thread::Builder::new()
                    .name("pitwall-live".into())
                    .spawn(move || run_live(live, publisher))
            }
            Self::Replay { path, from, looped } => {
                let mut replay = Replay::load(File::open(&path)?, |_| {})?;
                replay.seek(from.min(replay_duration(&replay)))?;
                thread::Builder::new()
                    .name("pitwall-replay".into())
                    .spawn(move || run_replay(replay, publisher, from, looped))
            }
        }
    }
}

fn replay_duration(replay: &Replay) -> Duration {
    Duration::from_secs_f64(replay.state(None).duration)
}

struct Throttle {
    last_snapshot: Option<Instant>,
}

impl Throttle {
    fn publish(&mut self, publisher: &mut Publisher, model: &SessionModel, had_input: bool) {
        let now = Instant::now();
        if had_input {
            publisher.publish_fast(model);
        }
        let due = self.last_snapshot.is_none_or(|last| {
            now - last
                >= if had_input {
                    SNAPSHOT_INTERVAL
                } else {
                    IDLE_PUBLISH
                }
        });
        if due {
            if !had_input {
                // Time-driven staleness reaches the fast path too.
                publisher.publish_fast(model);
            }
            publisher.publish_snapshot(model);
            self.last_snapshot = Some(now);
        }
    }
}

fn run_live(mut live: LiveUdp, mut publisher: Publisher) -> io::Result<()> {
    let origin = Instant::now();
    let now = || origin.elapsed().as_micros().min(u128::from(u64::MAX)) as u64;
    let mut model = SessionModel::default();
    let mut throttle = Throttle {
        last_snapshot: None,
    };
    let mut tap = |_: u64, _: &[u8]| {};
    loop {
        let received = live.poll(256, now, &mut tap)?;
        let had_input = !received.is_empty();
        for received in received {
            if let Ok((parsed, Disposition::Apply)) = received.result {
                model.apply(Duration::from_micros(received.arrival_time_us), &parsed);
            }
        }
        model.advance(Duration::from_micros(now()));
        throttle.publish(&mut publisher, &model, had_input);
        // Wake on arrival; the timeout only bounds idle staleness publishes.
        live.wait(WAKE_TIMEOUT)?;
    }
}

fn run_replay(
    mut replay: Replay,
    mut publisher: Publisher,
    from: Duration,
    looped: bool,
) -> io::Result<()> {
    let duration = replay_duration(&replay);
    let mut next = Instant::now();
    let mut last = Instant::now();
    let mut published = None;
    loop {
        let now = Instant::now();
        replay.tick(now - last)?;
        last = now;
        // A paused or finished replay publishes nothing, so the UI stops redrawing.
        if published != Some(replay.position()) {
            publisher.publish_fast(replay.model());
            publisher.publish_snapshot(replay.model());
            published = Some(replay.position());
        }
        if replay.position() >= duration && looped {
            replay.seek(from.min(duration))?;
            replay.pause(false);
        }
        next += REPLAY_TICK;
        let now = Instant::now();
        if next > now {
            thread::sleep(next - now);
        } else {
            next = now;
        }
    }
}
