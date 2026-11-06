//! Weather (parity WEATHER-01): a session-wide slow tile.
//! Port of `dashboard/src/tiles/types/weather.tsx` and its rules in `slow.css`.

use gpui::{Div, Hsla, IntoElement, ParentElement, Styled, canvas, div, px, relative};
use pitwall_core::{
    model::{Lifecycle, Readout, SessionModel},
    readout::{ReadoutState, view_of},
    telemetry::WeatherForecastSample,
    units::{self, Units},
};

use super::{
    Densities, Density, paint,
    slow::{self, Row},
    widgets,
};
use crate::theme;

pub const TYPE: &str = "weather";
pub const NAME: &str = "Weather";
pub const DENSITIES: Densities = Densities {
    regular: (280.0, 104.0),
    full: None,
};

const SKIES: [&str; 6] = [
    "Clear",
    "Light cloud",
    "Overcast",
    "Light rain",
    "Heavy rain",
    "Storm",
];

/// The game's sky name for a weather code; a code it does not know is named, not guessed.
pub fn sky(weather: u8) -> String {
    SKIES
        .get(usize::from(weather))
        .map_or_else(|| format!("Unknown sky ({weather})"), |s| (*s).to_owned())
}

/// The session facts the tile reads.
#[derive(Debug, Clone, PartialEq)]
pub struct Session {
    pub session_type: u8,
    pub weather: u8,
    pub air: i8,
    pub track: i8,
    pub forecast: Vec<WeatherForecastSample>,
}

pub struct Inputs {
    pub session: Readout<Session>,
    pub units: Units,
    pub lifecycle: Lifecycle,
    pub width: f32,
    pub height: f32,
}

