//! The edit grid: where tiles sit in pixels, and what a pointer drag on a tile's grip or
//! resize corner does to an arrangement. Ported from `dashboard/src/app/grid.tsx`, which
//! drew react-grid-layout with its vertical compactor.
//!
//! A drag is measured from where it started against the arrangement as it was then, so
//! repeated pointer moves never accumulate rounding. The result is positions only; feed
//! it to [`LayoutController::move_resize`](super::LayoutController::move_resize), which
//! keeps every other part of each tile.

use super::GridItem;
use crate::layouts::{ScreenClass, Tile};
use crate::screen::{GRID_MARGIN, cells_to_pixels, stream_grid_metrics};

/// Tallest a resize can make a tile, in rows.
const MAX_ROWS: u32 = 1000;
/// Lowest row a move can reach.
const MAX_ROW: u32 = 100_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gesture {
    /// Dragging the grip moves the tile.
    Move,
    /// Dragging the bottom-right corner resizes it.
    Resize,
}

/// One screen class drawn `width` logical pixels wide with rows `row` pixels high and
/// `margin` pixels between cells.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridGeometry {
    pub class: ScreenClass,
    pub width: f32,
    pub row: f32,
    pub margin: f32,
}

impl GridGeometry {
    pub fn new(class: ScreenClass, width: f32, row: f32) -> Self {
        Self {
            class,
            width,
            row,
            margin: GRID_MARGIN,
        }
    }

    /// A stream window's grid: rows and gaps share `height` exactly
    /// ([`stream_grid_metrics`]), so the last row never scrolls or overflows.
    pub fn stream(class: ScreenClass, width: f32, tiles: &[Tile], height: f32) -> Self {
        let rows: Vec<_> = tiles.iter().map(|t| (t.y, t.h)).collect();
        let metrics = stream_grid_metrics(&rows, height);
        Self {
            class,
            width,
            row: metrics.row_height,
            margin: metrics.margin,
        }
    }

    /// Distance from one column (row) to the next, in pixels.
    pub fn pitch(self) -> (f32, f32) {
        (
            (self.width + self.margin) / self.class.columns() as f32,
            self.row + self.margin,
        )
    }

    /// A tile's `(x, y, width, height)` in pixels, kept inside the class's columns.
    pub fn rect(self, tile: &Tile) -> (f32, f32, f32, f32) {
        let item = clamped(tile, self.class.columns());
        let (px, py) = self.pitch();
        let (w, h) = cells_to_pixels(
            self.class,
            self.width,
            item.w,
            item.h,
            self.row,
            self.margin,
        );
        (item.x as f32 * px, item.y as f32 * py, w, h)
    }

    /// Pixel height of the grid holding `tiles`.
    pub fn height<'a>(self, tiles: impl IntoIterator<Item = &'a Tile>) -> f32 {
        tiles
            .into_iter()
            .map(|tile| {
                let (_, y, _, h) = self.rect(tile);
                y + h
            })
            .fold(0.0, f32::max)
    }

    /// Positions after dragging tile `id` by `delta` pixels from where the drag began.
    /// Snaps to whole cells, keeps the tile inside the class and at least `minimum` cells
    /// (`(w, h)`, the width capped at the columns available), pushes tiles in the way
    /// down, then closes vertical gaps. An unknown `id` changes nothing.
    pub fn drag(
        self,
        tiles: &[Tile],
        id: &str,
        delta: (f32, f32),
        gesture: Gesture,
        minimum: (u32, u32),
    ) -> Vec<GridItem> {
        let columns = self.class.columns();
        let mut items: Vec<GridItem> = tiles.iter().map(|t| clamped(t, columns)).collect();
        let Some(at) = items.iter().position(|item| item.i == id) else {
            return items;
        };
        let (px, py) = self.pitch();
        let (dx, dy) = ((delta.0 / px).round() as i64, (delta.1 / py).round() as i64);
        let item = &mut items[at];
        match gesture {
            Gesture::Resize => {
                let room = columns - item.x;
                let min_w = minimum.0.min(room).max(1);
                item.w = (item.w as i64 + dx).clamp(min_w as i64, room as i64) as u32;
                let min_h = minimum.1.clamp(1, MAX_ROWS);
                item.h = (item.h as i64 + dy).clamp(min_h as i64, MAX_ROWS as i64) as u32;
            }
            Gesture::Move => {
                item.x = (item.x as i64 + dx).clamp(0, (columns - item.w) as i64) as u32;
                item.y = (item.y as i64 + dy).clamp(0, MAX_ROW as i64) as u32;
            }
        }

        // The dragged tile wins its place; the others move down out of its way.
        let mut placed = vec![items[at].clone()];
        let mut order: Vec<usize> = (0..items.len()).filter(|&i| i != at).collect();
        order.sort_by_key(|&i| (items[i].y, items[i].x));
        for i in order {
            items[i].y = first_free_row(&items[i], &placed, items[i].y);
            placed.push(items[i].clone());
        }

        // Vertical compaction: every tile rises as far as it can, top to bottom.
        let mut order: Vec<usize> = (0..items.len()).collect();
        order.sort_by_key(|&i| (items[i].y, items[i].x));
        placed.clear();
        for i in order {
            items[i].y = first_free_row(&items[i], &placed, 0);
            placed.push(items[i].clone());
        }
        items
    }
}

/// A tile's position with its width capped at `columns` and its column moved left so it
/// fits; older or hand-edited files may hold either.
fn clamped(tile: &Tile, columns: u32) -> GridItem {
    let w = tile.w.clamp(1, columns);
    GridItem {
        i: tile.i.clone(),
        x: tile.x.min(columns - w),
        y: tile.y,
        w,
        h: tile.h,
    }
}

/// The first row at or below `from` where `item` overlaps nothing in `placed`.
fn first_free_row(item: &GridItem, placed: &[GridItem], from: u32) -> u32 {
    let mut probe = GridItem {
        y: from,
        ..item.clone()
    };
    while let Some(bottom) = placed
        .iter()
        .filter(|p| overlaps(&probe, p))
        .map(|p| p.y.saturating_add(p.h))
        .max()
    {
        probe.y = bottom;
    }
    probe.y
}

fn overlaps(a: &GridItem, b: &GridItem) -> bool {
    a.x < b.x.saturating_add(b.w)
        && a.x.saturating_add(a.w) > b.x
        && a.y < b.y.saturating_add(b.h)
        && a.y.saturating_add(a.h) > b.y
}

/// The tiles a window draws. Outside edit mode a Notice alone on the last row (one row
/// high) is left out and the chrome's short line stands in for it, unless it is the only
/// tile; edit mode shows everything.
pub fn shown_tiles(tiles: &[Tile], editing: bool) -> Vec<&Tile> {
    let last_row = tiles
        .iter()
        .map(|t| t.y.saturating_add(t.h))
        .max()
        .unwrap_or(0);
    let notice_only = tiles
        .iter()
        .filter(|t| t.y.saturating_add(t.h) == last_row)
        .all(|t| t.kind == "notice" && t.h == 1);
    let hide = !editing && notice_only && tiles.len() > 1;
    tiles
        .iter()
        .filter(|t| !hide || t.kind != "notice" || t.y.saturating_add(t.h) < last_row)
        .collect()
}
