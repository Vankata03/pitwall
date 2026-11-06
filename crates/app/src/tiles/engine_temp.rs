//! Engine temp (parity ENGINE-01): a slow tile following one car.
//! Port of `dashboard/src/tiles/types/engine-temp.tsx`.

use gpui::{IntoElement, ParentElement, Styled, canvas, div, px};
use pitwall_core::{
    model::{Lifecycle, Readout, SessionModel},
    readout::{ReadoutState, view_of},
    units::{self, Units},
};

use super::{
    Densities, Density, paint,
    slow::{self, Row},
    widgets,
};
use crate::theme;

pub const TYPE: &str = "engine-temp";
pub const NAME: &str = "Engine temp";
pub const DENSITIES: Densities = Densities {
    regular: (140.0, 64.0),
    full: None,
};
/// The gauge's scale in °C. Decoration only: the number in its centre is the readout.
pub const SCALE_MIN_C: f32 = 40.0;
pub const SCALE_MAX_C: f32 = 140.0;
/// Smallest tile that draws the gauge rather than the number alone.
pub const GAUGE_MIN: (f32, f32) = (150.0, 130.0);

pub struct Inputs {
    /// Engine temperature in °C.
    pub celsius: Readout<u16>,
    pub units: Units,
    pub lifecycle: Lifecycle,
    pub width: f32,
    pub height: f32,
}

