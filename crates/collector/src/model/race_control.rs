use super::*;
use crate::telemetry::Event;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, ts_rs::TS)]
#[ts(rename = "RaceControlSource")]
pub enum Source {
    Game,
    Derived,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, ts_rs::TS)]
pub enum RaceControlKind {
    Game(Event),
    FlagChanged {
        previous: i8,
        flag: i8,
    },
    MarshalFlagChanged {
        zone: u8,
        previous: Option<i8>,
        flag: i8,
    },
    SafetyCarChanged {
        previous: u8,
        status: u8,
    },
    WeatherChanged {
        previous: u8,
        weather: u8,
    },
    PitEntry,
    PitExit,
    Flashback {
        from_time: f32,
        to_time: f32,
        event: Option<Event>,
    },
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, ts_rs::TS)]
pub struct RaceControlEntry {
    /// Session-local stable ID, retained when an entry is marked rewound.
    pub id: u64,
    pub session_time: f32,
    pub lap_number: Option<u8>,
    pub cars: Vec<usize>,
    pub source: Source,
    pub kind: RaceControlKind,
    pub rewound: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, ts_rs::TS)]
pub struct Penalty {
    /// ID of the corresponding race-control entry.
    pub id: u64,
    pub session_time: f32,
    pub lap_number: u8,
    pub penalty_type: u8,
    pub infringement_type: u8,
    pub other_car: Option<usize>,
    pub time_seconds: u8,
    pub places_gained: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, ts_rs::TS)]
pub struct PenaltyState {
    pub seconds: u8,
    pub total_warnings: u8,
    pub corner_cutting_warnings: u8,
    pub drive_throughs: u8,
    pub stop_gos: u8,
}

fn event_cars(event: &EventDetails) -> Vec<usize> {
    use EventDetails::*;
    let indices = match *event {
        FastestLap { vehicle_idx, .. }
        | Retirement { vehicle_idx, .. }
        | TeamMateInPits { vehicle_idx }
        | RaceWinner { vehicle_idx }
        | SpeedTrap { vehicle_idx, .. }
        | DriveThroughServed { vehicle_idx }
        | StopGoServed { vehicle_idx, .. } => [vehicle_idx, 255],
        Penalty {
            vehicle_idx,
            other_vehicle_idx,
            ..
        } => [vehicle_idx, other_vehicle_idx],
        Overtake {
            overtaking_vehicle_idx,
            being_overtaken_vehicle_idx,
        } => [overtaking_vehicle_idx, being_overtaken_vehicle_idx],
        Collision {
            vehicle1_idx,
            vehicle2_idx,
            ..
        } => [vehicle1_idx, vehicle2_idx],
        _ => [255, 255],
    };
    let mut cars = Vec::new();
    for index in indices {
        let index = usize::from(index);
        if index < MAX_CARS && !cars.contains(&index) {
            cars.push(index);
        }
    }
    cars
}

impl SessionModel {
    /// Arrival-ordered audit log, including entries abandoned by flashbacks.
    pub fn race_control(&self) -> &[RaceControlEntry] {
        &self.race_control
    }

    pub(super) fn log(
        &mut self,
        time: f32,
        cars: Vec<usize>,
        lap_number: Option<u8>,
        source: Source,
        kind: RaceControlKind,
    ) -> u64 {
        self.next_race_control_id += 1;
        let id = self.next_race_control_id;
        Arc::make_mut(&mut self.race_control).push(RaceControlEntry {
            id,
            session_time: time,
            lap_number,
            cars,
            source,
            kind,
            rewound: false,
        });
        id
    }

