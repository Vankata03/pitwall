//! `overlay.json` and window placement. The file format is the one the WebView2 overlay
//! wrote (ADR 0003): `{ "visible": bool, "position": [x, y] | null, "size": [w, h] | null }`
//! in physical pixels, with `size` optional for files saved before resizing existed.

use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// Default logical size of the overlay window.
pub const DEFAULT_SIZE: (f64, f64) = (360.0, 110.0);
/// Smallest logical size the overlay can be resized to while it stays readable.
pub const MIN_SIZE: (f64, f64) = (180.0, 70.0);

/// What the overlay remembers between runs, in `overlay.json` in the data folder.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverlaySettings {
    pub visible: bool,
    /// Physical screen position of the window's top-left corner; `None` until first placed.
    pub position: Option<(i32, i32)>,
    /// Physical size; `None` until first resized.
    #[serde(default)]
    pub size: Option<(u32, u32)>,
}

/// The outcome of reading `overlay.json`.
#[derive(Debug, PartialEq)]
pub enum Loaded {
    Read(OverlaySettings),
    /// No file (or it could not be read): start hidden at the default position.
    Missing(OverlaySettings),
    /// The file did not parse. It was moved to `kept` and the defaults apply (MIG-04).
    Malformed {
        settings: OverlaySettings,
        kept: PathBuf,
        error: String,
    },
}

impl Loaded {
    pub fn settings(&self) -> OverlaySettings {
        match self {
            Self::Read(s) | Self::Missing(s) | Self::Malformed { settings: s, .. } => *s,
        }
    }
}

impl OverlaySettings {
    /// Read `path`. A malformed file is never overwritten: it is renamed to the first free
    /// `<name>.bad`, `<name>.bad.2`, … and the defaults load.
    pub fn load(path: &Path) -> Loaded {
        let Ok(text) = fs::read_to_string(path) else {
            return Loaded::Missing(Self::default());
        };
        match serde_json::from_str(&text) {
            Ok(settings) => Loaded::Read(settings),
            Err(error) => {
                let kept = free_bad_path(path);
                if let Err(rename) = fs::rename(path, &kept) {
                    // Could not move it aside: still load defaults, and say so.
                    return Loaded::Malformed {
                        settings: Self::default(),
                        kept: path.to_owned(),
                        error: format!("{error}; could not keep it: {rename}"),
                    };
                }
                Loaded::Malformed {
                    settings: Self::default(),
                    kept,
                    error: error.to_string(),
                }
            }
        }
    }

    /// Write through a temporary file and a rename, so a crash never leaves half a file.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        let tmp = path.with_extension("json.tmp");
        fs::write(
            &tmp,
            serde_json::to_string_pretty(self).map_err(io::Error::other)?,
        )?;
        fs::rename(tmp, path)
    }
}

fn free_bad_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".bad");
    let first = path.with_file_name(&name);
    if !first.exists() {
        return first;
    }
    (2..)
        .map(|n| {
            let mut numbered = name.clone();
            numbered.push(format!(".{n}"));
            path.with_file_name(numbered)
        })
        .find(|p| !p.exists())
        .unwrap()
}

/// Default placement: horizontally centred, with the bottom edge a fifth of the monitor
/// height above the bottom. `monitor` is `(x, y, width, height)`.
pub fn default_position(monitor: (i32, i32, u32, u32), window: (u32, u32)) -> (i32, i32) {
    let (x, y, width, height) = monitor;
    let left = x + (width.saturating_sub(window.0) / 2) as i32;
    let top = y + height.saturating_sub(window.1 + height / 5) as i32;
    (left, top)
}

/// A monitor rectangle in physical pixels, right and bottom exclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    fn contains(&self, (x, y): (i32, i32)) -> bool {
        x >= self.left && y >= self.top && x < self.right && y < self.bottom
    }
}

/// Where the window opens, in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    pub position: (i32, i32),
    pub size: (u32, u32),
}

/// Open at the saved size (or the default size at `scale`), and at the saved spot if some
/// monitor still shows it; a spot on a monitor that is gone would leave the overlay
/// unreachable, so it falls back to the default spot on the primary monitor.
pub fn place(
    settings: &OverlaySettings,
    monitors: &[Rect],
    primary: Rect,
    scale: f64,
) -> Placement {
    let scaled = |(w, h): (f64, f64)| ((w * scale) as u32, (h * scale) as u32);
    let default = scaled(DEFAULT_SIZE);
    let min = scaled(MIN_SIZE);
    let size = settings
        .size
        .map_or(default, |(w, h)| (w.max(min.0), h.max(min.1)));
    let position = settings
        .position
        .filter(|&p| monitors.iter().any(|m| m.contains(p)))
        .unwrap_or_else(|| {
            default_position(
                (
                    primary.left,
                    primary.top,
                    (primary.right - primary.left) as u32,
                    (primary.bottom - primary.top) as u32,
                ),
                default,
            )
        });
    Placement { position, size }
}
