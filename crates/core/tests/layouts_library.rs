//! The layout library and its presets, tested directly instead of through the collector's
//! HTTP routes (`crates/collector/tests/layouts.rs`), which ADR 0001 removes. Parity
//! LAYOUT-01..03 and PRESET-01..03.
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use pitwall_core::layout_edit::generated_classes;
use pitwall_core::layouts::{Layout, LayoutError, LayoutLibrary, ScreenClass, Tile};
use serde_json::{Value, json};

fn temp_file() -> PathBuf {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "pitwall-layouts-library-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir.join("layouts.json")
}

fn open() -> LayoutLibrary {
    LayoutLibrary::open(temp_file()).unwrap()
}

fn names(library: &LayoutLibrary) -> Vec<String> {
    library.list().into_iter().map(|l| l.name).collect()
}

fn custom(name: &str) -> Layout {
    serde_json::from_value(json!({
        "schemaVersion": 1,
        "name": name,
        "arrangements": {"wide": [
            {"i": "a", "type": "fuel", "x": 0, "y": 0, "w": 6, "h": 2, "settings": {"unit": "kg"}},
            {"i": "b", "type": "gaps", "x": 6, "y": 0, "w": 8, "h": 2, "settings": {}},
            {"i": "c", "type": "notice", "x": 0, "y": 2, "w": 24, "h": 1, "settings": {}},
        ]},
    }))
    .unwrap()
}

fn rect(tile: &Tile) -> [u32; 4] {
    [tile.x, tile.y, tile.w, tile.h]
}

fn find<'a>(tiles: &'a [Tile], id: &str) -> &'a Tile {
    tiles
        .iter()
        .find(|t| t.i == id)
        .unwrap_or_else(|| panic!("{id}"))
}

fn types(tiles: &[Tile]) -> Vec<String> {
    let mut set: Vec<String> = tiles.iter().map(|t| t.kind.clone()).collect();
    set.sort();
    set
}

const CLASSES: [ScreenClass; 3] = [ScreenClass::Wide, ScreenClass::Tablet, ScreenClass::Phone];

// presets

#[test]
fn a_fresh_library_lists_the_three_read_only_presets_in_order() {
    let library = open();
    assert_eq!(names(&library), ["Pitwall Classic", "Driver", "Race"]);
    for name in names(&library) {
        assert!(library.is_preset(&name));
    }
}

#[test]
fn pitwall_classic_wide_matches_the_spec_table() {
    let classic = open().get("Pitwall Classic").unwrap();
    let expected = [
        ("speed-rpm-gear", [0, 0, 6, 6]),
        ("engine-temp", [0, 6, 3, 4]),
        ("g-force", [3, 6, 3, 4]),
        ("ers", [0, 10, 6, 4]),
        ("fuel", [0, 14, 6, 2]),
        ("race-control-feed", [6, 0, 12, 2]),
        ("sm", [10, 2, 4, 2]),
        ("car-status", [6, 4, 12, 12]),
        ("weather", [18, 0, 6, 2]),
        ("track-map", [18, 2, 6, 7]),
        ("lap-state", [18, 9, 6, 2]),
        ("lap-timing", [18, 11, 6, 5]),
        ("throttle-brake-traces", [0, 16, 7, 6]),
        ("live-delta", [7, 16, 6, 6]),
        ("stint", [13, 16, 6, 6]),
        ("timing-tower", [19, 16, 5, 6]),
        ("notice", [0, 22, 24, 1]),
    ];
    let wide = &classic.arrangements.wide;
    assert_eq!(wide.len(), expected.len());
    for (id, at) in expected {
        let tile = find(wide, id);
        assert_eq!(tile.kind, id);
        assert_eq!(rect(tile), at, "{id}");
        assert_eq!(tile.settings, json!({}), "{id}");
    }
    assert!(
        wide.iter()
            .all(|t| t.kind != "gaps" && t.kind != "track-discipline")
    );
}

#[test]
fn driver_and_race_carry_their_own_tile_sets() {
    let library = open();
    let driver = library.get("Driver").unwrap();
    let mut driver_types = types(&driver.arrangements.wide);
    driver_types.dedup();
    assert_eq!(driver_types.len(), 9);
    for kind in ["gaps", "track-discipline", "live-delta", "notice"] {
        assert!(driver_types.contains(&kind.to_string()), "{kind}");
    }
    let race = library.get("Race").unwrap();
    assert_eq!(
        types(&race.arrangements.wide),
        [
            "lap-state",
            "notice",
            "race-control-feed",
            "stint",
            "timing-tower",
            "track-map",
            "weather"
        ]
    );
}

