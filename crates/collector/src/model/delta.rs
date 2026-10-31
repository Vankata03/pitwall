//! Bounded distance traces; completed generations are immutable checkpoint data.
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reference {
    Best,
    Previous,
    PersonalBest,
    Rival,
}

/// Official summary comparison. No distance samples are required.
#[derive(Debug, Clone, PartialEq)]
pub struct LapResult {
    pub lap_number: u8,
    pub valid: bool,
    pub lap_time_ms: u32,
    pub reference_lap_number: Option<u8>,
    pub delta_ms: Option<i64>,
    pub sector_deltas_ms: [Option<i64>; 3],
}

#[derive(Debug, Clone, PartialEq)]
pub struct LapSample {
    pub distance_m: f32,
    pub time_ms: f32,
    /// Time of the latest input needed to produce this bucket.
    pub session_time: f32,
    pub speed_kph: Option<f32>,
    pub throttle: Option<f32>,
    pub brake: Option<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LapTrace {
    pub lap_number: u8,
    pub samples: Vec<LapSample>,
    start_time: f32,
    finish_time: Option<f32>,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub(super) struct DeltaState {
    current: Option<Arc<LapTrace>>,
    completed: Vec<Arc<LapTrace>>,
    last: Option<LapSample>,
    at_line: bool,
    former_best: Option<u8>,
    best: Option<u8>,
}

impl LapTrace {
    fn time_at(&self, distance: f32) -> Option<f32> {
        let index = self.samples.partition_point(|p| p.distance_m < distance);
        let right = self.samples.get(index)?;
        if right.distance_m == distance {
            return Some(right.time_ms);
        }
        let left = self.samples.get(index.checked_sub(1)?)?;
        (right.distance_m - left.distance_m <= 100.0).then(|| {
            let fraction = (distance - left.distance_m) / (right.distance_m - left.distance_m);
            left.time_ms + fraction * (right.time_ms - left.time_ms)
        })
    }
}

impl<'a> CarView<'a> {
    pub fn reference_lap(&self, reference: Reference) -> Option<&'a CompletedLap> {
        match reference {
            Reference::Best => self.car.derived.best_lap(),
            Reference::Previous => self.car.derived.laps.last().map(Arc::as_ref),
            _ => None,
        }
    }
    /// Time Trial extras belong only to the player car, and never seed best.
    pub fn time_trial_reference(
        &self,
        reference: Reference,
    ) -> Option<&'a crate::telemetry::TimeTrialDataSet> {
        if self.model.session.value.as_ref()?.session_type != 18 {
            return None;
        }
        let player = self.model.player_car_index()?;
        if !std::ptr::eq(self.car, self.model.cars[player].as_ref()) {
            return None;
        }
        let packet = self.model.time_trial.value.as_ref()?;
        let value = match reference {
            Reference::PersonalBest => &packet.personal_best_data_set,
            Reference::Rival => &packet.rival_data_set,
            _ => return None,
        };
        (value.lap_time_in_ms > 0).then_some(value)
    }
    /// Zero-based sector, signed milliseconds against the named lap only.
    pub fn sector_delta(&self, reference: Reference, sector: usize) -> Option<i64> {
        if sector >= 3 {
            return None;
        }
        if let Some(result) = self.completed_result(reference) {
            return result.sector_deltas_ms[sector];
        }
        let number = self.car.lap.value.as_ref()?.current_lap_num;
        let value = self
            .car
            .derived
            .sectors
            .iter()
            .find(|s| s.lap_number == number && usize::from(s.sector) == sector)?;
        let reference_time = if let Some(tt) = self.time_trial_reference(reference) {
            [
                tt.sector1_time_in_ms,
                tt.sector2_time_in_ms,
                tt.sector3_time_in_ms,
            ][sector]
        } else {
            self.reference_lap(reference)?.sectors_ms[sector]?
        };
        (reference_time > 0).then_some(i64::from(value.time_ms) - i64::from(reference_time))
    }
    /// The completed result remains visible until sector one of the next lap.
    /// Authoritative summary corrections are reflected on every read.
    pub fn completed_result(&self, reference: Reference) -> Option<LapResult> {
        let current = self.car.lap.value.as_ref()?;
        if current.sector1_time_ms_part != 0 || current.sector1_time_minutes_part != 0 {
            return None;
        }
        let lap = self
            .car
            .derived
            .laps
            .iter()
            .find(|l| l.lap_number.checked_add(1) == Some(current.current_lap_num))?;
        let tt = self.time_trial_reference(reference);
        let mut prior = self
            .car
            .derived
            .laps
            .iter()
            .filter(|l| l.lap_number < lap.lap_number);
        let reference = match reference {
            Reference::Best => prior
                .filter(|l| l.valid)
                .min_by_key(|l| (l.lap_time_ms, l.lap_number)),
            Reference::Previous => prior.next_back(),
            _ => None,
        };
        let reference_time = reference
            .map(|r| r.lap_time_ms)
            .or_else(|| tt.map(|r| r.lap_time_in_ms));
        Some(LapResult {
            lap_number: lap.lap_number,
            valid: lap.valid,
            lap_time_ms: lap.lap_time_ms,
            reference_lap_number: reference.map(|l| l.lap_number),
            delta_ms: reference_time.map(|r| i64::from(lap.lap_time_ms) - i64::from(r)),
            sector_deltas_ms: std::array::from_fn(|i| {
                lap.sectors_ms[i]
                    .zip(reference.and_then(|r| r.sectors_ms[i]).or_else(|| {
                        tt.map(|r| {
                            [
                                r.sector1_time_in_ms,
                                r.sector2_time_in_ms,
                                r.sector3_time_in_ms,
                            ][i]
                        })
                        .filter(|v| *v > 0)
                    }))
                    .map(|(a, b)| i64::from(a) - i64::from(b))
            }),
        })
    }
    /// Only the named reference is served, never a fallback with samples.
    pub fn lap_trace(&self, reference: Reference) -> Option<&'a Arc<LapTrace>> {
        let number = self.reference_lap(reference)?.lap_number;
        self.car
            .delta
            .completed
            .iter()
            .find(|t| t.lap_number == number)
    }
    pub fn current_lap_trace(&self) -> Option<&'a Arc<LapTrace>> {
        self.car.delta.current.as_ref()
    }
    /// Signed milliseconds; negative means ahead. State follows Lap Data freshness.
    pub fn live_delta(&self, reference: Reference) -> Readout<f32> {
        if matches!(reference, Reference::PersonalBest | Reference::Rival) {
            return Readout::Absent;
        }
        if self.car.delta.at_line {
            return self
                .completed_result(reference)
                .and_then(|r| r.delta_ms)
                .map_or(Readout::Absent, |delta| self.lap().map(|_| delta as f32));
        }
        let Some(trace) = self.lap_trace(reference) else {
            return Readout::Absent;
        };
        match self.lap() {
            Readout::Live(lap) => trace
                .time_at(lap.lap_distance)
                .map_or(Readout::Absent, |t| {
                    Readout::Live(lap.current_lap_time_in_ms as f32 - t)
                }),
            Readout::Stale(lap) => trace
                .time_at(lap.lap_distance)
                .map_or(Readout::Absent, |t| {
                    Readout::Stale(lap.current_lap_time_in_ms as f32 - t)
                }),
            _ => Readout::Absent,
        }
    }
}

