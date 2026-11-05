//! Public native tile registration boundary. See `docs/native/dashboard.md`.

use crate::tiles::{lap_timing, speed_rpm_gear};
use gpui::{AnyElement, IntoElement};
use pitwall_core::{
    binding::ResolvedCar,
    feed::{FastFrame, Snapshot},
    layouts::Tile,
    model::Readout,
    units::Units,
};

pub struct TileInputs<'a> {
    pub tile: &'a Tile,
    pub snapshot: &'a Snapshot,
    pub fast: &'a FastFrame,
    pub car: ResolvedCar,
    pub units: Units,
    /// Content size after native tile chrome has been reserved, in logical pixels.
    pub size: (f32, f32),
}

pub struct SettingField {
    pub key: &'static str,
    pub label: &'static str,
    pub choices: &'static [(&'static str, &'static str)],
    pub default: &'static str,
}

pub struct TileRegistration {
    pub kind: &'static str,
    pub name: &'static str,
    pub minimum: (u32, u32),
    pub default: (u32, u32),
    pub follows_car: bool,
    /// None explicitly marks a catalogue type whose native renderer is not ported.
    pub render: Option<fn(&TileInputs<'_>) -> AnyElement>,
    pub settings: &'static [SettingField],
}

pub fn get(kind: &str) -> Option<&'static TileRegistration> {
    ALL.iter().find(|t| t.kind == kind)
}

fn speed(inputs: &TileInputs<'_>) -> AnyElement {
    let car = inputs.car.index().and_then(|i| inputs.snapshot.car(i));
    speed_rpm_gear::render(speed_rpm_gear::view(&speed_rpm_gear::Inputs {
        car: inputs
            .car
            .index()
            .map_or(Readout::Absent, |i| inputs.fast.cars[i]),
        max_rpm: car
            .and_then(|c| c.status().value().map(|s| s.max_rpm))
            .filter(|&m| m > 0),
        units: inputs.units,
        lifecycle: inputs.snapshot.lifecycle(),
        width: inputs.size.0,
        height: inputs.size.1,
    }))
    .into_any_element()
}

fn timing(inputs: &TileInputs<'_>) -> AnyElement {
    let car = inputs.car.index().and_then(|i| inputs.snapshot.car(i));
    lap_timing::render(lap_timing::view(&lap_timing::Inputs::of(
        car,
        lap_timing::LapReference::of(&inputs.tile.settings),
        inputs.snapshot.lifecycle(),
        inputs.size,
    )))
    .into_any_element()
}

pub static ALL: &[TileRegistration] = &[
    TileRegistration {
        kind: "speed-rpm-gear",
        name: speed_rpm_gear::NAME,
        minimum: (4, 3),
        default: (6, 6),
        follows_car: true,
        render: Some(speed),
        settings: &[],
    },
    TileRegistration {
        kind: "lap-timing",
        name: lap_timing::NAME,
        minimum: (4, 3),
        default: (6, 4),
        follows_car: true,
        render: Some(timing),
        settings: &[SettingField {
            key: "reference",
            label: "Reference lap",
            choices: &[
                ("best", "Best"),
                ("previous", "Previous"),
                ("personal-best", "Time Trial personal best"),
                ("rival", "Time Trial rival"),
            ],
            default: "best",
        }],
    },
    pending("car-status", "Car status", (8, 6), (12, 12), true),
    pending("engine-temp", "Engine temp", (2, 2), (3, 2), true),
    pending("ers", "ERS", (4, 2), (6, 4), true),
    pending("fuel", "Fuel", (3, 2), (6, 2), true),
    pending("g-force", "G-force", (2, 2), (3, 2), true),
    pending("gaps", "Gaps", (4, 2), (8, 2), true),
    pending("lap-state", "Lap state", (4, 2), (6, 2), true),
    pending("live-delta", "Live delta", (6, 3), (12, 6), true),
    pending("notice", "Notice", (4, 1), (6, 1), false),
    pending(
        "race-control-feed",
        "Race-control feed",
        (4, 2),
        (6, 6),
        false,
    ),
    pending("sm", "SM", (2, 2), (4, 2), true),
    pending("stint", "Stint", (6, 4), (8, 7), true),
    pending(
        "throttle-brake-traces",
        "Throttle/brake traces",
        (6, 4),
        (10, 7),
        true,
    ),
    pending("timing-tower", "Timing tower", (4, 6), (6, 12), false),
    pending("track-discipline", "Track discipline", (4, 2), (6, 3), true),
    pending("track-map", "Track map", (4, 4), (6, 6), false),
    pending("weather", "Weather", (4, 2), (6, 2), false),
];

const fn pending(
    kind: &'static str,
    name: &'static str,
    minimum: (u32, u32),
    default: (u32, u32),
    follows_car: bool,
) -> TileRegistration {
    TileRegistration {
        kind,
        name,
        minimum,
        default,
        follows_car,
        render: None,
        settings: &[],
    }
}
