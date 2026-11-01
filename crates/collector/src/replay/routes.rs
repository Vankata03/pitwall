use super::Replay;
use crate::{
    http::{Access, HttpShell},
    recorder::{LoadedGuard, Recorder},
    wire,
};
use axum::{Json, extract::State, http::StatusCode, routing::post};
use serde::Deserialize;
use std::{
    fs::File,
    io,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

struct Active {
    replay: Replay,
    _guard: LoadedGuard,
}
struct Inner {
    active: Option<Active>,
    loading: bool,
    progress: f64,
    generation: u64,
    reset: bool,
    anchor: Instant,
    error: Option<String>,
}
#[derive(Clone)]
pub struct Controller {
    dir: PathBuf,
    recorder: Arc<Mutex<Recorder>>,
    inner: Arc<Mutex<Inner>>,
}
impl Controller {
    pub fn new(dir: PathBuf, recorder: Arc<Mutex<Recorder>>) -> Self {
        Self {
            dir,
            recorder,
            inner: Arc::new(Mutex::new(Inner {
                active: None,
                loading: false,
                progress: 0.0,
                generation: 0,
                reset: false,
                anchor: Instant::now(),
                error: None,
            })),
        }
    }
    /// Publish the selected model. Live ingest is owned by a different worker and never stops.
    pub fn publish(
        &self,
        transport: Duration,
        live: &crate::model::SessionModel,
        stream: &wire::Stream,
        mut context: wire::Context,
    ) {
        let mut i = self.inner.lock().unwrap();
        if let Err(error) = settle(&mut i) {
            i.error = Some(error.to_string());
            if let Some(a) = &mut i.active {
                a.replay.pause(true);
            }
        }
        if i.reset {
            stream.reset();
            i.reset = false;
        }
        let background = matches!(live.lifecycle(), crate::model::Lifecycle::Live)
            .then(|| "A live session is recording in the background".into());
        if let Some(a) = &i.active {
            let mut state = a.replay.state(background);
            if let Some(error) = &i.error {
                state.banner = format!("Replay error: {error}");
            }
            context.replay = Some(state);
            stream.tick(transport, wire::Source::Replay, a.replay.model(), context);
        } else if i.loading || i.error.is_some() {
            context.replay = Some(wire::ReplayState {
                position: 0.0,
                speed: 1.0,
                paused: true,
                banner: i.error.clone().unwrap_or_else(|| "Loading replay".into()),
                load_progress: i.progress,
                background_live: background,
            });
            stream.tick(
                transport,
                wire::Source::Replay,
                &crate::model::SessionModel::default(),
                context,
            );
        } else {
            stream.tick(transport, wire::Source::Live, live, context);
        }
    }
    /// Current transport state for the status page, including load failures.
    pub fn status(&self) -> Option<wire::ReplayState> {
        let i = self.inner.lock().unwrap();
        if let Some(a) = &i.active {
            let mut state = a.replay.state(None);
            if let Some(error) = &i.error {
                state.banner = error.clone();
            }
            Some(state)
        } else if i.loading || i.error.is_some() {
            Some(wire::ReplayState {
                position: 0.0,
                speed: 1.0,
                paused: true,
                banner: i.error.clone().unwrap_or_else(|| "Loading replay".into()),
                load_progress: i.progress,
                background_live: None,
            })
        } else {
            None
        }
    }
    async fn start(&self, name: String) -> Result<(), StatusCode> {
        if name.is_empty()
            || name.contains(['/', '\\', ':'])
            || !name.ends_with(".pitrec")
            || name.starts_with('.')
        {
            return Err(StatusCode::BAD_REQUEST);
        }
        // Reject symlinks as well as path traversal before acquiring a loaded guard.
        let path = self.dir.join(&name);
        let metadata = std::fs::symlink_metadata(&path).map_err(status)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(StatusCode::BAD_REQUEST);
        }
        let guard = self.recorder.lock().unwrap().mark_loaded(&name);
        let file = File::open(path).map_err(status)?;
        let generation = {
            let mut i = self.inner.lock().unwrap();
            if i.loading {
                return Err(StatusCode::CONFLICT);
            }
            i.generation += 1;
            i.loading = true;
            i.progress = 0.0;
            i.error = None;
            i.active = None;
            i.reset = true;
            i.generation
        };
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let result = Replay::load(file, |progress| {
                let mut i = inner.lock().unwrap();
                if i.generation == generation {
                    i.progress = progress;
                }
            });
            let mut i = inner.lock().unwrap();
            if i.generation != generation {
                return;
            }
            i.loading = false;
            i.anchor = Instant::now();
            i.reset = true;
            match result {
                Ok(replay) => {
                    i.active = Some(Active {
                        replay,
                        _guard: guard,
                    })
                }
                Err(e) => i.error = Some(format!("Replay load failed: {e}")),
            }
        });
        Ok(())
    }
}
fn status(e: io::Error) -> StatusCode {
    match e.kind() {
        io::ErrorKind::NotFound => StatusCode::NOT_FOUND,
        io::ErrorKind::InvalidInput | io::ErrorKind::InvalidData => StatusCode::BAD_REQUEST,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
fn settle(i: &mut Inner) -> io::Result<()> {
    let now = Instant::now();
    let elapsed = now.saturating_duration_since(i.anchor);
    i.anchor = now;
    if let Some(a) = &mut i.active {
        a.replay.tick(elapsed)?;
    }
    Ok(())
}
#[derive(Deserialize)]
struct Start {
    name: String,
}
#[derive(Deserialize)]
struct Pause {
    paused: bool,
}
#[derive(Deserialize)]
struct Speed {
    speed: f64,
}
#[derive(Deserialize)]
struct Seek {
    position: f64,
}
#[derive(Deserialize)]
struct Lap {
    car: usize,
    lap: u8,
}
async fn start(
    State(c): State<Controller>,
    Json(body): Json<Start>,
) -> Result<StatusCode, StatusCode> {
    c.start(body.name).await?;
    Ok(StatusCode::ACCEPTED)
}
async fn stop(State(c): State<Controller>) -> StatusCode {
    let mut i = c.inner.lock().unwrap();
    i.generation += 1;
    i.loading = false;
    i.active = None;
    i.error = None;
    i.reset = true;
    StatusCode::NO_CONTENT
}
fn change(
    c: Controller,
    f: impl FnOnce(&mut Replay) -> io::Result<()>,
    reset: bool,
) -> Result<StatusCode, StatusCode> {
    let mut i = c.inner.lock().unwrap();
    settle(&mut i).map_err(status)?;
    let active = i.active.as_mut().ok_or(StatusCode::CONFLICT)?;
    f(&mut active.replay).map_err(status)?;
    i.reset |= reset;
    Ok(StatusCode::NO_CONTENT)
}
async fn pause(
    State(c): State<Controller>,
    Json(b): Json<Pause>,
) -> Result<StatusCode, StatusCode> {
    change(
        c,
        |r| {
            r.pause(b.paused);
            Ok(())
        },
        false,
    )
}
async fn speed(
    State(c): State<Controller>,
    Json(b): Json<Speed>,
) -> Result<StatusCode, StatusCode> {
    change(c, |r| r.set_speed(b.speed), false)
}
async fn seek(State(c): State<Controller>, Json(b): Json<Seek>) -> Result<StatusCode, StatusCode> {
    let at = Duration::try_from_secs_f64(b.position).map_err(|_| StatusCode::BAD_REQUEST)?;
    change(c, |r| r.seek(at), true)
}
async fn lap(State(c): State<Controller>, Json(b): Json<Lap>) -> Result<StatusCode, StatusCode> {
    change(c, |r| r.jump_lap(b.car, b.lap), true)
}
pub fn routes(shell: HttpShell, c: Controller) -> HttpShell {
    shell
        .route(
            "/api/replay/start",
            post(start).with_state(c.clone()),
            Access::Host,
        )
        .route(
            "/api/replay/stop",
            post(stop).with_state(c.clone()),
            Access::Host,
        )
        .route(
            "/api/replay/pause",
            post(pause).with_state(c.clone()),
            Access::Any,
        )
        .route(
            "/api/replay/speed",
            post(speed).with_state(c.clone()),
            Access::Any,
        )
        .route(
            "/api/replay/seek",
            post(seek).with_state(c.clone()),
            Access::Any,
        )
        .route("/api/replay/lap", post(lap).with_state(c), Access::Any)
}
