//! In-game overlay: a transparent, always-on-top, click-through window over the game.
//!
//! The window hosts the dashboard's `/overlay/throttle-brake` page in a WebView2 view. It only
//! shows above the game in borderless or windowed display mode; exclusive fullscreen hides it.
//! The game itself is never touched.
pub use pitwall_core::overlay::OverlaySettings;
use std::io;

/// Dashboard path the overlay window loads.
pub const THROTTLE_BRAKE_PATH: &str = "/overlay/throttle-brake";
/// Default logical size of the throttle/brake overlay window.
pub const THROTTLE_BRAKE_SIZE: (f64, f64) = (360.0, 110.0);
/// Smallest logical size the overlay can be resized to while it stays readable.
pub const THROTTLE_BRAKE_MIN_SIZE: (f64, f64) = (180.0, 70.0);

/// The edge or corner of the overlay a move-mode resize starts from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    North,
    South,
    East,
    West,
    NorthEast,
    NorthWest,
    SouthEast,
    SouthWest,
}

impl Edge {
    /// Parse the compass code the overlay page sends, such as `se`.
    pub fn parse(code: &str) -> Option<Self> {
        Some(match code {
            "n" => Self::North,
            "s" => Self::South,
            "e" => Self::East,
            "w" => Self::West,
            "ne" => Self::NorthEast,
            "nw" => Self::NorthWest,
            "se" => Self::SouthEast,
            "sw" => Self::SouthWest,
            _ => return None,
        })
    }
}

/// Messages the overlay page sends over IPC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayEvent {
    /// The page got a mouse press in move mode: start a native window drag.
    Drag,
    /// The page got a mouse press on an edge or corner in move mode: start a native resize.
    Resize(Edge),
}

impl OverlayEvent {
    /// Parse an IPC message body: `drag` or `resize:<edge>`.
    pub fn parse(body: &str) -> Option<Self> {
        match body.split_once(':') {
            None if body == "drag" => Some(Self::Drag),
            Some(("resize", edge)) => Edge::parse(edge).map(Self::Resize),
            _ => None,
        }
    }
}

/// URL of the overlay page for a collector base URL. Move mode shows a drag handle.
pub fn page_url(base: &str, moving: bool) -> String {
    let base = base.trim_end_matches('/');
    if moving {
        format!("{base}{THROTTLE_BRAKE_PATH}?move=1")
    } else {
        format!("{base}{THROTTLE_BRAKE_PATH}")
    }
}

/// Default placement: horizontally centred, near the bottom of the monitor.
pub fn default_position(monitor: (i32, i32, u32, u32), window: (u32, u32)) -> (i32, i32) {
    let (x, y, width, height) = monitor;
    let left = x + (width.saturating_sub(window.0) / 2) as i32;
    let top = y + height.saturating_sub(window.1 + height / 5) as i32;
    (left, top)
}

pub use native::Overlay;

mod native {
    use super::*;
    use std::path::PathBuf;
    use tao::{
        dpi::{LogicalSize, PhysicalPosition, PhysicalSize},
        event_loop::{EventLoopProxy, EventLoopWindowTarget},
        platform::windows::WindowBuilderExtWindows,
        window::{ResizeDirection, Window, WindowBuilder, WindowId},
    };
    use wry::{WebContext, WebView, WebViewBuilder};

    /// The open overlay window. Dropping it closes the window.
    pub struct Overlay {
        // Field order matters: the view goes before its window and web context.
        webview: WebView,
        window: Window,
        _context: WebContext,
        base: String,
        moving: bool,
    }

