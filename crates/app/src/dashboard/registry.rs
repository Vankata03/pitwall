//! Native tile renderers. A tile type's name, car binding and grid sizes come from
//! `pitwall_core::tile_catalog`; this list pairs a type with the gpui code that draws it
//! and the settings its dialog offers. A declared type with no entry here draws a
//! "not yet ported" placeholder and keeps its data; see `notes/70-layouts.md`.

use crate::tiles::{lap_timing, speed_rpm_gear};
use gpui::{AnyElement, IntoElement};
use pitwall_core::{
    binding::ResolvedCar,
    feed::{FastFrame, Snapshot},
    layouts::Tile,
    units::Units,
};

/// Everything a renderer reads for one tile in one frame.
pub struct TileInputs<'a> {
    pub tile: &'a Tile,
    pub snapshot: &'a Snapshot,
    pub fast: &'a FastFrame,
    /// The car this tile follows, already resolved; session-wide tiles ignore it.
    pub car: ResolvedCar,
    pub units: Units,
    /// Content size after the tile header, in logical pixels.
    pub size: (f32, f32),
}

/// One choice of a [`SettingField`].
pub struct Choice {
    pub value: &'static str,
    pub label: &'static str,
    /// Offered only while the session is a Time Trial; a saved value is kept regardless.
    pub time_trial_only: bool,
}

/// A tile's own setting, stored as a string under `settings[key]`. Unknown saved values
/// read as `default` and are left alone until the user picks another.
pub struct SettingField {
    pub key: &'static str,
    pub label: &'static str,
    pub choices: &'static [Choice],
    pub default: &'static str,
}

pub struct Renderer {
    pub kind: &'static str,
    /// The tile module's own name; the same as the catalog's (tested).
    pub name: &'static str,
    pub render: fn(&TileInputs<'_>) -> AnyElement,
    pub settings: &'static [SettingField],
}

pub fn renderer(kind: &str) -> Option<&'static Renderer> {
    RENDERERS.iter().find(|r| r.kind == kind)
}

fn speed(inputs: &TileInputs<'_>) -> AnyElement {
    speed_rpm_gear::render(speed_rpm_gear::view(&speed_rpm_gear::Inputs::from_source(
        inputs.fast,
        inputs.snapshot,
        inputs.car.index(),
        inputs.units,
        inputs.size,
    )))
    .into_any_element()
}

fn timing(inputs: &TileInputs<'_>) -> AnyElement {
    lap_timing::render(lap_timing::view(&lap_timing::Inputs::from_model(
        inputs.snapshot,
        inputs.car.index(),
        lap_timing::LapReference::of(&inputs.tile.settings),
        inputs.size,
    )))
    .into_any_element()
}

pub static RENDERERS: &[Renderer] = &[
    Renderer {
        kind: "speed-rpm-gear",
        name: speed_rpm_gear::NAME,
        render: speed,
        settings: &[],
    },
    Renderer {
        kind: "lap-timing",
        name: lap_timing::NAME,
        render: timing,
        settings: &[SettingField {
            key: "reference",
            label: "Reference lap",
            choices: &[
                Choice {
                    value: "best",
                    label: "Best lap",
                    time_trial_only: false,
                },
                Choice {
                    value: "previous",
                    label: "Previous lap",
                    time_trial_only: false,
                },
                Choice {
                    value: "personal-best",
                    label: "Personal best (Time Trial)",
                    time_trial_only: true,
                },
                Choice {
                    value: "rival",
                    label: "Rival (Time Trial)",
                    time_trial_only: true,
                },
            ],
            default: "best",
        }],
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use pitwall_core::tile_catalog::declaration;

    #[test]
    fn every_renderer_draws_a_declared_type_once() {
        for (n, r) in RENDERERS.iter().enumerate() {
            let declared = declaration(r.kind).unwrap_or_else(|| panic!("{} is declared", r.kind));
            assert_eq!(declared.name, r.name);
            assert!(RENDERERS[n + 1..].iter().all(|o| o.kind != r.kind));
        }
    }

    #[test]
    fn setting_defaults_are_among_their_choices() {
        for r in RENDERERS {
            for field in r.settings {
                assert!(field.choices.iter().any(|c| c.value == field.default));
            }
        }
    }
}
