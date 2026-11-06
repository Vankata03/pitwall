//! Opens, restores and remembers the app's dashboard and stream windows over
//! `pitwall_core::window_set`: each window has its own id (and so its own choices file),
//! and the set records which windows are open and where they are.
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use gpui::{
    App, AppContext, Bounds, DisplayId, Pixels, TitlebarOptions, WindowBounds, WindowOptions,
    point, px, size,
};
use pitwall_core::{
    feed::Reader,
    layouts::LayoutLibrary,
    paths::AppPaths,
    units::Units,
    window::{WindowChoices, choices_path},
    window_set::{
        Background, Display, Kind, Placement, Rect, Window, WindowSet, place, window_set_path,
    },
};
use pitwall_native::{
    desktop,
    host::{Command, Control},
};

use crate::dashboard::{Dashboard, Stats};

/// A new stream window's size: 720p, a common OBS canvas.
const STREAM_SIZE: (f32, f32) = (1280.0, 720.0);
/// Moves and resizes are saved at most this often (and always on close and quit).
const SAVE_INTERVAL: Duration = Duration::from_secs(1);

struct Inner {
    set: WindowSet,
    dirty: bool,
    saved: Instant,
}

/// What every window needs from the app, and the window set. Cheap to clone; every
/// dashboard holds one to open stream windows.
#[derive(Clone)]
pub struct Windows {
    inner: Arc<Mutex<Inner>>,
    pub reader: Reader,
    pub stats: Arc<Stats>,
    pub library: LayoutLibrary,
    pub paths: AppPaths,
    pub control: Control,
    /// A new dashboard window's size (`--size`).
    pub dashboard_size: (f32, f32),
}

impl Windows {
    /// Load the window set from the data folder. An unreadable file is kept as `.bad` and
    /// logged; pitwall then starts with no windows to restore.
    pub fn load(
        reader: Reader,
        stats: Arc<Stats>,
        library: LayoutLibrary,
        paths: AppPaths,
        control: Control,
        dashboard_size: (f32, f32),
    ) -> Self {
        let path = window_set_path(&paths.root);
        let set = match WindowSet::load(&path) {
            Ok((set, bad)) => {
                if let Some(bad) = bad {
                    desktop::log(
                        &paths,
                        &format!(
                            "The window list could not be read; kept as {} and no windows were restored.",
                            bad.display()
                        ),
                    );
                }
                set
            }
            Err(error) => {
                desktop::log(
                    &paths,
                    &format!(
                        "The window list could not be read ({error}); it is kept in memory for this run."
                    ),
                );
                WindowSet::in_memory()
            }
        };
        Self {
            inner: Arc::new(Mutex::new(Inner {
                set,
                dirty: false,
                saved: Instant::now(),
            })),
            reader,
            stats,
            library,
            paths,
            control,
            dashboard_size,
        }
    }

    fn with<R>(&self, f: impl FnOnce(&mut WindowSet) -> R) -> R {
        let mut inner = self.inner.lock().unwrap();
        let result = f(&mut inner.set);
        inner.dirty = true;
        result
    }

    fn save_now(&self) {
        let mut inner = self.inner.lock().unwrap();
        if let Err(error) = inner.set.save() {
            desktop::log(&self.paths, &format!("Window list: {error}"));
        }
        inner.dirty = false;
        inner.saved = Instant::now();
    }

    /// Save moves and resizes, at most once a second; call from a timer.
    pub fn save_if_due(&self) {
        let due = {
            let inner = self.inner.lock().unwrap();
            inner.dirty && inner.saved.elapsed() >= SAVE_INTERVAL
        };
        if due {
            self.save_now();
        }
    }

    /// Reopen the windows that were open when pitwall last quit; false when there were
    /// none.
    pub fn restore(&self, cx: &mut App) -> bool {
        let windows = self.inner.lock().unwrap().set.to_restore();
        let mut opened = false;
        for window in windows {
            opened |= self.show(cx, &window).is_ok();
        }
        opened
    }

    /// Open another dashboard window (reusing a closed one's id, choices and position).
    pub fn open_dashboard(&self, cx: &mut App) -> Result<(), String> {
        let window = self.with(WindowSet::open_dashboard);
        self.save_now();
        self.show(cx, &window)
    }

    /// Open a stream window for `layout`, starting with `units`.
    pub fn open_stream(
        &self,
        cx: &mut App,
        layout: &str,
        background: Background,
        units: Units,
    ) -> Result<(), String> {
        let window = self.with(|set| set.open_stream(layout, background));
        self.save_now();
        // The stream window has no Display menu: it takes the opening window's units.
        let path = choices_path(&self.paths.root, &window.id);
        let choices = WindowChoices {
            units,
            ..WindowChoices::default()
        };
        if let Err(error) = choices.save(&path) {
            desktop::log(
                &self.paths,
                &format!("Stream window {}: units not saved: {error}", window.id),
            );
        }
        self.show(cx, &window)
    }

