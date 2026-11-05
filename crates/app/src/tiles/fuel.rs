//! Fuel (parity FUEL-01): a slow tile following one car.
//! Port of `dashboard/src/tiles/types/fuel.tsx`.

use gpui::IntoElement;
use pitwall_core::{
    model::{Lifecycle, Readout, SessionModel},
    readout::view_of,
};

use super::{
    Densities, Density,
    format::{MINUS, fixed},
    slow::{self, Row, Tone},
};

pub const TYPE: &str = "fuel";
pub const NAME: &str = "Fuel";
pub const DENSITIES: Densities = Densities {
    regular: (240.0, 64.0),
    full: None,
};

/// The two fuel facts, as the game sends them: kilograms in the tank and the laps of
/// fuel left over at the finish (negative: short).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fuel {
    pub in_tank: f32,
    pub remaining_laps: f32,
}

pub struct Inputs {
    pub fuel: Readout<Fuel>,
    pub lifecycle: Lifecycle,
    pub width: f32,
    pub height: f32,
}

impl Inputs {
    pub fn from_model(model: &SessionModel, index: Option<usize>, size: (f32, f32)) -> Self {
        Self {
            fuel: index
                .and_then(|i| model.car(i))
                .map_or(Readout::Absent, |car| {
                    car.fuel().map(|f| Fuel {
                        in_tank: f.in_tank,
                        remaining_laps: f.remaining_laps,
                    })
                }),
            lifecycle: model.lifecycle(),
            width: size.0,
            height: size.1,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct View {
    pub density: Density,
    /// Fuel delta, then fuel in tank: both densities keep both rows.
    pub rows: Vec<Row>,
    pub message: Option<&'static str>,
    pub size: (f32, f32),
}

pub fn view(inputs: &Inputs) -> View {
    let short = inputs.fuel.value().is_some_and(|f| f.remaining_laps < 0.0);
    let rows = vec![
        Row {
            label: "Fuel delta",
            value: view_of(&inputs.fuel, |f| {
                let sign = if f.remaining_laps < 0.0 { MINUS } else { '+' };
                format!("{sign}{} laps", fixed(f64::from(f.remaining_laps.abs()), 2))
            }),
            tone: Some(if short { Tone::Bad } else { Tone::Good }),
        },
        Row::new(
            "Fuel in tank",
            view_of(&inputs.fuel, |f| {
                format!("{} kg", fixed(f64::from(f.in_tank), 1))
            }),
        ),
    ];
    View {
        density: DENSITIES.pick(inputs.width, inputs.height),
        message: slow::message(&rows, inputs.lifecycle),
        rows,
        size: (inputs.width, inputs.height),
    }
}

pub fn render(view: View) -> impl IntoElement {
    slow::render(&view.rows, view.message, view.size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pitwall_core::readout::{ABSENT_TEXT, ReadoutState};

    fn base(fuel: Readout<Fuel>) -> Inputs {
        Inputs {
            fuel,
            lifecycle: Lifecycle::Live,
            width: 320.0,
            height: 160.0,
        }
    }
    const SHORT: Fuel = Fuel {
        in_tank: 18.25,
        remaining_laps: -1.2,
    };

    #[test]
    fn declares_the_catalogue_entry() {
        assert_eq!((TYPE, NAME), ("fuel", "Fuel"));
        assert_eq!(DENSITIES.pick(239.0, 64.0), Density::Compact);
        assert_eq!(DENSITIES.pick(240.0, 64.0), Density::Regular);
    }

    #[test]
    fn shows_tank_kilograms_and_a_signed_laps_delta_never_a_unit_conversion() {
        let v = view(&base(Readout::Live(SHORT)));
        assert_eq!(v.rows[1].label, "Fuel in tank");
        assert_eq!(v.rows[1].value.text, "18.3 kg");
        assert_eq!(v.rows[0].label, "Fuel delta");
        assert_eq!(v.rows[0].value.text, "\u{2212}1.20 laps");
        assert_eq!(v.rows[0].tone, Some(Tone::Bad));
        let spare = view(&base(Readout::Live(Fuel {
            in_tank: 10.0,
            remaining_laps: 1.0,
        })));
        assert_eq!(spare.rows[0].value.text, "+1.00 laps");
        assert_eq!(spare.rows[0].tone, Some(Tone::Good));
    }

    #[test]
    fn both_densities_keep_both_rows() {
        let compact = view(&Inputs {
            width: 100.0,
            ..base(Readout::Live(SHORT))
        });
        assert_eq!(compact.density, Density::Compact);
        assert_eq!(compact.rows.len(), 2);
    }

    #[test]
    fn keeps_stale_restricted_and_absent_explicit_and_never_substitutes_zero() {
        let stale = view(&base(Readout::Stale(Fuel {
            in_tank: 10.0,
            remaining_laps: 1.0,
        })));
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
