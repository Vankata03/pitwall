//! The edit grid (`dashboard/src/app/grid.tsx` over react-grid-layout with the vertical
//! compactor): where tiles sit in pixels and what a pointer drag on a grip or the resize
//! corner does to an arrangement.
use pitwall_core::layout_edit::GridItem;
use pitwall_core::layout_edit::grid::{Gesture, GridGeometry, shown_tiles};
use pitwall_core::layouts::{ScreenClass, Tile};
use serde_json::json;

fn tiles(value: serde_json::Value) -> Vec<Tile> {
    serde_json::from_value(value).unwrap()
}

fn rect(item: &GridItem) -> (u32, u32, u32, u32) {
    (item.x, item.y, item.w, item.h)
}

/// A phone grid 424 px wide: four 100 px columns with 8 px gaps, rows 32 px.
fn phone() -> GridGeometry {
    GridGeometry::new(ScreenClass::Phone, 424.0, 32.0)
}

#[test]
fn tiles_sit_on_columns_and_rows_with_8_px_gaps() {
    let t = tiles(json!([{"i":"a","type":"x","x":1,"y":2,"w":2,"h":3}]));
    assert_eq!(phone().rect(&t[0]), (108.0, 80.0, 208.0, 112.0));
    assert_eq!(phone().height(&t), 192.0);
}

#[test]
fn a_tile_wider_than_the_class_is_drawn_clamped_inside_it() {
    let t = tiles(json!([{"i":"a","type":"x","x":3,"y":0,"w":6,"h":1}]));
    assert_eq!(phone().rect(&t[0]), (0.0, 0.0, 424.0, 32.0));
}

#[test]
fn moving_snaps_to_cells_and_pushes_the_tile_in_the_way_down() {
    let t = tiles(json!([
        {"i":"a","type":"future","x":0,"y":0,"w":2,"h":2,"settings":{"future":7}},
        {"i":"b","type":"future","x":2,"y":0,"w":2,"h":2}
    ]));
    let moved = phone().drag(&t, "a", (220.0, 0.0), Gesture::Move, (1, 1));
    assert_eq!(rect(&moved[0]), (2, 0, 2, 2));
    assert_eq!(rect(&moved[1]), (2, 2, 2, 2));
}

#[test]
fn moving_stops_at_the_class_edges() {
    let t = tiles(json!([{"i":"a","type":"x","x":0,"y":0,"w":2,"h":1}]));
    let moved = phone().drag(&t, "a", (5000.0, -500.0), Gesture::Move, (1, 1));
    assert_eq!(rect(&moved[0]), (2, 0, 2, 1));
}

#[test]
fn moving_compacts_vertical_holes_like_the_web_grid() {
    let t = tiles(json!([
        {"i":"a","type":"future","x":0,"y":0,"w":2,"h":2},
        {"i":"b","type":"future","x":0,"y":2,"w":2,"h":2}
    ]));
    let moved = phone().drag(&t, "a", (0.0, 400.0), Gesture::Move, (1, 1));
    assert_eq!((moved[0].y, moved[1].y), (2, 0));
}

#[test]
fn resizing_snaps_and_never_goes_below_the_types_minimum() {
    let t = tiles(json!([{"i":"a","type":"x","x":0,"y":0,"w":3,"h":4}]));
    let grown = phone().drag(&t, "a", (-90.0, 75.0), Gesture::Resize, (1, 1));
    assert_eq!(rect(&grown[0]), (0, 0, 2, 6));
    let shrunk = phone().drag(&t, "a", (-999.0, -999.0), Gesture::Resize, (2, 3));
    assert_eq!(rect(&shrunk[0]), (0, 0, 2, 3));
}

#[test]
fn resizing_an_old_small_tile_at_the_right_edge_stays_inside_the_class() {
    let t = tiles(json!([{"i":"a","type":"speed-rpm-gear","x":3,"y":0,"w":1,"h":1}]));
    let resized = phone().drag(&t, "a", (999.0, -999.0), Gesture::Resize, (4, 3));
    assert_eq!(rect(&resized[0]), (3, 0, 1, 3));
}

#[test]
fn a_drag_of_an_unknown_id_changes_nothing() {
    let t = tiles(json!([{"i":"a","type":"x","x":1,"y":5,"w":2,"h":1}]));
    let same = phone().drag(&t, "zzz", (100.0, 100.0), Gesture::Move, (1, 1));
    assert_eq!(rect(&same[0]), (1, 5, 2, 1));
}

#[test]
fn a_notice_alone_on_the_last_row_is_left_to_the_chrome_outside_edit_mode() {
    let t = tiles(json!([
        {"i":"speed","type":"speed-rpm-gear","x":0,"y":0,"w":6,"h":6},
        {"i":"notice","type":"notice","x":0,"y":6,"w":24,"h":1}
    ]));
    let ids = |shown: Vec<&Tile>| shown.iter().map(|t| t.i.clone()).collect::<Vec<_>>();
    assert_eq!(ids(shown_tiles(&t, false)), ["speed"]);
    assert_eq!(ids(shown_tiles(&t, true)), ["speed", "notice"]);
    // A notice that is the only tile, or shares its row, stays.
    assert_eq!(ids(shown_tiles(&t[1..], false)), ["notice"]);
    let shared = tiles(json!([
        {"i":"notice","type":"notice","x":0,"y":0,"w":6,"h":1},
        {"i":"fuel","type":"fuel","x":6,"y":0,"w":6,"h":1}
    ]));
    assert_eq!(ids(shown_tiles(&shared, false)), ["notice", "fuel"]);
}
