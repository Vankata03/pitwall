//! Native tile renderers. A tile type's name, car binding and grid sizes come from
//! `pitwall_core::tile_catalog`; this list pairs a type with the gpui code that draws it
//! and the settings its dialog offers. A declared type with no entry here draws a
//! "not yet ported" placeholder and keeps its data; see `notes/70-layouts.md`.

use crate::tiles::{
    car_status, engine_temp, ers, fuel, g_force, gaps, lap_state, lap_timing, live_delta,
    memory::Memory, notice, race_control_feed, sm, speed_rpm_gear, stint, throttle_brake_traces,
    timing_tower, track_discipline, track_map, weather,
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
    /// Draws the tile; `Memory` is what this placed tile keeps between renders.
    pub prepare: fn(&TileInputs<'_>, &mut Memory) -> TileView,
    pub settings: &'static [SettingField],
}

/// A checkbox group. Empty selections and unknown saved strings are retained.
pub struct ListSettingField {
    pub key: &'static str,
    pub label: &'static str,
    pub choices: &'static [(&'static str, &'static str)],
}

impl ListSettingField {
    pub fn selected(&self, settings: &Value) -> Vec<String> {
        match settings.get(self.key) {
            Some(Value::Array(items)) => items
                .iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect(),
            _ => self
                .choices
                .iter()
                .map(|(value, _)| (*value).to_owned())
                .collect(),
        }
    }

    /// Value for `Command::Setting`; the controller preserves the other settings.
    pub fn toggled(&self, settings: &Value, choice: &str) -> Value {
        let mut selected = self.selected(settings);
        if selected.iter().any(|value| value == choice) {
            selected.retain(|value| value != choice);
        } else {
            selected.push(choice.to_owned());
        }
        Value::from(selected)
    }
}

const FEED_SETTINGS: &[ListSettingField] = &[ListSettingField {
    key: "entryTypes",
    label: "Entry types",
    choices: &race_control_feed::ENTRY_TYPES,
}];

impl Renderer {
    pub fn list_settings(&self) -> &'static [ListSettingField] {
        if self.kind == race_control_feed::TYPE {
            FEED_SETTINGS
        } else {
            &[]
        }
    }
    /// These types used the web frame channel; slow tiles retain their 5 Hz model.
    pub fn frame_rate(&self) -> bool {
        matches!(
            self.kind,
            "speed-rpm-gear"
                | "lap-timing"
                | "live-delta"
                | "g-force"
                | "ers"
                | "sm"
                | "throttle-brake-traces"
                | "track-map"
        )
    }
}

/// Prepared display values: equality is the repaint boundary, independent of feed generations.
#[derive(Debug, Clone, PartialEq)]
pub enum TileView {
    SpeedRpmGear(speed_rpm_gear::View),
    LapTiming(lap_timing::View),
    GForce(g_force::View),
    ThrottleBrakeTraces(throttle_brake_traces::View),
    LiveDelta(live_delta::View),
    Ers(ers::View),
    CarStatus(car_status::View),
    EngineTemp(engine_temp::View),
    Fuel(fuel::View),
    TimingTower(timing_tower::View),
    Gaps(gaps::View),
    LapState(lap_state::View),
    Stint(stint::View),
    Sm(sm::View),
    TrackDiscipline(track_discipline::View),
    Weather(weather::View),
    Notice(notice::View),
    RaceControlFeed(race_control_feed::View),
    TrackMap(track_map::View),
}
impl TileView {
    fn render(
        self,
        memory: &mut Memory,
        window: &mut gpui::Window,
        cx: &mut gpui::App,
    ) -> AnyElement {
        match self {
            Self::SpeedRpmGear(view) => speed_rpm_gear::render(view).into_any_element(),
            Self::LapTiming(view) => lap_timing::render(view).into_any_element(),
            Self::GForce(view) => g_force::render(view).into_any_element(),
            Self::ThrottleBrakeTraces(view) => {
                throttle_brake_traces::render(view).into_any_element()
            }
            Self::LiveDelta(view) => live_delta::render(view).into_any_element(),
            Self::Ers(view) => ers::render(view).into_any_element(),
            Self::CarStatus(view) => {
                car_status::render(view, &mut memory.car_status).into_any_element()
            }
            Self::EngineTemp(view) => engine_temp::render(view).into_any_element(),
            Self::Fuel(view) => fuel::render(view).into_any_element(),
            Self::TimingTower(view) => timing_tower::render(view).into_any_element(),
            Self::Gaps(view) => gaps::render(view).into_any_element(),
            Self::LapState(view) => lap_state::render(view).into_any_element(),
            Self::Stint(view) => {
                stint::render(view, &mut memory.stint, window, cx).into_any_element()
            }
            Self::Sm(view) => sm::render(view).into_any_element(),
            Self::TrackDiscipline(view) => track_discipline::render(view).into_any_element(),
            Self::Weather(view) => weather::render(view).into_any_element(),
            Self::Notice(view) => notice::render(view).into_any_element(),
            Self::RaceControlFeed(view) => race_control_feed::render(view).into_any_element(),
            Self::TrackMap(view) => track_map::render(view).into_any_element(),
        }
    }
}

