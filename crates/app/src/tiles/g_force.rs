//! G-force (parity GFORCE-01): a frame-rate tile following one car.
//! Port of `dashboard/src/tiles/types/g-force.tsx`.

use gpui::{IntoElement, ParentElement, Styled, canvas, div, px};
use pitwall_core::{
    model::{Lifecycle, Readout, SessionModel},
    readout::{ReadoutState, ReadoutView, readout_view, whole_tile_message},
};

use super::{Densities, Density, format::signed, paint, widgets};
use crate::theme;

pub const TYPE: &str = pitwall_core::tile_catalog::G_FORCE.kind;

pub const DENSITIES: Densities = Densities {
    regular: (170.0, 120.0),
    full: None,
};
/// The circle spans this many G from the centre to the rim.
pub const RANGE_G: f32 = 4.0;
const DOT_RADIUS: f32 = 5.0;

pub struct Inputs {
    /// Lateral and longitudinal G of the bound car; `Absent` when no car is bound.
    pub g: Readout<(f32, f32)>,
    pub lifecycle: Lifecycle,
    pub width: f32,
    pub height: f32,
}

impl Inputs {
    pub fn from_model(model: &SessionModel, index: Option<usize>, size: (f32, f32)) -> Self {
        Self {
            g: index
                .and_then(|i| model.car(i))
                .map_or(Readout::Absent, |car| {
                    car.motion()
                        .map(|m| (m.g_force_lateral, m.g_force_longitudinal))
                }),
            lifecycle: model.lifecycle(),
            width: size.0,
            height: size.1,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Circle {
    pub size: f32,
    /// The car's dot as a share of the radius, lateral right and acceleration up,
    /// stopped at the rim; `None` without a value.
    pub dot: Option<(f32, f32)>,
    pub stale: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct View {
    pub density: Density,
    pub lateral: ReadoutView,
    pub longitudinal: ReadoutView,
    pub total: ReadoutView,
    /// Regular only.
    pub circle: Option<Circle>,
    pub message: Option<&'static str>,
}

pub fn view(inputs: &Inputs) -> View {
    let density = DENSITIES.pick(inputs.width, inputs.height);
    let state = inputs.g.state();
    let (lat, lon) = inputs.g.value().copied().unwrap_or((0.0, 0.0));
    let size = (inputs.height - 24.0)
        .min((inputs.width * 0.62).round())
        .max(80.0);
    View {
        density,
        lateral: readout_view(state, &signed(f64::from(lat), 1)),
        longitudinal: readout_view(state, &signed(f64::from(lon), 1)),
        total: readout_view(
            state,
            &format!("{:.1}", f64::from(lat).hypot(f64::from(lon))),
        ),
        circle: (density != Density::Compact).then(|| Circle {
            size,
            dot: inputs.g.value().map(|_| {
                // Stop at the rim: the number beside the circle still tells the truth.
                let reach = lat.hypot(lon) / RANGE_G;
                let scale = if reach > 1.0 { 1.0 / reach } else { 1.0 };
                (lat / RANGE_G * scale, lon / RANGE_G * scale)
            }),
            stale: state == ReadoutState::Stale,
        }),
        message: whole_tile_message([state], inputs.lifecycle, true),
    }
}

pub fn render(view: View) -> impl IntoElement {
    let total = div()
        .flex()
        .flex_col()
        .child(widgets::label("G-force"))
        .child(
            div()
                .flex()
                .items_baseline()
                .gap_1()
                .child(widgets::readout(&view.total, 24.0))
                .child(widgets::label("G")),
        );
    let axis = |name: &'static str, value: &ReadoutView| {
        div()
            .flex()
            .items_baseline()
            .gap_2()
            .child(widgets::label(name))
            .child(widgets::readout(value, 16.0))
    };
    let numbers = div()
        .flex()
        .flex_col()
        .gap_2()
        .child(total)
        .child(axis("Lat", &view.lateral))
        .child(axis("Lon", &view.longitudinal));
    let mut body = div().flex().gap_3().items_center().size_full();
    if let Some(circle) = view.circle.clone() {
        body = body.child(
            div()
                .w(px(circle.size))
                .h(px(circle.size))
                .flex_none()
                .child(render_circle(circle)),
        );
    }
    widgets::tile_shell(body.child(numbers), view.message)
}

/// A polar plot: a disc with rings at each whole G and spokes every 45°, and a dot for the
/// car joined to the centre by a vector line.
fn render_circle(circle: Circle) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            let size = circle.size;
            let c = size / 2.0;
            let radius = c - DOT_RADIUS - 2.0;
            paint::disc(window, bounds, (c, c), radius, theme::raised());
            for ring in 1..=RANGE_G as usize {
                let r = radius * ring as f32 / RANGE_G;
                paint::arc(
                    window,
                    bounds,
                    (c, c),
                    r,
                    0.0,
                    std::f32::consts::TAU,
                    1.0,
                    theme::line(),
                );
            }
            for spoke in 0..4 {
                let a = spoke as f32 * std::f32::consts::FRAC_PI_4;
                let (dx, dy) = (a.cos() * radius, a.sin() * radius);
                paint::polyline(
                    window,
                    bounds,
                    &[(c - dx, c - dy), (c + dx, c + dy)],
                    1.0,
                    theme::line(),
                );
            }
            let Some((x, y)) = circle.dot else { return };
            let (x, y) = (c + x * radius, c - y * radius);
            let ink = if circle.stale {
                theme::muted()
            } else {
                theme::brake()
            };
            paint::polyline(window, bounds, &[(c, c), (x, y)], 2.0, ink);
            let halo = if circle.stale { 0.25 } else { 0.3 };
            paint::disc(window, bounds, (x, y), DOT_RADIUS * 2.2, ink.opacity(halo));
            let dot = if circle.stale { 0.6 } else { 1.0 };
            paint::disc(window, bounds, (x, y), DOT_RADIUS, ink.opacity(dot));
        },
    )
    .size_full()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pitwall_core::readout::ABSENT_TEXT;

