//! The app's windows apart from drawing them (ADR 0001): any number of dashboard windows
//! and chrome-less stream windows, each with its own id. The id names the window's choices
//! file ([`crate::window::choices_path`]), so each window keeps its own layout, focus car
//! and units.
//!
//! The set is saved to `<root>/windows.json`. Windows open when pitwall quits come back on
//! the next start, each on its monitor and at its position ([`place`]). A closed window
//! keeps its entry, and the next window of its kind reuses it, with its choices and
//! position.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::data::keep_as_bad;

/// Version written to the window set file.
pub const WINDOW_SET_SCHEMA_VERSION: u32 = 1;

/// Where the window set is saved: `<root>/windows.json`.
pub fn window_set_path(root: &Path) -> PathBuf {
    root.join("windows.json")
}

/// The id of the first dashboard window; its choices file predates the window set.
pub const MAIN: &str = "main";

/// What a window shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Kind {
    /// A dashboard window with chrome, edit mode and its own choices.
    Dashboard,
    /// A chrome-less window showing one layout for OBS Window Capture.
    Stream {
        layout: String,
        #[serde(default, deserialize_with = "lenient")]
        background: Background,
    },
}

/// A stream window's background: the dashboard's solid colour, or a chroma-key colour
/// for OBS to key out.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Background {
    #[default]
    Solid,
    Green,
    Magenta,
}

impl Background {
    pub const ALL: [Background; 3] = [Background::Solid, Background::Green, Background::Magenta];

    /// The colour as `0xRRGGBB`; `solid` is the dashboard background.
    pub fn rgb(self) -> u32 {
        match self {
            Background::Solid => 0x1B1F21,
            Background::Green => 0x00FF00,
            Background::Magenta => 0xFF00FF,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Background::Solid => "Solid",
            Background::Green => "Green key",
            Background::Magenta => "Magenta key",
        }
    }
}

/// An unknown value falls back to the default instead of dropping the window.
fn lenient<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Default + serde::de::DeserializeOwned,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(serde_json::from_value(value).unwrap_or_default())
}

/// A rectangle in the windowing system's logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Where a window was: its monitor, its restored bounds and whether it was maximized.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Placement {
    /// The monitor's id, as the windowing system reports it; `None` when unknown.
    pub display: Option<String>,
    pub bounds: Rect,
    #[serde(default)]
    pub maximized: bool,
}

/// A monitor as the windowing system reports it now.
#[derive(Debug, Clone, PartialEq)]
pub struct Display {
    pub id: String,
    /// The work area: the monitor less the taskbar.
    pub visible: Rect,
    pub primary: bool,
}

/// Where to open a window saved at `saved`, given the monitors there are now; `None`
/// opens it at the default size and position.
///
/// A window whose monitor is still there returns to it. It keeps its bounds while its
/// centre and top edge are on that monitor, so it can be reached and dragged; otherwise
/// it is moved (and shrunk if need be) onto it. A window whose monitor is gone opens
/// centred on the primary monitor, shrunk to fit it.
pub fn place(saved: Option<&Placement>, displays: &[Display]) -> Option<Placement> {
    let saved = saved?;
    let same = saved
        .display
        .as_ref()
        .and_then(|id| displays.iter().find(|d| &d.id == id));
    let Some(display) = same else {
        let display = displays.iter().find(|d| d.primary).or(displays.first())?;
        let area = display.visible;
        let (width, height) = (
            saved.bounds.width.min(area.width),
            saved.bounds.height.min(area.height),
        );
        return Some(Placement {
            display: Some(display.id.clone()),
            bounds: Rect {
                x: area.x + (area.width - width) / 2.0,
                y: area.y + (area.height - height) / 2.0,
                width,
                height,
            },
            maximized: saved.maximized,
        });
    };
    let (area, bounds) = (display.visible, saved.bounds);
    let centre = (
        bounds.x + bounds.width / 2.0,
        bounds.y + bounds.height / 2.0,
    );
    let reachable = centre.0 >= area.x
        && centre.0 <= area.x + area.width
        && centre.1 >= area.y
        && centre.1 <= area.y + area.height
        && bounds.y >= area.y;
    if reachable {
        return Some(saved.clone());
    }
    let (width, height) = (bounds.width.min(area.width), bounds.height.min(area.height));
    Some(Placement {
        display: Some(display.id.clone()),
        bounds: Rect {
            x: bounds.x.clamp(area.x, area.x + area.width - width),
            y: bounds.y.clamp(area.y, area.y + area.height - height),
            width,
            height,
        },
        maximized: saved.maximized,
    })
}