/// One retained gpui content entity per placed tile. Preparing a view can update
/// traces and announcement state, but painting is invalidated only by changed content.
#[derive(Default)]
pub struct TileSurface {
    memory: Memory,
    view: Option<TileView>,
}
impl TileSurface {
    pub fn prepare(&mut self, renderer: &Renderer, inputs: &TileInputs<'_>) -> bool {
        let next = (renderer.prepare)(inputs, &mut self.memory);
        if self.view.as_ref() == Some(&next) {
            return false;
        }
        self.view = Some(next);
        true
    }
}
impl gpui::Render for TileSurface {
    fn render(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        self.view
            .clone()
            .expect("tile prepared before render")
            .render(&mut self.memory, window, cx)
    }
}

pub fn renderer(kind: &str) -> Option<&'static Renderer> {
    RENDERERS.iter().find(|r| r.kind == kind)
}

fn speed(inputs: &TileInputs<'_>, _: &mut Memory) -> TileView {
    TileView::SpeedRpmGear(speed_rpm_gear::view(&speed_rpm_gear::Inputs::from_source(
        inputs.fast,
        inputs.snapshot,
        inputs.car.index(),
        inputs.units,
        inputs.size,
    )))
}

fn timing(inputs: &TileInputs<'_>, _: &mut Memory) -> TileView {
    TileView::LapTiming(lap_timing::view(&lap_timing::Inputs::from_model(
        inputs.snapshot,
        inputs.car.index(),
        lap_timing::LapReference::of(&inputs.tile.settings),
        inputs.size,
    )))
}

fn g_force(inputs: &TileInputs<'_>, _: &mut Memory) -> TileView {
    TileView::GForce(g_force::view(&g_force::Inputs::from_model(
        inputs.snapshot,
        inputs.car.index(),
        inputs.size,
    )))
}

fn traces(inputs: &TileInputs<'_>, memory: &mut Memory) -> TileView {
    TileView::ThrottleBrakeTraces(throttle_brake_traces::view(&memory.traces.inputs(
        inputs.reader,
        inputs.fast,
        inputs.snapshot,
        inputs.car.index(),
        &inputs.tile.settings,
        inputs.units,
        inputs.size,
    )))
}

fn delta(inputs: &TileInputs<'_>, memory: &mut Memory) -> TileView {
    TileView::LiveDelta(live_delta::view(&memory.delta.inputs(
        inputs.reader,
        inputs.snapshot,
        inputs.car.index(),
        live_delta::Reference::best_or_previous_of(&inputs.tile.settings),
        inputs.size,
    )))
}

fn ers(inputs: &TileInputs<'_>, _: &mut Memory) -> TileView {
    TileView::Ers(ers::view(&ers::Inputs::from_model(
        inputs.snapshot,
        inputs.car.index(),
        inputs.size,
    )))
}

fn car_status(inputs: &TileInputs<'_>, _: &mut Memory) -> TileView {
    TileView::CarStatus(car_status::view(&car_status::Inputs::from_model(
        inputs.snapshot,
        inputs.car.index(),
        matches!(inputs.car, ResolvedCar::Retired(_)),
        inputs.units,
        inputs.size,
    )))
}