impl Inputs {
    pub fn from_model(
        model: &SessionModel,
        index: Option<usize>,
        units: Units,
        size: (f32, f32),
    ) -> Self {
        Self {
            celsius: index
                .and_then(|i| model.car(i))
                .map_or(Readout::Absent, |car| {
                    car.telemetry().map(|t| t.engine_temperature)
                }),
            units,
            lifecycle: model.lifecycle(),
            width: size.0,
            height: size.1,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Gauge {
    /// Share of the decorative scale, clamped; `None` draws the empty track only.
    pub fill: Option<f32>,
    pub stale: bool,
    /// The scale's ends in the device's unit.
    pub labels: [String; 2],
}

#[derive(Debug, Clone, PartialEq)]
pub struct View {
    pub density: Density,
    pub row: Row,
    pub gauge: Option<Gauge>,
    pub message: Option<&'static str>,
    pub size: (f32, f32),
}

pub fn view(inputs: &Inputs) -> View {
    let row = Row::new(
        "Engine temperature",
        view_of(&inputs.celsius, |c| {
            let q = units::temperature(f64::from(*c), inputs.units);
            format!("{} {}", q.value.round(), q.unit)
        }),
    );
    let label = |c: f32| {
        format!(
            "{}",
            units::temperature(f64::from(c), inputs.units).value.round()
        )
    };
    let show_gauge = inputs.width >= GAUGE_MIN.0 && inputs.height >= GAUGE_MIN.1;
    View {
        density: DENSITIES.pick(inputs.width, inputs.height),
        gauge: show_gauge.then(|| Gauge {
            fill: inputs.celsius.value().map(|&c| {
                ((f32::from(c) - SCALE_MIN_C) / (SCALE_MAX_C - SCALE_MIN_C)).clamp(0.0, 1.0)
            }),
            stale: inputs.celsius.state() == ReadoutState::Stale,
            labels: [label(SCALE_MIN_C), label(SCALE_MAX_C)],
        }),
        message: slow::message(std::slice::from_ref(&row), inputs.lifecycle),
        row,
        size: (inputs.width, inputs.height),
    }
}

pub fn render(view: View) -> impl IntoElement {
    let Some(gauge) = view.gauge.clone() else {
        return slow::render(std::slice::from_ref(&view.row), view.message, view.size);
    };
    let (w, h) = view.size;
    let numerals = (w * 0.13).min(h * 0.15).clamp(24.0, 48.0);
    let body = div()
        .relative()
        .size_full()
        .child(render_gauge(gauge.clone()))
        .child(
            div()
                .absolute()
                .top_0()
                .right_0()
                .child(widgets::label(view.row.label)),
        )
        .child(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .child(widgets::readout(&view.row.value, numerals)),
        );
    widgets::tile_shell(body, view.message)
}

/// A 270° arc filled to the temperature on the fixed decorative scale, with 21 ticks and
/// the scale's ends under the arc's open ends. It never implies a limit.
fn render_gauge(gauge: Gauge) -> impl IntoElement {
    let [low, high] = gauge.labels.clone();
    let paint = canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
            // The web gauge's 200×180 view box, scaled to fit.
            let k = (w / 200.0).min(h / 180.0);
            let (cx, cy) = (w / 2.0, 96.0 * k + (h - 180.0 * k) / 2.0);
            let r = 74.0 * k;
            let start = 135f32.to_radians();
            let sweep = 270f32.to_radians();
            for i in 0..=20 {
                let major = i % 5 == 0;
                let a = start + sweep * i as f32 / 20.0;
                let (inner, outer) = (r + 9.0 * k, r + if major { 16.0 } else { 12.0 } * k);
                paint::polyline(
                    window,
                    bounds,
                    &[
                        (cx + a.cos() * inner, cy + a.sin() * inner),
                        (cx + a.cos() * outer, cy + a.sin() * outer),
                    ],
                    if major { 2.0 } else { 1.5 },
                    if major {
                        theme::muted()
                    } else {
                        theme::meter_empty()
                    },
                );
            }
            let width = 9.0 * k;
            paint::arc(
                window,
                bounds,
                (cx, cy),
                r,
                start,
                start + sweep,
                width,
                theme::meter_empty(),
            );
            let Some(fill) = gauge.fill else { return };
            if gauge.stale {
                paint::arc(
                    window,
                    bounds,
                    (cx, cy),
                    r,
                    start,
                    start + sweep * fill,
                    width,
                    theme::muted(),
                );
                return;
            }
            // The web gradient: cold, energy, slower, hot across the arc.
            let stops = [
                (0.0, 0.45, theme::cold()),
                (0.45, 0.7, theme::energy()),
                (0.7, 0.85, theme::slower()),
                (0.85, 1.0, theme::hot()),
            ];
            for (from, to, colour) in stops {
                if fill <= from {
                    break;
                }
                paint::arc(
                    window,
                    bounds,
                    (cx, cy),
                    r,
                    start + sweep * from,
                    start + sweep * fill.min(to),
                    width,
                    colour,
                );
            }
        },
    )
    .size_full();
    div().relative().size_full().child(paint).child(
        div()
            .absolute()
            .bottom_0()
            .left_0()
            .right_0()
            .flex()
            .justify_between()
            .px(px(28.0))
            .child(widgets::label(low))
            .child(widgets::label(high)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use pitwall_core::{readout::ABSENT_TEXT, units::TemperatureUnit};

    fn base(celsius: Readout<u16>) -> Inputs {
        Inputs {
            celsius,
            units: Units::DEFAULT,
            lifecycle: Lifecycle::Live,
            width: 320.0,
            height: 160.0,
        }
    }
    const FAHRENHEIT: Units = Units {
        temperature: TemperatureUnit::Fahrenheit,
        ..Units::DEFAULT
    };

    #[test]
    fn declares_the_catalogue_entry() {
        assert_eq!((TYPE, NAME), ("engine-temp", "Engine temp"));
        assert_eq!(DENSITIES.pick(139.0, 64.0), Density::Compact);
        assert_eq!(DENSITIES.pick(140.0, 64.0), Density::Regular);
    }

    #[test]
    fn follows_device_temperature_units_rounded() {
        let v = view(&Inputs {
            units: FAHRENHEIT,
            ..base(Readout::Live(110))
        });
        assert_eq!(v.row.label, "Engine temperature");
        assert_eq!(v.row.value.text, "230 °F");
        assert_eq!(view(&base(Readout::Live(105))).row.value.text, "105 °C");
    }

    #[test]
    fn draws_the_gauge_only_from_150_by_130_on_a_decorative_scale() {
        let small = view(&Inputs {
            width: 149.0,
            ..base(Readout::Live(90))
        });
        assert!(small.gauge.is_none());
        let short = view(&Inputs {
            height: 129.0,
            ..base(Readout::Live(90))
        });
        assert!(short.gauge.is_none());
        let gauge = view(&base(Readout::Live(90))).gauge.unwrap();
        assert_eq!(gauge.fill, Some(0.5));
        assert_eq!(gauge.labels, ["40".to_string(), "140".to_string()]);
        let hot = view(&base(Readout::Live(200))).gauge.unwrap();
        assert_eq!(hot.fill, Some(1.0));
        let f = view(&Inputs {
            units: FAHRENHEIT,
            ..base(Readout::Live(90))
        })
        .gauge
        .unwrap();
        assert_eq!(f.labels, ["104".to_string(), "284".to_string()]);
    }

    #[test]
    fn a_stale_reading_keeps_its_fill_and_no_value_draws_none() {
        let stale = view(&base(Readout::Stale(90))).gauge.unwrap();
        assert!(stale.stale);
        assert_eq!(stale.fill, Some(0.5));
        assert_eq!(view(&base(Readout::Absent)).gauge.unwrap().fill, None);
        assert_eq!(view(&base(Readout::Restricted)).gauge.unwrap().fill, None);
    }

    #[test]
    fn keeps_stale_restricted_and_absent_explicit_and_never_substitutes_zero() {
        let stale = view(&base(Readout::Stale(100)));
        assert_eq!(
            (stale.row.value.state, stale.row.value.text.as_str()),
            (ReadoutState::Stale, "100 °C")
        );
        assert_eq!(stale.message, None);
        let restricted = view(&base(Readout::Restricted));
        assert_eq!(restricted.row.value.text, "Restricted");
        assert_eq!(restricted.message, Some("No data"));
        let absent = view(&base(Readout::Absent));
        assert_eq!(absent.row.value.text, ABSENT_TEXT);
        assert_eq!(absent.message, Some("No data"));
        let waiting = view(&Inputs {
            lifecycle: Lifecycle::Waiting,
            ..base(Readout::Absent)
        });
        assert_eq!(waiting.message, Some("Waiting for the game"));
    }
}
