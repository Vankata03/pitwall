//! One dashboard window: its remembered choices (`dashboard/src/device/device.test.ts`,
//! natively one file per window, parity DEVICE-01 and ADR3-01) and the commands its chrome
//! and editor send (parity EDIT, BIND-01, CHROME-01).
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use pitwall_core::binding::{CarBinding, CarIdentity, CarTarget};
use pitwall_core::layout_edit::{ControllerOptions, GridItem};
use pitwall_core::layouts::{Layout, LayoutLibrary, ScreenClass};
use pitwall_core::units::{PressureUnit, SpeedUnit, TemperatureUnit, Units};
use pitwall_core::window::{Command, DashboardWindow, WindowChoices, WindowSetup, choices_path};
use serde_json::{Value, json};

fn temp_dir() -> PathBuf {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "pitwall-window-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn layout(value: Value) -> Layout {
    serde_json::from_value(value).unwrap()
}

fn custom() -> Layout {
    layout(
        json!({"schemaVersion":1,"name":"Custom","future":42,"arrangements":{"wide":[
            {"i":"future","type":"future-tile","x":0,"y":0,"w":4,"h":3,"settings":{"opaque":[1,2]},"extra":true},
            {"i":"lap-timing","type":"lap-timing","x":4,"y":0,"w":6,"h":4,
             "settings":{"reference":"previous","mine":"kept","car":{"target":"pinned","pin":{"kind":"driver","driverId":9}}}}
        ]}}),
    )
}

struct Fixture {
    root: PathBuf,
    library: LayoutLibrary,
}

impl Fixture {
    fn new() -> Self {
        let root = temp_dir();
        let library = LayoutLibrary::open(root.join("layouts.json")).unwrap();
        library.save("Custom", custom()).unwrap();
        Self { root, library }
    }
    fn path(&self) -> PathBuf {
        choices_path(&self.root, "main")
    }
    fn open(&self, choices: WindowChoices) -> DashboardWindow {
        self.open_at(choices, Some(self.path()))
    }
    fn open_at(&self, choices: WindowChoices, path: Option<PathBuf>) -> DashboardWindow {
        DashboardWindow::open(WindowSetup {
            library: self.library.clone(),
            choices,
            path,
            options: ControllerOptions::default(),
        })
    }
    fn custom_window(&self) -> DashboardWindow {
        self.open(WindowChoices {
            layout: Some("Custom".into()),
            ..Default::default()
        })
    }
}

fn editing_tile(window: &DashboardWindow, id: &str) -> pitwall_core::layouts::Tile {
    let edit = window.controller().edit().expect("editing");
    edit.draft
        .arrangement(edit.class)
        .into_iter()
        .find(|t| t.i == id)
        .expect("tile")
}

// choices

#[test]
fn a_new_window_starts_at_the_defaults_and_writes_nothing() {
    let f = Fixture::new();
    let (choices, issue) = WindowChoices::load(&f.path()).unwrap();
    assert_eq!(choices, WindowChoices::default());
    assert_eq!(choices.units, Units::DEFAULT);
    assert!(issue.is_none());
    let window = f.open(choices);
    assert_eq!(window.current_name(), Some("Pitwall Classic"));
    assert!(!f.path().exists());
}

#[test]
fn windows_remember_their_own_choices_across_a_reopen() {
    let f = Fixture::new();
    let first = choices_path(&f.root, "first");
    let second = choices_path(&f.root, "second");
    let mine = WindowChoices {
        layout: Some("Driver copy".into()),
        focus: Some(CarIdentity::Driver { driver_id: 9 }),
        units: Units {
            speed: SpeedUnit::Mph,
            temperature: TemperatureUnit::Fahrenheit,
            pressure: PressureUnit::Bar,
        },
    };
    mine.save(&first).unwrap();
    assert_eq!(WindowChoices::load(&first).unwrap().0, mine);
    assert_eq!(
        WindowChoices::load(&second).unwrap().0,
        WindowChoices::default()
    );
}

