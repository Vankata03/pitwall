//! Live delta (parity DELTA-01..03): a frame-rate tile following one car.
//! Port of `dashboard/src/tiles/types/live-delta.tsx` and `live-delta/{geometry,lap,chart}.ts`.
//!
//! The web tile rebuilds the lap's graph from frame rings it samples while open. Natively
//! the core already keeps the lap's 5 m distance trace, so the graph is derived from it:
//! each sample's lap time against the reference lap's time at the same distance. The whole
//! lap shows even when the window opens mid-lap, and no sample can be skipped by the paint
//! cadence. The headline is the core's live delta, which at the line is the official lap
//! figure; the official figure is never plotted.

use gpui::{Hsla, IntoElement, ParentElement, Styled, canvas, div, px};
use pitwall_core::{
    model::{LapSample, Lifecycle, Readout, SessionModel},
    readout::{ReadoutState, ReadoutView, readout_view, whole_tile_message},
};

use super::{Densities, Density, format::signed, paint, widgets};
use crate::theme;

pub const TYPE: &str = pitwall_core::tile_catalog::LIVE_DELTA.kind;

pub const DENSITIES: Densities = Densities {
    regular: (280.0, 170.0),
    full: Some((600.0, 240.0)),
};
/// Longest gap in lap distance (m) the graph joins with a straight line (spec section 6).
pub const MAX_JOIN_M: f32 = 100.0;

pub use pitwall_core::model::Reference;

/// Delta in seconds at a lap distance in metres. A NaN delta or distance is a break.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub distance: f32,
    pub delta: f32,
}

/// Lap time (ms) of `trace` at `distance`, interpolated between samples at most 100 m
/// apart; never extrapolated.
fn time_at(trace: &[LapSample], distance: f32) -> Option<f32> {
    let index = trace.partition_point(|p| p.distance_m < distance);
    let right = trace.get(index)?;
    if right.distance_m == distance {
        return Some(right.time_ms);
    }
    let left = trace.get(index.checked_sub(1)?)?;
    (right.distance_m - left.distance_m <= MAX_JOIN_M).then(|| {
        let f = (distance - left.distance_m) / (right.distance_m - left.distance_m);
        left.time_ms + f * (right.time_ms - left.time_ms)
    })
}

/// A lap's delta against `reference` at each of its samples, in seconds. Where the
/// reference has no time the point is a break.
pub fn lap_points(lap: &[LapSample], reference: &[LapSample]) -> Vec<Point> {
    lap.iter()
        .map(|s| Point {
            distance: s.distance_m,
            delta: time_at(reference, s.distance_m).map_or(f32::NAN, |t| (s.time_ms - t) / 1000.0),
        })
        .collect()
}

/// NaN or a gap over 100 m starts a new segment, as does going backwards; negative
/// distance is dropped.
pub fn segments(points: &[Point]) -> Vec<Vec<Point>> {
    let mut out: Vec<Vec<Point>> = vec![];
    let mut open = false;
    for &p in points {
        if !p.distance.is_finite() || p.distance < 0.0 || !p.delta.is_finite() {
            open = false;
            continue;
        }
        let last = out.last().and_then(|s| s.last()).copied();
        let joins = open
            && last
                .is_some_and(|l| p.distance - l.distance <= MAX_JOIN_M && p.distance >= l.distance);
        if !joins {
            out.push(vec![]);
            open = true;
        }
        out.last_mut().unwrap().push(p);
    }
    out
}

