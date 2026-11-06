//! Track map (parity MAPTILE-01/02): a session-wide frame-rate tile drawing the circuit
//! and every car's raw world position. Port of `dashboard/src/tiles/types/track-map.tsx`.
//!
//! The outline and pit lane come from the host's track store (cached file, shipped map or
//! what the learner has so far); the tile reads it again whenever the store's revision or
//! the track changes. Car dots use raw world X/Z, so they draw before any map exists. The
//! fit grows in 100 m steps and only resets with the track or epoch, so moving dots never
//! rescale the map.

use std::sync::OnceLock;

use gpui::{
    AnyElement, Hsla, IntoElement, ParentElement, SharedString, Styled, canvas, div, px, rgb,
};
use pitwall_core::{
    binding::{car_label, participants_of},
    livery::team_colour,
    model::SessionModel,
    names::track_name,
    readout::ReadoutState,
    telemetry::MAX_CARS,
    tracks::{TrackKey, TrackMap, TrackStore},
};

use super::{Densities, Density, format::fixed, paint, widgets};
use crate::theme;

pub const TYPE: &str = "track-map";
pub const NAME: &str = "Track map";
pub const DENSITIES: Densities = Densities {
    regular: (240.0, 160.0),
    full: None,
};
/// The host's track store; `main` sets it once the host runs.
pub static TRACKS: OnceLock<TrackStore> = OnceLock::new();

/// Height the footer takes from the tile; the title floats over the map's empty corner.
pub const CHROME_HEIGHT: f32 = 40.0;
/// Room around the track for the markers and their numbers.
const MAP_INSET: f32 = 20.0;
const LABEL_WIDTH: f32 = 16.0;
const LABEL_HEIGHT: f32 = 12.0;
/// Outline points further apart than this are not joined.
const JOIN_METRES: f32 = 40.0;
const BUCKET: f32 = 8.0;

fn hex(value: u32) -> Hsla {
    rgb(value).into()
}
/// `--pw-track`
fn track_colour() -> Hsla {
    hex(0x6B8496)
}
/// `--pw-page-bg`: the road's casing and the dots' rim.
fn casing() -> Hsla {
    hex(0x121517)
}

/// The session facts the map reads.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SessionFacts {
    pub track_id: i8,
    pub track_length: u16,
    pub sector2: f32,
    pub sector3: f32,
}

/// The store's answer for the current track.
#[derive(Debug, Clone, PartialEq)]
pub enum Lookup {
    /// No session, or no store to ask.
    None,
    /// The store knows nothing about this layout yet.
    NotFound,
    Found(TrackMap),
    /// The lookup failed: the tile says "Map unavailable".
    Failed,
}

/// One car's raw position. Only live and stale positions are kept.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Car {
    pub index: usize,
    pub x: f32,
    pub z: f32,
    pub stale: bool,
    /// Race position, when Lap Data has a positive one.
    pub position: Option<u8>,
    /// The first game livery colour.
    pub colour: Option<(u8, u8, u8)>,
}

pub struct Inputs {
    pub session: Option<SessionFacts>,
    pub epoch: u64,
    pub map: Lookup,
    pub cars: Vec<Car>,
    /// Cars whose position is restricted: counted, never drawn at zero.
    pub restricted: usize,
    /// The window focus car, or the player when the window follows the player.
    pub focus: Option<usize>,
    /// The focus car's shown name, upper case.
    pub focus_name: Option<String>,
    pub width: f32,
    pub height: f32,
}

/// The map's fit, kept between frames.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub min_x: f32,
    pub min_z: f32,
    pub max_x: f32,
    pub max_z: f32,
}

#[derive(Default)]
pub struct Fit {
    identity: Option<(u64, Option<SessionFacts>)>,
    bounds: Option<Bounds>,
}

/// What a placed Track map keeps: the fit and the last store answer, by track and
/// store revision.
#[derive(Default)]
pub struct Memory {
    pub fit: Fit,
    cached: Option<(TrackKey, u64, Lookup)>,
}

impl Memory {
    /// The store's map for `key`, asked again only when the key or revision changes.
    pub fn lookup(&mut self, store: Option<&TrackStore>, key: Option<TrackKey>) -> Lookup {
        let (Some(store), Some(key)) = (store, key) else {
            return if key.is_some() {
                Lookup::Failed
            } else {
                Lookup::None
            };
        };
        let revision = store.revision();
        match &self.cached {
            Some((k, r, map)) if *k == key && *r == revision => map.clone(),
            _ => {
                let map = store.get(key).map_or(Lookup::NotFound, Lookup::Found);
                self.cached = Some((key, revision, map.clone()));
                map
            }
        }
    }

