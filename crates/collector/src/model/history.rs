//! Session-long summaries. Completed laps are immutable and shared by checkpoints.
use super::*;

#[derive(Debug, Clone, PartialEq, serde::Serialize, ts_rs::TS)]
pub struct CompletedLap {
    pub lap_number: u8,
    /// Finish time when observed live; first observation time for history-only laps.
    pub session_time: f32,
    pub lap_time_ms: u32,
    pub sectors_ms: [Option<u32>; 3],
    pub valid: bool,
    pub sector_valid: [bool; 3],
    pub authoritative: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, ts_rs::TS)]
pub struct Sector {
    pub lap_number: u8,
    /// Zero-based sector index.
    pub sector: u8,
    pub session_time: f32,
    pub time_ms: u32,
    pub valid: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, ts_rs::TS)]
pub struct Stint {
    pub id: u64,
    pub session_time: f32,
    pub start_lap: u8,
    pub end_time: Option<f32>,
    pub end_lap: Option<u8>,
    pub actual_compound: u8,
    pub visual_compound: u8,
    pub starting_age: Option<u8>,
    pub tyre_age_laps: Option<u8>,
    /// Last observed condition on this set, not an estimate for a new set.
    pub tyre_condition: Readout<[f32; 4]>,
    pub authoritative: bool,
    #[serde(skip)]
    age_updated_at: f32,
    #[serde(skip)]
    condition_updated_at: f32,
}

impl Stint {
    pub fn used_set(&self) -> Option<bool> {
        self.starting_age.map(|age| age > 0)
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, ts_rs::TS)]
pub struct PitStop {
    pub id: u64,
    pub session_time: f32,
    pub lap_number: u8,
    pub exit_time: Option<f32>,
    pub lane_time_ms: Option<u32>,
    pub stopped_time_ms: Option<u32>,
    #[serde(skip)]
    timing_updated_at: f32,
}

pub(super) fn was_in_pit(car: &Car) -> bool {
    car.lap.value.as_ref().map_or_else(
        || {
            car.derived
                .pit_stops
                .last()
                .is_some_and(|stop| stop.exit_time.is_none())
        },
        |lap| lap.pit_status != 0,
    )
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct CarHistory {
    pub laps: Vec<Arc<CompletedLap>>,
    pub sectors: Vec<Sector>,
    pub best_lap_number: Option<u8>,
    pub best_sectors_ms: [Option<u32>; 3],
    pub stints: Vec<Stint>,
    pub pit_stops: Vec<PitStop>,
    pub penalties: Vec<Penalty>,
    next_id: u64,
}

fn millis(part: u16, minutes: u8) -> Option<u32> {
    let value = u32::from(minutes) * 60_000 + u32::from(part);
    (value > 0).then_some(value)
}

impl CarHistory {
    fn next_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }
    pub fn best_lap(&self) -> Option<&CompletedLap> {
        self.laps
            .iter()
            .find(|lap| Some(lap.lap_number) == self.best_lap_number)
            .map(Arc::as_ref)
    }

    pub(super) fn recompute_bests(&mut self) {
        self.best_lap_number = self
            .laps
            .iter()
            .filter(|lap| lap.valid)
            .min_by_key(|lap| (lap.lap_time_ms, lap.lap_number))
            .map(|lap| lap.lap_number);
        self.best_sectors_ms = std::array::from_fn(|sector| {
            self.sectors
                .iter()
                .filter(|value| usize::from(value.sector) == sector && value.valid)
                .map(|value| value.time_ms)
                .min()
        });
    }

    fn put_lap(&mut self, lap: CompletedLap) {
        let number = lap.lap_number;
        // Replace by lap number so late authoritative corrections keep identity.
        if let Some(old) = self.laps.iter_mut().find(|old| old.lap_number == number) {
            if **old != lap {
                *old = Arc::new(lap);
            }
        } else {
            self.laps.push(Arc::new(lap));
            self.laps.sort_by_key(|lap| lap.lap_number);
        }
        self.recompute_bests();
    }

    fn put_sector(&mut self, sector: Sector) {
        if let Some(old) = self
            .sectors
            .iter_mut()
            .find(|old| old.lap_number == sector.lap_number && old.sector == sector.sector)
        {
            *old = sector;
        } else {
            self.sectors.push(sector);
        }
    }

    fn correct_stints(&mut self, packet: &PacketSessionHistoryData, time: f32) {
        if packet.num_tyre_stints == 0 {
            return;
        }
        let old = std::mem::take(&mut self.stints);
        let mut start = 1;
        for value in packet
            .tyre_stints_history_data
            .iter()
            .take(usize::from(packet.num_tyre_stints))
        {
            let end = (value.end_lap != 255).then_some(value.end_lap);
            if end.is_some_and(|end| end < start) {
                continue;
            }
            let matched = old.iter().find(|stint| {
                stint.start_lap == start && stint.actual_compound == value.tyre_actual_compound
            });
            let start_time = matched.map_or_else(
                || {
                    self.laps
                        .iter()
                        .find(|lap| lap.lap_number == start.saturating_sub(1))
                        .map_or(time, |lap| lap.session_time)
                },
                |stint| stint.session_time,
            );
            let end_time = end.map(|number| {
                self.laps
                    .iter()
                    .find(|lap| lap.lap_number == number)
                    .map_or(time, |lap| lap.session_time)
            });
            let id = matched.map_or_else(|| self.next_id(), |stint| stint.id);
            self.stints.push(Stint {
                id,
                session_time: start_time,
                start_lap: start,
                end_time,
                end_lap: end,
                actual_compound: value.tyre_actual_compound,
                visual_compound: value.tyre_visual_compound,
                starting_age: matched.and_then(|stint| stint.starting_age),
                tyre_age_laps: matched.and_then(|stint| stint.tyre_age_laps),
                authoritative: true,
                tyre_condition: matched.map_or(Readout::Absent, |stint| stint.tyre_condition),
                age_updated_at: matched.map_or(time, |stint| stint.age_updated_at),
                condition_updated_at: matched.map_or(time, |stint| stint.condition_updated_at),
            });
            if let Some(end) = end {
                start = end.saturating_add(1);
            } else {
                break;
            }
        }
    }
}