#[test]
fn every_preset_authors_valid_wide_tablet_and_phone_arrangements_with_a_notice() {
    let library = open();
    for name in ["Pitwall Classic", "Driver", "Race"] {
        let layout = library.get(name).unwrap();
        assert!(generated_classes(&layout).is_empty(), "{name}");
        let wide = types(&layout.arrangements.wide);
        for class in CLASSES {
            let tiles = layout.arrangement(class);
            for (n, a) in tiles.iter().enumerate() {
                assert!(a.w >= 1 && a.h >= 1 && a.x + a.w <= class.columns());
                for b in &tiles[n + 1..] {
                    let overlap =
                        a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
                    assert!(!overlap, "{name} {class:?}: {} overlaps {}", a.i, b.i);
                }
            }
            assert_eq!(types(&tiles), wide, "{name} {class:?}");
            assert!(tiles.iter().any(|t| t.kind == "notice"));
            assert!(
                tiles
                    .iter()
                    .all(|t| t.i == t.kind && t.settings == json!({}))
            );
        }
        for class in [ScreenClass::Wide, ScreenClass::Tablet] {
            for tile in layout.arrangement(class) {
                let d = pitwall_core::tile_catalog::declaration(&tile.kind).unwrap();
                assert!(
                    tile.w >= d.minimum.0 && tile.h >= d.minimum.1,
                    "{name} {}",
                    tile.i
                );
            }
        }
    }
}

#[test]
fn phone_puts_lap_timing_then_live_delta_first_and_race_starts_with_lap_state() {
    let library = open();
    let first = |name: &str| {
        let mut phone = library.get(name).unwrap().arrangement(ScreenClass::Phone);
        phone.sort_by_key(|t| t.y);
        phone
            .into_iter()
            .map(|t| t.kind)
            .take(3)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        first("Pitwall Classic"),
        ["lap-timing", "live-delta", "race-control-feed"]
    );
    assert_eq!(first("Driver"), ["lap-timing", "live-delta", "gaps"]);
    assert_eq!(first("Race")[0], "lap-state");
}

#[test]
fn authored_tablet_and_narrow_positions_are_kept() {
    let library = open();
    let classic = library.get("Pitwall Classic").unwrap();
    let tablet = classic.arrangement(ScreenClass::Tablet);
    let timing = rect(find(&tablet, "lap-timing"));
    assert_eq!(rect(find(&tablet, "live-delta")), [0, 26, 12, 6]);
    assert_eq!(timing[1] + timing[3], 26);
    assert_eq!(rect(find(&tablet, "ers")), [0, 32, 12, 4]);
    assert_eq!(rect(find(&tablet, "notice")), [0, 59, 12, 1]);
    let driver = library.get("Driver").unwrap();
    assert_eq!(
        rect(find(&driver.arrangements.wide, "live-delta")),
        [0, 13, 24, 6]
    );
    assert_eq!(
        rect(find(&driver.arrangements.wide, "notice")),
        [0, 19, 24, 1]
    );
    assert_eq!(
        rect(find(&driver.arrangement(ScreenClass::Tablet), "live-delta")),
        [0, 21, 12, 6]
    );
    let race = library.get("Race").unwrap();
    assert!(
        race.arrangements
            .wide
            .iter()
            .all(|t| t.kind != "live-delta")
    );
}

// library

#[test]
fn saving_creates_then_replaces_and_survives_a_restart() {
    let file = temp_file();
    let library = LayoutLibrary::open(file.clone()).unwrap();
    assert!(library.save("Mine", custom("Mine")).unwrap());
    let mut replaced = custom("Mine");
    replaced.arrangements.wide.pop();
    assert!(!library.save("Mine", replaced).unwrap());
    let reopened = LayoutLibrary::open(file).unwrap();
    assert_eq!(
        names(&reopened),
        ["Pitwall Classic", "Driver", "Race", "Mine"]
    );
    assert_eq!(reopened.get("Mine").unwrap().arrangements.wide.len(), 2);
    assert!(!reopened.is_preset("Mine"));
}

#[test]
fn the_revision_advances_on_every_change_and_only_then() {
    let library = open();
    let start = library.revision();
    library.save("Mine", custom("Mine")).unwrap();
    library.duplicate("Mine", Some("Two")).unwrap();
    library.rename("Two", "Three").unwrap();
    library.delete("Three").unwrap();
    assert_eq!(library.revision(), start + 4);
    assert!(library.delete("Driver").is_err());
    library.get("Mine");
    assert_eq!(library.revision(), start + 4);
}