    fn base(g: Readout<(f32, f32)>) -> Inputs {
        Inputs {
            g,
            lifecycle: Lifecycle::Live,
            width: 200.0,
            height: 200.0,
        }
    }

    #[test]
    fn declares_the_catalogue_entry() {
        assert_eq!(
            (TYPE, pitwall_core::tile_catalog::G_FORCE.name),
            ("g-force", "G-force")
        );
        assert_eq!(DENSITIES.pick(169.0, 200.0), Density::Compact);
        assert_eq!(DENSITIES.pick(170.0, 120.0), Density::Regular);
        assert_eq!(DENSITIES.pick(2000.0, 2000.0), Density::Regular);
    }

    #[test]
    fn shows_lateral_and_longitudinal_g_signed() {
        let v = view(&base(Readout::Live((1.83, -2.14))));
        assert_eq!(v.lateral.text, "+1.8");
        assert_eq!(v.longitudinal.text, "\u{2212}2.1");
    }

    #[test]
    fn zero_carries_no_sign_and_large_values_are_never_clamped() {
        let v = view(&base(Readout::Live((0.0, 10.4))));
        assert_eq!(v.lateral.text, "+0.0");
        assert_eq!(v.longitudinal.text, "+10.4");
    }

    #[test]
    fn the_four_readout_states() {
        let live = view(&base(Readout::Live((1.5, 0.5))));
        assert_eq!(
            (live.lateral.state, live.lateral.text.as_str()),
            (ReadoutState::Live, "+1.5")
        );
        let stale = view(&base(Readout::Stale((1.5, 0.5))));
        assert_eq!(
            (stale.lateral.state, stale.lateral.text.as_str()),
            (ReadoutState::Stale, "+1.5")
        );
        assert_eq!(stale.longitudinal.state, ReadoutState::Stale);
        let restricted = view(&base(Readout::Restricted));
        assert_eq!(restricted.lateral.text, "Restricted");
        let absent = view(&base(Readout::Absent));
        assert_eq!(absent.lateral.text, ABSENT_TEXT);
        assert_eq!(absent.longitudinal.text, ABSENT_TEXT);
    }

    #[test]
    fn with_no_data_the_whole_tile_says_so_and_keeps_its_shape() {
        let v = view(&base(Readout::Absent));
        assert_eq!(v.message, Some("No data"));
        assert!(v.circle.is_some());
        assert_eq!(view(&base(Readout::Stale((1.0, 1.0)))).message, None);
    }

    #[test]
    fn compact_keeps_the_numbers_regular_adds_the_circle_and_total() {
        let compact = view(&Inputs {
            width: 150.0,
            height: 70.0,
            ..base(Readout::Live((3.0, 4.0)))
        });
        assert!(compact.circle.is_none());
        assert_eq!(compact.lateral.text, "+3.0");
        let regular = view(&base(Readout::Live((3.0, 4.0))));
        assert!(regular.circle.is_some());
        assert_eq!(regular.total.text, "5.0");
    }

    #[test]
    fn the_dot_sits_where_the_g_puts_it_stops_at_the_rim_and_is_absent_without_value() {
        let dot = |g| view(&base(Readout::Live(g))).circle.unwrap().dot.unwrap();
        let right = dot((2.0, 0.0));
        let left = dot((-2.0, 0.0));
        assert!(right.0 > left.0);
        assert_eq!(right.1, left.1);
        assert_eq!(dot((0.0, 2.0)).1, 0.5, "acceleration is up");
        let rim = dot((10.0, 0.0));
        assert_eq!(rim, (1.0, 0.0));
        let none = view(&base(Readout::Absent)).circle.unwrap();
        assert!(none.dot.is_none());
        let stale = view(&base(Readout::Stale((1.0, 0.0)))).circle.unwrap();
        assert!(stale.stale && stale.dot.is_some());
    }
}
