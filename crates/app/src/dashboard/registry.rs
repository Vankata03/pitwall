//! Native tile renderers. A tile type's name, car binding and grid sizes come from
//! `pitwall_core::tile_catalog`; this list pairs a type with the gpui code that draws it
//! and the settings its dialog offers. A declared type with no entry here draws a
//! "not yet ported" placeholder and keeps its data; see `notes/70-layouts.md`.

use crate::tiles::{
    car_status, engine_temp, ers, fuel, g_force, lap_timing, live_delta, memory::Memory, notice,
    race_control_feed, sm, speed_rpm_gear, throttle_brake_traces, track_discipline, track_map,
    weather,
};
use gpui::{AnyElement, IntoElement};
use pitwall_core::{
    binding::ResolvedCar,
    feed::{FastFrame, Reader, Snapshot},
    layouts::Tile,
    units::Units,
};

/// Everything a renderer reads for one tile in one frame.
pub struct TileInputs<'a> {
    pub tile: &'a Tile,
    pub snapshot: &'a Snapshot,
    pub fast: &'a FastFrame,
    /// For tiles that catch up on every fast frame since their last render.
    pub reader: &'a Reader,
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
    /// Draws the tile; `Memory` is what this placed tile keeps between renders.
    pub render: fn(&TileInputs<'_>, &mut Memory) -> AnyElement,
    pub settings: &'static [SettingField],
}

pub fn renderer(kind: &str) -> Option<&'static Renderer> {
    RENDERERS.iter().find(|r| r.kind == kind)
}

fn speed(inputs: &TileInputs<'_>, _: &mut Memory) -> AnyElement {
    speed_rpm_gear::render(speed_rpm_gear::view(&speed_rpm_gear::Inputs::from_source(
        inputs.fast,
        inputs.snapshot,
        inputs.car.index(),
        inputs.units,
        inputs.size,
    )))
    .into_any_element()
}

fn timing(inputs: &TileInputs<'_>, _: &mut Memory) -> AnyElement {
    lap_timing::render(lap_timing::view(&lap_timing::Inputs::from_model(
        inputs.snapshot,
        inputs.car.index(),
        lap_timing::LapReference::of(&inputs.tile.settings),
        inputs.size,
    )))
    .into_any_element()
}

fn g_force(inputs: &TileInputs<'_>, _: &mut Memory) -> AnyElement {
    g_force::render(g_force::view(&g_force::Inputs::from_model(
        inputs.snapshot,
        inputs.car.index(),
        inputs.size,
    )))
    .into_any_element()
}

fn traces(inputs: &TileInputs<'_>, memory: &mut Memory) -> AnyElement {
    throttle_brake_traces::render(throttle_brake_traces::view(&memory.traces.inputs(
        inputs.reader,
        inputs.fast,
        inputs.snapshot,
        inputs.car.index(),
        &inputs.tile.settings,
        inputs.units,
        inputs.size,
    )))
    .into_any_element()
}

fn delta(inputs: &TileInputs<'_>, _: &mut Memory) -> AnyElement {
    live_delta::render(live_delta::view(&live_delta::Inputs::from_model(
        inputs.snapshot,
        inputs.car.index(),
        live_delta::Reference::of(&inputs.tile.settings),
        inputs.size,
    )))
    .into_any_element()
}

fn ers(inputs: &TileInputs<'_>, _: &mut Memory) -> AnyElement {
    ers::render(ers::view(&ers::Inputs::from_model(
        inputs.snapshot,
        inputs.car.index(),
        inputs.size,
    )))
    .into_any_element()
}

fn car_status(inputs: &TileInputs<'_>, memory: &mut Memory) -> AnyElement {
    car_status::render(
        car_status::view(&car_status::Inputs::from_model(
            inputs.snapshot,
            inputs.car.index(),
            matches!(inputs.car, ResolvedCar::Retired(_)),
            inputs.units,
            inputs.size,
        )),
        &mut memory.car_status,
    )
    .into_any_element()
}

fn engine_temp(inputs: &TileInputs<'_>, _: &mut Memory) -> AnyElement {
    engine_temp::render(engine_temp::view(&engine_temp::Inputs::from_model(
        inputs.snapshot,
        inputs.car.index(),
        inputs.units,
        inputs.size,
    )))
    .into_any_element()
}