#[test]
fn presets_refuse_save_rename_and_delete_but_can_be_duplicated() {
    let library = open();
    assert!(matches!(
        library.save("Race", custom("Race")),
        Err(LayoutError::ReadOnly)
    ));
    assert!(matches!(
        library.rename("Race", "X"),
        Err(LayoutError::ReadOnly)
    ));
    assert!(matches!(library.delete("Race"), Err(LayoutError::ReadOnly)));
    let copy = library.duplicate("Race", None).unwrap();
    assert_eq!(copy.name, "Race copy");
    assert!(!library.is_preset("Race copy"));
    assert_eq!(copy.arrangements, library.get("Race").unwrap().arrangements);
    assert_eq!(library.duplicate("Race", None).unwrap().name, "Race copy 2");
}

#[test]
fn names_are_trimmed_unique_ignoring_case_and_valid() {
    let library = open();
    library.save("Mine", custom("Mine")).unwrap();
    assert!(matches!(
        library.duplicate("Mine", Some("mine")),
        Err(LayoutError::NameTaken)
    ));
    assert!(matches!(
        library.rename("Mine", "Race"),
        Err(LayoutError::NameTaken)
    ));
    assert!(matches!(
        library.save("Pitwall classic", custom("Pitwall classic")),
        Err(LayoutError::ReadOnly)
    ));
    for bad in ["  ", &"x".repeat(65), "tab\there"] {
        assert!(
            matches!(library.rename("Mine", bad), Err(LayoutError::InvalidName)),
            "{bad:?}"
        );
    }
    assert!(matches!(
        library.save("Other", custom("Mine")),
        Err(LayoutError::InvalidLayout(_))
    ));
    assert!(matches!(
        library.rename("Nope", "Z"),
        Err(LayoutError::NotFound)
    ));
    assert!(matches!(library.delete("Nope"), Err(LayoutError::NotFound)));
    assert_eq!(library.rename("Mine", "  Mine2  ").unwrap().name, "Mine2");
    assert_eq!(
        library.rename("Mine2", &"y".repeat(64)).unwrap().name.len(),
        64
    );
    assert!(library.get("Mine").is_none());
}

#[test]
fn unknown_types_settings_and_fields_survive_a_save_and_restart() {
    let file = temp_file();
    let library = LayoutLibrary::open(file.clone()).unwrap();
    let mut body = serde_json::to_value(custom("Future")).unwrap();
    body["schemaVersion"] = json!(7);
    body["futureField"] = json!({"keep": [1, 2]});
    body["arrangements"]["tabletFuture"] = json!("kept");
    body["arrangements"]["wide"][0] = json!({
        "i": "z", "type": "hologram", "x": 0, "y": 0, "w": 4, "h": 4,
        "settings": {"mode": "spooky", "nested": {"n": 1}}, "tileExtra": true,
    });
    let layout: Layout = serde_json::from_value(body.clone()).unwrap();
    library.save("Future", layout).unwrap();
    let saved = serde_json::to_value(LayoutLibrary::open(file).unwrap().get("Future")).unwrap();
    assert_eq!(saved["schemaVersion"], 7);
    assert_eq!(saved["futureField"], body["futureField"]);
    assert_eq!(saved["arrangements"]["tabletFuture"], "kept");
    assert_eq!(
        saved["arrangements"]["wide"][0],
        body["arrangements"]["wide"][0]
    );
}

#[test]
fn unarranged_classes_are_generated_from_wide_until_one_is_edited() {
    let library = open();
    library.save("Mine", custom("Mine")).unwrap();
    let mine = library.get("Mine").unwrap();
    assert_eq!(
        generated_classes(&mine),
        [ScreenClass::Tablet, ScreenClass::Phone]
    );
    // Wide reading order; gaps (8) cannot sit beside fuel (6) in 12 columns, so it wraps.
    let tablet: Vec<_> = mine
        .arrangement(ScreenClass::Tablet)
        .iter()
        .map(rect)
        .collect();
    assert_eq!(tablet, [[0, 0, 6, 2], [0, 2, 8, 2], [0, 4, 12, 1]]);
    let phone: Vec<_> = mine
        .arrangement(ScreenClass::Phone)
        .iter()
        .map(rect)
        .collect();
    assert_eq!(phone, [[0, 0, 4, 2], [0, 2, 4, 2], [0, 4, 4, 1]]);
    assert_eq!(
        mine.arrangement(ScreenClass::Tablet)[0].settings,
        json!({"unit": "kg"})
    );

    let mut edited = mine.clone();
    edited.arrangements.wide[0].w = 10;
    library.save("Mine", edited).unwrap();
    let mine = library.get("Mine").unwrap();
    assert_eq!(generated_classes(&mine).len(), 2);
    assert_eq!(
        rect(&mine.arrangement(ScreenClass::Tablet)[0]),
        [0, 0, 10, 2]
    );

    let mut arranged = pitwall_core::layout_edit::replace_arrangement(
        &mine,
        ScreenClass::Tablet,
        mine.arrangement(ScreenClass::Tablet),
    );
    arranged.arrangements.tablet.as_mut().unwrap()[0].w = 12;
    library.save("Mine", arranged).unwrap();
    let mine = library.get("Mine").unwrap();
    assert_eq!(generated_classes(&mine), [ScreenClass::Phone]);
    assert_eq!(
        rect(&mine.arrangement(ScreenClass::Tablet)[0]),
        [0, 0, 12, 2]
    );
}