/// Symmetric 1/2/5 steps, with a useful floor of ±0.1 s.
pub fn delta_extent(lists: &[&[Point]]) -> f32 {
    // In f32, the points' own precision, so a delta of exactly 0.2 lands on the 0.2 step.
    let mut max = 0.1f32;
    for p in lists.iter().flat_map(|l| l.iter()) {
        if p.delta.is_finite() {
            max = max.max(p.delta.abs());
        }
    }
    let power = 10f32.powi(max.log10().floor() as i32);
    [1.0, 2.0, 5.0, 10.0]
        .into_iter()
        .find(|n| n * power >= max)
        .unwrap_or(10.0)
        * power
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arrangement {
    /// The headline alone.
    Headline,
    /// The headline above the graph.
    Stacked,
    /// The headline beside the graph.
    Side,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Geometry {
    pub arrangement: Arrangement,
    /// Headline font size, chosen from the tile box, never from the density alone.
    pub font_size: f32,
    /// Graph box, or `None` when the density or the box leaves no room.
    pub chart: Option<(f32, f32)>,
}

/// Tile padding, the label line and the gap between parts (`live-delta/geometry.ts`).
const PAD_X: f32 = 32.0;
const PAD_Y: f32 = 24.0;
const LABEL: f32 = 16.0;
const GAP: f32 = 8.0;
/// Width of the longest headline ("+123.456") in ems of the tile font.
const HEADLINE_EMS: f32 = 5.2;
const MIN_FONT: f32 = 36.0;
const MAX_FONT: f32 = 168.0;
const MIN_CHART_HEIGHT: f32 = 60.0;
const MIN_CHART_WIDTH: f32 = 160.0;

/// The tile box decides everything: the headline is as large as the box allows at every
/// density, and the graph takes what is left.
pub fn geometry(width: f32, height: f32, density: Density) -> Geometry {
    let w = (width.floor() - PAD_X).max(0.0);
    let h = (height.floor() - PAD_Y).max(0.0);
    let clamp = |n: f32| n.clamp(MIN_FONT, MAX_FONT);
    let alone = Geometry {
        arrangement: Arrangement::Headline,
        font_size: clamp((w / HEADLINE_EMS).min(h - LABEL - GAP).floor()),
        chart: None,
    };
    if density == Density::Compact {
        return alone;
    }
    if w >= 880.0 && w >= 2.2 * h {
        let font_size = clamp(((w * 0.36) / HEADLINE_EMS).min(h - LABEL - GAP).floor());
        let chart_width = w - (font_size * HEADLINE_EMS).ceil() - 3.0 * GAP;
        return Geometry {
            arrangement: Arrangement::Side,
            font_size,
            chart: (chart_width >= MIN_CHART_WIDTH && h >= MIN_CHART_HEIGHT)
                .then_some((chart_width, h)),
        };
    }
    let font_size = clamp(
        (w / HEADLINE_EMS)
            .min((h - LABEL - 2.0 * GAP) * 0.42)
            .floor(),
    );
    let chart_height = h - LABEL - font_size - 2.0 * GAP;
    if chart_height < MIN_CHART_HEIGHT || w < MIN_CHART_WIDTH {
        return alone;
    }
    Geometry {
        arrangement: Arrangement::Stacked,
        font_size,
        chart: Some((w, chart_height)),
    }
}

pub struct Inputs {
    /// Signed milliseconds against the reference; negative is ahead.
    pub delta: Readout<f32>,
    pub reference: Reference,
    /// Metres; 0 when unknown (no graph is drawn).
    pub track_length: f32,
    /// Sector 2 and 3 start distances, metres.
    pub sectors: [f32; 2],
    /// This lap's graph points.
    pub current: Vec<Point>,
    /// The finished lap's graph points, against the reference it was driven against.
    pub previous: Vec<Point>,
    pub lifecycle: Lifecycle,
    pub width: f32,
    pub height: f32,
}

impl Inputs {
    pub fn from_model(
        model: &SessionModel,
        index: Option<usize>,
        reference: Reference,
        size: (f32, f32),
    ) -> Self {
        let session = model.session().value().map(|s| {
            (
                f32::from(s.track_length),
                [s.sector2_lap_distance_start, s.sector3_lap_distance_start],
            )
        });
        let car = index.and_then(|i| model.car(i));
        let current_trace = car.and_then(|c| c.current_lap_trace());
        let current = car
            .zip(current_trace)
            .and_then(|(c, lap)| {
                let reference = c.lap_trace(reference)?;
                Some(lap_points(&lap.samples, &reference.samples))
            })
            .unwrap_or_default();
        let previous = car
            .zip(current_trace)
            .and_then(|(c, lap)| {
                let finished = c.completed_lap_trace(lap.lap_number.checked_sub(1)?)?;
                // The reference that lap was driven against: the lap before it, or the best
                // valid lap completed before it.
                let against = match reference {
                    Reference::Previous => finished.lap_number.checked_sub(1)?,
                    _ => {
                        c.session_history()
                            .laps
                            .iter()
                            .filter(|l| l.valid && l.lap_number < finished.lap_number)
                            .min_by_key(|l| (l.lap_time_ms, l.lap_number))?
                            .lap_number
                    }
                };
                let against = c.completed_lap_trace(against)?;
                Some(lap_points(&finished.samples, &against.samples))
            })
            .filter(|points| points.iter().filter(|p| p.delta.is_finite()).count() > 1)
            .unwrap_or_default();
        Self {
            delta: car.map_or(Readout::Absent, |c| c.live_delta(reference)),
            reference,
            track_length: session.map_or(0.0, |s| s.0),
            sectors: session.map_or([0.0; 2], |s| s.1),
            current,
            previous,
            lifecycle: model.lifecycle(),
            width: size.0,
            height: size.1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sign {
    Ahead,
    Behind,
    Equal,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Chart {
    pub width: f32,
    pub height: f32,
    pub track_length: f32,
    pub sectors: [f32; 2],
    /// Seconds from the zero line to the top and bottom.
    pub extent: f32,
    pub current: Vec<Vec<Point>>,
    /// The finished lap, only ahead of the car: the new lap overwrites it as it passes.
    pub ahead: Vec<Vec<Point>>,
    /// The car's dot: the newest drawable point of this lap.
    pub head: Option<Point>,
    /// Stale or missing: drawn in the muted colour only.
    pub muted: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct View {
    pub density: Density,
    pub label: String,
    pub headline: ReadoutView,
    /// Live and stale only.
    pub sign: Option<Sign>,
    pub geometry: Geometry,
    pub chart: Option<Chart>,
    pub message: Option<&'static str>,
}

pub fn view(inputs: &Inputs) -> View {
    let density = DENSITIES.pick(inputs.width, inputs.height);
    let state = inputs.delta.state();
    let ms = inputs.delta.value().copied().unwrap_or(0.0);
    let geometry = geometry(inputs.width, inputs.height, density);
    // A first lap has no reference, so absent is normal: say why only when the game is
    // the reason.
    let message = if state == ReadoutState::Absent {
        whole_tile_message([state], inputs.lifecycle, true).filter(|m| *m != "No data")
    } else {
        None
    };
    let chart = geometry
        .chart
        .filter(|_| inputs.track_length > 0.0)
        .map(|(width, height)| {
            let head = inputs
                .current
                .iter()
                .rev()
                .find(|p| p.distance.is_finite() && p.delta.is_finite())
                .copied();
            let passed = head.map_or(0.0, |p| p.distance);
            let ahead: Vec<Point> = inputs
                .previous
                .iter()
                .filter(|p| p.distance >= passed || p.distance.is_nan())
                .copied()
                .collect();
            Chart {
                width,
                height,
                track_length: inputs.track_length,
                sectors: inputs.sectors,
                extent: delta_extent(&[&inputs.current, &ahead]),
                current: segments(&inputs.current),
                ahead: segments(&ahead),
                head,
                muted: state != ReadoutState::Live,
            }
        });
    View {
        density,
        label: format!("{} \u{b7} delta / s", inputs.reference.label()),
        headline: readout_view(state, &signed(f64::from(ms) / 1000.0, 3)),
        sign: state.has_data().then_some(if ms < 0.0 {
            Sign::Ahead
        } else if ms > 0.0 {
            Sign::Behind
        } else {
            Sign::Equal
        }),
        geometry,
        chart,
        message,
    }
}

fn sign_colour(sign: Option<Sign>) -> Option<Hsla> {
    match sign? {
        Sign::Ahead => Some(theme::personal_best()),
        Sign::Behind => Some(theme::slower()),
        Sign::Equal => None,
    }
}

pub fn render(view: View) -> impl IntoElement {
    let colour = if view.headline.state == ReadoutState::Live {
        sign_colour(view.sign)
    } else {
        None
    };
    let head = div()
        .flex()
        .flex_col()
        .child(widgets::label(view.label.clone()))
        .child(widgets::readout_coloured(
            &view.headline,
            view.geometry.font_size,
            colour,
        ));
    let mut body = div().flex().gap_2().size_full();
    body = match view.geometry.arrangement {
        Arrangement::Side => body.flex_row().items_center(),
        _ => body.flex_col(),
    };
    body = body.child(head);
    if let Some(chart) = view.chart {
        body = body.child(
            div()
                .w(px(chart.width))
                .h(px(chart.height))
                .flex_none()
                .child(render_chart(chart)),
        );
    }
    widgets::tile_shell(body, view.message)
}

const LEFT: f32 = 48.0;
const RIGHT: f32 = 8.0;
const TOP: f32 = 8.0;
const BOTTOM: f32 = 20.0;
const LINE: f32 = 3.0;
const FILL_ALPHA: f32 = 0.22;
const FAINT: f32 = 0.28;

/// Delta against lap distance for the whole lap. Ahead (negative) is above the zero line
/// and labelled in words; each half of a run takes its sign colour and its area to zero.
fn render_chart(chart: Chart) -> impl IntoElement {
    let (w, h) = (chart.width, chart.height);
    let extent = chart.extent;
    let labels = [
        (
            0.0,
            TOP - 2.0,
            format!("{} s", signed(f64::from(-extent), 1)),
        ),
        (0.0, TOP + 11.0, "ahead".to_string()),
        (
            LEFT - 14.0,
            TOP + (h - BOTTOM - TOP) / 2.0 - 8.0,
            "0".to_string(),
        ),
        (
            0.0,
            h - BOTTOM - 27.0,
            format!("{} s", signed(f64::from(extent), 1)),
        ),
        (0.0, h - BOTTOM - 14.0, "behind".to_string()),
        (LEFT, h - 16.0, "0 m".to_string()),
    ];
    let length_label = format!("{} m", chart.track_length.round());
    let paint = canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            let (left, right, top, bottom) = (LEFT, w - RIGHT, TOP, h - BOTTOM);
            let x = |d: f32| left + d / chart.track_length * (right - left);
            let y = |delta: f32| top + (extent + delta) / (extent * 2.0) * (bottom - top);
            let zero = y(0.0);
            for &d in &chart.sectors {
                if d > 0.0 && d < chart.track_length {
                    paint::polyline(
                        window,
                        bounds,
                        &[(x(d), top), (x(d), bottom)],
                        1.0,
                        theme::muted().opacity(0.3),
                    );
                }
            }
            paint::polyline(
                window,
                bounds,
                &[(left, zero), (right, zero)],
                1.0,
                theme::muted().opacity(0.5),
            );
            let (ahead_ink, behind_ink) = if chart.muted {
                (theme::muted(), theme::muted())
            } else {
                (theme::personal_best(), theme::slower())
            };
            let run = |window: &mut gpui::Window, runs: &[Vec<Point>], alpha: f32| {
                for segment in runs.iter().filter(|s| s.len() >= 2) {
                    // Split each run at the zero line so each side takes its colour.
                    for (above, ink) in [(true, ahead_ink), (false, behind_ink)] {
                        let clip = |delta: f32| {
                            if above {
                                delta.min(0.0)
                            } else {
                                delta.max(0.0)
                            }
                        };
                        let line: Vec<(f32, f32)> = segment
                            .iter()
                            .map(|p| (x(p.distance), y(clip(p.delta))))
                            .collect();
                        if line.iter().all(|&(_, py)| (py - zero).abs() < 0.01) {
                            continue;
                        }
                        let mut area = line.clone();
                        area.push((line[line.len() - 1].0, zero));
                        area.push((line[0].0, zero));
                        paint::polygon(window, bounds, &area, ink.opacity(alpha * FILL_ALPHA));
                        paint::polyline(window, bounds, &line, LINE, ink.opacity(alpha));
                    }
                }
            };
            run(window, &chart.ahead, FAINT);
            run(window, &chart.current, 1.0);
            if let Some(head) = chart.head {
                let ink = if chart.muted || head.delta == 0.0 {
                    theme::muted()
                } else if head.delta < 0.0 {
                    theme::personal_best()
                } else {
                    theme::slower()
                };
                paint::disc(window, bounds, (x(head.distance), y(head.delta)), 4.0, ink);
            }
        },
    )
    .size_full();
    let mut root = div().relative().size_full().child(paint);
    for (lx, ly, text) in labels {
        root = root.child(
            div()
                .absolute()
                .left(px(lx))
                .top(px(ly))
                .text_size(px(11.0))
                .line_height(px(14.0))
                .text_color(theme::muted())
                .child(text),
        );
    }
    root.child(
        div()
            .absolute()
            .right(px(RIGHT))
            .top(px(h - 16.0))
            .text_size(px(11.0))
            .line_height(px(14.0))
            .text_color(theme::muted())
            .child(length_label),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use pitwall_core::readout::ABSENT_TEXT;
    use serde_json::json;

    fn p(distance: f32, delta: f32) -> Point {
        Point { distance, delta }
    }
    fn sample(distance_m: f32, time_ms: f32) -> LapSample {
        LapSample {
            distance_m,
            time_ms,
            session_time: 0.0,
            speed_kph: None,
            throttle: None,
            brake: None,
        }
    }
    fn base(delta: Readout<f32>) -> Inputs {
        Inputs {
            delta,
            reference: Reference::Best,
            track_length: 1000.0,
            sectors: [300.0, 700.0],
            current: vec![],
            previous: vec![],
            lifecycle: Lifecycle::Live,
            width: 600.0,
            height: 300.0,
        }
    }
    fn distances(segments: &[Vec<Point>]) -> Vec<Vec<f32>> {
        segments
            .iter()
            .map(|s| s.iter().map(|p| p.distance).collect())
            .collect()
    }

    #[test]
    fn declares_the_catalogue_entry() {
        assert_eq!(
            (TYPE, pitwall_core::tile_catalog::LIVE_DELTA.name),
            ("live-delta", "Live delta")
        );
        assert_eq!(DENSITIES.pick(279.0, 170.0), Density::Compact);
        assert_eq!(DENSITIES.pick(280.0, 170.0), Density::Regular);
        assert_eq!(DENSITIES.pick(600.0, 240.0), Density::Full);
    }

    #[test]
    fn the_headline_is_signed_to_three_decimals_and_its_sign_picks_the_colour() {
        let cases = [
            (-123.0, "\u{2212}0.123", Sign::Ahead),
            (124.0, "+0.124", Sign::Behind),
            (0.0, "+0.000", Sign::Equal),
            (-1.0, "\u{2212}0.001", Sign::Ahead),
            (12345.0, "+12.345", Sign::Behind),
        ];
        for (ms, text, sign) in cases {
            let v = view(&base(Readout::Live(ms)));
            assert_eq!(v.headline.text, text);
            assert_eq!(v.sign, Some(sign));
            assert_eq!(v.headline.state, ReadoutState::Live);
        }
    }

    #[test]
    fn the_reference_setting_is_best_or_previous_defaulting_to_best() {
        assert_eq!(Reference::best_or_previous_of(&json!({})), Reference::Best);
        assert_eq!(
            Reference::best_or_previous_of(&json!({"reference": "previous"})),
            Reference::Previous
        );
        assert_eq!(
            Reference::best_or_previous_of(&json!({"reference": "nonsense"})),
            Reference::Best
        );
        let v = view(&base(Readout::Live(-50.0)));
        assert!(v.label.starts_with("Best lap"));
        let prev = view(&Inputs {
            reference: Reference::Previous,
            ..base(Readout::Live(432.0))
        });
        assert!(prev.label.starts_with("Previous lap"));
    }

    #[test]
    fn the_four_readout_states_show_on_the_headline() {
        let stale = view(&base(Readout::Stale(-123.0)));
        assert_eq!(
            (stale.headline.state, stale.headline.text.as_str()),
            (ReadoutState::Stale, "\u{2212}0.123")
        );
        let restricted = view(&base(Readout::Restricted));
        assert_eq!(
            (restricted.headline.text.as_str(), restricted.sign),
            ("Restricted", None)
        );
        let absent = view(&base(Readout::Absent));
        assert_eq!(
            (absent.headline.text.as_str(), absent.sign),
            (ABSENT_TEXT, None)
        );
        // Absent on a first lap is normal: no whole-tile message over it.
        assert_eq!(absent.message, None);
    }

    #[test]
    fn with_the_game_away_and_no_value_the_tile_says_why() {
        let v = view(&Inputs {
            lifecycle: Lifecycle::Waiting,
            ..base(Readout::Absent)
        });
        assert_eq!(v.message, Some("Waiting for the game"));
        let lost = view(&Inputs {
            lifecycle: Lifecycle::Lost,
            ..base(Readout::Absent)
        });
        assert_eq!(lost.message, Some("No data: game paused or closed"));
    }

    #[test]
    fn the_headline_stays_huge_at_every_size_and_fits_its_box() {
        for (w, h) in [
            (300.0, 112.0),
            (220.0, 112.0),
            (320.0, 232.0),
            (700.0, 232.0),
            (1450.0, 232.0),
            (1450.0, 400.0),
            (400.0, 600.0),
        ] {
            for density in [Density::Compact, Density::Regular, Density::Full] {
                let g = geometry(w, h, density);
                assert!(g.font_size >= 36.0, "{w}x{h} {density:?}");
                assert!(
                    g.font_size * 5.2 <= w - 32.0 + 0.5,
                    "eight characters fit the width"
                );
                assert!(g.font_size + 16.0 <= h - 24.0 + 0.5, "and the height");
            }
        }
    }

    #[test]
    fn compact_is_the_headline_alone_and_larger_tiles_add_a_graph_that_fits() {
        assert!(geometry(300.0, 112.0, Density::Compact).chart.is_none());
        let wide = geometry(1450.0, 232.0, Density::Full);
        assert_eq!(wide.arrangement, Arrangement::Side);
        let (ww, wh) = wide.chart.unwrap();
        assert!(ww > 700.0 && wh > 150.0);
        let stacked = geometry(700.0, 232.0, Density::Regular);
        assert_eq!(stacked.arrangement, Arrangement::Stacked);
        assert_eq!(stacked.chart.unwrap().0, 700.0 - 32.0);
        assert!(stacked.chart.unwrap().1 >= 60.0);
        assert!(geometry(400.0, 600.0, Density::Full).chart.unwrap().1 > 250.0);
        assert!(geometry(300.0, 150.0, Density::Regular).chart.is_none());
        let g = geometry(1001.0, 233.0, Density::Full);
        assert_eq!(g.font_size.fract(), 0.0);
        let (cw, ch) = g.chart.unwrap();
        assert_eq!((cw.fract(), ch.fract()), (0.0, 0.0));
        assert!(geometry(300.0, 112.0, Density::Compact).font_size >= 48.0);
        assert!(geometry(1450.0, 232.0, Density::Full).font_size >= 90.0);
    }

    #[test]
    fn graph_joins_up_to_100_m_breaks_longer_gaps_and_nan_with_symmetric_steps() {
        let points: Vec<Point> = [0.0, 100.0, 201.0, 220.0, 240.0, 260.0]
            .into_iter()
            .enumerate()
            .map(|(i, d)| p(d, if i == 4 { f32::NAN } else { -0.23 }))
            .collect();
        assert_eq!(
            distances(&segments(&points)),
            vec![vec![0.0, 100.0], vec![201.0, 220.0], vec![260.0]]
        );
        assert_eq!(delta_extent(&[&points]), 0.5);
        assert_eq!(delta_extent(&[&[p(0.0, 1.4)]]), 2.0);
        assert_eq!(delta_extent(&[&[]]), 0.1);
        assert_eq!(delta_extent(&[&[p(0.0, -3.2)]]), 5.0);
    }

    #[test]
    fn delta_extent_lands_exactly_on_a_step_at_each_decade_boundary() {
        let extent = |d| delta_extent(&[&[p(0.0, d)]]);
        assert_eq!(extent(0.2), 0.2);
        assert_eq!(extent(1.0), 1.0);
        assert_eq!(extent(5.0), 5.0);
        assert_eq!(extent(5.01), 10.0);
        assert_eq!(extent(10.0), 10.0);
        assert_eq!(extent(-60.0), 100.0);
        assert_eq!(extent(0.05), 0.1);
        assert_eq!(
            delta_extent(&[&[p(0.0, f32::INFINITY), p(1.0, f32::NAN)]]),
            0.1
        );
    }

    #[test]
    fn segments_start_over_when_distance_goes_backwards_and_drop_negative_distance() {
        let points: Vec<Point> = [10.0, 20.0, 15.0, 25.0, -1.0, 30.0]
            .into_iter()
            .map(|d| p(d, 0.0))
            .collect();
        assert_eq!(
            distances(&segments(&points)),
            vec![vec![10.0, 20.0], vec![15.0, 25.0], vec![30.0]]
        );
    }

    #[test]
    fn lap_points_compare_lap_time_with_the_reference_at_the_same_distance() {
        let reference = [
            sample(0.0, 0.0),
            sample(50.0, 1000.0),
            sample(100.0, 2000.0),
            sample(300.0, 6000.0),
        ];
        let lap = [
            sample(0.0, 0.0),
            sample(25.0, 400.0),
            sample(100.0, 2300.0),
            sample(200.0, 4000.0),
        ];
        let points = lap_points(&lap, &reference);
        assert_eq!(points[0], p(0.0, 0.0));
        assert_eq!(points[1], p(25.0, -0.1), "interpolated 500 ms at 25 m");
        assert_eq!(points[2], p(100.0, 0.3));
        assert!(
            points[3].delta.is_nan(),
            "reference gap over 100 m is never bridged"
        );
        assert!(lap_points(&lap, &[]).iter().all(|p| p.delta.is_nan()));
    }

    #[test]
    fn the_chart_colours_by_sign_and_mutes_when_stale() {
        let current = vec![
            p(0.0, -0.1),
            p(50.0, -0.2),
            p(100.0, -0.3),
            p(150.0, 0.15),
            p(200.0, 0.3),
        ];
        let v = view(&Inputs {
            current: current.clone(),
            ..base(Readout::Live(300.0))
        });
        let chart = v.chart.unwrap();
        assert_eq!(chart.current.len(), 1);
        assert_eq!(chart.current[0].len(), 5, "every sample is a vertex");
        assert_eq!(chart.head, Some(p(200.0, 0.3)));
        assert!(!chart.muted);
        assert_eq!(chart.extent, 0.5);
        let stale = view(&Inputs {
            current,
            ..base(Readout::Stale(300.0))
        });
        assert!(stale.chart.unwrap().muted);
        let unknown_track = view(&Inputs {
            track_length: 0.0,
            ..base(Readout::Live(0.0))
        });
        assert!(unknown_track.chart.is_none());
    }

    #[test]
    fn the_finished_lap_stays_faint_ahead_of_the_car_until_passed() {
        let previous: Vec<Point> = (2..=18).map(|i| p(i as f32 * 50.0, -0.1)).collect();
        let early = view(&Inputs {
            current: vec![p(10.0, 0.04), p(60.0, 0.04)],
            previous: previous.clone(),
            ..base(Readout::Live(40.0))
        })
        .chart
        .unwrap();
        assert_eq!(early.ahead.len(), 1);
        assert_eq!(early.ahead[0].first().unwrap().distance, 100.0);
        let passed = view(&Inputs {
            current: vec![p(950.0, 0.04), p(960.0, 0.04)],
            previous,
            ..base(Readout::Live(40.0))
        })
        .chart
        .unwrap();
        assert!(passed.ahead.is_empty());
    }
}
