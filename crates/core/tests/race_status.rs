//! Port of dashboard/src/session/race-status.test.ts.
use pitwall_core::model::{RaceControlEntry, RaceControlKind, Source};
use pitwall_core::race_status::race_status;
use pitwall_core::telemetry::{Event, EventDetails};

fn game(details: EventDetails, rewound: bool) -> RaceControlEntry {
    RaceControlEntry {
        id: 0,
        session_time: 0.0,
        lap_number: None,
        cars: Vec::new(),
        source: Source::Game,
        kind: RaceControlKind::Game(Event {
            code: *b"XXXX",
            details,
            raw_details: [0; 12],
        }),
        rewound,
    }
}

#[test]
fn the_status_is_the_safety_car_state_empty_while_nothing_is_declared() {
    assert_eq!(race_status(Some(0), &[]), "");
    assert_eq!(race_status(Some(1), &[]), "Safety car");
    assert_eq!(race_status(Some(2), &[]), "Virtual safety car");
    assert_eq!(race_status(Some(3), &[]), "Formation lap");
    assert_eq!(race_status(None, &[]), "");
    assert_eq!(race_status(Some(9), &[]), "");
}

#[test]
fn a_red_flag_holds_until_lights_out_and_a_rewound_one_never_counted() {
    use EventDetails::*;
    assert_eq!(race_status(Some(0), &[game(RedFlag, false)]), "Red flag");
    assert_eq!(
        race_status(Some(0), &[game(RedFlag, false), game(LightsOut, false)]),
        ""
    );
    assert_eq!(
        race_status(
            Some(1),
            &[game(RedFlag, false), game(SessionStarted, false)]
        ),
        "Safety car"
    );
    assert_eq!(race_status(Some(0), &[game(RedFlag, true)]), "");
}
