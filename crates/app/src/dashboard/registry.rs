//! Native tile renderers. A tile type's name, car binding and grid sizes come from
//! `pitwall_core::tile_catalog`; this list pairs a type with the gpui code that draws it
//! and the settings its dialog offers. A declared type with no entry here draws a
//! "not yet ported" placeholder and keeps its data; see `notes/70-layouts.md`.

use crate::tiles::{
    car_status, engine_temp, ers, fuel, g_force, gaps, lap_state, lap_timing, live_delta,
    memory::Memory, speed_rpm_gear, stint, throttle_brake_traces, timing_tower,
};
use gpui::{AnyElement, IntoElement};
use pitwall_core::{
    binding::ResolvedCar,
    feed::{FastFrame, Reader, Snapshot},
    layouts::Tile,
    units::Units,
};
use serde_json::Value;

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

impl SettingField {
    /// A field whose choices are all whole numbers saves a JSON number (the timing
    /// tower's cars each side), as the web build does.
    fn numeric(&self) -> bool {
        self.choices.iter().all(|c| c.value.parse::<i64>().is_ok())
    }

    /// The choice the saved settings select; anything unknown reads as the default.
    pub fn selected(&self, settings: &Value) -> &'static str {
        let saved = settings.get(self.key);
        let text = match saved {
            Some(Value::String(text)) if !self.numeric() => Some(text.clone()),
            Some(Value::Number(n)) if self.numeric() => {
                n.as_f64().map(|n| (n.floor() as i64).to_string())
            }
            _ => None,
        };
        self.choices
            .iter()
            .find(|c| Some(c.value) == text.as_deref())
            .map_or(self.default, |c| c.value)
    }

    /// The JSON value saved for `choice`.
    pub fn stored(&self, choice: &Choice) -> Value {
        match choice.value.parse::<i64>() {
            Ok(n) if self.numeric() => Value::from(n),
            _ => Value::from(choice.value),
        }
    }
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

fn tower(inputs: &TileInputs<'_>, _: &mut Memory) -> AnyElement {
    // A session-wide tile resolves the default Focus target: the window's focus car.
    timing_tower::render(timing_tower::view(&timing_tower::Inputs::from_model(
        inputs.snapshot,
        inputs.car.index(),
        pitwall_core::tower::TowerSettings::from_json(&inputs.tile.settings),
        inputs.size,
    )))
}

fn gaps(inputs: &TileInputs<'_>, _: &mut Memory) -> AnyElement {
    gaps::render(gaps::view(&gaps::Inputs::from_model(
        inputs.snapshot,
        inputs.car.index(),
        inputs.size,
    )))
    .into_any_element()
}

fn lap_state(inputs: &TileInputs<'_>, _: &mut Memory) -> AnyElement {
    lap_state::render(lap_state::view(&lap_state::Inputs::from_model(
        inputs.snapshot,
        inputs.car.index(),
        inputs.size,
    )))
    .into_any_element()
}

fn stint(inputs: &TileInputs<'_>, memory: &mut Memory) -> AnyElement {
    stint::render(
        stint::view(&stint::Inputs::from_model(
            inputs.snapshot,
            inputs.car.index(),
            matches!(inputs.car, ResolvedCar::Retired(_)),
            inputs.size,
        )),
        &mut memory.stint,
    )
}

const fn choice(value: &'static str, label: &'static str) -> Choice {
    Choice {
        value,
        label,
        time_trial_only: false,
    }
}

/// Timing tower: gap reference, rows, and cars each side of focus (0–12, default 3).
const TOWER: &[SettingField] = &[
    SettingField {
        key: "gap",
        label: "Gap reference",
        choices: &[
            choice("interval", "Interval"),
            choice("leader", "Leader"),
            choice("focus", "Focus-relative"),
        ],
        default: "interval",
    },
    SettingField {
        key: "rows",
        label: "Rows",
        choices: &[
            choice("auto", "Density default"),
            choice("full", "Full field"),
            choice("around", "Around focus"),
        ],
        default: "auto",
    },
    SettingField {
        key: "radius",
        label: "Cars each side",
        choices: &[
            choice("0", "0"),
            choice("1", "1"),
            choice("2", "2"),
            choice("3", "3"),
            choice("4", "4"),
            choice("5", "5"),
            choice("6", "6"),
            choice("7", "7"),
            choice("8", "8"),
            choice("9", "9"),
            choice("10", "10"),
            choice("11", "11"),
            choice("12", "12"),
        ],
        default: "3",
    },
];

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
        kind: timing_tower::TYPE,
        name: timing_tower::NAME,
        render: tower,
        settings: TOWER,
    },
    Renderer {
        kind: gaps::TYPE,
        name: gaps::NAME,
        render: gaps,
        settings: &[],
    },
    Renderer {
        kind: lap_state::TYPE,
        name: lap_state::NAME,
        render: lap_state,
        settings: &[],
    },
    Renderer {
        kind: stint::TYPE,
        name: stint::NAME,
        render: stint,
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
    fn tower_settings_keep_web_values_and_save_cars_each_side_as_a_number() {
        let tower = renderer(timing_tower::TYPE).unwrap();
        let field = |key: &str| tower.settings.iter().find(|f| f.key == key).unwrap();
        let (gap, rows, radius) = (field("gap"), field("rows"), field("radius"));
        assert_eq!(gap.selected(&serde_json::json!({})), "interval");
        assert_eq!(
            gap.selected(&serde_json::json!({ "gap": "focus" })),
            "focus"
        );
        assert_eq!(rows.selected(&serde_json::json!({ "rows": "odd" })), "auto");
        assert_eq!(radius.selected(&serde_json::json!({})), "3");
        assert_eq!(radius.selected(&serde_json::json!({ "radius": 5 })), "5");
        assert_eq!(radius.selected(&serde_json::json!({ "radius": 5.7 })), "5");
        assert_eq!(radius.selected(&serde_json::json!({ "radius": "5" })), "3");
        let five = radius.choices.iter().find(|c| c.value == "5").unwrap();
        assert_eq!(radius.stored(five), serde_json::json!(5));
        let focus = gap.choices.iter().find(|c| c.value == "focus").unwrap();
        assert_eq!(gap.stored(focus), serde_json::json!("focus"));
        // A saved number round-trips into the tower's own reading.
        let saved = serde_json::json!({ "other": "keep", "radius": radius.stored(five) });
        assert_eq!(
            pitwall_core::tower::TowerSettings::from_json(&saved).radius,
            5
        );
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