    pub(super) fn derive_race_control(&mut self, datagram: &ParsedDatagram) {
        let time = datagram.header.session_time;
        let mut changes = Vec::new();
        match &datagram.packet {
            Packet::Lap(packet) => {
                for (index, (car, value)) in self.cars.iter().zip(&packet.lap_data).enumerate() {
                    let Some(value) = value else { continue };
                    if car.retired || matches!(value.result_status, 0 | 1) {
                        continue;
                    }
                    let in_pit = value.pit_status != 0;
                    let was_in_pit = history::was_in_pit(car).unwrap_or(in_pit);
                    if was_in_pit != in_pit {
                        changes.push((
                            vec![index],
                            Some(value.current_lap_num),
                            if in_pit {
                                RaceControlKind::PitEntry
                            } else {
                                RaceControlKind::PitExit
                            },
                        ));
                    }
                }
            }
            Packet::CarStatus(packet) => {
                for (index, (car, value)) in
                    self.cars.iter().zip(&packet.car_status_data).enumerate()
                {
                    if !car.valid || car.retired {
                        continue;
                    }
                    if let (Some(old), Some(value)) = (car.status.value.as_deref(), value)
                        && old.vehicle_fia_flags != value.vehicle_fia_flags
                    {
                        changes.push((
                            vec![index],
                            car.lap.value.as_ref().map(|lap| lap.current_lap_num),
                            RaceControlKind::FlagChanged {
                                previous: old.vehicle_fia_flags,
                                flag: value.vehicle_fia_flags,
                            },
                        ));
                    }
                }
            }
            Packet::Session(packet) => {
                if let Some(old) = self.session.value.as_deref() {
                    if old.weather != packet.weather {
                        changes.push((
                            vec![],
                            None,
                            RaceControlKind::WeatherChanged {
                                previous: old.weather,
                                weather: packet.weather,
                            },
                        ));
                    }
                    if old.safety_car_status != packet.safety_car_status {
                        changes.push((
                            vec![],
                            None,
                            RaceControlKind::SafetyCarChanged {
                                previous: old.safety_car_status,
                                status: packet.safety_car_status,
                            },
                        ));
                    }
                    for (zone, value) in packet
                        .marshal_zones
                        .iter()
                        .take(usize::from(packet.num_marshal_zones))
                        .enumerate()
                    {
                        let previous = (zone < usize::from(old.num_marshal_zones))
                            .then(|| old.marshal_zones[zone].zone_flag);
                        if previous != Some(value.zone_flag) {
                            changes.push((
                                vec![],
                                None,
                                RaceControlKind::MarshalFlagChanged {
                                    zone: zone as u8,
                                    previous,
                                    flag: value.zone_flag,
                                },
                            ));
                        }
                    }
                }
            }
            Packet::Event(event) => {
                if matches!(event.details, EventDetails::Flashback { .. }) {
                    return;
                }
                let cars = event_cars(&event.details);
                let lap_number = match event.details {
                    EventDetails::Penalty { lap_num, .. } => Some(lap_num),
                    _ => cars
                        .first()
                        .copied()
                        .or_else(|| self.player_car_index())
                        .and_then(|i| self.cars[i].lap.value.as_ref())
                        .map(|lap| lap.current_lap_num),
                };
                let id = self.log(
                    time,
                    cars,
                    lap_number,
                    Source::Game,
                    RaceControlKind::Game((**event).clone()),
                );
                if let EventDetails::Penalty {
                    vehicle_idx,
                    other_vehicle_idx,
                    lap_num,
                    penalty_type,
                    infringement_type,
                    time: seconds,
                    places_gained,
                } = event.details
                    && let Some(car) = self.cars.get_mut(usize::from(vehicle_idx))
                {
                    Arc::make_mut(&mut Arc::make_mut(car).derived)
                        .penalties
                        .push(Penalty {
                            id,
                            session_time: time,
                            lap_number: lap_num,
                            penalty_type,
                            infringement_type,
                            other_car: (usize::from(other_vehicle_idx) < MAX_CARS)
                                .then_some(usize::from(other_vehicle_idx)),
                            time_seconds: seconds,
                            places_gained,
                        });
                }
            }
            _ => {}
        }
        let player_lap = self
            .player_car_index()
            .and_then(|i| self.cars[i].lap.value.as_ref())
            .map(|lap| lap.current_lap_num);
        for (cars, lap, kind) in changes {
            self.log(time, cars, lap.or(player_lap), Source::Derived, kind);
        }
    }
}

impl CarView<'_> {
    pub fn penalty_state(&self) -> Readout<PenaltyState> {
        self.lap().map(|lap| PenaltyState {
            seconds: lap.penalties,
            total_warnings: lap.total_warnings,
            corner_cutting_warnings: lap.corner_cutting_warnings,
            drive_throughs: lap.num_unserved_drive_through_pens,
            stop_gos: lap.num_unserved_stop_go_pens,
        })
    }
}