fn fuel(inputs: &TileInputs<'_>, _: &mut Memory) -> AnyElement {
    fuel::render(fuel::view(&fuel::Inputs::from_model(
        inputs.snapshot,
        inputs.car.index(),
        inputs.size,
    )))
    .into_any_element()
}

fn sm(inputs: &TileInputs<'_>, _: &mut Memory) -> AnyElement {
    sm::render(sm::view(&sm::Inputs::from_model(
        inputs.snapshot,
        inputs.car.index(),
        inputs.size,
    )))
    .into_any_element()
}

fn track_discipline(inputs: &TileInputs<'_>, _: &mut Memory) -> AnyElement {
    track_discipline::render(track_discipline::view(
        &track_discipline::Inputs::from_model(inputs.snapshot, inputs.car.index(), inputs.size),
    ))
    .into_any_element()
}

fn weather(inputs: &TileInputs<'_>, _: &mut Memory) -> AnyElement {
    weather::render(weather::view(&weather::Inputs::from_model(
        inputs.snapshot,
        inputs.units,
        inputs.size,
    )))
    .into_any_element()
}

fn notice(inputs: &TileInputs<'_>, _: &mut Memory) -> AnyElement {
    notice::render(notice::view(inputs.size)).into_any_element()
}

fn race_control_feed(inputs: &TileInputs<'_>, memory: &mut Memory) -> AnyElement {
    let feed =
        race_control_feed::Inputs::from_model(inputs.snapshot, &inputs.tile.settings, inputs.size);
    let shown =
        memory
            .race_control_feed
            .announce_now(feed.candidates(), feed.epoch, &feed.entry_types);
    race_control_feed::render(race_control_feed::view(&feed, &shown)).into_any_element()
}

/// Session-wide: `inputs.car` is the window focus, or the player while the window follows
/// the player.
fn track_map(inputs: &TileInputs<'_>, memory: &mut Memory) -> AnyElement {
    let memory = &mut memory.track_map;
    let map = memory.inputs(
        inputs.snapshot,
        track_map::TRACKS.get(),
        inputs.car.index(),
        inputs.size,
    );
    track_map::render(track_map::view(&map, &mut memory.fit)).into_any_element()
}

/// Best (default) or previous: Live delta and Throttle/brake traces.
const BEST_OR_PREVIOUS: &[SettingField] = &[SettingField {
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
    ],
    default: "best",
}];

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
    Renderer {
        kind: g_force::TYPE,
        name: g_force::NAME,
        render: g_force,
        settings: &[],
    },
    Renderer {
        kind: throttle_brake_traces::TYPE,
        name: throttle_brake_traces::NAME,
        render: traces,
        // The channels setting is a list; its choice arrives with list settings.
        settings: BEST_OR_PREVIOUS,
    },
    Renderer {
        kind: live_delta::TYPE,
        name: live_delta::NAME,
        render: delta,
        settings: BEST_OR_PREVIOUS,
    },
    Renderer {
        kind: ers::TYPE,
        name: ers::NAME,
        render: ers,
        settings: &[],
    },
    Renderer {
        kind: car_status::TYPE,
        name: car_status::NAME,
        render: car_status,
        settings: &[],
    },
    Renderer {
        kind: engine_temp::TYPE,
        name: engine_temp::NAME,
        render: engine_temp,
        settings: &[],
    },
    Renderer {
        kind: fuel::TYPE,
        name: fuel::NAME,
        render: fuel,
        settings: &[],
    },
    Renderer {
        kind: sm::TYPE,
        name: sm::NAME,
        render: sm,
        settings: &[],
    },
    Renderer {
        kind: track_discipline::TYPE,
        name: track_discipline::NAME,
        render: track_discipline,
        settings: &[],
    },
    Renderer {
        kind: weather::TYPE,
        name: weather::NAME,
        render: weather,
        settings: &[],
    },
    Renderer {
        kind: notice::TYPE,
        name: notice::NAME,
        render: notice,
        settings: &[],
    },
    Renderer {
        kind: race_control_feed::TYPE,
        name: race_control_feed::NAME,
        render: race_control_feed,
        // The entryTypes setting is a list; its choice arrives with list settings.
        settings: &[],
    },
    Renderer {
        kind: track_map::TYPE,
        name: track_map::NAME,
        render: track_map,
        settings: &[],
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
