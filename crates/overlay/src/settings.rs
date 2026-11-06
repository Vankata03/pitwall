//! `overlay.json` and window placement. The file format is the one the WebView2 overlay
//! wrote (ADR 0003): `{ "visible": bool, "position": [x, y] | null, "size": [w, h] | null }`
//! in physical pixels, with `size` optional for files saved before resizing existed.

pub use pitwall_core::overlay::{Loaded, OverlaySettings};

/// Default logical size of the overlay window.
pub const DEFAULT_SIZE: (f64, f64) = (360.0, 110.0);
/// Smallest logical size the overlay can be resized to while it stays readable.
pub const MIN_SIZE: (f64, f64) = (180.0, 70.0);

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