#[test]
fn malformed_or_overflowing_layouts_are_refused_and_change_nothing() {
    let library = open();
    library.save("Mine", custom("Mine")).unwrap();
    let before = library.get("Mine").unwrap();
    let revision = library.revision();
    let mut cases: Vec<Layout> = Vec::new();
    let mut duplicate_id = custom("Mine");
    duplicate_id.arrangements.wide[1].i = "a".into();
    cases.push(duplicate_id);
    let mut no_size = custom("Mine");
    no_size.arrangements.wide[0].w = 0;
    cases.push(no_size);
    for field in ["stack", "x", "y"] {
        let mut body = custom("Mine");
        for tile in &mut body.arrangements.wide {
            match field {
                "stack" => {
                    tile.h = u32::MAX / 2;
                    tile.y = 0;
                }
                "x" => tile.x = u32::MAX,
                _ => tile.y = u32::MAX,
            }
        }
        cases.push(body);
    }
    for body in cases {
        assert!(matches!(
            library.save("Mine", body),
            Err(LayoutError::InvalidLayout(_))
        ));
        assert_eq!(library.get("Mine").unwrap(), before);
    }
    assert_eq!(library.revision(), revision);
}

#[test]
fn a_failed_write_keeps_the_previous_layouts_in_memory_and_on_disk() {
    let file = temp_file();
    let library = LayoutLibrary::open(file.clone()).unwrap();
    library.save("Mine", custom("Mine")).unwrap();
    let on_disk = std::fs::read(&file).unwrap();
    let revision = library.revision();
    // A reader denying replacement makes the public save fail without depending on
    // the storage helper's temporary filename.
    use std::os::windows::fs::OpenOptionsExt;
    let _held = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(&file)
        .unwrap();
    let mut changed = custom("Mine");
    changed.arrangements.wide.pop();
    assert!(matches!(
        library.save("Mine", changed),
        Err(LayoutError::Io(_))
    ));
    assert!(matches!(library.delete("Mine"), Err(LayoutError::Io(_))));
    assert_eq!(library.get("Mine").unwrap().arrangements.wide.len(), 3);
    assert_eq!(library.revision(), revision);
    assert_eq!(std::fs::read(&file).unwrap(), on_disk);
}

#[test]
fn only_user_layouts_are_written_to_disk() {
    let file = temp_file();
    let library = LayoutLibrary::open(file.clone()).unwrap();
    library.save("Mine", custom("Mine")).unwrap();
    let stored: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    let stored_names: Vec<&str> = stored["layouts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["name"].as_str().unwrap())
        .collect();
    assert_eq!(stored_names, ["Mine"]);
    assert_eq!(stored["schemaVersion"], 1);
}

#[test]
fn presets_are_exactly_the_source_baselines_three_authored_arrangements() {
    // Serialised from `presets::all()` at the inventory baseline 1074e84 (the file is
    // unchanged since); any change to a preset must be deliberate and update it.
    let fixture: Vec<Layout> = serde_json::from_str(include_str!("fixtures/presets-1074e84.json"))
        .expect("preset fixture");
    let shipped: Vec<Layout> = open().list().into_iter().take(3).collect();
    assert_eq!(shipped, fixture);
    for layout in &fixture {
        assert!(layout.arrangements.tablet.is_some() && layout.arrangements.phone.is_some());
    }
}

#[test]
fn migration_keeps_all_earlier_layout_backups() {
    let file = temp_file();
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    let original = r#"{"schemaVersion":0,"layouts":[]}"#;
    std::fs::write(&file, original).unwrap();
    std::fs::write(file.with_extension("json.bak"), "earliest original").unwrap();
    let library = LayoutLibrary::open(file.clone()).unwrap();
    library.save("Mine", custom("Mine")).unwrap();
    assert_eq!(
        std::fs::read_to_string(file.with_extension("json.bak")).unwrap(),
        "earliest original"
    );
    assert_eq!(
        std::fs::read_to_string(file.with_extension("json.2.bak")).unwrap(),
        original
    );
}
