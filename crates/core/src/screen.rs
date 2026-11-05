//! Screen classes, grid geometry and tile densities (spec section 10, Grid), ported from
//! `dashboard/src/layout/screen.ts` and the density rules of `runtime/tile.ts`.
//!
//! Sizes are logical pixels. Natively the screen class follows the window's content
//! width; the column counts live on [`ScreenClass::columns`].

pub use crate::layouts::ScreenClass;

/// Narrowest window width of each class, logical pixels.
pub const WIDE_MIN_WIDTH: f32 = 1280.0;
pub const TABLET_MIN_WIDTH: f32 = 768.0;

pub const ROW_HEIGHT: f32 = 32.0;
pub const MAX_FIT_ROW_HEIGHT: f32 = 64.0;
pub const GRID_MARGIN: f32 = 8.0;
/// Space between the window edge and the grid.
pub const PAGE_PADDING: f32 = 8.0;
/// A classic vertical scrollbar; the grid scrolls when a layout is taller than the window.
pub const SCROLLBAR: f32 = 15.0;

/// Container width of the narrowest wide screen: the class floor less a scrollbar and the
/// page padding. A cell is narrowest here, so a threshold that holds here holds on every
/// wider screen and on tablet and phone.
pub const NARROWEST_WIDE_CONTAINER: f32 = WIDE_MIN_WIDTH - SCROLLBAR - 2.0 * PAGE_PADDING;

pub fn screen_class_of(window_width: f32) -> ScreenClass {
    if window_width >= WIDE_MIN_WIDTH {
        ScreenClass::Wide
    } else if window_width >= TABLET_MIN_WIDTH {
        ScreenClass::Tablet
    } else {
        ScreenClass::Phone
    }
}

/// Pixels `(width, height)` a block of `w` by `h` grid cells occupies in a grid
/// `container_width` wide.
pub fn cells_to_pixels(
    class: ScreenClass,
    container_width: f32,
    w: u32,
    h: u32,
    row_height: f32,
    margin: f32,
) -> (f32, f32) {
    let columns = class.columns() as f32;
    let column = (container_width - margin * (columns - 1.0)) / columns;
    let gaps = |n: u32| n.saturating_sub(1) as f32 * margin;
    (w as f32 * column + gaps(w), h as f32 * row_height + gaps(h))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Density {
    Compact,
    Regular,
    Full,
}

/// A density applies once the tile is at least this many pixels in both directions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DensityRule {
    pub density: Density,
    pub min_width: f32,
    pub min_height: f32,
}

/// A density threshold written in grid cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DensityCells {
    pub density: Density,
    pub w: u32,
    pub h: u32,
}

/// Density rules written in grid cells, converted to whole pixels at the narrowest wide
/// screen, so a tile a preset sizes at, say, 9×16 reliably reaches Full at 1280 px.
pub fn density_rules_from_cells(cells: &[DensityCells]) -> Vec<DensityRule> {
    cells
        .iter()
        .map(|&DensityCells { density, w, h }| {
            if w == 0 && h == 0 {
                return DensityRule {
                    density,
                    min_width: 0.0,
                    min_height: 0.0,
                };
            }
            let (width, height) = cells_to_pixels(
                ScreenClass::Wide,
                NARROWEST_WIDE_CONTAINER,
                w,
                h,
                ROW_HEIGHT,
                GRID_MARGIN,
            );
            DensityRule {
                density,
                min_width: width.floor(),
                min_height: height.floor(),
            }
        })
        .collect()
}

/// Density for the tile's actual size: the last rule whose width and height both fit,
/// else the first. Content is never scaled.
pub fn pick_density(rules: &[DensityRule], width: f32, height: f32) -> Density {
    let mut chosen = rules.first().map_or(Density::Compact, |rule| rule.density);
    for rule in rules {
        if width >= rule.min_width && height >= rule.min_height {
            chosen = rule.density;
        }
    }
    chosen
}

/// Validate a tile's density rules: two or three, in order compact, regular, full, and
/// never shrinking as they grow. The error names the broken rule.
pub fn check_densities(rules: &[DensityRule]) -> Result<(), String> {
    const ORDER: [Density; 3] = [Density::Compact, Density::Regular, Density::Full];
    let ordered = (2..=3).contains(&rules.len())
        && rules.iter().zip(ORDER).all(|(rule, d)| rule.density == d);
    if !ordered {
        return Err("densities must be 2 or 3 entries in order compact, regular, full".into());
    }
    for pair in rules.windows(2) {
        if pair[1].min_width < pair[0].min_width || pair[1].min_height < pair[0].min_height {
            return Err("densities must not shrink as they grow".into());
        }
    }
    Ok(())
}

/// Rows a layout spans, counting positions as well as heights. `tiles` are `(y, h)`.
fn rows_of(tiles: &[(u32, u32)]) -> f32 {
    tiles
        .iter()
        .map(|&(y, h)| y.saturating_add(h))
        .max()
        .unwrap_or(0)
        .max(1) as f32
}

/// Row height that lets a layout fill the height left under the chrome: whole pixels,
/// at least [`ROW_HEIGHT`] (a taller layout scrolls), at most [`MAX_FIT_ROW_HEIGHT`].
/// `tiles` are `(y, h)`.
pub fn fit_row_height(tiles: &[(u32, u32)], available: f32) -> f32 {
    let rows = rows_of(tiles);
    let fit = ((available - GRID_MARGIN * (rows - 1.0)) / rows).floor();
    fit.clamp(ROW_HEIGHT, MAX_FIT_ROW_HEIGHT)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StreamGridMetrics {
    pub row_height: f32,
    pub margin: f32,
}

/// Divide a stream window's height between rows after reserving gaps, so the last row
/// never scrolls. Empty rows stay empty on air. `tiles` are `(y, h)`.
pub fn stream_grid_metrics(tiles: &[(u32, u32)], viewport_height: f32) -> StreamGridMetrics {
    let rows = rows_of(tiles);
    let height = viewport_height.max(0.0);
    let margin = GRID_MARGIN.min(height / (2.0 * rows));
    StreamGridMetrics {
        row_height: (height - margin * (rows - 1.0)) / rows,
        margin,
    }
}
