//! Pointer-to-grid commands shared by the native editor and its tests.

use pitwall_core::{
    layout_edit::GridItem,
    layouts::{ScreenClass, Tile},
    screen::{GRID_MARGIN, cells_to_pixels},
};

#[derive(Clone, Copy)]
pub struct GridGeometry {
    pub class: ScreenClass,
    pub width: f32,
    pub row: f32,
}

impl GridGeometry {
    pub fn new(class: ScreenClass, width: f32, row: f32) -> Self {
        Self { class, width, row }
    }
    pub fn pitch(self) -> (f32, f32) {
        (
            (self.width + GRID_MARGIN) / self.class.columns() as f32,
            self.row + GRID_MARGIN,
        )
    }
    pub fn rect(self, tile: &Tile) -> (f32, f32, f32, f32) {
        let (x, y) = self.pitch();
        let (w, h) = cells_to_pixels(
            self.class,
            self.width,
            tile.w.min(self.class.columns()),
            tile.h,
            self.row,
            GRID_MARGIN,
        );
        (tile.x as f32 * x, tile.y as f32 * y, w, h)
    }
    /// Snap the pointer displacement to cells, clamp at the class edges and push
    /// collisions down. Tile data never passes through this geometry-only result.
    pub fn change(
        self,
        tiles: &[Tile],
        id: &str,
        delta: (f32, f32),
        resize: bool,
        min: (u32, u32),
    ) -> Vec<GridItem> {
        let mut items: Vec<_> = tiles
            .iter()
            .map(|t| GridItem {
                i: t.i.clone(),
                x: t.x.min(
                    self.class
                        .columns()
                        .saturating_sub(t.w.min(self.class.columns())),
                ),
                y: t.y,
                w: t.w.min(self.class.columns()),
                h: t.h,
            })
            .collect();
        let Some(at) = items.iter().position(|t| t.i == id) else {
            return items;
        };
        let (px, py) = self.pitch();
        let (dx, dy) = ((delta.0 / px).round() as i64, (delta.1 / py).round() as i64);
        let t = &mut items[at];
        let cols = self.class.columns();
        if resize {
            t.w = (t.w as i64 + dx).clamp(min.0.min(cols - t.x).max(1) as i64, (cols - t.x) as i64)
                as u32;
            t.h = (t.h as i64 + dy).clamp(min.1 as i64, 1000) as u32;
        } else {
            t.x = (t.x as i64 + dx).clamp(0, (cols - t.w) as i64) as u32;
            t.y = (t.y as i64 + dy).clamp(0, 100_000) as u32;
        }
        let mut placed = vec![items[at].clone()];
        let mut order: Vec<_> = (0..items.len()).filter(|&i| i != at).collect();
        order.sort_by_key(|&i| (items[i].y, items[i].x));
        for i in order {
            while let Some(bottom) = placed
                .iter()
                .filter(|p| overlaps(&items[i], p))
                .map(|p| p.y + p.h)
                .max()
            {
                items[i].y = bottom;
            }
            placed.push(items[i].clone());
        }
        // Normal dashboard grids compact vertically; saved gaps in untouched
        // layouts remain intact until the user makes an arrangement command.
        let mut order: Vec<_> = (0..items.len()).collect();
        order.sort_by_key(|&i| (items[i].y, items[i].x));
        placed.clear();
        for i in order {
            items[i].y = 0;
            while let Some(bottom) = placed
                .iter()
                .filter(|p| overlaps(&items[i], p))
                .map(|p| p.y.saturating_add(p.h))
                .max()
            {
                items[i].y = bottom;
            }
            placed.push(items[i].clone());
        }
        items
    }
}

fn overlaps(a: &GridItem, b: &GridItem) -> bool {
    a.x < b.x.saturating_add(b.w)
        && a.x.saturating_add(a.w) > b.x
        && a.y < b.y.saturating_add(b.h)
        && a.y.saturating_add(a.h) > b.y
}

#[cfg(test)]
mod tests {
    use super::*;
    use pitwall_core::layouts::ScreenClass;

    #[test]
    fn pointer_move_snaps_clamps_and_keeps_other_tiles_below() {
        let tiles: Vec<Tile> = serde_json::from_value(serde_json::json!([
            {"i":"a","type":"future","x":0,"y":0,"w":2,"h":2,"settings":{"future":7}},
            {"i":"b","type":"future","x":2,"y":0,"w":2,"h":2}
        ]))
        .unwrap();
        let grid = GridGeometry::new(ScreenClass::Phone, 424.0, 32.0);
        let moved = grid.change(&tiles, "a", (220.0, 0.0), false, (1, 1));
        assert_eq!((moved[0].x, moved[0].y), (2, 0));
        assert_eq!((moved[1].x, moved[1].y), (2, 2));
    }

    #[test]
    fn resizing_an_old_small_tile_at_right_edge_does_not_panic_or_cross_edge() {
        let tiles: Vec<Tile> = serde_json::from_value(serde_json::json!([
            {"i":"a","type":"speed-rpm-gear","x":3,"y":0,"w":1,"h":1}
        ]))
        .unwrap();
        let grid = GridGeometry::new(ScreenClass::Phone, 424.0, 32.0);
        let resized = grid.change(&tiles, "a", (999.0, -999.0), true, (4, 3));
        assert_eq!((resized[0].x, resized[0].w, resized[0].h), (3, 1, 3));
    }

    #[test]
    fn move_compacts_vertical_holes_like_the_web_grid() {
        let tiles: Vec<Tile> = serde_json::from_value(serde_json::json!([
            {"i":"a","type":"future","x":0,"y":0,"w":2,"h":2},
            {"i":"b","type":"future","x":0,"y":2,"w":2,"h":2}
        ]))
        .unwrap();
        let grid = GridGeometry::new(ScreenClass::Phone, 424.0, 32.0);
        let moved = grid.change(&tiles, "a", (0.0, 400.0), false, (1, 1));
        assert_eq!((moved[0].y, moved[1].y), (2, 0));
    }
}