    pub fn inputs(
        &mut self,
        model: &SessionModel,
        store: Option<&TrackStore>,
        focus: Option<usize>,
        size: (f32, f32),
    ) -> Inputs {
        let session = model.session().value().map(|s| SessionFacts {
            track_id: s.track_id,
            track_length: s.track_length,
            sector2: s.sector2_lap_distance_start,
            sector3: s.sector3_lap_distance_start,
        });
        let key = session
            .filter(|s| s.track_id >= 0 && s.track_length > 0)
            .map(|s| TrackKey::new(s.track_id, s.track_length));
        let mut cars = Vec::new();
        let mut restricted = 0;
        for index in 0..MAX_CARS {
            let Some(car) = model.car(index) else {
                continue;
            };
            let motion = car.motion();
            if motion.state() == ReadoutState::Restricted {
                restricted += 1;
            }
            let Some(m) = motion.value() else { continue };
            cars.push(Car {
                index,
                x: m.world_position_x,
                z: m.world_position_z,
                stale: motion.state() == ReadoutState::Stale,
                position: car.lap().value().map(|l| l.car_position).filter(|&p| p > 0),
                colour: team_colour(car.participant().value().copied()).map(|c| (c.r, c.g, c.b)),
            });
        }
        let focus_name = focus.and_then(|f| {
            participants_of(model)
                .iter()
                .find(|p| p.index == f)
                .map(|p| car_label(p).to_uppercase())
        });
        Inputs {
            session,
            epoch: model.epoch(),
            map: self.lookup(store, key),
            cars,
            restricted,
            focus,
            focus_name,
            width: size.0,
            height: size.1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Fill {
    /// The team's livery colour.
    Team(u8, u8, u8),
    /// No livery colour: the focus car takes the accent, others the field colour.
    Accent,
    Field,
    Stale,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Dot {
    pub index: usize,
    pub at: (f32, f32),
    pub radius: f32,
    pub fill: Fill,
    pub focused: bool,
    pub stale: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Label {
    pub text: String,
    pub at: (f32, f32),
    pub focused: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct View {
    pub density: Density,
    /// The canvas size: the tile less the footer.
    pub canvas: (f32, f32),
    pub road_width: f32,
    /// Outline runs, joined only across covered gaps (≤ 40 m); a complete map closes.
    pub road: Vec<Vec<(f32, f32)>>,
    pub pit: Vec<(f32, f32)>,
    /// S/F, S2, S3, only where the outline covers the distance.
    pub markers: Vec<Label>,
    /// Drawn in order: the focus car last, so it is never hidden.
    pub dots: Vec<Dot>,
    /// Race positions, two digits, dropped where they would stack.
    pub numbers: Vec<Label>,
    /// The focus car's name beside its dot; `true` aligns it to the right.
    pub focus_name: Option<(Label, bool)>,
    pub learning: bool,
    pub unavailable: bool,
    pub track_name: String,
    pub length: Option<String>,
    pub summary: String,
}

fn finite(x: f32, z: f32) -> bool {
    x.is_finite() && z.is_finite()
}

fn summary(inputs: &Inputs) -> String {
    let n = inputs.cars.len();
    if n == 0 {
        return if inputs.restricted > 0 {
            "Restricted".into()
        } else {
            "\u{2014}".into()
        };
    }
    format!(
        "{n} car{}{}{}",
        if n == 1 { "" } else { "s" },
        if inputs.cars.iter().any(|c| c.stale) {
            " \u{B7} stale \u{25F7}"
        } else {
            ""
        },
        if inputs.restricted > 0 {
            format!(" \u{B7} {} restricted", inputs.restricted)
        } else {
            String::new()
        }
    )
}

pub fn view(inputs: &Inputs, fit: &mut Fit) -> View {
    let w = inputs.width;
    let h = (inputs.height - CHROME_HEIGHT).max(1.0);
    let map = match &inputs.map {
        Lookup::Found(map) => Some(map),
        _ => None,
    };
    let mut outline: Vec<(u32, f32, f32)> = map
        .map(|m| {
            m.outline
                .iter()
                .filter(|p| finite(p.x, p.z))
                .map(|p| (p.bucket, p.x, p.z))
                .collect()
        })
        .unwrap_or_default();
    outline.sort_by_key(|p| p.0);
    let pit: Vec<(f32, f32)> = map
        .and_then(|m| m.pit.as_ref())
        .map(|pit| {
            pit.iter()
                .filter(|p| finite(p[0], p[1]))
                .map(|p| (p[0], p[1]))
                .collect()
        })
        .unwrap_or_default();
    let cars: Vec<&Car> = inputs.cars.iter().filter(|c| finite(c.x, c.z)).collect();

    let identity = (inputs.epoch, inputs.session);
    if fit.identity != Some(identity) {
        *fit = Fit {
            identity: Some(identity),
            bounds: None,
        };
    }
    // Expand in 100 m steps. Moving dots inside this envelope never change the fit.
    for (x, z) in outline
        .iter()
        .map(|p| (p.1, p.2))
        .chain(pit.iter().copied())
        .chain(cars.iter().map(|c| (c.x, c.z)))
    {
        let step = |v: f32| (v / 100.0).floor() * 100.0;
        let b = fit.bounds.get_or_insert(Bounds {
            min_x: step(x),
            min_z: step(z),
            max_x: step(x) + 100.0,
            max_z: step(z) + 100.0,
        });
        if x < b.min_x {
            b.min_x = step(x);
        }
        if z < b.min_z {
            b.min_z = step(z);
        }
        if x > b.max_x {
            b.max_x = (x / 100.0).ceil() * 100.0;
        }
        if z > b.max_z {
            b.max_z = (z / 100.0).ceil() * 100.0;
        }
    }

    let session = inputs.session;
    let mut view = View {
        density: DENSITIES.pick(inputs.width, inputs.height),
        canvas: (w, h),
        road_width: (w.min(h) / 22.0).clamp(4.0, 10.0),
        road: vec![],
        pit: vec![],
        markers: vec![],
        dots: vec![],
        numbers: vec![],
        focus_name: None,
        learning: session.is_some() && !map.is_some_and(|m| m.complete),
        unavailable: inputs.map == Lookup::Failed,
        track_name: session
            .and_then(|s| track_name(s.track_id))
            .unwrap_or("Track")
            .to_owned(),
        length: session
            .filter(|s| s.track_length > 0)
            .map(|s| format!("{} km", fixed(f64::from(s.track_length) / 1000.0, 3))),
        summary: summary(inputs),
    };
    let Some(b) = fit.bounds else { return view };
    let scale = ((w - 2.0 * MAP_INSET).max(1.0) / (b.max_x - b.min_x))
        .min((h - 2.0 * MAP_INSET).max(1.0) / (b.max_z - b.min_z))
        .max(0.001);
    let project = |x: f32, z: f32| {
        (
            w / 2.0 + (x - (b.min_x + b.max_x) / 2.0) * scale,
            h / 2.0 + (z - (b.min_z + b.max_z) / 2.0) * scale,
        )
    };

    let mut run: Vec<(f32, f32)> = Vec::new();
    for (i, p) in outline.iter().enumerate() {
        let joined = i > 0 && (p.0 - outline[i - 1].0) as f32 * BUCKET <= JOIN_METRES;
        if !joined && !run.is_empty() {
            view.road.push(std::mem::take(&mut run));
        }
        run.push(project(p.1, p.2));
    }
    if let (Some(first), Some(last), Some(s)) = (outline.first(), outline.last(), session)
        && map.is_some_and(|m| m.complete)
        && f32::from(s.track_length) - (last.0 - first.0) as f32 * BUCKET <= JOIN_METRES
    {
        run.push(project(first.1, first.2));
    }
    if !run.is_empty() {
        view.road.push(run);
    }
    view.pit = pit.iter().map(|&(x, z)| project(x, z)).collect();

    let sectors = [
        (Some(0.0), "S/F"),
        (session.map(|s| s.sector2), "S2"),
        (session.map(|s| s.sector3), "S3"),
    ];
    for (distance, label) in sectors {
        let Some(distance) = distance else { continue };
        if label != "S/F" && distance <= 0.0 {
            continue;
        }
        let mut marker = None;
        for (i, p) in outline.iter().enumerate() {
            let at = p.0 as f32 * BUCKET;
            if at == distance {
                marker = Some((p.1, p.2));
                break;
            }
            if let Some(next) = outline.get(i + 1) {
                let next_at = next.0 as f32 * BUCKET;
                if at < distance && next_at > distance && next_at - at <= JOIN_METRES {
                    let f = (distance - at) / (next_at - at);
                    marker = Some((p.1 + (next.1 - p.1) * f, p.2 + (next.2 - p.2) * f));
                    break;
                }
            }
        }
        if let Some((x, z)) = marker {
            view.markers.push(Label {
                text: label.into(),
                at: project(x, z),
                focused: false,
            });
        }
    }

    let mut ordered = cars.clone();
    ordered.sort_by_key(|c| Some(c.index) == inputs.focus);
    for car in &ordered {
        let focused = Some(car.index) == inputs.focus;
        view.dots.push(Dot {
            index: car.index,
            at: project(car.x, car.z),
            radius: if focused && !car.stale { 6.0 } else { 4.0 },
            fill: match (car.stale, car.colour) {
                (true, _) => Fill::Stale,
                (false, Some((r, g, b))) => Fill::Team(r, g, b),
                (false, None) if focused => Fill::Accent,
                (false, None) => Fill::Field,
            },
            focused,
            stale: car.stale,
        });
    }
    let mut taken: Vec<(f32, f32)> = Vec::new();
    for car in ordered.iter().rev() {
        let Some(position) = car.position else {
            continue;
        };
        let (x, y) = project(car.x, car.z);
        if taken
            .iter()
            .any(|(a, b)| (a - x).abs() < LABEL_WIDTH && (b - y).abs() < LABEL_HEIGHT)
        {
            continue;
        }
        taken.push((x, y));
        view.numbers.push(Label {
            text: format!("{position:02}"),
            at: (x + 7.0, y - 7.0),
            focused: Some(car.index) == inputs.focus,
        });
    }
    if let (Some(car), Some(name)) = (
        cars.iter().find(|c| Some(c.index) == inputs.focus),
        &inputs.focus_name,
    ) {
        let (x, y) = project(car.x, car.z);
        let right = x > w * 0.6;
        view.focus_name = Some((
            Label {
                text: name.clone(),
                at: (if right { x - 10.0 } else { x + 10.0 }, y + 16.0),
                focused: true,
            },
            right,
        ));
    }
    view
}

/// Split a polyline into `on`/`off` pixel dashes.
fn dashes(points: &[(f32, f32)], on: f32, off: f32) -> Vec<Vec<(f32, f32)>> {
    let mut out = Vec::new();
    let mut current = vec![];
    let mut drawing = true;
    let mut left = on;
    for pair in points.windows(2) {
        let (mut a, b) = (pair[0], pair[1]);
        let mut length = (b.0 - a.0).hypot(b.1 - a.1);
        while length > 0.0 {
            let step = left.min(length);
            let f = step / length;
            let next = (a.0 + (b.0 - a.0) * f, a.1 + (b.1 - a.1) * f);
            if drawing {
                if current.is_empty() {
                    current.push(a);
                }
                current.push(next);
            }
            left -= step;
            length -= step;
            a = next;
            if left <= 0.0 {
                if drawing {
                    out.push(std::mem::take(&mut current));
                }
                drawing = !drawing;
                left = if drawing { on } else { off };
            }
        }
    }
    if current.len() > 1 {
        out.push(current);
    }
    out
}

fn text_at(label: &Label, size: f32, colour: Hsla, right_edge: Option<f32>) -> AnyElement {
    // Canvas text sits on its baseline; a div is placed by its top.
    let top = label.at.1 - size;
    let mut el = div()
        .absolute()
        .top(px(top))
        .text_size(px(size))
        .line_height(px(size + 2.0))
        .whitespace_nowrap()
        .text_color(colour)
        .child(SharedString::from(label.text.clone()));
    el = match right_edge {
        Some(width) => el.right(px(width - label.at.0)),
        None => el.left(px(label.at.0)),
    };
    el.into_any_element()
}

pub fn render(view: View) -> impl IntoElement {
    let (w, h) = view.canvas;
    let road = view.road.clone();
    let pit = view.pit.clone();
    let markers: Vec<(f32, f32)> = view.markers.iter().map(|m| m.at).collect();
    let dots = view.dots.clone();
    let road_width = view.road_width;
    let drawing = canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            let layers = [
                (road_width + 6.0, casing()),
                (road_width + 2.0, theme::line()),
                (road_width, track_colour()),
                (1.0, theme::text().opacity(0.18)),
            ];
            for (width, colour) in layers {
                for run in &road {
                    if run.len() == 1 {
                        paint::disc(window, bounds, run[0], width / 2.0, colour);
                    }
                    paint::polyline(window, bounds, run, width, colour);
                }
            }
            for dash in dashes(&pit, 4.0, 4.0) {
                paint::polyline(window, bounds, &dash, 2.0, theme::muted());
            }
            for &(x, y) in &markers {
                paint::polyline(
                    window,
                    bounds,
                    &[(x - 4.0, y - 4.0), (x + 4.0, y + 4.0)],
                    2.0,
                    theme::text(),
                );
            }
            for dot in &dots {
                let fill = match dot.fill {
                    Fill::Team(r, g, b) => {
                        hex(u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b))
                    }
                    Fill::Accent => theme::hot(),
                    Fill::Field => theme::energy(),
                    Fill::Stale => theme::muted(),
                };
                if dot.stale {
                    paint::disc(window, bounds, dot.at, dot.radius, fill);
                    paint::arc(
                        window,
                        bounds,
                        dot.at,
                        6.0,
                        0.0,
                        std::f32::consts::TAU,
                        2.0,
                        theme::text(),
                    );
                } else {
                    // A dark rim keeps dark liveries visible; the focus car gets an accent ring.
                    let (rim, colour) = if dot.focused {
                        (2.0, theme::hot())
                    } else {
                        (1.0, casing())
                    };
                    paint::disc(window, bounds, dot.at, dot.radius + rim / 2.0, colour);
                    paint::disc(window, bounds, dot.at, dot.radius - rim / 2.0, fill);
                }
            }
        },
    )
    .size_full();

    let mut map = div()
        .relative()
        .w(px(w))
        .h(px(h))
        .flex_none()
        .overflow_hidden()
        .child(drawing);
    for m in &view.markers {
        map = map.child(text_at(
            &Label {
                at: (m.at.0 + 5.0, m.at.1 - 5.0),
                ..m.clone()
            },
            11.0,
            theme::text(),
            None,
        ));
    }
    for n in &view.numbers {
        let colour = if n.focused {
            theme::hot()
        } else {
            theme::text()
        };
        map = map.child(text_at(n, 11.0, colour, None));
    }
    if let Some((label, right)) = &view.focus_name {
        map = map.child(
            div()
                .absolute()
                .size_full()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(text_at(label, 12.0, theme::hot(), right.then_some(w))),
        );
    }
    let mut title = div()
        .absolute()
        .top(px(8.0))
        .left(px(12.0))
        .flex()
        .gap_2()
        .child(widgets::title("Circuit position"));
    if view.learning {
        title = title.child(widgets::label("Learning map"));
    }
    if view.unavailable {
        title = title.child(widgets::label("Map unavailable"));
    }
    let mut facts = div()
        .flex()
        .gap_2()
        .child(widgets::label(view.summary.clone()));
    if let Some(length) = &view.length {
        facts = facts.child(widgets::label(length.clone()));
    }
    div()
        .relative()
        .flex()
        .flex_col()
        .size_full()
        .overflow_hidden()
        .child(map)
        .child(title)
        .child(
            div()
                .flex()
                .justify_between()
                .items_center()
                .h(px(CHROME_HEIGHT))
                .px_3()
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(theme::text())
                        .child(view.track_name.clone()),
                )
                .child(facts),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use pitwall_core::tracks::{OutlinePoint, Source};

    fn facts(track_id: i8) -> SessionFacts {
        SessionFacts {
            track_id,
            track_length: 1000,
            sector2: 80.0,
            sector3: 400.0,
        }
    }
    fn learned(complete: bool, outline: &[(u32, f32, f32)]) -> Lookup {
        Lookup::Found(TrackMap {
            key: TrackKey::new(31, 1000),
            source: Source::Learned,
            complete,
            outline: outline
                .iter()
                .map(|&(bucket, x, z)| OutlinePoint { bucket, x, z })
                .collect(),
            pit: Some(vec![[20.0, 10.0], [30.0, 20.0]]),
        })
    }
    fn car(index: usize, x: f32) -> Car {
        Car {
            index,
            x,
            z: 50.0,
            stale: false,
            position: None,
            colour: None,
        }
    }
    fn base(map: Lookup, cars: Vec<Car>) -> Inputs {
        Inputs {
            session: Some(facts(31)),
            epoch: 1,
            map,
            cars,
            restricted: 0,
            focus: None,
            focus_name: None,
            width: 300.0,
            height: 240.0,
        }
    }
    const OUTLINE: [(u32, f32, f32); 3] = [(0, 0.0, 0.0), (10, 100.0, 0.0), (11, 100.0, 100.0)];
    fn labels(v: &View) -> Vec<&str> {
        v.markers.iter().map(|m| m.text.as_str()).collect()
    }

    #[test]
    fn declares_the_catalogue_entry() {
        assert_eq!((TYPE, NAME), ("track-map", "Track map"));
        assert_eq!(DENSITIES.pick(239.0, 160.0), Density::Compact);
        assert_eq!(DENSITIES.pick(240.0, 160.0), Density::Regular);
    }

    #[test]
    fn draws_learned_line_pit_lane_and_only_covered_sector_markers() {
        let v = view(&base(learned(false, &OUTLINE), vec![]), &mut Fit::default());
        assert!(v.learning);
        assert!(!v.road.is_empty());
        assert_eq!(v.pit.len(), 2);
        assert_eq!(labels(&v), ["S/F", "S2"]);
        // Buckets 0 and 10 are 80 m apart: not joined.
        assert_eq!(v.road.iter().map(Vec::len).collect::<Vec<_>>(), [1, 2]);
    }

    #[test]
    fn car_dots_use_raw_xz_before_any_map_and_refit_only_when_bounds_widen() {
        let mut fit = Fit::default();
        let at = |fit: &mut Fit, x| view(&base(Lookup::NotFound, vec![car(0, x)]), fit).dots[0].at;
        assert_eq!(at(&mut fit, 20.0), (102.0, 100.0));
        assert_eq!(at(&mut fit, 30.0), (118.0, 100.0));
        assert_eq!(at(&mut fit, 150.0), (215.0, 100.0));
    }

    #[test]
    fn a_complete_map_drops_the_learning_label_and_closes_the_loop() {
        let outline: Vec<_> = (0..125).map(|b| (b, b as f32, 0.0)).collect();
        let v = view(&base(learned(true, &outline), vec![]), &mut Fit::default());
        assert!(!v.learning);
        assert_eq!(v.road.len(), 1);
        assert_eq!(v.road[0].first(), v.road[0].last());
    }

    #[test]
    fn stale_dots_keep_positions_and_marker_restricted_and_absent_never_draw_at_zero() {
        let mut stale = car(0, 20.0);
        stale.stale = true;
        let v = view(&base(Lookup::NotFound, vec![stale]), &mut Fit::default());
        assert_eq!(v.dots[0].radius, 4.0);
        assert_eq!(v.dots[0].fill, Fill::Stale);
        assert_eq!(v.summary, "1 car \u{B7} stale \u{25F7}");
        let mut restricted = base(Lookup::NotFound, vec![]);
        restricted.restricted = 1;
        let v = view(&restricted, &mut Fit::default());
        assert!(v.dots.is_empty());
        assert_eq!(v.summary, "Restricted");
        let v = view(&base(Lookup::NotFound, vec![]), &mut Fit::default());
        assert_eq!(v.summary, "\u{2014}");
        let mut nan = car(0, f32::NAN);
        nan.z = 0.0;
        assert!(
            view(&base(Lookup::NotFound, vec![nan]), &mut Fit::default())
                .dots
                .is_empty()
        );
    }

    #[test]
    fn a_new_track_or_epoch_resets_the_fit() {
        let mut fit = Fit::default();
        view(&base(Lookup::NotFound, vec![car(0, 950.0)]), &mut fit);
        let mut other = base(Lookup::NotFound, vec![car(0, 20.0)]);
        other.session = Some(facts(32));
        assert_eq!(view(&other, &mut fit).dots[0].at, (102.0, 100.0));
        let mut later = base(Lookup::NotFound, vec![car(0, 20.0)]);
        view(&base(Lookup::NotFound, vec![car(0, 950.0)]), &mut fit);
        later.epoch = 2;
        assert_eq!(view(&later, &mut fit).dots[0].at, (102.0, 100.0));
    }

    #[test]
    fn resize_redraws_at_the_new_size() {
        let mut fit = Fit::default();
        let old = view(&base(Lookup::NotFound, vec![car(0, 20.0)]), &mut fit).dots[0].at;
        let mut wide = base(Lookup::NotFound, vec![car(0, 20.0)]);
        wide.width = 400.0;
        assert_ne!(view(&wide, &mut fit).dots[0].at, old);
    }

    #[test]
    fn a_sector_marker_appears_on_a_short_supported_segment_between_buckets() {
        let v = view(
            &base(learned(false, &[(9, 90.0, 0.0), (11, 110.0, 20.0)]), vec![]),
            &mut Fit::default(),
        );
        assert_eq!(labels(&v), ["S2"]);
    }

    #[test]
    fn a_failed_lookup_says_map_unavailable() {
        let v = view(&base(Lookup::Failed, vec![]), &mut Fit::default());
        assert!(v.unavailable);
        assert!(v.learning);
    }

    #[test]
    fn dots_wear_team_colours_and_the_focus_car_draws_last_with_its_name() {
        let mut a = car(0, 20.0);
        a.colour = Some((255, 0, 0));
        a.position = Some(2);
        let mut b = car(1, 60.0);
        b.position = Some(1);
        let mut c = car(2, 80.0);
        c.position = Some(3);
        let mut inputs = base(Lookup::NotFound, vec![a, b, c]);
        inputs.focus = Some(1);
        inputs.focus_name = Some("BRAVO".into());
        let v = view(&inputs, &mut Fit::default());
        assert_eq!(
            v.dots.iter().map(|d| d.index).collect::<Vec<_>>(),
            [0, 2, 1]
        );
        assert_eq!(v.dots[0].fill, Fill::Team(255, 0, 0));
        assert_eq!(v.dots[1].fill, Fill::Field);
        assert_eq!((v.dots[2].fill, v.dots[2].radius), (Fill::Accent, 6.0));
        let numbers: Vec<_> = v.numbers.iter().map(|n| n.text.as_str()).collect();
        assert_eq!(numbers, ["01", "03", "02"]);
        let (name, right) = v.focus_name.unwrap();
        assert_eq!((name.text.as_str(), right), ("BRAVO", false));
    }

    #[test]
    fn crowded_numbers_are_dropped_rather_than_stacked() {
        let mut a = car(0, 20.0);
        a.position = Some(1);
        let mut b = car(1, 21.0);
        b.position = Some(2);
        let v = view(&base(Lookup::NotFound, vec![a, b]), &mut Fit::default());
        assert_eq!(v.numbers.len(), 1);
    }

    #[test]
    fn the_footer_keeps_the_track_name_and_three_decimal_kilometres() {
        let mut inputs = base(Lookup::NotFound, vec![]);
        inputs.session = Some(SessionFacts {
            track_id: 0,
            track_length: 5276,
            sector2: 0.0,
            sector3: 0.0,
        });
        let v = view(&inputs, &mut Fit::default());
        assert_eq!(v.track_name, "Melbourne");
        assert_eq!(v.length.as_deref(), Some("5.276 km"));
        inputs.session = None;
        let v = view(&inputs, &mut Fit::default());
        assert_eq!((v.track_name.as_str(), v.length), ("Track", None));
        assert!(!v.learning);
    }

    #[test]
    fn the_store_is_asked_again_only_on_a_new_track_or_revision() {
        let dir = std::env::temp_dir().join(format!("pitwall-map-{}", std::process::id()));
        let store = TrackStore::open(dir);
        let mut memory = Memory::default();
        let melbourne = TrackKey::new(0, 5276);
        let Lookup::Found(map) = memory.lookup(Some(&store), Some(melbourne)) else {
            panic!("Melbourne ships")
        };
        assert_eq!((map.key, map.source), (melbourne, Source::Shipped));
        // A different layout never reuses the previous answer.
        assert_eq!(
            memory.lookup(Some(&store), Some(TrackKey::new(0, 5000))),
            Lookup::NotFound
        );
        assert_eq!(memory.lookup(Some(&store), None), Lookup::None);
        assert_eq!(memory.lookup(None, Some(melbourne)), Lookup::Failed);
    }

    #[test]
    fn dashes_alternate_four_pixels_on_and_off() {
        let d = dashes(&[(0.0, 0.0), (20.0, 0.0)], 4.0, 4.0);
        assert_eq!(d.len(), 3);
        assert_eq!(d[1], vec![(8.0, 0.0), (12.0, 0.0)]);
    }
}
