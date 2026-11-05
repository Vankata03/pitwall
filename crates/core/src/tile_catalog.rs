//! The registered tile types: name, car binding and grid sizes of each, ported from the
//! declarations in `dashboard/src/tiles/types/*.tsx`. The native registry pairs each type
//! with its renderer; the editor reads sizes and the add-tile picker order from here.
//!
//! A type missing from this list is unknown: its tiles keep their place and data, can be
//! moved, resized and removed, and have no settings.

/// Whether a tile follows a car (and so has a car setting) or shows the whole session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Binding {
    FollowsCar,
    SessionWide,
}

/// One registered tile type. Sizes are grid cells, `(w, h)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Declaration {
    pub kind: &'static str,
    pub name: &'static str,
    pub binding: Binding,
    pub minimum: (u32, u32),
    pub default: (u32, u32),
}

impl Declaration {
    pub fn follows_car(&self) -> bool {
        self.binding == Binding::FollowsCar
    }
}

const fn car(
    kind: &'static str,
    name: &'static str,
    minimum: (u32, u32),
    default: (u32, u32),
) -> Declaration {
    Declaration {
        kind,
        name,
        binding: Binding::FollowsCar,
        minimum,
        default,
    }
}

const fn session(
    kind: &'static str,
    name: &'static str,
    minimum: (u32, u32),
    default: (u32, u32),
) -> Declaration {
    Declaration {
        kind,
        name,
        binding: Binding::SessionWide,
        minimum,
        default,
    }
}

/// Every registered type, in display-name order (the add-tile picker's order).
pub const DECLARATIONS: [Declaration; 19] = [
    car("car-status", "Car status", (8, 6), (12, 12)),
    car("engine-temp", "Engine temp", (2, 2), (3, 2)),
    car("ers", "ERS", (4, 2), (6, 4)),
    car("fuel", "Fuel", (3, 2), (6, 2)),
    car("g-force", "G-force", (2, 2), (3, 2)),
    car("gaps", "Gaps", (4, 2), (8, 2)),
    car("lap-state", "Lap state", (4, 2), (6, 2)),
    car("lap-timing", "Lap timing", (4, 3), (6, 4)),
    car("live-delta", "Live delta", (6, 3), (12, 6)),
    session("notice", "Notice", (4, 1), (6, 1)),
    session("race-control-feed", "Race-control feed", (4, 2), (6, 6)),
    car("sm", "SM", (2, 2), (4, 2)),
    car("speed-rpm-gear", "Speed/RPM/gear", (4, 3), (6, 6)),
    car("stint", "Stint", (6, 4), (8, 7)),
    car(
        "throttle-brake-traces",
        "Throttle/brake traces",
        (6, 4),
        (10, 7),
    ),
    session("timing-tower", "Timing tower", (4, 6), (6, 12)),
    car("track-discipline", "Track discipline", (4, 2), (6, 3)),
    session("track-map", "Track map", (4, 4), (6, 6)),
    session("weather", "Weather", (4, 2), (6, 2)),
];

/// The declaration of `kind`; `None` for a type this build does not know.
pub fn declaration(kind: &str) -> Option<&'static Declaration> {
    DECLARATIONS.iter().find(|d| d.kind == kind)
}

/// Every registered type for the add-tile picker, sorted by display name ignoring case.
/// The picker never filters by availability.
pub fn picker() -> Vec<&'static Declaration> {
    let mut all: Vec<_> = DECLARATIONS.iter().collect();
    all.sort_by_key(|d| d.name.to_lowercase());
    all
}
