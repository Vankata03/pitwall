//! Port of the Gaps cases of dashboard/src/tiles/slow-tiles.test.tsx.
use pitwall_core::field::{FieldCar, LapFacts};
use pitwall_core::gaps::gaps_view;
use pitwall_core::model::{Gaps, Readout};

fn car(index: usize, position: Readout<u8>, to_car_in_front_ms: Option<u32>) -> FieldCar {
    FieldCar {
        index,
        label: Some(format!("Driver {}", index + 1)),
        retired: false,
        lap: position.map(|position| LapFacts {
            position,
            result_status: 2,
            pit_status: 0,
        }),
        gaps: Readout::Live(Gaps {
            to_leader_ms: 0,
            to_car_in_front_ms,
            laps_behind: Some(0),
        }),
        best_lap_ms: None,
    }
}

#[test]
fn behind_uses_the_next_positions_interval_irrespective_of_car_order() {
    let field = [
        car(0, Readout::Live(2), Some(1250)),
        car(1, Readout::Live(4), Some(9000)),
        car(2, Readout::Live(3), Some(2750)),
    ];
    let view = gaps_view(&field, 0);
    assert_eq!(view.ahead, Readout::Live("+1.250 s".to_string()));
    assert_eq!(view.behind, Readout::Live("\u{2212}2.750 s".to_string()));
    assert_eq!(view.behind_note.as_deref(), Some("P3 \u{b7} Driver 3"));
    assert_eq!(view.ahead_note, None, "nobody listed at P1");
}

#[test]
fn at_the_field_boundary_ahead_stays_absent_and_a_stale_neighbour_taints_behind() {
    let field = [
        car(0, Readout::Live(1), None),
        car(1, Readout::Stale(2), Some(2000)),
    ];
    let view = gaps_view(&field, 0);
    assert_eq!(view.ahead, Readout::Absent);
    assert_eq!(view.behind, Readout::Stale("\u{2212}2.000 s".to_string()));
    assert_eq!(view.ahead_note, None);
    assert_eq!(view.behind_note.as_deref(), Some("P2 \u{b7} Driver 2"));
}

#[test]
fn without_lap_data_there_is_no_behind_and_no_notes() {
    let field = [
        car(0, Readout::Absent, Some(500)),
        car(1, Readout::Live(2), Some(700)),
    ];
    let view = gaps_view(&field, 0);
    assert_eq!(view.ahead, Readout::Live("+0.500 s".to_string()));
    assert_eq!(view.behind, Readout::Absent);
    assert_eq!((view.ahead_note, view.behind_note), (None, None));
}

#[test]
fn restricted_gaps_stay_restricted() {
    let mut mine = car(0, Readout::Live(2), Some(1));
    mine.gaps = Readout::Restricted;
    let view = gaps_view(&[mine], 0);
    assert_eq!(view.ahead, Readout::Restricted);
}

#[test]
fn the_race_leader_keeps_the_bound_car_interval_like_the_web_tile() {
    // The web tile displays the bound car interval even when P1 has no ahead note.
    let field = [
        car(0, Readout::Live(1), Some(0)),
        car(1, Readout::Live(2), Some(11_004)),
    ];
    let view = gaps_view(&field, 0);
    assert_eq!(view.ahead, Readout::Live("+0.000 s".to_string()));
    assert_eq!(view.ahead_note, None);
    assert_eq!(view.behind, Readout::Live("\u{2212}11.004 s".to_string()));
}
