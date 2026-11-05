//! Port of dashboard/src/layout/model.test.ts. The Rust layout keeps an unarranged class
//! as `None` instead of the web's served `generated` list; `generated_classes` reads it.
use pitwall_core::layout_edit::{
    add_tile, generated_classes, new_tile_id, place_below, remove_tile, replace_arrangement,
    set_tile_settings,
};
use pitwall_core::layouts::{Arrangements, Layout, ScreenClass, Tile};
use serde_json::{Map, Value, json};

fn tile(i: &str, kind: &str, x: u32, y: u32, w: u32, h: u32) -> Tile {
    Tile {
        i: i.into(),
        kind: kind.into(),
        x,
        y,
        w,
        h,
        settings: json!({}),
        extra: Map::new(),
    }
}

fn mine() -> Layout {
    Layout {
        schema_version: 1,
        name: "Mine".into(),
        arrangements: Arrangements {
            wide: vec![tile("notice", "notice", 0, 0, 24, 1)],
            tablet: None,
            phone: None,
            extra: Map::new(),
        },
        extra: Map::new(),
    }
}

#[test]
fn arrangement_returns_the_tiles_of_one_screen_class() {
    let mut layout = mine();
    layout.arrangements.tablet = Some(vec![tile("notice", "notice", 0, 3, 12, 1)]);
    assert_eq!(layout.arrangement(ScreenClass::Tablet)[0].y, 3);
    assert_eq!(layout.arrangement(ScreenClass::Phone)[0].w, 4); // generated from wide
}

#[test]
fn editing_a_generated_class_makes_it_the_layouts_own() {
    let before = mine();
    assert_eq!(generated_classes(&before), [ScreenClass::Tablet, ScreenClass::Phone]);
    let next = replace_arrangement(&before, ScreenClass::Tablet, vec![tile("notice", "notice", 0, 2, 12, 1)]);
    assert_eq!(generated_classes(&next), [ScreenClass::Phone]);
    assert_eq!(next.arrangement(ScreenClass::Tablet)[0].y, 2);
}

#[test]
fn editing_wide_leaves_the_generated_classes_generated() {
    let next = replace_arrangement(&mine(), ScreenClass::Wide, vec![tile("notice", "notice", 0, 5, 24, 1)]);
    assert_eq!(generated_classes(&next), [ScreenClass::Tablet, ScreenClass::Phone]);
    assert_eq!(next.arrangements.wide[0].y, 5);
}

#[test]
fn does_not_change_the_layout_it_was_given() {
    let before = mine();
    let snapshot = before.clone();
    let _ = replace_arrangement(&before, ScreenClass::Phone, Vec::new());
    assert_eq!(before, snapshot);
}

#[test]
fn new_tile_id_is_the_kebab_case_type_when_free_then_numbered() {
    let taken = |ids: &[&str]| ids.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(new_tile_id("lap-timing", &taken(&[])), "lap-timing");
    assert_eq!(new_tile_id("lap-timing", &taken(&["lap-timing"])), "lap-timing-2");
    assert_eq!(
        new_tile_id("lap-timing", &taken(&["lap-timing", "lap-timing-2"])),
        "lap-timing-3"
    );
}

#[test]
fn add_places_the_tile_under_everything_else_at_its_default_size() {
    let tiles = [tile("a", "a", 0, 0, 6, 4), tile("b", "b", 6, 0, 6, 7)];
    let (next, added) = add_tile(&tiles, "lap-timing", 6, 4, 24);
    assert_eq!(added, tile("lap-timing", "lap-timing", 0, 7, 6, 4));
    assert_eq!(next.len(), 3);
    assert_eq!(next[2], added);
}

#[test]
fn add_clamps_the_width_to_the_classs_columns() {
    let (_, added) = add_tile(&[], "x", 8, 2, 4);
    assert_eq!(added.w, 4);
}

#[test]
fn a_second_tile_of_a_type_gets_its_own_id_and_settings() {
    let (first, _) = add_tile(&[], "weather", 6, 2, 24);
    let (_, added) = add_tile(&first, "weather", 6, 2, 24);
    assert_eq!(added.i, "weather-2");
    assert_eq!(added.settings, json!({}));
}

#[test]
fn place_below_returns_the_first_free_row_under_the_lowest_tile() {
    assert_eq!(place_below(&[]), 0);
    assert_eq!(
        place_below(&[tile("a", "a", 0, 3, 2, 4), tile("b", "b", 2, 0, 2, 2)]),
        7
    );
}

fn with_mystery() -> Vec<Tile> {
    let mut a = tile("a", "a", 0, 0, 2, 2);
    a.extra.insert("mystery".into(), json!(1));
    vec![a, tile("b", "b", 2, 0, 2, 2)]
}

#[test]
fn remove_tile_drops_one_tile_by_id() {
    let ids: Vec<String> = remove_tile(&with_mystery(), "a").into_iter().map(|t| t.i).collect();
    assert_eq!(ids, ["b"]);
}

#[test]
fn set_tile_settings_replaces_that_tiles_settings_and_keeps_everything_else_on_it() {
    let tiles = with_mystery();
    let settings = json!({ "car": { "target": "focus" }, "shown": ["x"] });
    let next = set_tile_settings(&tiles, "a", settings.clone());
    let mut expected = tiles[0].clone();
    expected.settings = settings;
    assert_eq!(next[0], expected);
    assert_eq!(next[1], tiles[1]);
}

// toSavedBody: natively the layout is saved as the typed value; `preset` is the library's
// to decide and never stored. Unknown fields must still survive a save.
#[test]
fn unknown_fields_on_tiles_and_on_the_layout_survive_serialisation() {
    let mut layout = mine();
    layout.extra.insert("future".into(), json!({ "a": 1 }));
    let mut odd = tile("n", "mystery-type", 0, 0, 2, 2);
    odd.extra.insert("colour".into(), json!("red"));
    odd.settings = json!({ "z": 1 });
    layout.arrangements.wide = vec![odd];
    let body: Value = serde_json::to_value(&layout).unwrap();
    assert_eq!(body["future"], json!({ "a": 1 }));
    assert_eq!(body["arrangements"]["wide"][0]["type"], "mystery-type");
    assert_eq!(body["arrangements"]["wide"][0]["colour"], "red");
    assert_eq!(body["arrangements"]["wide"][0]["settings"], json!({ "z": 1 }));
    assert!(body.get("preset").is_none());
    let back: Layout = serde_json::from_value(body).unwrap();
    assert_eq!(back, layout);
}