    impl Overlay {
        pub fn open<T: From<OverlayEvent> + 'static>(
            target: &EventLoopWindowTarget<T>,
            proxy: EventLoopProxy<T>,
            base: &str,
            settings: OverlaySettings,
            data: PathBuf,
        ) -> io::Result<Self> {
            let position = settings.position;
            let (width, height) = THROTTLE_BRAKE_SIZE;
            let (min_width, min_height) = THROTTLE_BRAKE_MIN_SIZE;
            let mut builder = WindowBuilder::new()
                .with_title("pitwall overlay")
                .with_decorations(false)
                .with_transparent(true)
                .with_always_on_top(true)
                .with_resizable(false)
                .with_focused(false)
                .with_focusable(false)
                .with_skip_taskbar(true)
                .with_undecorated_shadow(false)
                .with_min_inner_size(LogicalSize::new(min_width, min_height))
                .with_inner_size(LogicalSize::new(width, height));
            if let Some((w, h)) = settings.size {
                builder = builder.with_inner_size(PhysicalSize::new(w, h));
            }
            // A saved spot on a monitor that is gone would leave the overlay unreachable.
            let on_screen = |(x, y): (i32, i32)| {
                target.available_monitors().any(|m| {
                    let (at, size) = (m.position(), m.size());
                    x >= at.x
                        && y >= at.y
                        && x < at.x + size.width as i32
                        && y < at.y + size.height as i32
                })
            };
            let place = position.filter(|&p| on_screen(p)).or_else(|| {
                let monitor = target.primary_monitor()?;
                let at = monitor.position();
                let size = monitor.size();
                let scale = monitor.scale_factor();
                Some(default_position(
                    (at.x, at.y, size.width, size.height),
                    ((width * scale) as u32, (height * scale) as u32),
                ))
            });
            if let Some((x, y)) = place {
                builder = builder.with_position(PhysicalPosition::new(x, y));
            }
            let window = builder.build(target).map_err(io::Error::other)?;
            window
                .set_ignore_cursor_events(true)
                .map_err(io::Error::other)?;
            let mut context = WebContext::new(Some(data));
            let webview = WebViewBuilder::new_with_web_context(&mut context)
                .with_url(page_url(base, false))
                .with_transparent(true)
                .with_focused(false)
                .with_ipc_handler(move |request| {
                    if let Some(event) = OverlayEvent::parse(request.body()) {
                        let _ = proxy.send_event(event.into());
                    }
                })
                .build(&window)
                .map_err(io::Error::other)?;
            Ok(Self {
                webview,
                window,
                _context: context,
                base: base.into(),
                moving: false,
            })
        }

        pub fn id(&self) -> WindowId {
            self.window.id()
        }

        /// Physical position of the window, to remember between runs.
        pub fn position(&self) -> Option<(i32, i32)> {
            self.window.outer_position().ok().map(|p| (p.x, p.y))
        }

        /// Move mode takes the mouse so the overlay can be dragged; locked passes it through.
        pub fn set_moving(&mut self, moving: bool) -> io::Result<()> {
            if moving == self.moving {
                return Ok(());
            }
            self.window
                .set_ignore_cursor_events(!moving)
                .map_err(io::Error::other)?;
            self.window.set_resizable(moving);
            self.webview
                .load_url(&page_url(&self.base, moving))
                .map_err(io::Error::other)?;
            self.moving = moving;
            Ok(())
        }

        /// Follow a collector that restarted its HTTP listener on another address.
        pub fn set_base(&mut self, base: &str) -> io::Result<()> {
            if base == self.base {
                return Ok(());
            }
            self.base = base.into();
            self.webview
                .load_url(&page_url(base, self.moving))
                .map_err(io::Error::other)
        }

        /// Physical inner size of the window, to remember between runs.
        pub fn size(&self) -> (u32, u32) {
            let size = self.window.inner_size();
            (size.width, size.height)
        }

        /// Start a native move or resize for a press the page reported. Ignored when locked.
        pub fn handle(&self, event: OverlayEvent) {
            if !self.moving {
                return;
            }
            let _ = match event {
                OverlayEvent::Drag => self.window.drag_window(),
                OverlayEvent::Resize(edge) => self.window.drag_resize_window(match edge {
                    Edge::North => ResizeDirection::North,
                    Edge::South => ResizeDirection::South,
                    Edge::East => ResizeDirection::East,
                    Edge::West => ResizeDirection::West,
                    Edge::NorthEast => ResizeDirection::NorthEast,
                    Edge::NorthWest => ResizeDirection::NorthWest,
                    Edge::SouthEast => ResizeDirection::SouthEast,
                    Edge::SouthWest => ResizeDirection::SouthWest,
                }),
            };
        }
    }
}
