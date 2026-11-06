//! Speed/RPM/gear (parity SPEED-01/02): a frame-rate tile following one car.
//! Port of `dashboard/src/tiles/types/speed-rpm-gear.tsx`.

use gpui::{Hsla, IntoElement, ParentElement, PathBuilder, Styled, canvas, div, point, px};
use pitwall_core::{
    feed::{CarInputs, FastFrame},
    model::{Lifecycle, Readout, SessionModel},
    readout::{ReadoutState, ReadoutView, readout_view, whole_tile_message},
    units::{self, Units},
};

use super::{Densities, Density, format::grouped, widgets};
use crate::theme;

pub const TYPE: &str = pitwall_core::tile_catalog::SPEED_RPM_GEAR.kind;

pub const DENSITIES: Densities = Densities {
    regular: (230.0, 150.0),
    full: Some((330.0, 370.0)),
};
/// Segments in the shift strip.
pub const SEGMENTS: u8 = 20;
/// At and above this share of the maximum the strip is full and steady: the limiter.
const LIMITER_AT: f32 = 0.985;
const DIAL_MIN: f32 = 110.0;

pub struct Inputs {
    /// The bound car's fast-path inputs; `Absent` when no car is bound.
    pub car: Readout<CarInputs>,
    /// The car's maximum RPM from Car Status, when known and non-zero.
    pub max_rpm: Option<u16>,
    pub units: Units,
    pub lifecycle: Lifecycle,
    pub width: f32,
    pub height: f32,
}

