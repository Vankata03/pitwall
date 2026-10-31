use super::*;

impl<T: Clone> Group<T> {
    fn rewind(&mut self, time: f32) {
        if self.session_time > time {
            self.value = None;
            // Keep permission conservative until Participants arrives again.
        }
    }
}

impl SessionModel {
    pub(super) fn rewind_for(&mut self, datagram: &ParsedDatagram) {
        let explicit = match &datagram.packet {
            Packet::Event(event) => match event.details {
                EventDetails::Flashback { session_time, .. } => {
                    Some((session_time, (**event).clone()))
                }
                _ => None,
            },
            _ => None,
        };
        let time = explicit
            .as_ref()
            .map_or(datagram.header.session_time, |(time, _)| *time);
        if !time.is_finite() || time < 0.0 {
            return;
        }
        // The late results burst is exempt from session-time ordering too.
        if matches!(
            datagram.packet,
            Packet::SessionHistory(_) | Packet::FinalClassification(_)
        ) {
            return;
        }
        let rewind = explicit.is_some()
            || self
                .last_session_time
                .is_some_and(|previous| time < previous);
        let from_time = self.last_session_time.unwrap_or(time);
        self.last_session_time = Some(time);
        if !rewind {
            return;
        }
        self.epoch = self.epoch.wrapping_add(1);
        for car in &mut self.cars {
            let car = Arc::make_mut(car);
            Arc::make_mut(&mut car.derived).rewind(time);
            car.delta.rewind(time, car.derived.best_lap_number);
            car.participant.rewind(time);
            car.lap.rewind(time);
            car.motion.rewind(time);
            car.telemetry.rewind(time);
            car.aero.rewind(time);
            car.history.rewind(time);
            car.status.rewind(time);
            car.fuel.rewind(time);
            car.ers.rewind(time);
            car.front_brake_bias.rewind(time);
            car.damage.rewind(time);
            car.tyre_sets.rewind(time);
            if car.retired_at.is_some_and(|at| at > time) {
                car.retired = false;
                car.retirement_reason = None;
                car.retired_at = None;
            }
            if car.valid_since.is_some_and(|at| at > time) {
                car.valid = false;
                car.valid_since = None;
            }
        }
        self.time_trial.rewind(time);
        self.session.rewind(time);
        self.pit_window.rewind(time);
        if self
            .classification
            .as_ref()
            .is_some_and(|value| value.session_time > time)
        {
            self.classification = None;
        }
        for entry in Arc::make_mut(&mut self.race_control) {
            if entry.session_time > time {
                entry.rewound = true;
            }
        }
        let source = if explicit.is_some() {
            Source::Game
        } else {
            Source::Derived
        };
        let lap = self
            .player_car_index()
            .and_then(|i| self.cars[i].lap.value.as_ref())
            .map(|lap| lap.current_lap_num);
        self.log(
            time,
            vec![],
            lap,
            source,
            RaceControlKind::Flashback {
                from_time,
                to_time: time,
                event: explicit.map(|(_, event)| event),
            },
        );
    }
}
