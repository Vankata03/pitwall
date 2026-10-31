use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, ts_rs::TS)]
pub struct Gaps {
    /// Race leader in races; session-best valid lap in timed sessions.
    pub to_leader_ms: u32,
    pub to_car_in_front_ms: Option<u32>,
    /// Whole race-distance laps behind the position-one car; absent in timed sessions.
    pub laps_behind: Option<u32>,
}

impl CarView<'_> {
    /// The 4 MJ capacity is assumed pending the 2026 capture check.
    pub fn ers_percentage(&self) -> Readout<f32> {
        self.ers().map(|ers| ers.store_energy / 4_000_000.0 * 100.0)
    }

    /// RL, RR, FL, FR order, preserving the damage packet's readout state.
    pub fn tyre_condition(&self) -> Readout<[f32; 4]> {
        self.damage()
            .map(|damage| damage.tyres_wear.map(|wear| 100.0 - wear))
    }

    pub fn gaps(&self) -> Readout<Gaps> {
        let Some(session) = self.model.session.value.as_deref() else {
            return Readout::Absent;
        };
        // IDs are shared by both formats: 15–17 races, 1–14 timed sessions, 18 TT.
        if matches!(session.session_type, 15..=17) {
            return self.lap().map(|lap| {
                let laps_behind = self
                    .model
                    .cars
                    .iter()
                    .filter(|car| car.valid)
                    .filter_map(|car| car.lap.value.as_deref())
                    .find(|car| car.car_position == 1)
                    .filter(|leader| {
                        leader.total_distance.is_finite()
                            && lap.total_distance.is_finite()
                            && session.track_length > 0
                    })
                    .map(|leader| {
                        ((leader.total_distance - lap.total_distance).max(0.0)
                            / f32::from(session.track_length))
                        .floor() as u32
                    });
                Gaps {
                    to_leader_ms: u32::from(lap.delta_to_race_leader_minutes_part) * 60_000
                        + u32::from(lap.delta_to_race_leader_ms_part),
                    to_car_in_front_ms: Some(
                        u32::from(lap.delta_to_car_in_front_minutes_part) * 60_000
                            + u32::from(lap.delta_to_car_in_front_ms_part),
                    ),
                    laps_behind,
                }
            });
        }
        if !matches!(session.session_type, 1..=14 | 18) {
            return Readout::Absent;
        }
        let Some(best) = self.car.derived.best_lap() else {
            return Readout::Absent;
        };
        let Some(session_best) = self
            .model
            .cars
            .iter()
            .filter(|car| car.valid)
            .filter_map(|car| car.derived.best_lap())
            .map(|lap| lap.lap_time_ms)
            .min()
        else {
            return Readout::Absent;
        };
        let gaps = Gaps {
            to_leader_ms: best.lap_time_ms.saturating_sub(session_best),
            to_car_in_front_ms: None,
            laps_behind: None,
        };
        if self.car.history.value.is_some() && self.car.history.arrival >= self.car.lap.arrival {
            self.history().map(|_| gaps)
        } else {
            self.lap().map(|_| gaps)
        }
    }
}
