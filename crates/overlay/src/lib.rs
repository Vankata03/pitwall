//! The native in-game throttle/brake overlay (ADR 0002): a framework-free Win32 window on
//! its own thread, layered, click-through, topmost, never activated and kept off the
//! taskbar, drawn with Direct2D from the core's in-process read API
//! ([`pitwall_core::feed`]). It never touches the game process.
//!
//! The platform-free parts (settings, layout, the view model) build everywhere so their
//! tests run on any host; the window itself is Windows only.

pub mod layout;
pub mod settings;
pub mod view;

#[cfg(windows)]
mod window;

#[cfg(windows)]
pub use window::{Overlay, RenderMetrics, spawn};
