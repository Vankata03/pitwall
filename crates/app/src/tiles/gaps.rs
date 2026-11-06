//! Gaps (parity GAPS-01): a slow tile following one car, with the interval to the car
//! ahead and of the car behind. Port of `dashboard/src/tiles/types/gaps.tsx`; the
//! arithmetic is `pitwall_core::gaps`.

use gpui::{IntoElement, ParentElement, Styled, div, px};
use pitwall_core::{
    field::{FieldCar, field_of},
    gaps::gaps_view,
    model::{Lifecycle, Readout, SessionModel},
    readout::view_of,
};

use super::{
    Densities, Density,
    slow::{self, Row},
    widgets,
};

pub const TYPE: &str = pitwall_core::tile_catalog::GAPS.kind;

pub const DENSITIES: Densities = Densities {
    regular: (320.0, 64.0),
    full: None,
};

pub struct Inputs {
    pub field: Vec<FieldCar>,
    /// The bound car; `None` reads absent.
    pub index: Option<usize>,
    pub lifecycle: Lifecycle,
    pub width: f32,
    pub height: f32,
}

impl Inputs {
    pub fn from_model(model: &SessionModel, index: Option<usize>, size: (f32, f32)) -> Self {
        Self {
            field: field_of(model),
            index,
            lifecycle: model.lifecycle(),
            width: size.0,
            height: size.1,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct View {
    pub density: Density,
    /// Ahead, then behind; both densities keep both.
    pub rows: Vec<Row>,
    /// `P<n> · <name>` under each row's label: which car the gap is to.
    pub notes: Vec<Option<String>>,
    pub message: Option<&'static str>,
    pub size: (f32, f32),
}

pub fn view(inputs: &Inputs) -> View {
    let gaps = inputs.index.map(|i| gaps_view(&inputs.field, i));
    let text = |r: Option<&Readout<String>>| view_of(r.unwrap_or(&Readout::Absent), String::clone);
    let rows = vec![
        Row::new("Ahead", text(gaps.as_ref().map(|g| &g.ahead))),
        Row::new("Behind", text(gaps.as_ref().map(|g| &g.behind))),
    ];
    let notes = match gaps {
        Some(g) => vec![g.ahead_note, g.behind_note],
        None => vec![None, None],
    };
    View {
        density: DENSITIES.pick(inputs.width, inputs.height),
        message: slow::message(&rows, inputs.lifecycle),
        rows,
        notes,
        size: (inputs.width, inputs.height),
    }
}

pub fn render(view: View) -> impl IntoElement {
    let numerals = slow::value_size(view.size.0, view.size.1);
    let mut line = div().flex().justify_between().gap_4().w_full();
    for (i, (r, note)) in view.rows.iter().zip(&view.notes).enumerate() {
        let mut cell = slow::row(r, numerals);
        if let Some(note) = note {
            cell = cell.child(widgets::label(note.clone()));
        }
        if i > 0 {
            cell = cell.items_end();
        }
        line = line.child(cell);
    }
    widgets::tile_shell(
        div()
            .flex()
            .flex_col()
            .justify_center()
            .size_full()
            .child(line)
            .text_size(px(14.0)),
        view.message,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use pitwall_core::{
        field::LapFacts,
        model::Gaps,
        readout::{ABSENT_TEXT, ReadoutState},
    };

    fn car(index: usize, position: Readout<u8>, interval: Readout<Option<u32>>) -> FieldCar {
        FieldCar {
            index,
            label: Some(format!("Driver {}", index + 1)),
            retired: false,
            lap: position.map(|position| LapFacts {
                position,
                result_status: 2,
                pit_status: 0,
            }),
            gaps: interval.map(|to_car_in_front_ms| Gaps {
                to_leader_ms: 0,
                to_car_in_front_ms,
                laps_behind: Some(0),
            }),
            best_lap_ms: None,
        }
    }

    fn base(field: Vec<FieldCar>) -> Inputs {
        Inputs {
            field,
            index: Some(0),
            lifecycle: Lifecycle::Live,
            width: 320.0,
            height: 160.0,
        }
    }

    #[test]
    fn declares_the_catalogue_entry() {
        assert_eq!(
            (TYPE, pitwall_core::tile_catalog::GAPS.name),
            ("gaps", "Gaps")
        );
        assert_eq!(DENSITIES.pick(319.0, 64.0), Density::Compact);
        assert_eq!(DENSITIES.pick(320.0, 64.0), Density::Regular);
    }

    #[test]
    fn behind_uses_the_next_positions_interval_irrespective_of_car_order() {
        let v = view(&base(vec![
            car(0, Readout::Live(2), Readout::Live(Some(1250))),
            car(1, Readout::Live(4), Readout::Live(Some(9000))),
            car(2, Readout::Live(3), Readout::Live(Some(2750))),
        ]));
        assert_eq!(v.rows[0].label, "Ahead");
        assert_eq!(v.rows[0].value.text, "+1.250 s");
        assert_eq!(v.rows[1].label, "Behind");
        assert_eq!(v.rows[1].value.text, "\u{2212}2.750 s");
        // Notes name the cars, by position.
        assert_eq!(v.notes[1].as_deref(), Some("P3 \u{b7} Driver 3"));
        assert_eq!(v.notes[0], None);
    }

    #[test]
    fn at_the_field_boundary_ahead_stays_absent_and_stale_neighbour_positions_taint_behind() {
        let v = view(&base(vec![
            car(0, Readout::Live(1), Readout::Live(None)),
            car(1, Readout::Stale(2), Readout::Live(Some(2000))),
        ]));
        assert_eq!(v.rows[0].value.text, ABSENT_TEXT);
        assert_eq!(v.rows[1].value.state, ReadoutState::Stale);
        assert_eq!(v.message, None);
    }

    #[test]
    fn keeps_stale_restricted_and_absent_explicit_and_never_substitutes_zero() {
        let stale = view(&base(vec![car(
            0,
            Readout::Stale(2),
            Readout::Stale(Some(1234)),
        )]));
        assert_eq!(stale.rows[0].value.state, ReadoutState::Stale);
        assert_eq!(stale.rows[0].value.text, "+1.234 s");
        assert_eq!(stale.message, None);
        let restricted = view(&base(vec![car(
            0,
            Readout::Restricted,
            Readout::Restricted,
        )]));
        assert_eq!(restricted.rows[0].value.text, "Restricted");
        assert_eq!(restricted.rows[1].value.text, ABSENT_TEXT);
        assert_eq!(restricted.message, Some("No data"));
        let absent = view(&base(vec![car(0, Readout::Absent, Readout::Absent)]));
        assert!(absent.rows.iter().all(|r| r.value.text == ABSENT_TEXT));
        assert_eq!(absent.message, Some("No data"));
    }

    #[test]
    fn the_race_leader_keeps_the_web_interval_display() {
        let v = view(&base(vec![
            car(0, Readout::Live(1), Readout::Live(Some(0))),
            car(1, Readout::Live(2), Readout::Live(Some(11_004))),
        ]));
        assert_eq!(v.rows[0].value.state, ReadoutState::Live);
        assert_eq!(v.rows[0].value.text, "+0.000 s");
        assert_eq!(v.rows[1].value.text, "\u{2212}11.004 s");
        assert_eq!(v.notes[1].as_deref(), Some("P2 \u{b7} Driver 2"));
    }

    #[test]
    fn without_a_bound_car_everything_is_absent() {
        let v = view(&Inputs {
            index: None,
            ..base(vec![car(0, Readout::Live(1), Readout::Live(None))])
        });
        assert!(v.rows.iter().all(|r| r.value.state == ReadoutState::Absent));
        assert_eq!(v.notes, vec![None, None]);
    }

    #[test]
    fn both_densities_keep_both_rows() {
        let compact = view(&Inputs {
            width: 100.0,
            ..base(vec![car(0, Readout::Live(1), Readout::Live(None))])
        });
        assert_eq!(compact.density, Density::Compact);
        assert_eq!(compact.rows.len(), 2);
    }
}
