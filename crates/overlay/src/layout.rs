//! Overlay geometry in device-independent pixels (the page's CSS pixels), the move-mode
//! grips, and the window frame a drag produces. Mirrors `dashboard/src/overlay/overlay.css`.

/// A box in DIPs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Box2 {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Box2 {
    pub fn width(&self) -> f32 {
        self.right - self.left
    }
    pub fn height(&self) -> f32 {
        self.bottom - self.top
    }
}

/// One pedal bar: the column, its value text row, its track and its label row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BarLayout {
    pub column: Box2,
    pub value: Box2,
    pub track: Box2,
    pub label: Box2,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layout {
    pub panel: Box2,
    /// The trace's drawing area, inside its left and bottom border.
    pub trace: Box2,
    /// Throttle, then brake.
    pub bars: [BarLayout; 2],
    /// Top-left of the "Dim: best lap" legend.
    pub legend: (f32, f32),
}

pub const RADIUS: f32 = 8.0;
const PAD_X: f32 = 10.0;
const PAD_Y: f32 = 8.0;
const GAP: f32 = 10.0;
const BAR_WIDTH: f32 = 30.0;
const BAR_GAP: f32 = 6.0;
const TRACK_WIDTH: f32 = 14.0;
pub const TRACK_RADIUS: f32 = 3.0;
/// `line-height: normal` rows of the 13 px value and the 10 px label, and the 3 px gaps.
const VALUE_ROW: f32 = 17.0;
const LABEL_ROW: f32 = 12.0;
const ROW_GAP: f32 = 3.0;
pub const BORDER: f32 = 1.0;

/// Lay out a `width` × `height` DIP panel.
pub fn layout(width: f32, height: f32) -> Layout {
    let panel = Box2 {
        left: 0.0,
        top: 0.0,
        right: width,
        bottom: height,
    };
    let top = PAD_Y;
    let bottom = (height - PAD_Y).max(top);
    let bars_right = (width - PAD_X).max(PAD_X);
    let bars_left = (bars_right - 2.0 * BAR_WIDTH - BAR_GAP).max(PAD_X);
    let trace_right = (bars_left - GAP).max(PAD_X + BORDER);
    let trace = Box2 {
        left: PAD_X + BORDER,
        top,
        right: trace_right,
        bottom: (bottom - BORDER).max(top),
    };
    let bar = |left: f32| {
        let column = Box2 {
            left,
            top,
            right: left + BAR_WIDTH,
            bottom,
        };
        let value = Box2 {
            bottom: top + VALUE_ROW,
            ..column
        };
        let label = Box2 {
            top: (bottom - LABEL_ROW).max(top),
            ..column
        };
        let track_left = left + (BAR_WIDTH - TRACK_WIDTH) / 2.0;
        let track_top = (value.bottom + ROW_GAP).min(bottom);
        let track = Box2 {
            left: track_left,
            right: track_left + TRACK_WIDTH,
            top: track_top,
            bottom: (label.top - ROW_GAP).max(track_top),
        };
        BarLayout {
            column,
            value,
            track,
            label,
        }
    };
    Layout {
        panel,
        trace,
        bars: [bar(bars_left), bar(bars_left + BAR_WIDTH + BAR_GAP)],
        legend: (16.0, 6.0),
    }
}

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

/// What a move-mode press on the panel starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    Move,
    Resize(Edge),
}

const STRIP: f32 = 6.0;
const STRIP_INSET: f32 = 8.0;
const CORNER: f32 = 10.0;
/// The bottom-right grip is drawn, and larger, so resizing is discoverable.
pub const SE_GRIP: f32 = 14.0;

/// Which grip, if any, sits under `(x, y)` DIPs in a `width` × `height` panel. Corners
/// sit above the edge strips, as on the page.
pub fn hit_test(x: f32, y: f32, width: f32, height: f32) -> Hit {
    let (right, bottom) = (width - x, height - y);
    let corner = if x < CORNER && y < CORNER {
        Some(Edge::NorthWest)
    } else if right < SE_GRIP && bottom < SE_GRIP {
        Some(Edge::SouthEast)
    } else if right < CORNER && y < CORNER {
        Some(Edge::NorthEast)
    } else if x < CORNER && bottom < CORNER {
        Some(Edge::SouthWest)
    } else {
        None
    };
    if let Some(edge) = corner {
        return Hit::Resize(edge);
    }
    let across = x >= STRIP_INSET && right > STRIP_INSET;
    let down = y >= STRIP_INSET && bottom > STRIP_INSET;
    let edge = if across && y < STRIP {
        Some(Edge::North)
    } else if across && bottom <= STRIP {
        Some(Edge::South)
    } else if down && right <= STRIP {
        Some(Edge::East)
    } else if down && x < STRIP {
        Some(Edge::West)
    } else {
        None
    };
    edge.map_or(Hit::Move, Hit::Resize)
}

/// The window's frame in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// The frame after dragging `hit` by `delta` physical pixels from `start`. A resize moves
/// only the grabbed edges and stops at `min` without pushing the opposite edge.
pub fn drag(hit: Hit, start: Frame, delta: (i32, i32), min: (u32, u32)) -> Frame {
    let (dx, dy) = delta;
    let edge = match hit {
        Hit::Move => {
            return Frame {
                x: start.x + dx,
                y: start.y + dy,
                ..start
            };
        }
        Hit::Resize(edge) => edge,
    };
    let (west, east, north, south) = match edge {
        Edge::North => (false, false, true, false),
        Edge::South => (false, false, false, true),
        Edge::East => (false, true, false, false),
        Edge::West => (true, false, false, false),
        Edge::NorthEast => (false, true, true, false),
        Edge::NorthWest => (true, false, true, false),
        Edge::SouthEast => (false, true, false, true),
        Edge::SouthWest => (true, false, false, true),
    };
    let (mut left, mut top) = (start.x, start.y);
    let (mut right, mut bottom) = (left + start.width as i32, top + start.height as i32);
    let (min_w, min_h) = (min.0 as i32, min.1 as i32);
    if west {
        left = (left + dx).min(right - min_w);
    }
    if east {
        right = (right + dx).max(left + min_w);
    }
    if north {
        top = (top + dy).min(bottom - min_h);
    }
    if south {
        bottom = (bottom + dy).max(top + min_h);
    }
    Frame {
        x: left,
        y: top,
        width: (right - left) as u32,
        height: (bottom - top) as u32,
    }
}