impl SessionModel {
    pub(super) fn derive_delta(&mut self, datagram: &ParsedDatagram) {
        if let Packet::Lap(packet) = &datagram.packet {
            for (car, lap) in self.cars.iter_mut().zip(&packet.lap_data) {
                let Some(lap) = lap else { continue };
                if car.retired || matches!(lap.result_status, 0 | 1) {
                    continue;
                }
                let car = Arc::make_mut(car);
                let delta = &mut car.delta;
                delta.at_line = false;
                if delta
                    .current
                    .as_ref()
                    .is_none_or(|t| t.lap_number != lap.current_lap_num)
                {
                    if let Some(mut trace) = delta.current.take()
                        && trace.lap_number.checked_add(1) == Some(lap.current_lap_num)
                        && lap.last_lap_time_in_ms > 0
                    {
                        Arc::make_mut(&mut trace).finish_time = Some(
                            datagram.header.session_time
                                - lap.current_lap_time_in_ms as f32 / 1000.0,
                        );
                        delta.completed.push(trace);
                        delta.at_line = true;
                    }
                    delta.current = Some(Arc::new(LapTrace {
                        lap_number: lap.current_lap_num,
                        samples: vec![],
                        start_time: datagram.header.session_time
                            - lap.current_lap_time_in_ms as f32 / 1000.0,
                        finish_time: None,
                    }));
                    delta.last = None;
                }
                let distance = lap.lap_distance;
                if !distance.is_finite() || !(0.0..=100_000.0).contains(&distance) {
                    continue;
                }
                let telemetry = match car.telemetry.read(self.now, self.menu_interval, false) {
                    Readout::Live(t) => Some(t),
                    _ => None,
                };
                let point = LapSample {
                    distance_m: distance,
                    time_ms: lap.current_lap_time_in_ms as f32,
                    session_time: datagram.header.session_time,
                    speed_kph: telemetry.map(|t| f32::from(t.speed)),
                    throttle: telemetry.map(|t| t.throttle),
                    brake: telemetry.map(|t| t.brake),
                };
                let trace = Arc::make_mut(delta.current.as_mut().unwrap());
                if let Some(last) = &delta.last {
                    if distance <= last.distance_m || point.time_ms < last.time_ms {
                        continue;
                    }
                    if distance - last.distance_m <= 100.0 {
                        let mut bucket = (last.distance_m / 5.0).floor() * 5.0 + 5.0;
                        while bucket <= distance {
                            let f = (bucket - last.distance_m) / (distance - last.distance_m);
                            let lerp = |a: Option<f32>, b: Option<f32>| {
                                a.zip(b).map(|(a, b)| a + f * (b - a))
                            };
                            trace.samples.push(LapSample {
                                distance_m: bucket,
                                time_ms: last.time_ms + f * (point.time_ms - last.time_ms),
                                session_time: point.session_time,
                                speed_kph: lerp(last.speed_kph, point.speed_kph),
                                throttle: lerp(last.throttle, point.throttle),
                                brake: lerp(last.brake, point.brake),
                            });
                            bucket += 5.0;
                        }
                    } else if distance % 5.0 == 0.0 {
                        trace.samples.push(point.clone());
                    }
                } else if distance % 5.0 == 0.0 {
                    trace.samples.push(point.clone());
                }
                delta.last = Some(point);
            }
        }
        if matches!(datagram.packet, Packet::Lap(_) | Packet::SessionHistory(_)) {
            for car in &mut self.cars {
                let car = Arc::make_mut(car);
                let delta = &mut car.delta;
                let best = car.derived.best_lap_number;
                if best != delta.best {
                    delta.former_best = delta.best;
                    delta.best = best;
                }
                let mut laps = car.derived.laps.iter().rev();
                let recent = [laps.next(), laps.next()].map(|lap| lap.map(|l| l.lap_number));
                delta.completed.retain(|t| {
                    recent.contains(&Some(t.lap_number))
                        || Some(t.lap_number) == best
                        || Some(t.lap_number) == delta.former_best
                });
            }
        }
    }
}

impl DeltaState {
    pub(super) fn rewind(&mut self, time: f32, best: Option<u8>) {
        let restored = self
            .current
            .iter()
            .chain(self.completed.iter())
            .filter(|t| t.start_time <= time && t.finish_time.is_none_or(|end| end > time))
            .max_by_key(|t| t.lap_number)
            .cloned();
        self.completed
            .retain(|t| t.finish_time.is_some_and(|end| end <= time));
        self.current = restored.map(|mut t| {
            let trace = Arc::make_mut(&mut t);
            trace.samples.retain(|s| s.session_time <= time);
            trace.finish_time = None;
            t
        });
        self.last = self
            .current
            .as_ref()
            .and_then(|t| t.samples.last())
            .cloned();
        self.at_line = false;
        self.best = best;
        self.former_best = None;
    }
}