impl Inputs {
    /// Bind a fast frame to the matching model timeline before reading car values.
    pub fn from_source(
        frame: &FastFrame,
        model: &SessionModel,
        index: Option<usize>,
        units: Units,
        size: (f32, f32),
    ) -> Self {
        let index = index.filter(|i| frame.epoch == model.epoch() && model.car(*i).is_some());
        Self {
            car: index
                .and_then(|i| frame.cars.get(i).copied())
                .unwrap_or(Readout::Absent),
            max_rpm: index
                .and_then(|i| model.car(i))
                .and_then(|c| c.status().value().map(|s| s.max_rpm))
                .filter(|m| *m > 0),
            units,
            lifecycle: model.lifecycle(),
            width: size.0,
            height: size.1,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Strip {
    pub lit: u8,
    pub limiter: bool,
    /// `Absent` when the car is live but its maximum RPM is unknown.
    pub state: ReadoutState,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Dial {
    pub size: f32,
    /// Share of the maximum RPM, clamped; `None` draws the empty track only.
    pub fill: Option<f32>,
    pub stale: bool,
    /// Quarter-sweep labels, in thousands of RPM.
    pub labels: Option<[String; 5]>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct View {
    pub density: Density,
    pub speed: ReadoutView,
    pub unit: &'static str,
    pub gear: ReadoutView,
    /// Regular and full only.
    pub rpm: Option<ReadoutView>,
    pub strip: Option<Strip>,
    pub dial: Option<Dial>,
    pub message: Option<&'static str>,
}

pub fn gear_text(gear: i8) -> String {
    match gear {
        g if g < 0 => "R".into(),
        0 => "N".into(),
        g => g.to_string(),
    }
}

/// Lit segments for an RPM against the maximum; none when the maximum is unknown.
pub fn lit_segments(rpm: u16, max_rpm: Option<u16>) -> u8 {
    max_rpm.map_or(0, |max| {
        ((f32::from(rpm) / f32::from(max)).clamp(0.0, 1.0) * f32::from(SEGMENTS)).round() as u8
    })
}

pub fn view(inputs: &Inputs) -> View {
    let density = DENSITIES.pick(inputs.width, inputs.height);
    let state = inputs.car.state();
    let car = inputs.car.value().copied();
    let rpm = car.map_or(0, |c| c.engine_rpm);
    let max_rpm = inputs.max_rpm.filter(|&m| m > 0);
    let shown = units::speed(car.map_or(0.0, |c| f64::from(c.speed)), inputs.units);
    let show_strip = density != Density::Compact;
    let dial_size = if density == Density::Full {
        (inputs.width - 32.0).min(inputs.height - 200.0).max(120.0)
    } else {
        (inputs.height - 24.0).min((inputs.width - 28.0) * 0.55)
    }
    .round();
    View {
        density,
        speed: readout_view(state, &format!("{}", shown.value.round())),
        unit: shown.unit,
        gear: readout_view(state, &car.map_or(String::new(), |c| gear_text(c.gear))),
        rpm: show_strip.then(|| readout_view(state, &grouped(f64::from(rpm)))),
        strip: show_strip.then(|| Strip {
            lit: if car.is_some() {
                lit_segments(rpm, max_rpm)
            } else {
                0
            },
            limiter: car.is_some()
                && max_rpm.is_some_and(|max| f32::from(rpm) >= f32::from(max) * LIMITER_AT),
            state: if car.is_some() && max_rpm.is_none() {
                ReadoutState::Absent
            } else {
                state
            },
        }),
        dial: (show_strip && dial_size >= DIAL_MIN).then(|| Dial {
            size: dial_size,
            fill: car
                .zip(max_rpm)
                .map(|(c, max)| (f32::from(c.engine_rpm) / f32::from(max)).clamp(0.0, 1.0)),
            stale: state == ReadoutState::Stale,
            labels: max_rpm.map(|max| {
                std::array::from_fn(|i| format!("{}", (f32::from(max) * i as f32 / 4000.0).round()))
            }),
        }),
        message: whole_tile_message([state], inputs.lifecycle, true),
    }
}

/// Strip zone colours: low, mid, high, so dial and strip agree.
fn zone_colour(segment: u8) -> Hsla {
    match segment {
        s if s < 8 => theme::personal_best(),
        s if s < 14 => theme::slower(),
        _ => theme::hot(),
    }
}

pub fn render(view: View) -> impl IntoElement {
    let big = div()
        .flex()
        .items_baseline()
        .gap_1()
        .child(widgets::readout(&view.speed, 44.0))
        .child(widgets::label(view.unit));
    let gear = div()
        .flex()
        .flex_col()
        .child(widgets::label("Gear"))
        .child(widgets::readout(&view.gear, 30.0));
    let mut body = div().flex().flex_col().gap_2().size_full();
    if view.density != Density::Compact {
        body = body.child(widgets::title("Speed / RPM"));
    }
    if let Some(strip) = &view.strip {
        body = body.child(render_strip(strip));
    }
    let mut row = div().flex().justify_between().items_end().child(gear);
    if let Some(rpm) = &view.rpm {
        row = row.child(
            div()
                .flex()
                .flex_col()
                .items_end()
                .child(widgets::label("Engine speed"))
                .child(
                    div()
                        .flex()
                        .items_baseline()
                        .gap_1()
                        .child(widgets::readout(rpm, 22.0))
                        .child(widgets::label("RPM")),
                ),
        );
    }
    let main = match view.dial.clone() {
        Some(dial) => {
            let size = dial.size;
            let dial_box = div()
                .relative()
                .w(px(size))
                .h(px(size))
                .flex_none()
                .child(render_dial(dial))
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(big),
                );
            if view.density == Density::Full {
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_2()
                    .child(dial_box)
                    .child(div().w_full().child(row))
            } else {
                div()
                    .flex()
                    .gap_3()
                    .items_center()
                    .child(dial_box)
                    .child(div().flex_1().child(row))
            }
        }
        None => div().flex().flex_col().gap_2().child(big).child(row),
    };
    body = body.child(main);
    widgets::tile_shell(body, view.message)
}

fn render_strip(strip: &Strip) -> impl IntoElement + use<> {
    let mut row = div().flex().gap(px(2.0)).h(px(10.0)).w_full();
    for i in 0..SEGMENTS {
        let on = i < strip.lit;
        let colour = if !on {
            theme::meter_empty()
        } else if strip.state == ReadoutState::Stale {
            theme::muted()
        } else if strip.limiter {
            theme::hot()
        } else {
            zone_colour(i)
        };
        row = row.child(div().flex_1().h_full().rounded(px(1.0)).bg(colour));
    }
    row
}

/// The RPM dial: a 270° track with ticks outside it and a fill to the share of the
/// maximum, coloured by the strip's zones. Steady, never animated.
fn render_dial(dial: Dial) -> impl IntoElement {
    let labels = dial.labels.clone();
    let size = dial.size;
    let paint = canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            let size = dial.size;
            let origin = bounds.origin;
            let c = size / 2.0;
            let band = (size * 0.055).round().max(6.0);
            let radius = c - 24.0 - band / 2.0;
            let start = std::f32::consts::PI * 0.75;
            let sweep = std::f32::consts::PI * 1.5;
            let at = |angle: f32, r: f32| {
                point(
                    origin.x + px(c + angle.cos() * r),
                    origin.y + px(c + angle.sin() * r),
                )
            };
            let outer = radius + band / 2.0 + 4.0;
            for i in 0..=40 {
                let angle = start + sweep * i as f32 / 40.0;
                let major = i % 10 == 0;
                let mut tick = PathBuilder::stroke(px(if major { 2.0 } else { 1.0 }));
                tick.move_to(at(angle, outer));
                tick.line_to(at(angle, outer + if major { 7.0 } else { 3.0 }));
                if let Ok(path) = tick.build() {
                    window.paint_path(
                        path,
                        if major {
                            theme::muted()
                        } else {
                            theme::meter_empty()
                        },
                    );
                }
            }
            let arc = |from: f32, to: f32| {
                let steps = ((to - from).abs() / sweep * 96.0).ceil().max(1.0) as usize;
                let mut path = PathBuilder::stroke(px(band));
                path.move_to(at(from, radius));
                for step in 1..=steps {
                    path.line_to(at(from + (to - from) * step as f32 / steps as f32, radius));
                }
                path.build().ok()
            };
            if let Some(track) = arc(start, start + sweep) {
                window.paint_path(track, theme::meter_empty());
            }
            let Some(fill) = dial.fill else { return };
            if dial.stale {
                if let Some(path) = arc(start, start + sweep * fill) {
                    window.paint_path(path, theme::muted().opacity(0.6));
                }
                return;
            }
            // Zone colours by share of the sweep, matching the strip.
            let stops = [
                (0.0, 8.0 / 20.0, theme::personal_best()),
                (8.0 / 20.0, 14.0 / 20.0, theme::slower()),
                (14.0 / 20.0, 1.0, theme::hot()),
            ];
            for (from, to, colour) in stops {
                if fill <= from {
                    break;
                }
                if let Some(path) = arc(start + sweep * from, start + sweep * fill.min(to)) {
                    window.paint_path(path, colour);
                }
            }
        },
    )
    .size_full();
    let mut result = div().relative().size_full().child(paint);
    if let Some(labels) = labels {
        let center = size / 2.0;
        let radius = center - 6.0;
        for (i, label) in labels.into_iter().enumerate() {
            let angle = std::f32::consts::PI * (0.75 + 1.5 * i as f32 / 4.0);
            result = result.child(
                div()
                    .absolute()
                    .left(px(center + angle.cos() * radius - 12.0))
                    .top(px(center + angle.sin() * radius - 7.0))
                    .w(px(24.0))
                    .h(px(14.0))
                    .text_center()
                    .text_size(px(11.0))
                    .line_height(px(14.0))
                    .text_color(theme::muted())
                    .child(label),
            );
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use pitwall_core::{readout::ABSENT_TEXT, units::SpeedUnit};

    fn inputs(speed: u16, rpm: u16, gear: i8) -> CarInputs {
        CarInputs {
            speed,
            throttle: 0.0,
            brake: 0.0,
            steer: 0.0,
            clutch: 0,
            gear,
            engine_rpm: rpm,
            drs: false,
            rev_lights_percent: 0,
        }
    }

    fn base(car: Readout<CarInputs>) -> Inputs {
        Inputs {
            car,
            max_rpm: Some(12000),
            units: Units::DEFAULT,
            lifecycle: Lifecycle::Live,
            width: 400.0,
            height: 400.0,
        }
    }

    #[test]
    fn dial_labels_show_quarters_of_maximum_rpm_in_rounded_thousands() {
        let v = view(&Inputs {
            max_rpm: Some(13000),
            ..base(Readout::Live(inputs(200, 6500, 4)))
        });
        assert_eq!(v.dial.unwrap().labels.unwrap(), ["0", "3", "7", "10", "13"]);
        let missing = view(&Inputs {
            max_rpm: None,
            ..base(Readout::Absent)
        });
        assert!(missing.dial.unwrap().labels.is_none());
    }

    #[test]
    fn source_to_tile_rejects_fast_values_from_another_timeline() {
        use pitwall_core::{feed, model::SessionModel};
        let (_publisher, reader) = feed::channel();
        let mut model = SessionModel::default();
        let output = std::process::Command::new("python")
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tools/telemetry/assumed_datagram.py"
            ))
            .args([
                "2026",
                "2",
                "--set",
                "header.session_uid=101",
                "--set",
                "lap_data.0.result_status=2",
            ])
            .output()
            .unwrap();
        assert!(output.status.success());
        model.apply(
            std::time::Duration::ZERO,
            &pitwall_core::telemetry::parse_datagram(&output.stdout).unwrap(),
        );
        let mut frame = reader.fast();
        frame.cars[0] = Readout::Live(inputs(287, 11800, 7));
        frame.epoch = model.epoch().wrapping_add(1);
        let v = view(&Inputs::from_source(
            &frame,
            &model,
            Some(0),
            Units::DEFAULT,
            (400.0, 400.0),
        ));
        assert_eq!(v.speed.state, ReadoutState::Absent);
        assert_eq!(v.gear.state, ReadoutState::Absent);
        assert!(v.dial.unwrap().fill.is_none());
        frame.epoch = model.epoch();
        let matched = view(&Inputs::from_source(
            &frame,
            &model,
            Some(0),
            Units {
                speed: SpeedUnit::Mph,
                ..Units::DEFAULT
            },
            (400.0, 400.0),
        ));
        assert_eq!(matched.speed.text, "178");
        assert_eq!(matched.gear.text, "7");
        let unbound = view(&Inputs::from_source(
            &frame,
            &model,
            None,
            Units::DEFAULT,
            (400.0, 400.0),
        ));
        assert_eq!(unbound.speed.state, ReadoutState::Absent);
    }

    #[test]
    fn draws_speed_rpm_and_gear_of_the_car_in_the_devices_units() {
        let v = view(&base(Readout::Live(inputs(287, 11800, 7))));
        assert_eq!(v.speed.text, "287");
        assert_eq!(v.unit, "km/h");
        assert_eq!(v.gear.text, "7");
        assert_eq!(v.rpm.unwrap().text, "11,800");
        let mph = view(&Inputs {
            units: Units {
                speed: SpeedUnit::Mph,
                ..Units::DEFAULT
            },
            ..base(Readout::Live(inputs(287, 11800, 7)))
        });
        assert_eq!((mph.speed.text.as_str(), mph.unit), ("178", "mph"));
    }

    #[test]
    fn gear_reads_r_n_and_1_to_8() {
        assert_eq!(gear_text(-1), "R");
        assert_eq!(gear_text(0), "N");
        assert_eq!(gear_text(1), "1");
        assert_eq!(gear_text(8), "8");
    }

    #[test]
    fn the_four_readout_states_value_dimmed_value_label_em_dash() {
        let stale = view(&base(Readout::Stale(inputs(100, 5000, 3))));
        assert_eq!(
            (stale.speed.state, stale.speed.text.as_str()),
            (ReadoutState::Stale, "100")
        );
        let restricted = view(&base(Readout::Restricted));
        assert_eq!(restricted.speed.text, "Restricted");
        let absent = view(&base(Readout::Absent));
        assert_eq!(absent.speed.text, ABSENT_TEXT);
        assert_eq!(absent.gear.text, ABSENT_TEXT);
    }

    #[test]
    fn with_no_data_one_whole_tile_message_says_why() {
        assert_eq!(view(&base(Readout::Absent)).message, Some("No data"));
        let waiting = view(&Inputs {
            lifecycle: Lifecycle::Waiting,
            ..base(Readout::Absent)
        });
        assert_eq!(waiting.message, Some("Waiting for the game"));
        assert_eq!(view(&base(Readout::Stale(inputs(1, 1, 1)))).message, None);
    }

    #[test]
    fn the_shift_strip_fills_with_rpm_against_the_maximum() {
        assert_eq!(lit_segments(6000, Some(12000)), 10);
        assert_eq!(lit_segments(0, Some(12000)), 0);
        assert_eq!(lit_segments(13000, Some(12000)), SEGMENTS);
    }

    #[test]
    fn at_the_limiter_the_strip_is_full_and_steady() {
        let strip = view(&base(Readout::Live(inputs(300, 11900, 8))))
            .strip
            .unwrap();
        assert!(strip.limiter);
        assert_eq!(strip.lit, SEGMENTS);
        let below = view(&base(Readout::Live(inputs(300, 11000, 8))))
            .strip
            .unwrap();
        assert!(!below.limiter);
    }

    #[test]
    fn the_strip_lights_nothing_when_maximum_rpm_is_unknown() {
        let strip = view(&Inputs {
            max_rpm: None,
            ..base(Readout::Live(inputs(300, 11900, 8)))
        })
        .strip
        .unwrap();
        assert_eq!(strip.lit, 0);
        assert!(!strip.limiter);
        assert_eq!(strip.state, ReadoutState::Absent);
    }

    #[test]
    fn compact_keeps_speed_and_gear_only_full_adds_the_dial() {
        let compact = view(&Inputs {
            width: 200.0,
            height: 120.0,
            ..base(Readout::Live(inputs(1, 1, 1)))
        });
        assert_eq!(compact.density, Density::Compact);
        assert!(compact.rpm.is_none() && compact.strip.is_none() && compact.dial.is_none());
        let full = view(&base(Readout::Live(inputs(1, 6000, 1))));
        assert_eq!(full.density, Density::Full);
        assert_eq!(full.dial.unwrap().fill, Some(0.5));
    }
}