fn engine_temp(inputs: &TileInputs<'_>, _: &mut Memory) -> TileView {
    TileView::EngineTemp(engine_temp::view(&engine_temp::Inputs::from_model(
        inputs.snapshot,
        inputs.car.index(),
        inputs.units,
        inputs.size,
    )))
}

fn fuel(inputs: &TileInputs<'_>, _: &mut Memory) -> TileView {
    TileView::Fuel(fuel::view(&fuel::Inputs::from_model(
        inputs.snapshot,
        inputs.car.index(),
        inputs.size,
    )))
}

fn tower(inputs: &TileInputs<'_>, _: &mut Memory) -> TileView {
    // A session-wide tile resolves the default Focus target: the window's focus car.
    TileView::TimingTower(timing_tower::view(&timing_tower::Inputs::from_model(
        inputs.snapshot,
        inputs.car.index(),
        pitwall_core::tower::TowerSettings::from_json(&inputs.tile.settings),
        inputs.size,
    )))
}

fn gaps(inputs: &TileInputs<'_>, _: &mut Memory) -> TileView {
    TileView::Gaps(gaps::view(&gaps::Inputs::from_model(
        inputs.snapshot,
        inputs.car.index(),
        inputs.size,
    )))
}

fn lap_state(inputs: &TileInputs<'_>, _: &mut Memory) -> TileView {
    TileView::LapState(lap_state::view(&lap_state::Inputs::from_model(
        inputs.snapshot,
        inputs.car.index(),
        inputs.size,
    )))
}