/// One window of the set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Window {
    pub id: String,
    #[serde(flatten)]
    pub kind: Kind,
    #[serde(default)]
    pub placement: Option<Placement>,
    /// Open now, or open when pitwall last quit.
    #[serde(default)]
    pub open: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct File {
    #[serde(default)]
    schema_version: u32,
    #[serde(default)]
    windows: Vec<serde_json::Value>,
}

#[derive(Debug, Default)]
pub struct WindowSet {
    path: Option<PathBuf>,
    windows: Vec<Window>,
    quitting: bool,
}

impl WindowSet {
    /// Read the window set. A missing file gives an empty set. A file that is not JSON is
    /// moved aside to `.bad` (returned, for the log and status). Entries that cannot be
    /// read, or repeat an id, are skipped one by one.
    pub fn load(path: &Path) -> io::Result<(Self, Option<PathBuf>)> {
        let empty = |path: &Path| Self {
            path: Some(path.to_owned()),
            ..Self::default()
        };
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok((empty(path), None)),
            Err(e) => return Err(e),
        };
        let Ok(file) = serde_json::from_slice::<File>(&bytes) else {
            return Ok((empty(path), Some(keep_as_bad(path)?)));
        };
        let mut set = empty(path);
        for value in file.windows {
            if let Ok(window) = serde_json::from_value::<Window>(value)
                && !window.id.is_empty()
                && set.get(&window.id).is_none()
            {
                set.windows.push(window);
            }
        }
        Ok((set, None))
    }

    /// A set kept in memory only.
    pub fn in_memory() -> Self {
        Self::default()
    }

    /// Write atomically (temporary file, then rename), creating the folder.
    pub fn save(&self) -> io::Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = serde_json::json!({
            "schemaVersion": WINDOW_SET_SCHEMA_VERSION,
            "windows": self.windows,
        });
        crate::storage::atomic_write(
            path,
            serde_json::to_vec_pretty(&file).map_err(io::Error::other)?,
        )
    }

    pub fn windows(&self) -> &[Window] {
        &self.windows
    }

    pub fn get(&self, id: &str) -> Option<&Window> {
        self.windows.iter().find(|w| w.id == id)
    }

    /// The windows to open at start: those open when pitwall last quit.
    pub fn to_restore(&self) -> Vec<Window> {
        self.windows.iter().filter(|w| w.open).cloned().collect()
    }

    /// Open a dashboard window: the first closed one, with its choices and position, or a
    /// new one (`main`, then `dashboard-2`, `dashboard-3`, ...).
    pub fn open_dashboard(&mut self) -> Window {
        let reused = self
            .windows
            .iter()
            .position(|w| !w.open && w.kind == Kind::Dashboard);
        let index = reused.unwrap_or_else(|| {
            let id = self.free_id(MAIN, "dashboard");
            self.windows.push(Window {
                id,
                kind: Kind::Dashboard,
                placement: None,
                open: false,
            });
            self.windows.len() - 1
        });
        self.windows[index].open = true;
        self.windows[index].clone()
    }

    /// Open a stream window for `layout`: the first closed one, keeping its position, or a
    /// new one (`stream-1`, `stream-2`, ...).
    pub fn open_stream(&mut self, layout: &str, background: Background) -> Window {
        let kind = Kind::Stream {
            layout: layout.to_owned(),
            background,
        };
        let reused = self
            .windows
            .iter()
            .position(|w| !w.open && matches!(w.kind, Kind::Stream { .. }));
        let index = reused.unwrap_or_else(|| {
            let id = self.free_id("stream-1", "stream");
            self.windows.push(Window {
                id,
                kind: kind.clone(),
                placement: None,
                open: false,
            });
            self.windows.len() - 1
        });
        let window = &mut self.windows[index];
        window.kind = kind;
        window.open = true;
        window.clone()
    }

    /// The window `id` moved or resized.
    pub fn moved(&mut self, id: &str, placement: Placement) {
        if let Some(window) = self.windows.iter_mut().find(|w| w.id == id) {
            window.placement = Some(placement);
        }
    }

    /// The window `id` closed. Ignored once pitwall is quitting, so the windows open at
    /// quit come back on the next start.
    pub fn closed(&mut self, id: &str) {
        if self.quitting {
            return;
        }
        if let Some(window) = self.windows.iter_mut().find(|w| w.id == id) {
            window.open = false;
        }
    }

    /// pitwall is quitting: windows closing from now on stay marked open.
    pub fn quitting(&mut self) {
        self.quitting = true;
    }

    /// The first of `first`, `<prefix>-2`, `<prefix>-3`, ... no window uses.
    fn free_id(&self, first: &str, prefix: &str) -> String {
        std::iter::once(first.to_owned())
            .chain((2..).map(|n| format!("{prefix}-{n}")))
            .find(|id| self.get(id).is_none())
            .expect("a free id exists")
    }
}