impl SessionModel {
    pub(super) fn derive_history(&mut self, datagram: &ParsedDatagram) {
        let time = datagram.header.session_time;
        if let Packet::Participants(packet) = &datagram.packet {
            for (car, value) in self
                .cars
                .iter_mut()
                .zip(&packet.participants)
                .take(usize::from(packet.num_active_cars))
            {
                if value.as_ref().is_some_and(|p| p.your_telemetry != 1)
                    && car
                        .derived
                        .stints
                        .iter()
                        .any(|stint| stint.tyre_condition != Readout::Restricted)
                {
                    for stint in &mut Arc::make_mut(&mut Arc::make_mut(car).derived).stints {
                        stint.tyre_condition = Readout::Restricted;
                        stint.condition_updated_at = time;
                    }
                }
            }
        }
        if let Packet::CarDamage(packet) = &datagram.packet {
            for (car, value) in self.cars.iter_mut().zip(&packet.car_damage_data) {
                if !car.retired
                    && car.public_telemetry()
                    && let Some(value) = value
                    && car
                        .derived
                        .stints
                        .last()
                        .is_some_and(|stint| stint.end_lap.is_none())
                {
                    let history = Arc::make_mut(&mut Arc::make_mut(car).derived);
                    let stint = history.stints.last_mut().unwrap();
                    stint.tyre_condition = Readout::Live(value.tyres_wear.map(|wear| 100.0 - wear));
                    stint.condition_updated_at = time;
                }
            }
        }
        if let Packet::CarStatus(packet) = &datagram.packet {
            for (car, value) in self.cars.iter_mut().zip(&packet.car_status_data) {
                let Some(value) = value else { continue };
                if car.retired {
                    continue;
                }
                let number = car
                    .lap
                    .value
                    .as_ref()
                    .map_or(1, |lap| lap.current_lap_num.max(1));
                let changed = car.derived.stints.last().is_none_or(|stint| {
                    stint.end_lap.is_some()
                        || stint.actual_compound != value.actual_tyre_compound
                        || stint.visual_compound != value.visual_tyre_compound
                        || stint
                            .tyre_age_laps
                            .is_some_and(|age| value.tyres_age_laps < age)
                });
                let age_changed = car
                    .derived
                    .stints
                    .last()
                    .is_some_and(|stint| stint.tyre_age_laps != Some(value.tyres_age_laps));
                if !changed && !age_changed {
                    continue;
                }
                let condition = if car.damage.restricted {
                    Readout::Restricted
                } else {
                    Readout::Absent
                };
                let history = Arc::make_mut(&mut Arc::make_mut(car).derived);
                if changed {
                    if let Some(stint) = history.stints.last_mut()
                        && stint.end_lap.is_none()
                    {
                        stint.end_lap = Some(number.saturating_sub(1));
                        stint.end_time = Some(time);
                    }
                    let id = history.next_id();
                    history.stints.push(Stint {
                        id,
                        session_time: time,
                        start_lap: number,
                        end_time: None,
                        end_lap: None,
                        actual_compound: value.actual_tyre_compound,
                        visual_compound: value.visual_tyre_compound,
                        starting_age: Some(value.tyres_age_laps),
                        tyre_age_laps: Some(value.tyres_age_laps),
                        authoritative: false,
                        age_updated_at: time,
                        tyre_condition: condition,
                        condition_updated_at: time,
                    });
                } else if let Some(stint) = history.stints.last_mut() {
                    stint.tyre_age_laps = Some(value.tyres_age_laps);
                    stint.age_updated_at = time;
                }
            }
        }
        if let Packet::SessionHistory(packet) = &datagram.packet {
            let Some(car) = self.cars.get_mut(usize::from(packet.car_idx)) else {
                return;
            };
            let current_number = car.lap.value.as_ref().map(|lap| lap.current_lap_num);
            let history = Arc::make_mut(&mut Arc::make_mut(car).derived);
            for (index, value) in packet
                .lap_history_data
                .iter()
                .take(usize::from(packet.num_laps))
                .enumerate()
            {
                let number = index as u8 + 1;
                let known_end = history
                    .laps
                    .iter()
                    .find(|lap| lap.lap_number == number)
                    .map(|lap| lap.session_time);
                let end = known_end.unwrap_or(time);
                let sectors_ms = [
                    millis(value.sector1_time_ms_part, value.sector1_time_minutes_part),
                    millis(value.sector2_time_ms_part, value.sector2_time_minutes_part),
                    millis(value.sector3_time_ms_part, value.sector3_time_minutes_part),
                ];
                let completed = value.lap_time_in_ms > 0
                    && (known_end.is_some()
                        || current_number.is_some_and(|current| current > number)
                        || sectors_ms[2].is_some());
                let sector_valid =
                    std::array::from_fn(|s| value.lap_valid_bit_flags & (2 << s) != 0);
                history.sectors.retain(|sector| {
                    sector.lap_number != number || sectors_ms[usize::from(sector.sector)].is_some()
                });
                for (sector, duration) in sectors_ms.into_iter().enumerate() {
                    if let Some(time_ms) = duration {
                        let remaining: u32 = sectors_ms[sector + 1..].iter().flatten().sum();
                        let observed = history
                            .sectors
                            .iter()
                            .find(|old| {
                                old.lap_number == number && usize::from(old.sector) == sector
                            })
                            .map(|old| old.session_time);
                        history.put_sector(Sector {
                            lap_number: number,
                            sector: sector as u8,
                            session_time: observed.unwrap_or_else(|| {
                                if known_end.is_some() {
                                    end - remaining as f32 / 1000.0
                                } else {
                                    time
                                }
                            }),
                            time_ms,
                            valid: sector_valid[sector],
                        });
                    }
                }
                if completed {
                    history.put_lap(CompletedLap {
                        lap_number: number,
                        session_time: end,
                        lap_time_ms: value.lap_time_in_ms,
                        sectors_ms,
                        valid: value.lap_valid_bit_flags & 1 != 0,
                        sector_valid,
                        authoritative: true,
                    });
                }
            }
            history.recompute_bests();
            history.correct_stints(packet, time);
        }
        if let Packet::Lap(packet) = &datagram.packet {
            for (car, value) in self.cars.iter_mut().zip(&packet.lap_data) {
                let Some(value) = value else { continue };
                if car.retired || matches!(value.result_status, 0 | 1) {
                    continue;
                }
                let previous = car.lap.value.clone();
                let completed = previous.as_deref().filter(|old| {
                    value.current_lap_num == old.current_lap_num.saturating_add(1)
                        && old.current_lap_num > 0
                        && value.last_lap_time_in_ms > 0
                });
                let sector_times = [
                    millis(value.sector1_time_ms_part, value.sector1_time_minutes_part),
                    millis(value.sector2_time_ms_part, value.sector2_time_minutes_part),
                ];
                let in_pit = value.pit_status != 0;
                let was_in_pit = was_in_pit(car);
                let timing_changed = in_pit
                    && car.derived.pit_stops.last().is_some_and(|stop| {
                        (value.pit_lane_time_in_lane_in_ms > 0
                            && stop.lane_time_ms
                                != Some(u32::from(value.pit_lane_time_in_lane_in_ms)))
                            || (value.pit_stop_timer_in_ms > 0
                                && stop.stopped_time_ms
                                    != Some(u32::from(value.pit_stop_timer_in_ms)))
                    });
                if completed.is_none()
                    && sector_times.iter().all(Option::is_none)
                    && in_pit == was_in_pit
                    && !timing_changed
                {
                    continue;
                }
                let history = Arc::make_mut(&mut Arc::make_mut(car).derived);
                if in_pit && !was_in_pit {
                    let id = history.next_id();
                    history.pit_stops.push(PitStop {
                        id,
                        session_time: time,
                        lap_number: value.current_lap_num,
                        exit_time: None,
                        lane_time_ms: None,
                        stopped_time_ms: None,
                        timing_updated_at: time,
                    });
                } else if !in_pit
                    && was_in_pit
                    && let Some(stop) = history.pit_stops.last_mut()
                {
                    stop.exit_time = Some(time);
                }
                if (in_pit || was_in_pit)
                    && let Some(stop) = history.pit_stops.last_mut()
                {
                    if value.pit_lane_time_in_lane_in_ms > 0 {
                        stop.lane_time_ms = Some(u32::from(value.pit_lane_time_in_lane_in_ms));
                        stop.timing_updated_at = time;
                    }
                    if value.pit_stop_timer_in_ms > 0 {
                        stop.stopped_time_ms = Some(u32::from(value.pit_stop_timer_in_ms));
                        stop.timing_updated_at = time;
                    }
                }
                if let Some(old) = completed.filter(|old| {
                    !history
                        .laps
                        .iter()
                        .any(|lap| lap.lap_number == old.current_lap_num && lap.authoritative)
                }) {
                    let s1 = millis(old.sector1_time_ms_part, old.sector1_time_minutes_part);
                    let s2 = millis(old.sector2_time_ms_part, old.sector2_time_minutes_part);
                    let s3 = s1
                        .zip(s2)
                        .and_then(|(a, b)| value.last_lap_time_in_ms.checked_sub(a + b))
                        .filter(|v| *v > 0);
                    let finish = time - value.current_lap_time_in_ms as f32 / 1000.0;
                    for (sector, duration) in [s1, s2, s3].into_iter().enumerate() {
                        if let Some(time_ms) = duration {
                            let end = match sector {
                                0 => {
                                    finish
                                        - value.last_lap_time_in_ms.saturating_sub(time_ms) as f32
                                            / 1000.0
                                }
                                1 => finish - s3.unwrap_or(0) as f32 / 1000.0,
                                _ => finish,
                            };
                            history.put_sector(Sector {
                                lap_number: old.current_lap_num,
                                sector: sector as u8,
                                session_time: end,
                                time_ms,
                                valid: old.current_lap_invalid == 0,
                            });
                        }
                    }
                    history.put_lap(CompletedLap {
                        lap_number: old.current_lap_num,
                        session_time: finish,
                        lap_time_ms: value.last_lap_time_in_ms,
                        sectors_ms: [s1, s2, s3],
                        valid: old.current_lap_invalid == 0,
                        sector_valid: [old.current_lap_invalid == 0; 3],
                        authoritative: false,
                    });
                }
                for (sector, duration) in sector_times.into_iter().enumerate() {
                    if usize::from(value.sector) > sector
                        && let Some(time_ms) = duration
                    {
                        // The packet's current time gives a deterministic sector endpoint.
                        let elapsed =
                            sector_times[0].unwrap_or(0) + if sector == 1 { time_ms } else { 0 };
                        history.put_sector(Sector {
                            lap_number: value.current_lap_num,
                            sector: sector as u8,
                            session_time: time - value.current_lap_time_in_ms as f32 / 1000.0
                                + elapsed as f32 / 1000.0,
                            time_ms,
                            valid: value.current_lap_invalid == 0,
                        });
                    }
                }
                history.recompute_bests();
            }
        }
    }
}

