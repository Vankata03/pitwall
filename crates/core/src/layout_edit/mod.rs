//! Editing layouts: tile operations on one screen class (ported from
//! `dashboard/src/layout/model.ts`) and the per-window [`LayoutController`] (ported from
//! `dashboard/src/layout/controller.ts`).
//!
//! Every function returns new values and leaves its input untouched. Unknown tile types,
//! settings and fields ride along unchanged.

mod controller;

pub use controller::{
    ControllerOptions, DEFAULT_LAYOUT, EditState, GridItem, LayoutController, LayoutStore,
    LayoutSummary, Notice, NoticeKind, Status,
};

use crate::layouts::{Layout, ScreenClass, Tile};
use serde_json::{Map, Value};

/// Classes nobody has arranged, generated from wide until edited.
pub fn generated_classes(layout: &Layout) -> Vec<ScreenClass> {
    let mut out = Vec::new();
    if layout.arrangements.tablet.is_none() {
        out.push(ScreenClass::Tablet);
    }
    if layout.arrangements.phone.is_none() {
        out.push(ScreenClass::Phone);
    }
    out
}

/// Replace one class's tiles. A class replaced this way is the layout's own from now on,
/// so a save keeps it; other classes are untouched.
pub fn replace_arrangement(layout: &Layout, class: ScreenClass, tiles: Vec<Tile>) -> Layout {
    let mut next = layout.clone();
    match class {
        ScreenClass::Wide => next.arrangements.wide = tiles,
        ScreenClass::Tablet => next.arrangements.tablet = Some(tiles),
        ScreenClass::Phone => next.arrangements.phone = Some(tiles),
    }
    next
}

/// First row under every tile.
pub fn place_below(tiles: &[Tile]) -> u32 {
    tiles
        .iter()
        .map(|tile| tile.y.saturating_add(tile.h))
        .max()
        .unwrap_or(0)
}

/// A kebab-case tile ID: the type itself while free, then `type-2`, `type-3`.
pub fn new_tile_id(kind: &str, taken: &[String]) -> String {
    if !taken.iter().any(|id| id == kind) {
        return kind.to_string();
    }
    (2..)
        .map(|n| format!("{kind}-{n}"))
        .find(|candidate| !taken.contains(candidate))
        .expect("an unused ID")
}

/// Add a tile of `kind` at its default size (`w` by `h` cells, width clamped to
/// `columns`) under the others, with empty settings. Returns the new tiles and the tile
/// added.
pub fn add_tile(tiles: &[Tile], kind: &str, w: u32, h: u32, columns: u32) -> (Vec<Tile>, Tile) {
    let taken: Vec<String> = tiles.iter().map(|tile| tile.i.clone()).collect();
    let added = Tile {
        i: new_tile_id(kind, &taken),
        kind: kind.to_string(),
        x: 0,
        y: place_below(tiles),
        w: w.min(columns),
        h,
        settings: Value::Object(Map::new()),
        extra: Map::new(),
    };
    let mut next = tiles.to_vec();
    next.push(added.clone());
    (next, added)
}

pub fn remove_tile(tiles: &[Tile], id: &str) -> Vec<Tile> {
    tiles.iter().filter(|tile| tile.i != id).cloned().collect()
}

/// Replace one tile's settings, keeping everything else on it.
pub fn set_tile_settings(tiles: &[Tile], id: &str, settings: Value) -> Vec<Tile> {
    tiles
        .iter()
        .map(|tile| {
            if tile.i == id {
                Tile {
                    settings: settings.clone(),
                    ..tile.clone()
                }
            } else {
                tile.clone()
            }
        })
        .collect()
}