fn stint(inputs: &TileInputs<'_>, _: &mut Memory) -> TileView {
    TileView::Stint(stint::view(&stint::Inputs::from_model(
        inputs.snapshot,
        inputs.car.index(),
        matches!(inputs.car, ResolvedCar::Retired(_)),
        inputs.size,
    )))
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

fn sm(inputs: &TileInputs<'_>, _: &mut Memory) -> TileView {
    TileView::Sm(sm::view(&sm::Inputs::from_model(
        inputs.snapshot,
        inputs.car.index(),
        inputs.size,
    )))
}

fn track_discipline(inputs: &TileInputs<'_>, _: &mut Memory) -> TileView {
    TileView::TrackDiscipline(track_discipline::view(
        &track_discipline::Inputs::from_model(inputs.snapshot, inputs.car.index(), inputs.size),
    ))
}

fn weather(inputs: &TileInputs<'_>, _: &mut Memory) -> TileView {
    TileView::Weather(weather::view(&weather::Inputs::from_model(
        inputs.snapshot,
        inputs.units,
        inputs.size,
    )))
}

fn notice(inputs: &TileInputs<'_>, _: &mut Memory) -> TileView {
    TileView::Notice(notice::view(inputs.size))
}

fn race_control_feed(inputs: &TileInputs<'_>, memory: &mut Memory) -> TileView {
    let feed =
        race_control_feed::Inputs::from_model(inputs.snapshot, &inputs.tile.settings, inputs.size);
    let shown =
        memory
            .race_control_feed
            .announce_now(feed.candidates(), feed.epoch, &feed.entry_types);
    TileView::RaceControlFeed(race_control_feed::view(&feed, &shown))
}

/// Session-wide: `inputs.car` is the window focus, or the player while the window follows
/// the player.
fn track_map(inputs: &TileInputs<'_>, memory: &mut Memory) -> TileView {
    let memory = &mut memory.track_map;
    let map = memory.inputs(
        inputs.snapshot,
        track_map::TRACKS.get(),
        inputs.car.index(),
        inputs.size,
    );
    TileView::TrackMap(track_map::view(&map, &mut memory.fit))
}

/// Best (default) or previous: Live delta and Throttle/brake traces.
const BEST_OR_PREVIOUS: &[SettingField] = &[SettingField {
    key: "reference",
    label: "Reference lap",
    choices: &[
        Choice {
            value: pitwall_core::model::Reference::Best.key(),
            label: "Best lap",
            time_trial_only: false,
        },
        Choice {
            value: pitwall_core::model::Reference::Previous.key(),
            label: "Previous lap",
            time_trial_only: false,
        },
    ],
    default: "best",
}];

pub static RENDERERS: &[Renderer] = &[
    Renderer {
        kind: speed_rpm_gear::TYPE,
        prepare: speed,
        settings: &[],
    },
    Renderer {
        kind: lap_timing::TYPE,
        prepare: timing,
        settings: &[SettingField {
            key: "reference",
            label: "Reference lap",
            choices: &[
                Choice {
                    value: pitwall_core::model::Reference::Best.key(),
                    label: "Best lap",
                    time_trial_only: false,
                },
                Choice {
                    value: pitwall_core::model::Reference::Previous.key(),
                    label: "Previous lap",
                    time_trial_only: false,
                },
                Choice {
                    value: pitwall_core::model::Reference::PersonalBest.key(),
                    label: "Personal best (Time Trial)",
                    time_trial_only: true,
                },
                Choice {
                    value: pitwall_core::model::Reference::Rival.key(),
                    label: "Rival (Time Trial)",
                    time_trial_only: true,
                },
            ],
            default: "best",
        }],
    },
    Renderer {
        kind: g_force::TYPE,
        prepare: g_force,
        settings: &[],
    },
    Renderer {
        kind: throttle_brake_traces::TYPE,
        prepare: traces,
        // The channels setting is a list; its choice arrives with list settings.
        settings: BEST_OR_PREVIOUS,
    },
    Renderer {
        kind: live_delta::TYPE,
        prepare: delta,
        settings: BEST_OR_PREVIOUS,
    },
    Renderer {
        kind: ers::TYPE,
        prepare: ers,
        settings: &[],
    },
    Renderer {
        kind: car_status::TYPE,
        prepare: car_status,
        settings: &[],
    },
    Renderer {
        kind: engine_temp::TYPE,
        prepare: engine_temp,
        settings: &[],
    },
    Renderer {
        kind: fuel::TYPE,
        prepare: fuel,
        settings: &[],
    },
    Renderer {
        kind: timing_tower::TYPE,
        prepare: tower,
        settings: TOWER,
    },
    Renderer {
        kind: gaps::TYPE,
        prepare: gaps,
        settings: &[],
    },
    Renderer {
        kind: lap_state::TYPE,
        prepare: lap_state,
        settings: &[],
    },
    Renderer {
        kind: stint::TYPE,
        prepare: stint,
        settings: &[],
    },
    Renderer {
        kind: sm::TYPE,
        prepare: sm,
        settings: &[],
    },
    Renderer {
        kind: track_discipline::TYPE,
        prepare: track_discipline,
        settings: &[],
    },
    Renderer {
        kind: weather::TYPE,
        prepare: weather,
        settings: &[],
    },
    Renderer {
        kind: notice::TYPE,
        prepare: notice,
        settings: &[],
    },
    Renderer {
        kind: race_control_feed::TYPE,
        prepare: race_control_feed,
        settings: &[],
    },
    Renderer {
        kind: track_map::TYPE,
        prepare: track_map,
        settings: &[],
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use pitwall_core::tile_catalog::declaration;

    #[test]
    fn feed_settings_toggle_known_types_without_losing_future_types_or_empty_selection() {
        let field = &renderer(race_control_feed::TYPE).unwrap().list_settings()[0];
        assert_eq!(field.label, "Entry types");
        assert_eq!(
            field.selected(&serde_json::json!({})),
            vec![
                "Game",
                "FlagChanged",
                "MarshalFlagChanged",
                "SafetyCarChanged",
                "WeatherChanged",
                "PitEntry",
                "PitExit",
                "Flashback"
            ]
        );
        let saved = serde_json::json!({"entryTypes": ["Future", "Game", 7], "other": true});
        assert_eq!(field.toggled(&saved, "Game"), serde_json::json!(["Future"]));
        assert_eq!(
            field.toggled(&serde_json::json!({"entryTypes": ["Game"]}), "Game"),
            serde_json::json!([])
        );
        assert_eq!(
            field.toggled(&serde_json::json!({"entryTypes": []}), "PitExit"),
            serde_json::json!(["PitExit"])
        );
    }

    #[test]
    fn feed_checkbox_changes_use_the_layout_draft_and_preserve_other_settings_on_save() {
        use pitwall_core::{
            layouts::{LayoutLibrary, ScreenClass},
            window::{Command, DashboardWindow, WindowChoices, WindowSetup},
        };
        let root = std::env::temp_dir().join(format!(
            "pitwall-feed-settings-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("layouts.json");
        let library = LayoutLibrary::open(path.clone()).unwrap();
        library.save("Feed", serde_json::from_value(serde_json::json!({"schemaVersion":1,"name":"Feed","arrangements":{"wide":[{"i":"feed","type":"race-control-feed","x":0,"y":0,"w":6,"h":6,"settings":{"entryTypes":["Future","Game"],"other":{"keep":true}}}]}})).unwrap()).unwrap();
        let mut window = DashboardWindow::open(WindowSetup {
            library: library.clone(),
            choices: WindowChoices {
                layout: Some("Feed".into()),
                ..Default::default()
            },
            path: None,
            options: Default::default(),
        });
        let field = &renderer(race_control_feed::TYPE).unwrap().list_settings()[0];
        window.apply(Command::Edit(ScreenClass::Wide));
        let saved = window.tiles(ScreenClass::Wide)[0].settings.clone();
        window.apply(Command::Setting(
            "feed".into(),
            field.key.into(),
            field.toggled(&saved, "Game"),
        ));
        assert_eq!(
            window.tiles(ScreenClass::Wide)[0].settings,
            serde_json::json!({"entryTypes":["Future"],"other":{"keep":true}})
        );
        assert_eq!(
            library.get("Feed").unwrap().arrangement(ScreenClass::Wide)[0].settings["entryTypes"],
            serde_json::json!(["Future", "Game"])
        );
        window.apply(Command::Cancel);
        assert_eq!(window.tiles(ScreenClass::Wide)[0].settings, saved);
        window.apply(Command::Edit(ScreenClass::Wide));
        window.apply(Command::Setting(
            "feed".into(),
            field.key.into(),
            field.toggled(&saved, "Game"),
        ));
        window.apply(Command::Done);
        let reloaded = LayoutLibrary::open(path.clone()).unwrap();
        assert_eq!(
            reloaded.get("Feed").unwrap().arrangement(ScreenClass::Wide)[0].settings,
            serde_json::json!({"entryTypes":["Future"],"other":{"keep":true}})
        );
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn retained_tiles_change_only_when_their_displayed_view_changes() {
        let (mut publisher, reader) = pitwall_core::feed::channel();
        let model = pitwall_core::model::SessionModel::default();
        let tile: Tile = serde_json::from_value(serde_json::json!({"i":"notice", "type":"notice", "x":0,"y":0,"w":4,"h":2,"settings":{}})).unwrap();
        let mut surface = TileSurface::default();
        let mut prepare = |size| {
            let snapshot = reader.snapshot();
            let fast = reader.fast();
            let inputs = TileInputs {
                tile: &tile,
                snapshot: &snapshot,
                fast: &fast,
                reader: &reader,
                car: ResolvedCar::NotInSession,
                units: Units::default(),
                size,
            };
            surface.prepare(renderer("notice").unwrap(), &inputs)
        };
        assert!(prepare((400.0, 100.0)));
        publisher.publish_fast(&model);
        assert!(
            !prepare((400.0, 100.0)),
            "a fast frame cannot repaint a static tile"
        );
        publisher.publish_snapshot(&model);
        assert!(
            !prepare((400.0, 100.0)),
            "an unchanged slow view also keeps its paint"
        );
        assert!(
            prepare((600.0, 100.0)),
            "density changes invalidate the tile"
        );
    }

    #[test]
    fn every_renderer_draws_a_declared_type_once() {
        for (n, r) in RENDERERS.iter().enumerate() {
            let declared = declaration(r.kind).unwrap_or_else(|| panic!("{} is declared", r.kind));
            assert!(!declared.name.is_empty());
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
