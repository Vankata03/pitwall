//! The three read-only presets (spec section 12). Wide, tablet and phone arrangements are
//! all authored here; each tile's `i` is its type, as no preset holds two of one type.

use super::{Arrangements, Layout, SCHEMA_VERSION, Tile};
use serde_json::{Map, Value};

/// Presets in display order; the first, Pitwall Classic, is the default for a new device.
pub const CLASSIC: &str = "Pitwall Classic";
pub const DRIVER: &str = "Driver";
pub const RACE: &str = "Race";

fn tile(kind: &str, x: u32, y: u32, w: u32, h: u32) -> Tile {
    Tile {
        i: kind.to_string(),
        kind: kind.to_string(),
        x,
        y,
        w,
        h,
        settings: Value::Object(Map::new()),
        extra: Map::new(),
    }
}

fn layout(name: &str, wide: &[Tile], tablet: &[Tile], phone: &[Tile]) -> Layout {
    Layout {
        schema_version: SCHEMA_VERSION,
        name: name.to_string(),
        arrangements: Arrangements {
            wide: wide.to_vec(),
            tablet: Some(tablet.to_vec()),
            phone: Some(phone.to_vec()),
            extra: Map::new(),
        },
        extra: Map::new(),
    }
}

pub fn all() -> Vec<Layout> {
    vec![classic(), driver(), race()]
}

fn classic() -> Layout {
    let wide = [
        tile("speed-rpm-gear", 0, 0, 6, 6),
        tile("engine-temp", 0, 6, 3, 2),
        tile("g-force", 3, 6, 3, 2),
        tile("ers", 0, 8, 6, 4),
        tile("fuel", 0, 12, 6, 2),
        tile("gaps", 6, 0, 8, 2),
        tile("sm", 14, 0, 4, 2),
        tile("car-status", 6, 2, 12, 12),
        tile("weather", 18, 0, 6, 2),
        tile("track-map", 18, 2, 6, 6),
        tile("lap-state", 18, 8, 6, 2),
        tile("lap-timing", 18, 10, 6, 4),
        tile("throttle-brake-traces", 0, 14, 10, 7),
        tile("stint", 10, 14, 8, 7),
        tile("timing-tower", 18, 14, 6, 7),
        tile("notice", 0, 21, 24, 1),
    ];
    let tablet = [
        tile("speed-rpm-gear", 0, 0, 6, 6),
        tile("gaps", 6, 0, 6, 2),
        tile("sm", 6, 2, 6, 2),
        tile("weather", 6, 4, 6, 2),
        tile("car-status", 0, 6, 12, 12),
        tile("track-map", 0, 18, 6, 8),
        tile("engine-temp", 6, 18, 3, 2),
        tile("g-force", 9, 18, 3, 2),
        tile("lap-state", 6, 20, 6, 2),
        tile("lap-timing", 6, 22, 6, 4),
        tile("ers", 0, 26, 12, 4),
        tile("fuel", 0, 30, 12, 2),
        tile("throttle-brake-traces", 0, 32, 12, 7),
        tile("stint", 0, 39, 12, 7),
        tile("timing-tower", 0, 46, 12, 7),
        tile("notice", 0, 53, 12, 1),
    ];
    let phone = [
        tile("lap-timing", 0, 0, 4, 4),
        tile("gaps", 0, 4, 4, 2),
        tile("speed-rpm-gear", 0, 6, 4, 6),
        tile("sm", 0, 12, 4, 2),
        tile("lap-state", 0, 14, 4, 2),
        tile("ers", 0, 16, 4, 4),
        tile("engine-temp", 0, 20, 2, 2),
        tile("g-force", 2, 20, 2, 2),
        tile("fuel", 0, 22, 4, 2),
        tile("weather", 0, 24, 4, 2),
        tile("car-status", 0, 26, 4, 8),
        tile("track-map", 0, 34, 4, 5),
        tile("throttle-brake-traces", 0, 39, 4, 6),
        tile("stint", 0, 45, 4, 5),
        tile("timing-tower", 0, 50, 4, 8),
        tile("notice", 0, 58, 4, 1),
    ];
    layout(CLASSIC, &wide, &tablet, &phone)
}

fn driver() -> Layout {
    let wide = [
        tile("speed-rpm-gear", 0, 0, 10, 8),
        tile("lap-timing", 10, 0, 8, 5),
        tile("sm", 18, 0, 6, 3),
        tile("fuel", 18, 3, 6, 2),
        tile("gaps", 10, 5, 14, 3),
        tile("ers", 0, 8, 10, 5),
        tile("track-discipline", 10, 8, 14, 5),
        tile("notice", 0, 13, 24, 1),
    ];
    let tablet = [
        tile("speed-rpm-gear", 0, 0, 7, 8),
        tile("lap-timing", 7, 0, 5, 5),
        tile("sm", 7, 5, 5, 3),
        tile("fuel", 0, 8, 12, 2),
        tile("gaps", 0, 10, 12, 3),
        tile("ers", 0, 13, 12, 5),
        tile("track-discipline", 0, 18, 12, 3),
        tile("notice", 0, 21, 12, 1),
    ];
    let phone = [
        tile("lap-timing", 0, 0, 4, 5),
        tile("gaps", 0, 5, 4, 3),
        tile("speed-rpm-gear", 0, 8, 4, 7),
        tile("sm", 0, 15, 4, 3),
        tile("fuel", 0, 18, 4, 2),
        tile("ers", 0, 20, 4, 5),
        tile("track-discipline", 0, 25, 4, 3),
        tile("notice", 0, 28, 4, 1),
    ];
    layout(DRIVER, &wide, &tablet, &phone)
}

/// Timing tower at Full density: density follows tile size (spec section 10), so the
/// tower is sized large rather than carrying a density setting.
fn race() -> Layout {
    let wide = [
        tile("timing-tower", 0, 0, 9, 16),
        tile("track-map", 9, 0, 8, 8),
        tile("weather", 17, 0, 7, 2),
        tile("lap-state", 17, 2, 7, 2),
        tile("stint", 17, 4, 7, 12),
        tile("race-control-feed", 9, 8, 8, 8),
        tile("notice", 0, 16, 24, 1),
    ];
    let tablet = [
        tile("timing-tower", 0, 0, 7, 14),
        tile("track-map", 7, 0, 5, 6),
        tile("weather", 7, 6, 5, 2),
        tile("lap-state", 7, 8, 5, 2),
        tile("stint", 0, 14, 12, 6),
        tile("race-control-feed", 0, 20, 12, 6),
        tile("notice", 0, 26, 12, 1),
    ];
    // Race has no Lap timing or Gaps tile, so Lap state leads.
    let phone = [
        tile("lap-state", 0, 0, 4, 2),
        tile("timing-tower", 0, 2, 4, 14),
        tile("track-map", 0, 16, 4, 6),
        tile("stint", 0, 22, 4, 6),
        tile("race-control-feed", 0, 28, 4, 6),
        tile("weather", 0, 34, 4, 2),
        tile("notice", 0, 36, 4, 1),
    ];
    layout(RACE, &wide, &tablet, &phone)
}
