//! The field of cars as the Gaps and Timing tower tiles read it: one plain row per car
//! slot, built from a model snapshot by [`field_of`]. The tile logic in
//! [`crate::gaps`] and [`crate::tower`] works on these rows, so it is testable without a
//! session.

use crate::car_names::car_label;
use crate::model::{Gaps, Readout, Reference, SessionModel};
use crate::telemetry::MAX_CARS;

/// The Lap Data facts the ordering needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LapFacts {
    /// `m_carPosition`; 0 when the game has none.
    pub position: u8,
    /// `m_resultStatus`: 0 invalid, 1 inactive, 2 active, 3 finished, 4 did not finish,
    /// 5 disqualified, 6 not classified, 7 retired.
    pub result_status: u8,
    pub pit_status: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldCar {
    /// Vehicle index.
    pub index: usize,
    /// Shown name while the participant readout is live or stale; `None` means the car is
    /// not a participant the tiles list.
    pub label: Option<String>,
    pub retired: bool,
    pub lap: Readout<LapFacts>,
    pub gaps: Readout<Gaps>,
    /// Best valid lap, ms.
    pub best_lap_ms: Option<u32>,
}

impl FieldCar {
    /// Race position while the lap readout has a value.
    pub fn position(&self) -> Option<u8> {
        match self.lap {
            Readout::Live(lap) | Readout::Stale(lap) => Some(lap.position),
            _ => None,
        }
    }
}

/// Every car slot of the snapshot, in vehicle-index order.
pub fn field_of(model: &SessionModel) -> Vec<FieldCar> {
    (0..MAX_CARS)
        .filter_map(|index| {
            let car = model.car(index)?;
            let label = match car.participant() {
                Readout::Live(p) | Readout::Stale(p) => Some(car_label(p)),
                _ => None,
            };
            Some(FieldCar {
                index,
                label,
                retired: car.retired(),
                lap: car.lap().map(|lap| LapFacts {
                    position: lap.car_position,
                    result_status: lap.result_status,
                    pit_status: lap.pit_status,
                }),
                gaps: car.gaps(),
                best_lap_ms: car.reference_lap(Reference::Best).map(|l| l.lap_time_ms),
            })
        })
        .collect()
}

/// `+1.234` or `−1.234` (U+2212) seconds from signed milliseconds, three decimals.
pub fn signed_seconds(ms: i64) -> String {
    let sign = if ms < 0 { '\u{2212}' } else { '+' };
    format!("{sign}{:.3}", ms.unsigned_abs() as f64 / 1000.0)
}