    /// pitwall is quitting: the windows still open stay in the list for the next start.
    pub fn quit(&self) {
        self.inner.lock().unwrap().set.quitting();
        self.save_now();
    }

    pub(crate) fn moved(&self, id: &str, placement: Placement) {
        self.with(|set| set.moved(id, placement));
    }

    pub(crate) fn closed(&self, id: &str) {
        self.with(|set| set.closed(id));
        self.save_now();
        let _ = self
            .control
            .send(Command::WindowProblem(id.to_owned(), None));
    }

    fn show(&self, cx: &mut App, window: &Window) -> Result<(), String> {
        let (title, default_size, chrome) = match &window.kind {
            Kind::Dashboard => (None, self.dashboard_size, true),
            Kind::Stream { layout, .. } => (
                Some(format!("pitwall stream: {layout}").into()),
                STREAM_SIZE,
                false,
            ),
        };
        let (display_id, window_bounds) = self.placement(cx, window, default_size);
        let services = self.clone();
        let spec = window.clone();
        let result = cx.open_window(
            WindowOptions {
                window_bounds: Some(window_bounds),
                display_id,
                titlebar: Some(TitlebarOptions {
                    title,
                    // A stream window has no title bar or frame for OBS to capture.
                    appears_transparent: !chrome,
                    traffic_light_position: None,
                }),
                // A dashboard or stream window on another screen is inactive while the
                // game has focus: never throttle it.
                inactive_frame_interval: None,
                low_power_frame_interval: None,
                thermal_frame_interval: None,
                ..Default::default()
            },
            |gpui_window, cx| cx.new(|cx| Dashboard::open(&spec, services, gpui_window, cx)),
        );
        match result {
            Ok(_) => Ok(()),
            Err(error) => {
                let message = format!("Window {}: {error:#}", window.id);
                desktop::log(&self.paths, &message);
                desktop::notify("pitwall window failed", &message);
                // A window that never opened is not restored next time.
                self.with(|set| set.closed(&window.id));
                self.save_now();
                Err(message)
            }
        }
    }

    /// The monitor and bounds to open `window` at: where it was, if that is still on a
    /// monitor, else somewhere visible.
    fn placement(
        &self,
        cx: &App,
        window: &Window,
        default_size: (f32, f32),
    ) -> (Option<DisplayId>, WindowBounds) {
        let primary = cx.primary_display().map(|d| d.id());
        let displays: Vec<_> = cx
            .displays()
            .into_iter()
            .filter_map(|d| {
                let id = d.uuid().ok()?.to_string();
                Some((
                    d.id(),
                    Display {
                        id,
                        visible: rect_of(d.visible_bounds()),
                        primary: Some(d.id()) == primary,
                    },
                ))
            })
            .collect();
        let monitors: Vec<_> = displays.iter().map(|(_, d)| d.clone()).collect();
        let Some(placed) = place(window.placement.as_ref(), &monitors) else {
            let bounds = Bounds::centered(None, size(px(default_size.0), px(default_size.1)), cx);
            return (None, WindowBounds::Windowed(bounds));
        };
        let display_id = displays
            .iter()
            .find(|(_, d)| Some(&d.id) == placed.display.as_ref())
            .map(|(id, _)| *id);
        let bounds = bounds_of(placed.bounds);
        let window_bounds = if placed.maximized {
            WindowBounds::Maximized(bounds)
        } else {
            WindowBounds::Windowed(bounds)
        };
        (display_id, window_bounds)
    }
}

/// Where a window is now, for the window list.
pub fn placement_of(window: &gpui::Window, cx: &App) -> Placement {
    let (bounds, maximized) = match window.window_bounds() {
        WindowBounds::Windowed(bounds) => (bounds, false),
        WindowBounds::Maximized(bounds) | WindowBounds::Fullscreen(bounds) => (bounds, true),
    };
    Placement {
        display: window
            .display(cx)
            .and_then(|d| d.uuid().ok())
            .map(|uuid| uuid.to_string()),
        bounds: rect_of(bounds),
        maximized,
    }
}

fn rect_of(bounds: Bounds<Pixels>) -> Rect {
    Rect {
        x: f32::from(bounds.origin.x),
        y: f32::from(bounds.origin.y),
        width: f32::from(bounds.size.width),
        height: f32::from(bounds.size.height),
    }
}

fn bounds_of(rect: Rect) -> Bounds<Pixels> {
    Bounds {
        origin: point(px(rect.x), px(rect.y)),
        size: size(px(rect.width), px(rect.height)),
    }
}