impl CarHistory {
    pub(super) fn rewind(&mut self, time: f32) {
        self.laps.retain(|lap| lap.session_time <= time);
        self.sectors.retain(|sector| sector.session_time <= time);
        self.stints.retain(|stint| stint.session_time <= time);
        for stint in &mut self.stints {
            if stint.end_time.is_some_and(|end| end > time) {
                stint.end_time = None;
                stint.end_lap = None;
            }
            if stint.age_updated_at > time {
                stint.tyre_age_laps = None;
            }
            if stint.condition_updated_at > time && stint.tyre_condition != Readout::Restricted {
                stint.tyre_condition = Readout::Absent;
            }
        }
        self.pit_stops.retain(|stop| stop.session_time <= time);
        for stop in &mut self.pit_stops {
            if stop.exit_time.is_some_and(|end| end > time) {
                stop.exit_time = None;
            }
            if stop.timing_updated_at > time {
                stop.lane_time_ms = None;
                stop.stopped_time_ms = None;
            }
        }
        self.penalties
            .retain(|penalty| penalty.session_time <= time);
        self.recompute_bests();
    }
}

impl<'a> CarView<'a> {
    /// Full-session summaries, independent of the one-second Session History packet.
    pub fn session_history(&self) -> &'a CarHistory {
        &self.car.derived
    }
}
