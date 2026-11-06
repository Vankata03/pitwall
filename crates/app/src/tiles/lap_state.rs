//! Lap state (parity LAPSTATE-01): a slow tile following one car.
//! Port of `dashboard/src/tiles/types/lap-state.tsx`.

use gpui::IntoElement;
use pitwall_core::{
    model::{Lifecycle, Readout, SessionModel},
    readout::{ReadoutView, view_of},
};

use super::{
    Densities, Density,
    slow::{self, Row},
};

pub const TYPE: &str = "lap-state";
pub const NAME: &str = "Lap state";
pub const DENSITIES: Densities = Densities {
    regular: (280.0, 64.0),
    full: None,
};

/// The Lap Data fields the tile reads, as the game sends them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LapFacts {
    pub current_lap_num: u8,
    /// `m_carPosition`; 0 when the game has none.
    pub car_position: u8,
    /// Zero-based sector.
    pub sector: u8,
    /// 0 on track, 1 pitting, 2 in the pit area.
    pub pit_status: u8,
}

pub struct Inputs {
    pub lap: Readout<LapFacts>,
    /// The session's `m_totalLaps`.
    pub total_laps: Readout<u8>,
    pub lifecycle: Lifecycle,
    pub width: f32,
    pub height: f32,
}

impl Inputs {
    pub fn from_model(model: &SessionModel, index: Option<usize>, size: (f32, f32)) -> Self {
        Self {
            lap: index
                .and_then(|i| model.car(i))
                .map_or(Readout::Absent, |car| {
                    car.lap().map(|l| LapFacts {
                        current_lap_num: l.current_lap_num,
                        car_position: l.car_position,
                        sector: l.sector,
                        pit_status: l.pit_status,
                    })
                }),
            total_laps: model.session().map(|s| s.total_laps),
            lifecycle: model.lifecycle(),
            width: size.0,
            height: size.1,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct View {
    pub density: Density,
    /// Lap, total, position, sector, pit status: both densities show every row.
    pub rows: Vec<Row>,
    pub message: Option<&'static str>,
    pub size: (f32, f32),
}

/// A readout whose value may have no meaning (web `mapValue`): `None` reads absent.
fn maybe<T>(readout: &Readout<T>, format: impl FnOnce(&T) -> Option<String>) -> ReadoutView {
    match readout.value().map(format) {
        Some(None) => view_of(&Readout::<()>::Absent, |_| String::new()),
        Some(Some(text)) => view_of(readout, |_| text),
        None => view_of(readout, |_| String::new()),
    }
}

pub fn view(inputs: &Inputs) -> View {
    let rows = vec![
        Row::new(
            "Lap",
            view_of(&inputs.lap, |l| l.current_lap_num.to_string()),
        ),
        Row::new(
            "Total",
            maybe(&inputs.total_laps, |&t| (t > 0).then(|| t.to_string())),
        ),
        Row::new(
            "Position",
            maybe(&inputs.lap, |l| {
                (l.car_position > 0).then(|| l.car_position.to_string())
            }),
        ),
        Row::new(
            "Sector",
            maybe(&inputs.lap, |l| {
                (l.sector < 3).then(|| (l.sector + 1).to_string())
            }),
        ),
        Row::new(
            "Pit status",
            maybe(&inputs.lap, |l| {
                ["On track", "Pitting", "In pit"]
                    .get(usize::from(l.pit_status))
                    .map(|s| (*s).to_string())
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

    const LAP: LapFacts = LapFacts {
        current_lap_num: 12,
        car_position: 3,
        sector: 1,
        pit_status: 2,
    };

    fn base(lap: Readout<LapFacts>, total: Readout<u8>) -> Inputs {
        Inputs {
            lap,
            total_laps: total,
            lifecycle: Lifecycle::Live,
            width: 320.0,
            height: 160.0,
        }
    }

    fn text(v: &View, label: &str) -> String {
        v.rows
            .iter()
            .find(|r| r.label == label)
            .unwrap()
            .value
            .text
            .clone()
    }

    #[test]
    fn declares_the_catalogue_entry() {
        assert_eq!((TYPE, NAME), ("lap-state", "Lap state"));
        assert_eq!(DENSITIES.pick(279.0, 64.0), Density::Compact);
        assert_eq!(DENSITIES.pick(280.0, 64.0), Density::Regular);
    }

    #[test]
    fn shows_lap_total_position_one_based_sector_and_named_pit_status() {
        let v = view(&base(Readout::Live(LAP), Readout::Live(44)));
        assert_eq!(text(&v, "Lap"), "12");
        assert_eq!(text(&v, "Total"), "44");
        assert_eq!(text(&v, "Position"), "3");
        assert_eq!(text(&v, "Sector"), "2");
        assert_eq!(text(&v, "Pit status"), "In pit");
        for (status, name) in [(0, "On track"), (1, "Pitting")] {
            let v = view(&base(
                Readout::Live(LapFacts {
                    pit_status: status,
                    ..LAP
                }),
                Readout::Live(44),
            ));
            assert_eq!(text(&v, "Pit status"), name);
        }
    }

    #[test]
    fn values_without_meaning_read_absent_never_zero() {
        let v = view(&base(
            Readout::Live(LapFacts {
                car_position: 0,
                sector: 3,
                pit_status: 9,
                ..LAP
            }),
            Readout::Live(0),
        ));
        for label in ["Total", "Position", "Sector", "Pit status"] {
            let row = v.rows.iter().find(|r| r.label == label).unwrap();
            assert_eq!(row.value.state, ReadoutState::Absent, "{label}");
            assert_eq!(row.value.text, ABSENT_TEXT);
        }
        assert_eq!(text(&v, "Lap"), "12");
    }

    #[test]
    fn both_densities_show_the_same_rows() {
        let regular = view(&base(Readout::Live(LAP), Readout::Live(44)));
        let compact = view(&Inputs {
            width: 100.0,
            ..base(Readout::Live(LAP), Readout::Live(44))
        });
        assert_eq!(compact.density, Density::Compact);
        assert_eq!(regular.rows, compact.rows);
    }

    #[test]
    fn keeps_stale_restricted_and_absent_explicit_and_never_substitutes_zero() {
        let stale = view(&base(Readout::Stale(LAP), Readout::Stale(44)));
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
        let waiting = view(&Inputs {
            lifecycle: Lifecycle::Waiting,
            ..base(Readout::Absent, Readout::Absent)
        });
        assert_eq!(waiting.message, Some("Waiting for the game"));
    }
}
