//! Canvas helpers shared by the drawn tiles: polygons approximating circles and arcs,
//! since gpui paths have no circle primitive.

use gpui::{Bounds, Hsla, PathBuilder, Pixels, Point, Window, point, px};

/// A point `(x, y)` in tile pixels from the canvas origin.
pub fn at(bounds: Bounds<Pixels>, x: f32, y: f32) -> Point<Pixels> {
    point(bounds.origin.x + px(x), bounds.origin.y + px(y))
}

/// Fill a disc.
pub fn disc(window: &mut Window, bounds: Bounds<Pixels>, c: (f32, f32), r: f32, colour: Hsla) {
    let steps = 48;
    let mut path = PathBuilder::fill();
    for i in 0..=steps {
        let a = std::f32::consts::TAU * i as f32 / steps as f32;
        let p = at(bounds, c.0 + a.cos() * r, c.1 + a.sin() * r);
        if i == 0 {
            path.move_to(p);
        } else {
            path.line_to(p);
        }
    }
    path.close();
    if let Ok(path) = path.build() {
        window.paint_path(path, colour);
    }
}

/// Stroke an arc from angle `from` to `to` (radians, clockwise from 3 o'clock).
#[allow(clippy::too_many_arguments)]
pub fn arc(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    c: (f32, f32),
    r: f32,
    from: f32,
    to: f32,
    width: f32,
    colour: Hsla,
) {
    let steps = (((to - from).abs() / std::f32::consts::TAU) * 96.0)
        .ceil()
        .max(1.0) as usize;
    let mut path = PathBuilder::stroke(px(width));
    for i in 0..=steps {
        let a = from + (to - from) * i as f32 / steps as f32;
        let p = at(bounds, c.0 + a.cos() * r, c.1 + a.sin() * r);
        if i == 0 {
            path.move_to(p);
        } else {
            path.line_to(p);
        }
    }
    if let Ok(path) = path.build() {
        window.paint_path(path, colour);
    }
}

/// Stroke a polyline of tile-pixel points.
pub fn polyline(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    points: &[(f32, f32)],
    width: f32,
    colour: Hsla,
) {
    if points.len() < 2 {
        return;
    }
    let mut path = PathBuilder::stroke(px(width));
    path.move_to(at(bounds, points[0].0, points[0].1));
    for &(x, y) in &points[1..] {
        path.line_to(at(bounds, x, y));
    }
    if let Ok(path) = path.build() {
        window.paint_path(path, colour);
    }
}

/// Fill a closed polygon of tile-pixel points.
pub fn polygon(window: &mut Window, bounds: Bounds<Pixels>, points: &[(f32, f32)], colour: Hsla) {
    if points.len() < 3 {
        return;
    }
    let mut path = PathBuilder::fill();
    path.move_to(at(bounds, points[0].0, points[0].1));
    for &(x, y) in &points[1..] {
        path.line_to(at(bounds, x, y));
    }
    path.close();
    if let Ok(path) = path.build() {
        window.paint_path(path, colour);
    }
}
