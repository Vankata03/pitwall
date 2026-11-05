//! The layout library: the shared `layouts.json` and the three read-only presets (spec
//! sections 10 and 12). The collector serves it over `/api/layouts`.
//!
//! Read the library's revision counter with [`LayoutLibrary::revision`]; it advances on
//! every successful change.

mod library;
mod presets;

pub use library::{LayoutError, LayoutLibrary};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Version written to every layout this build saves.
pub const SCHEMA_VERSION: u32 = 1;

/// Screen classes, chosen from the viewport width (spec section 10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenClass {
    Wide,
    Tablet,
    Phone,
}

impl ScreenClass {
    pub fn columns(self) -> u32 {
        match self {
            ScreenClass::Wide => 24,
            ScreenClass::Tablet => 12,
            ScreenClass::Phone => 4,
        }
    }
}

/// One tile instance. Unknown fields survive a save, as does any `type` or `settings`
/// this build does not know.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tile {
    pub i: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    #[serde(default = "empty_settings")]
    pub settings: Value,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

fn empty_settings() -> Value {
    Value::Object(Map::new())
}

/// One arrangement per screen class. A class that is `None` has not been arranged and is
/// generated from wide until it is edited.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Arrangements {
    #[serde(default)]
    pub wide: Vec<Tile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tablet: Option<Vec<Tile>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phone: Option<Vec<Tile>>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A stored layout.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    #[serde(rename = "schemaVersion", default = "schema_version")]
    pub schema_version: u32,
    pub name: String,
    pub arrangements: Arrangements,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

fn schema_version() -> u32 {
    SCHEMA_VERSION
}

impl Layout {
    /// The tiles a screen of this class shows: its own arrangement, or one generated from
    /// wide when nobody has arranged it.
    pub fn arrangement(&self, class: ScreenClass) -> Vec<Tile> {
        let own = match class {
            ScreenClass::Wide => return self.arrangements.wide.clone(),
            ScreenClass::Tablet => &self.arrangements.tablet,
            ScreenClass::Phone => &self.arrangements.phone,
        };
        match own {
            Some(tiles) => tiles.clone(),
            None => generate(&self.arrangements.wide, class),
        }
    }
}

/// Derive a narrower arrangement from wide. Tiles keep their sizes, clamped to the
/// class's columns, and are placed in wide reading order (top to bottom, then left to
/// right) so nothing overlaps and no tile shrinks below its minimum.
pub fn generate(wide: &[Tile], class: ScreenClass) -> Vec<Tile> {
    let columns = class.columns();
    if class == ScreenClass::Wide {
        return wide.to_vec();
    }
    let mut ordered: Vec<&Tile> = wide.iter().collect();
    ordered.sort_by_key(|tile| (tile.y, tile.x));

    let (mut cursor_x, mut row_y, mut row_height) = (0u32, 0u32, 0u32);
    ordered
        .into_iter()
        .map(|tile| {
            let w = tile.w.min(columns);
            if cursor_x + w > columns {
                // Saves reject overflow; also tolerate older malformed files.
                row_y = row_y.saturating_add(row_height);
                cursor_x = 0;
                row_height = 0;
            }
            let placed = Tile {
                x: cursor_x,
                y: row_y,
                w,
                ..tile.clone()
            };
            cursor_x += w;
            row_height = row_height.max(tile.h);
            placed
        })
        .collect()
}