#[test]
fn corrupt_or_unknown_choices_fall_back_one_by_one() {
    let f = Fixture::new();
    std::fs::create_dir_all(f.path().parent().unwrap()).unwrap();
    std::fs::write(
        f.path(),
        r#"{"layout":5,"focus":{"kind":"robot"},"units":{"speed":"mph","temperature":"kelvin"}}"#,
    )
    .unwrap();
    let (choices, issue) = WindowChoices::load(&f.path()).unwrap();
    assert!(issue.is_none());
    assert_eq!(choices.layout, None);
    assert_eq!(choices.focus, None);
    assert_eq!(
        choices.units,
        Units {
            speed: SpeedUnit::Mph,
            ..Units::DEFAULT
        }
    );
}

#[test]
fn an_unreadable_choices_file_is_kept_as_bad_and_the_window_starts_at_defaults() {
    let f = Fixture::new();
    std::fs::create_dir_all(f.path().parent().unwrap()).unwrap();
    std::fs::write(f.path(), "{not json").unwrap();
    let (choices, issue) = WindowChoices::load(&f.path()).unwrap();
    assert_eq!(choices, WindowChoices::default());
    let kept = issue.expect("reported");
    assert_eq!(kept.file_name().unwrap(), "main.json.bad");
    assert_eq!(std::fs::read_to_string(&kept).unwrap(), "{not json");
    assert!(!f.path().exists());
}

#[test]
fn choosing_a_layout_focus_and_units_is_remembered_for_this_window() {
    let f = Fixture::new();
    let mut window = f.open(WindowChoices::default());
    window.apply(Command::Select("Custom".into()));
    window.apply(Command::Focus(Some(CarIdentity::Name {
        name: "Max".into(),
    })));
    window.apply(Command::Units(Units {
        pressure: PressureUnit::Bar,
        ..Units::DEFAULT
    }));
    let saved = WindowChoices::load(&f.path()).unwrap().0;
    assert_eq!(saved.layout.as_deref(), Some("Custom"));
    assert_eq!(saved.focus, Some(CarIdentity::Name { name: "Max".into() }));
    assert_eq!(saved.units.pressure, PressureUnit::Bar);
    window.apply(Command::Focus(None));
    assert_eq!(WindowChoices::load(&f.path()).unwrap().0.focus, None);
}

#[test]
fn when_choices_cannot_be_saved_the_window_keeps_them_and_says_so() {
    let f = Fixture::new();
    let blocker = f.root.join("blocker");
    std::fs::write(&blocker, "a file where a folder should be").unwrap();
    let mut window = f.open_at(WindowChoices::default(), Some(blocker.join("main.json")));
    window.apply(Command::Units(Units {
        speed: SpeedUnit::Mph,
        ..Units::DEFAULT
    }));
    assert_eq!(window.choices().units.speed, SpeedUnit::Mph);
    assert!(window.error().unwrap().contains("Could not save"));
    window.apply(Command::Select("Custom".into()));
    assert_eq!(window.current_name(), Some("Custom"));
}

// editing

#[test]
fn editing_unknown_tiles_keeps_their_data_and_changes_only_the_active_class() {
    let f = Fixture::new();
    let mut window = f.custom_window();
    window.apply(Command::Edit(ScreenClass::Tablet));
    window.apply(Command::MoveResize(vec![GridItem {
        i: "future".into(),
        x: 2,
        y: 3,
        w: 5,
        h: 4,
    }]));
    window.apply(Command::Done);
    let saved = f.library.get("Custom").unwrap();
    assert_eq!(saved.arrangements.wide, custom().arrangements.wide);
    let tablet = saved.arrangements.tablet.as_ref().unwrap();
    let tile = tablet.iter().find(|t| t.i == "future").unwrap();
    assert_eq!((tile.x, tile.y, tile.w, tile.h), (2, 3, 5, 4));
    assert_eq!(tile.settings, json!({"opaque":[1,2]}));
    assert_eq!(tile.extra["extra"], true);
    assert_eq!(saved.extra["future"], 42);
}