impl Inputs {
    pub fn from_model(model: &SessionModel, units: Units, size: (f32, f32)) -> Self {
        let forecast = model.forecast().map(<[_]>::to_vec);
        Self {
            session: model.session().map(|s| Session {
                session_type: s.session_type,
                weather: s.weather,
                air: s.air_temperature,
                track: s.track_temperature,
                forecast: forecast.value().cloned().unwrap_or_default(),
            }),
            units,
            lifecycle: model.lifecycle(),
            width: size.0,
            height: size.1,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct View {
    pub density: Density,
    /// Air, Track, Rain chance, Sky, Forecast, then (regular) Forecast air / track.
    pub rows: Vec<Row>,
    /// The current sky code for the icon, while the session has a value.
    pub icon: Option<u8>,
    /// The forecast line and the sky icon fit (the web build hides them on a tile 400 px
    /// wide or narrower, and the forecast line on one 90 px tall or shorter).
    pub show_icon: bool,
    pub show_forecast: bool,
    pub message: Option<&'static str>,
    pub size: (f32, f32),
}

/// `Math.round`: halves go up.
fn round(v: f64) -> f64 {
    (v + 0.5).floor()
}

pub fn view(inputs: &Inputs) -> View {
    let density = DENSITIES.pick(inputs.width, inputs.height);
    let temp = |c: i8| {
        let q = units::temperature(f64::from(c), inputs.units);
        format!("{} {}", round(q.value), q.unit)
    };
    // The earliest sample for the current session type only.
    let forecast: Readout<WeatherForecastSample> = match &inputs.session {
        Readout::Live(s) | Readout::Stale(s) => s
            .forecast
            .iter()
            .filter(|f| f.session_type == s.session_type)
            .min_by_key(|f| f.time_offset)
            .map_or(Readout::Absent, |f| {
                if matches!(inputs.session, Readout::Live(_)) {
                    Readout::Live(f.clone())
                } else {
                    Readout::Stale(f.clone())
                }
            }),
        Readout::Restricted => Readout::Restricted,
        Readout::Absent => Readout::Absent,
    };
    let s = &inputs.session;
    let mut rows = vec![
        Row::new("Air", view_of(s, |s| temp(s.air))),
        Row::new("Track", view_of(s, |s| temp(s.track))),
        Row::new(
            "Rain chance",
            view_of(&forecast, |f| format!("{}%", f.rain_percentage)),
        ),
        Row::new("Sky", view_of(s, |s| sky(s.weather))),
        Row::new(
            "Forecast",
            view_of(&forecast, |f| {
                if density == Density::Compact {
                    format!("{} +{}m", sky(f.weather), f.time_offset)
                } else {
                    format!("+{} min: {}", f.time_offset, sky(f.weather))
                }
            }),
        ),
    ];
    if density != Density::Compact {
        rows.push(Row::new(
            "Forecast air / track",
            view_of(&forecast, |f| {
                format!(
                    "{} / {}",
                    temp(f.air_temperature),
                    temp(f.track_temperature)
                )
            }),
        ));
    }
    View {
        density,
        message: slow::message(&rows, inputs.lifecycle),
        icon: s.value().map(|s| s.weather),
        show_icon: inputs.width > 400.0,
        show_forecast: inputs.width > 400.0 && inputs.height > 90.0,
        rows,
        size: (inputs.width, inputs.height),
    }
}

fn cell(r: &Row, size: f32, colour: Option<Hsla>) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .min_w_0()
        .overflow_hidden()
        .whitespace_nowrap()
        .child(widgets::label(r.label))
        .child(widgets::readout_coloured(
            &r.value,
            size,
            colour.filter(|_| r.value.state == ReadoutState::Live),
        ))
}

pub fn render(view: View) -> impl IntoElement {
    let (w, h) = view.size;
    let narrow = w <= 400.0;
    let numerals = if narrow {
        14.0
    } else {
        (w * 0.054).min(h * 0.40).clamp(18.0, 30.0)
    };
    let rain = if narrow { 20.0 } else { numerals };
    let sky_size = if narrow {
        14.0
    } else {
        (w * 0.04).clamp(14.0, 24.0)
    };
    let rows = &view.rows;
    let mut sky = cell(&rows[3], sky_size, Some(theme::hot()));
    if view.show_icon
        && let Some(code) = view.icon
    {
        let live = rows[3].value.state == ReadoutState::Live;
        sky = div()
            .flex()
            .flex_col()
            .gap_1()
            .min_w_0()
            .child(widgets::label(rows[3].label))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(sky_icon(
                        code,
                        if live { theme::hot() } else { theme::muted() },
                    ))
                    .child(widgets::readout_coloured(
                        &rows[3].value,
                        sky_size,
                        live.then(theme::hot),
                    )),
            );
    }
    let mut first = div().flex().items_center().gap(px(6.0)).w_full();
    for (i, (r, grow)) in rows.iter().zip([1.0, 1.0, 1.1]).enumerate() {
        let size = if i == 2 { rain } else { numerals };
        let colour = (i == 2).then(theme::cold);
        first = first.child(
            cell(r, size, colour)
                .flex_basis(relative(0.0))
                .flex_grow(grow),
        );
    }
    first = first.child(sky.flex_basis(relative(0.0)).flex_grow(1.4));
    let mut body = div()
        .flex()
        .flex_col()
        .justify_center()
        .gap(px(10.0))
        .size_full()
        .child(first);
    if view.show_forecast {
        let mut second = div().flex().gap(px(6.0)).w_full();
        for r in &rows[4..] {
            second = second.child(cell(r, 14.0, None).flex_basis(relative(0.0)).flex_grow(1.0));
        }
        body = body.child(second);
    }
    widgets::tile_shell(body, view.message)
}

/// Outline glyphs for the sky, in the value's colour. Decorative: the word says it.
fn sky_icon(code: u8, colour: Hsla) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            // A 24-unit viewBox drawn at 22 px.
            let k = 22.0 / 24.0;
            let p = |x: f32, y: f32| (x * k, y * k);
            let stroke = 1.6;
            let sun = |window: &mut gpui::Window, cx: f32, cy: f32, s: f32| {
                paint::arc(
                    window,
                    bounds,
                    p(cx, cy),
                    4.0 * s * k,
                    0.0,
                    std::f32::consts::TAU,
                    stroke,
                    colour,
                );
                for i in 0..8 {
                    let a = i as f32 * std::f32::consts::FRAC_PI_4;
                    let (c, sn) = (a.cos(), a.sin());
                    paint::polyline(
                        window,
                        bounds,
                        &[
                            p(cx + c * 6.5 * s, cy + sn * 6.5 * s),
                            p(cx + c * 8.5 * s, cy + sn * 8.5 * s),
                        ],
                        stroke,
                        colour,
                    );
                }
            };
            let cloud = |window: &mut gpui::Window, dx: f32, dy: f32, s: f32| {
                let q = |x: f32, y: f32| p(dx + x * s, dy + y * s);
                paint::arc(
                    window,
                    bounds,
                    q(8.0, 14.6),
                    3.4 * s * k,
                    1.6,
                    4.4,
                    stroke,
                    colour,
                );
                paint::arc(
                    window,
                    bounds,
                    q(12.3, 11.0),
                    5.0 * s * k,
                    3.6,
                    5.9,
                    stroke,
                    colour,
                );
                paint::arc(
                    window,
                    bounds,
                    q(16.6, 14.0),
                    3.6 * s * k,
                    4.9,
                    7.85,
                    stroke,
                    colour,
                );
                paint::polyline(
                    window,
                    bounds,
                    &[q(8.0, 18.0), q(16.6, 17.6)],
                    stroke,
                    colour,
                );
            };
            match code {
                0 => sun(window, 12.0, 12.0, 1.0),
                1 => {
                    sun(window, 6.0, 6.0, 0.75);
                    cloud(window, 3.0, 2.0, 0.85);
                }
                2 => cloud(window, 0.0, 0.0, 1.0),
                5 => {
                    cloud(window, 0.0, -3.0, 1.0);
                    paint::polyline(
                        window,
                        bounds,
                        &[p(12.0, 16.0), p(10.0, 20.0), p(13.0, 20.0), p(11.0, 23.0)],
                        stroke,
                        colour,
                    );
                }
                _ => {
                    cloud(window, 0.0, -3.0, 1.0);
                    let drops: &[(f32, f32)] = if code == 4 {
                        &[(8.0, 18.0), (12.0, 18.0), (16.0, 18.0)]
                    } else {
                        &[(9.0, 18.0), (14.0, 18.0)]
                    };
                    for &(x, y) in drops {
                        paint::polyline(
                            window,
                            bounds,
                            &[p(x, y), p(x - 1.0, y + 3.0)],
                            stroke,
                            colour,
                        );
                    }
                }
            }
        },
    )
    .w(px(22.0))
    .h(px(22.0))
    .flex_none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pitwall_core::{
        readout::ABSENT_TEXT,
        units::{SpeedUnit, TemperatureUnit},
    };

    fn sample(session_type: u8, time_offset: u8, weather: u8, rain: u8) -> WeatherForecastSample {
        WeatherForecastSample {
            session_type,
            time_offset,
            weather,
            track_temperature: 27,
            track_temperature_change: 0,
            air_temperature: 19,
            air_temperature_change: 0,
            rain_percentage: rain,
        }
    }
    fn session(forecast: Vec<WeatherForecastSample>) -> Session {
        Session {
            session_type: 10,
            weather: 2,
            air: 20,
            track: 30,
            forecast,
        }
    }
    fn base(session: Readout<Session>) -> Inputs {
        Inputs {
            session,
            units: Units::DEFAULT,
            lifecycle: Lifecycle::Live,
            width: 470.0,
            height: 160.0,
        }
    }
    fn text<'a>(v: &'a View, label: &str) -> &'a str {
        &v.rows.iter().find(|r| r.label == label).unwrap().value.text
    }

    #[test]
    fn declares_the_catalogue_entry() {
        assert_eq!((TYPE, NAME), ("weather", "Weather"));
        assert_eq!(DENSITIES.pick(279.0, 104.0), Density::Compact);
        assert_eq!(DENSITIES.pick(280.0, 104.0), Density::Regular);
    }

    #[test]
    fn shows_sky_temperatures_and_forecast_rain_with_its_horizon() {
        let v = view(&base(Readout::Live(session(vec![
            sample(11, 1, 5, 100),
            sample(10, 10, 3, 20),
            sample(10, 5, 4, 70),
        ]))));
        assert_eq!(text(&v, "Air"), "20 °C");
        assert_eq!(text(&v, "Track"), "30 °C");
        assert_eq!(text(&v, "Sky"), "Overcast");
        assert_eq!(text(&v, "Forecast"), "+5 min: Heavy rain");
        assert_eq!(text(&v, "Rain chance"), "70%");
        assert_eq!(text(&v, "Forecast air / track"), "19 °C / 27 °C");
    }

    #[test]
    fn temperatures_follow_the_device_units() {
        let v = view(&Inputs {
            units: Units {
                temperature: TemperatureUnit::Fahrenheit,
                speed: SpeedUnit::Kmh,
                ..Units::DEFAULT
            },
            ..base(Readout::Live(session(vec![])))
        });
        assert_eq!(text(&v, "Air"), "68 °F");
        assert_eq!(text(&v, "Track"), "86 °F");
    }

    #[test]
    fn never_invents_forecast_rain_when_no_matching_session_sample_exists() {
        let v = view(&base(Readout::Live(Session {
            weather: 0,
            ..session(vec![sample(11, 5, 5, 100)])
        })));
        assert_eq!(text(&v, "Rain chance"), ABSENT_TEXT);
        assert_eq!(text(&v, "Forecast"), ABSENT_TEXT);
        assert_eq!(v.message, None);
    }

    #[test]
    fn an_unknown_current_sky_names_its_number() {
        let v = view(&base(Readout::Live(Session {
            weather: 9,
            ..session(vec![])
        })));
        assert_eq!(text(&v, "Sky"), "Unknown sky (9)");
    }

    #[test]
    fn compact_shortens_the_horizon_and_omits_forecast_temperatures() {
        let v = view(&Inputs {
            width: 240.0,
            ..base(Readout::Live(session(vec![sample(10, 5, 4, 70)])))
        });
        assert_eq!(v.density, Density::Compact);
        assert_eq!(text(&v, "Forecast"), "Heavy rain +5m");
        assert_eq!(v.rows.len(), 5);
    }

    #[test]
    fn keeps_stale_restricted_and_absent_explicit() {
        let stale = view(&base(Readout::Stale(session(vec![sample(10, 5, 4, 70)]))));
        assert!(
            stale
                .rows
                .iter()
                .all(|r| r.value.state == ReadoutState::Stale)
        );
        assert_eq!(stale.message, None);
        let restricted = view(&base(Readout::Restricted));
        assert!(restricted.rows.iter().all(|r| r.value.text == "Restricted"));
        assert_eq!(restricted.message, Some("No data"));
        let absent = view(&base(Readout::Absent));
        assert!(absent.rows.iter().all(|r| r.value.text == ABSENT_TEXT));
        assert_eq!(absent.message, Some("No data"));
    }
}
