//! Track discipline (parity DISCIPLINE-01): a slow tile following one car.
//! Port of `dashboard/src/tiles/types/track-discipline.tsx`.

use gpui::IntoElement;
use pitwall_core::{
    model::{Lifecycle, PenaltyState, Readout, SessionModel},
    readout::view_of,
};

use super::{
    Densities, Density,
    slow::{self, Row},
};

pub const TYPE: &str = pitwall_core::tile_catalog::TRACK_DISCIPLINE.kind;

pub const DENSITIES: Densities = Densities {
    regular: (240.0, 104.0),
    full: None,
};

pub struct Inputs {
    pub penalties: Readout<PenaltyState>,
    /// The current lap's validity: `true` when invalid.
    pub lap_invalid: Readout<bool>,
    pub lifecycle: Lifecycle,
    pub width: f32,
    pub height: f32,
}

impl Inputs {
    pub fn from_model(model: &SessionModel, index: Option<usize>, size: (f32, f32)) -> Self {
        let car = index.and_then(|i| model.car(i));
        Self {
            penalties: car.map_or(Readout::Absent, |c| c.penalty_state()),
            lap_invalid: car.map_or(Readout::Absent, |c| {
                c.lap().map(|l| l.current_lap_invalid != 0)
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
    pub rows: Vec<Row>,
    pub message: Option<&'static str>,
    pub size: (f32, f32),
}

pub fn view(inputs: &Inputs) -> View {
    let density = DENSITIES.pick(inputs.width, inputs.height);
    let p = &inputs.penalties;
    let mut rows = vec![
        Row::new(
            "Corner cuts",
            view_of(p, |p| p.corner_cutting_warnings.to_string()),
        ),
        Row::new("Warnings", view_of(p, |p| p.total_warnings.to_string())),
        Row::new("Penalties", view_of(p, |p| format!("{} s", p.seconds))),
        Row::new(
            "Lap validity",
            view_of(&inputs.lap_invalid, |&invalid| {
                if invalid { "Invalid" } else { "Valid" }.into()
            }),
        ),
    ];
    if density != Density::Compact {
        rows.push(Row::new(
            "Drive-through",
            view_of(p, |p| p.drive_throughs.to_string()),
        ));
        rows.push(Row::new("Stop-go", view_of(p, |p| p.stop_gos.to_string())));
    }
    View {
        density,
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

    const PENALTIES: PenaltyState = PenaltyState {
        seconds: 5,
        total_warnings: 3,
        corner_cutting_warnings: 2,
        drive_throughs: 1,
        stop_gos: 0,
    };
    fn base(penalties: Readout<PenaltyState>, lap_invalid: Readout<bool>) -> Inputs {
        Inputs {
            penalties,
            lap_invalid,
            lifecycle: Lifecycle::Live,
            width: 320.0,
            height: 160.0,
        }
    }
    fn text<'a>(v: &'a View, label: &str) -> &'a str {
        &v.rows.iter().find(|r| r.label == label).unwrap().value.text
    }

    #[test]
    fn declares_the_catalogue_entry() {
        assert_eq!(
            (TYPE, pitwall_core::tile_catalog::TRACK_DISCIPLINE.name),
            ("track-discipline", "Track discipline")
        );
        assert_eq!(DENSITIES.pick(239.0, 104.0), Density::Compact);
        assert_eq!(DENSITIES.pick(240.0, 104.0), Density::Regular);
    }

    #[test]
    fn keeps_penalties_warnings_and_invalid_status_distinct() {
        let v = view(&base(Readout::Live(PENALTIES), Readout::Live(true)));
        assert_eq!(text(&v, "Corner cuts"), "2");
        assert_eq!(text(&v, "Warnings"), "3");
        assert_eq!(text(&v, "Penalties"), "5 s");
        assert_eq!(text(&v, "Drive-through"), "1");
        assert_eq!(text(&v, "Stop-go"), "0");
        assert_eq!(text(&v, "Lap validity"), "Invalid");
    }

    #[test]
    fn compact_drops_only_drive_through_and_stop_go() {
        let v = view(&Inputs {
            width: 200.0,
            ..base(Readout::Live(PENALTIES), Readout::Live(false))
        });
        assert_eq!(text(&v, "Penalties"), "5 s");
        assert_eq!(text(&v, "Lap validity"), "Valid");
        assert_eq!(v.rows.len(), 4);
        assert!(v.rows.iter().all(|r| r.label != "Stop-go"));
    }

    #[test]
    fn missing_penalty_state_never_fabricates_zero_or_hides_validity() {
        let v = view(&base(Readout::Absent, Readout::Live(false)));
        assert_eq!(text(&v, "Penalties"), ABSENT_TEXT);
        assert_eq!(text(&v, "Corner cuts"), ABSENT_TEXT);
        assert_eq!(text(&v, "Lap validity"), "Valid");
        assert_eq!(v.message, None);
    }

    #[test]
    fn keeps_stale_restricted_and_absent_explicit() {
        let stale = view(&base(Readout::Stale(PENALTIES), Readout::Stale(false)));
        assert!(
            stale
                .rows
                .iter()
                .all(|r| r.value.state == ReadoutState::Stale)
        );
        assert_eq!(stale.message, None);
        let restricted = view(&base(Readout::Restricted, Readout::Restricted));
        assert!(restricted.rows.iter().all(|r| r.value.text == "Restricted"));
        assert_eq!(restricted.message, Some("No data"));
        let absent = view(&base(Readout::Absent, Readout::Absent));
        assert!(absent.rows.iter().all(|r| r.value.text == ABSENT_TEXT));
        assert_eq!(absent.message, Some("No data"));
    }
}