#[test]
fn every_registered_type_can_be_added_at_its_default_size_and_unknown_types_cannot() {
    let f = Fixture::new();
    let mut window = f.custom_window();
    window.apply(Command::Edit(ScreenClass::Wide));
    window.apply(Command::Add("weather".into()));
    window.apply(Command::Add("weather".into()));
    window.apply(Command::Add("future-tile".into()));
    let tiles = window.tiles(ScreenClass::Wide);
    let added: Vec<_> = tiles
        .iter()
        .skip(2)
        .map(|t| (t.i.as_str(), t.x, t.y, t.w, t.h))
        .collect();
    assert_eq!(added, [("weather", 0, 4, 6, 2), ("weather-2", 0, 6, 6, 2)]);
}

#[test]
fn a_tile_setting_keeps_every_other_setting() {
    let f = Fixture::new();
    let mut window = f.custom_window();
    window.apply(Command::Edit(ScreenClass::Wide));
    window.apply(Command::Setting(
        "lap-timing".into(),
        "reference".into(),
        json!("best"),
    ));
    let tile = editing_tile(&window, "lap-timing");
    assert_eq!(tile.settings["reference"], "best");
    assert_eq!(tile.settings["mine"], "kept");
    assert_eq!(tile.settings["car"]["pin"]["driverId"], 9);
}

#[test]
fn changing_what_a_tile_follows_keeps_its_pin_for_later() {
    let f = Fixture::new();
    let mut window = f.custom_window();
    window.apply(Command::Edit(ScreenClass::Wide));
    window.apply(Command::Car(
        "lap-timing".into(),
        CarBinding {
            target: CarTarget::CarAhead,
            pin: Some(CarIdentity::Driver { driver_id: 9 }),
        },
    ));
    let tile = editing_tile(&window, "lap-timing");
    assert_eq!(
        tile.settings["car"],
        json!({"target":"car-ahead","pin":{"kind":"driver","driverId":9}})
    );
    assert_eq!(tile.settings["mine"], "kept");
}

#[test]
fn unknown_tiles_have_no_settings_to_change() {
    let f = Fixture::new();
    let mut window = f.custom_window();
    window.apply(Command::Edit(ScreenClass::Wide));
    window.apply(Command::Setting("future".into(), "x".into(), json!(1)));
    window.apply(Command::Car("future".into(), CarBinding::default()));
    assert_eq!(
        editing_tile(&window, "future").settings,
        json!({"opaque":[1,2]})
    );
    assert!(!window.controller().edit().unwrap().dirty);
}

#[test]
fn session_wide_tiles_have_no_car_to_follow() {
    let f = Fixture::new();
    let mut window = f.custom_window();
    window.apply(Command::Edit(ScreenClass::Wide));
    window.apply(Command::Add("weather".into()));
    window.apply(Command::Car(
        "weather".into(),
        CarBinding {
            target: CarTarget::CarAhead,
            pin: None,
        },
    ));
    assert_eq!(editing_tile(&window, "weather").settings, json!({}));
}

#[test]
fn the_window_shows_its_draft_while_editing_and_the_saved_layout_otherwise() {
    let f = Fixture::new();
    let mut window = f.custom_window();
    window.apply(Command::Edit(ScreenClass::Wide));
    window.apply(Command::RemoveTile("future".into()));
    assert_eq!(window.tiles(ScreenClass::Wide).len(), 1);
    window.apply(Command::Cancel);
    assert_eq!(window.tiles(ScreenClass::Wide).len(), 2);
    assert!(!window.controller().is_editing());
}

#[test]
fn a_preset_edit_saves_a_copy_that_this_window_then_remembers() {
    let f = Fixture::new();
    let mut window = f.open(WindowChoices::default());
    window.apply(Command::Edit(ScreenClass::Wide));
    window.apply(Command::RemoveTile("notice".into()));
    window.apply(Command::Done);
    assert_eq!(window.current_name(), Some("Pitwall Classic copy"));
    let saved = WindowChoices::load(&f.path()).unwrap().0;
    assert_eq!(saved.layout.as_deref(), Some("Pitwall Classic copy"));
}

#[test]
fn choices_live_in_one_file_per_window_under_the_data_folder() {
    assert_eq!(
        choices_path(Path::new("data"), "main"),
        Path::new("data").join("windows").join("main.json")
    );
}
