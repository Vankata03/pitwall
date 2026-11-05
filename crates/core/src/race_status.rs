//! The race status band (spec section 11, Race status), ported from
//! `dashboard/src/session/race-status.ts`.

use crate::model::{RaceControlEntry, RaceControlKind};
use crate::telemetry::EventDetails;

/// Band text for the session's `safety_car_status`; empty while nothing is declared.
fn safety_car_band(status: Option<u8>) -> &'static str {
    match status {
        Some(1) => "Safety car",
        Some(2) => "Virtual safety car",
        Some(3) => "Formation lap",
        _ => "",
    }
}

/// The race status band, empty while nothing is declared. Pass the session's
/// `safety_car_status` (None when the session readout has no value) and the race-control
/// log. Event chronology decides when a red flag is superseded by a restart or lights
/// out; rewound entries never count.
pub fn race_status(
    safety_car_status: Option<u8>,
    race_control: &[RaceControlEntry],
) -> &'static str {
    let mut band = safety_car_band(safety_car_status);
    for entry in race_control.iter().filter(|entry| !entry.rewound) {
        if let RaceControlKind::Game(event) = &entry.kind {
            match event.details {
                EventDetails::RedFlag => band = "Red flag",
                EventDetails::LightsOut | EventDetails::SessionStarted => {
                    band = safety_car_band(safety_car_status)
                }
                _ => {}
            }
        }
    }
    band
}
