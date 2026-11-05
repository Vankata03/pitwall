//! Visual tokens from `dashboard/src/styles/tokens.css` (VISUAL-01).

use gpui::{Hsla, rgb};

pub const FONT: &str = "Oxanium";

fn colour(hex: u32) -> Hsla {
    rgb(hex).into()
}

pub fn background() -> Hsla {
    colour(0x1B1F21)
}
pub fn surface() -> Hsla {
    colour(0x22282B)
}
pub fn text() -> Hsla {
    colour(0xF3F5F6)
}
/// Also the colour of a stale value.
pub fn muted() -> Hsla {
    colour(0x9AA7AD)
}
pub fn meter_empty() -> Hsla {
    colour(0x2C363B)
}
pub fn hot() -> Hsla {
    colour(0xFF9C35)
}
pub fn personal_best() -> Hsla {
    colour(0x7ED99C)
}
pub fn slower() -> Hsla {
    colour(0xF2D14B)
}

pub fn session_best() -> Hsla {
    colour(0xC58BFF)
}
