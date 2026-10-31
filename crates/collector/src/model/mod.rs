//! Pure session state driven by parsed datagrams and explicit monotonic time.
//!
//! The caller handles session routing/closed identities and frame-drop gating.
//! Arrival times use the recording's time origin; no wall clock is read here.
//! `apply` receives a `telemetry::ParsedDatagram`; `advance` carries silence.
//! Both inputs use monotonic `Duration` timestamps, independent of session time.
//!
//! Read a valid slot with `car(index)`. Its typed readouts borrow model storage;
//! restricted fields are discarded, and unknown permission leaves them absent.
//! Session History remains available after retirement and session end for the
//! derived-history layer. Model clones share cars and packet groups through
//! copy-on-write `Arc`s; advancing or applying to a clone leaves others intact.
//!
//! `CarView::session_history` exposes full-session summaries, while `history`
//! retains the latest raw Session History packet. Completed summaries are shared
//! `Arc`s. `gaps`, `penalty_state`, `ers_percentage` and `tyre_condition` preserve
//! their source readout states. `race_control` is an arrival-ordered audit log;
//! rewinds mark abandoned entries and bump `epoch` before accepting new facts.
//!
//! `CarView` exposes named lap references, live and sector deltas, and the held
//! official `completed_result`. `lap_trace` and `current_lap_trace` borrow shared
//! 5 m traces with elapsed time, speed, throttle and brake. Missing telemetry
//! channels remain `None`; Time Trial extras provide summary comparisons only.
mod delta;
pub use delta::{LapResult, LapSample, LapTrace, Reference};
mod facts;
mod history;
mod race_control;
mod rewind;
mod status;
use crate::telemetry::{
    CarDamageData, CarMotionData, CarTelemetry2Data, CarTelemetryData, EventDetails, LapData,
    MAX_CARS, Packet, PacketFinalClassificationData, PacketHeader, PacketSessionData,
    PacketSessionHistoryData, PacketTyreSetsData, ParsedDatagram, ParticipantData,
    WeatherForecastSample,
};
pub use facts::Gaps;
pub use history::{CarHistory, CompletedLap, PitStop, Sector, Stint};
pub use race_control::{Penalty, PenaltyState, RaceControlEntry, RaceControlKind, Source};
pub use status::{ErsData, FuelData, StatusData};
use std::{sync::Arc, time::Duration};

/// Missing and withheld data never carry a numeric placeholder.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Readout<T> {
    Live(T),
    Stale(T),
    Restricted,
    Absent,
}

impl<T> Readout<T> {
    pub fn map<U>(self, map: impl FnOnce(T) -> U) -> Readout<U> {
        match self {
            Self::Live(value) => Readout::Live(map(value)),
            Self::Stale(value) => Readout::Stale(map(value)),
            Self::Restricted => Readout::Restricted,
            Self::Absent => Readout::Absent,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Regulations {
    Pre2026,
    Regulations2026,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PitWindow {
    pub car_index: usize,
    pub ideal_lap: u8,
    pub latest_lap: u8,
    pub rejoin_position: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Classification {
    /// Header session time, retained exactly as sent (not arrival time).
    pub session_time: f32,
    pub data: PacketFinalClassificationData,
}

#[derive(Debug, Clone, PartialEq)]
struct Group<T> {
    value: Option<Arc<T>>,
    arrival: Duration,
    session_time: f32,
    restricted: bool,
}

impl<T> Default for Group<T> {
    fn default() -> Self {
        Self {
            value: None,
            arrival: Duration::ZERO,
            session_time: 0.0,
            restricted: false,
        }
    }
}

impl<T: Clone> Group<T> {
    fn set(&mut self, time: Duration, session_time: f32, value: &T) {
        self.value = Some(Arc::new(value.clone()));
        self.arrival = time;
        self.session_time = session_time;
        self.restricted = false;
    }
    fn permission(&mut self, restricted: bool) {
        if restricted {
            self.value = None;
        }
        self.restricted = restricted;
    }
    fn read(&self, now: Duration, interval: Duration, lost: bool) -> Readout<&T> {
        if self.restricted {
            return Readout::Restricted;
        }
        match self.value.as_deref() {
            None => Readout::Absent,
            Some(value)
                if lost || now.saturating_sub(self.arrival) >= interval.saturating_mul(3) =>
            {
                Readout::Stale(value)
            }
            Some(value) => Readout::Live(value),
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
struct Car {
    delta: delta::DeltaState,
    derived: Arc<CarHistory>,
    valid: bool,
    valid_since: Option<f32>,
    retired: bool,
    retired_at: Option<f32>,
    retirement_reason: Option<u8>,
    participant: Group<ParticipantData>,
    lap: Group<LapData>,
    motion: Group<CarMotionData>,
    telemetry: Group<CarTelemetryData>,
    aero: Group<CarTelemetry2Data>,
    history: Group<PacketSessionHistoryData>,
    status: Group<StatusData>,
    fuel: Group<FuelData>,
    ers: Group<ErsData>,
    front_brake_bias: Group<u8>,
    damage: Group<CarDamageData>,
    tyre_sets: Group<PacketTyreSetsData>,
}

impl Car {
    fn public_telemetry(&self) -> bool {
        self.participant
            .value
            .as_ref()
            .is_some_and(|p| p.your_telemetry == 1)
    }
    fn permission(&mut self, restricted: bool) {
        self.fuel.permission(restricted);
        self.ers.permission(restricted);
        self.front_brake_bias.permission(restricted);
        self.damage.permission(restricted);
        self.tyre_sets.permission(restricted);
    }
}

/// A valid vehicle-index slot, viewed at the model's current input time.
#[derive(Debug, Clone, Copy)]
pub struct CarView<'a> {
    car: &'a Car,
    model: &'a SessionModel,
}

impl<'a> CarView<'a> {
    pub fn regulations(&self) -> Option<Regulations> {
        if self.model.identity?.format == 2025 {
            return Some(Regulations::Pre2026);
        }
        self.car.aero.value.as_ref().map(|aero| {
            if aero.regulations_2026 == 0 {
                Regulations::Pre2026
            } else {
                Regulations::Regulations2026
            }
        })
    }
    pub fn aero(&self) -> Readout<&'a CarTelemetry2Data> {
        self.car.aero.read(
            self.model.now,
            self.model.menu_interval,
            self.model.lifecycle == Lifecycle::Lost,
        )
    }
    pub fn retired(&self) -> bool {
        self.car.retired
    }
    pub fn retirement_reason(&self) -> Option<u8> {
        self.car.retirement_reason
    }
    pub fn is_my_team(&self) -> bool {
        self.car
            .participant
            .value
            .as_ref()
            .is_some_and(|p| p.my_team != 0)
    }
    pub fn participant(&self) -> Readout<&'a ParticipantData> {
        self.car.participant.read(
            self.model.now,
            Duration::from_secs(5),
            self.model.lifecycle == Lifecycle::Lost,
        )
    }
    pub fn lap(&self) -> Readout<&'a LapData> {
        self.car.lap.read(
            self.model.now,
            self.model.menu_interval,
            self.model.lifecycle == Lifecycle::Lost,
        )
    }
    pub fn motion(&self) -> Readout<&'a CarMotionData> {
        self.car.motion.read(
            self.model.now,
            self.model.menu_interval,
            self.model.lifecycle == Lifecycle::Lost,
        )
    }
    pub fn telemetry(&self) -> Readout<&'a CarTelemetryData> {
        self.car.telemetry.read(
            self.model.now,
            self.model.menu_interval,
            self.model.lifecycle == Lifecycle::Lost,
        )
    }
    pub fn history(&self) -> Readout<&'a PacketSessionHistoryData> {
        self.car.history.read(
            self.model.now,
            Duration::from_secs(1),
            self.model.lifecycle == Lifecycle::Lost,
        )
    }
    pub fn status(&self) -> Readout<&'a StatusData> {
        self.car.status.read(
            self.model.now,
            self.model.menu_interval,
            self.model.lifecycle == Lifecycle::Lost,
        )
    }
    pub fn fuel(&self) -> Readout<&'a FuelData> {
        self.car.fuel.read(
            self.model.now,
            self.model.menu_interval,
            self.model.lifecycle == Lifecycle::Lost,
        )
    }
    pub fn ers(&self) -> Readout<&'a ErsData> {
        self.car.ers.read(
            self.model.now,
            self.model.menu_interval,
            self.model.lifecycle == Lifecycle::Lost,
        )
    }
    pub fn front_brake_bias(&self) -> Readout<&'a u8> {
        self.car.front_brake_bias.read(
            self.model.now,
            self.model.menu_interval,
            self.model.lifecycle == Lifecycle::Lost,
        )
    }
    pub fn damage(&self) -> Readout<&'a CarDamageData> {
        self.car.damage.read(
            self.model.now,
            Duration::from_millis(100),
            self.model.lifecycle == Lifecycle::Lost,
        )
    }
    pub fn tyre_sets(&self) -> Readout<&'a PacketTyreSetsData> {
        self.car.tyre_sets.read(
            self.model.now,
            Duration::from_secs(1),
            self.model.lifecycle == Lifecycle::Lost,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionId {
    pub uid: u64,
    pub format: u16,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Lifecycle {
    #[default]
    Waiting,
    Live,
    Ended,
    Lost,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SessionModel {
    epoch: u64,
    last_session_time: Option<f32>,
    race_control: Arc<Vec<RaceControlEntry>>,
    next_race_control_id: u64,
    identity: Option<SessionId>,
    lifecycle: Lifecycle,
    now: Duration,
    last_arrival: Option<Duration>,
    cars: [Arc<Car>; MAX_CARS],
    menu_interval: Duration,
    header: Option<PacketHeader>,
    session: Group<PacketSessionData>,
    pit_window: Group<PitWindow>,
    classification: Option<Arc<Classification>>,
    time_trial: Group<crate::telemetry::PacketTimeTrialData>,
}

impl Default for SessionModel {
    fn default() -> Self {
        Self {
            epoch: 0,
            last_session_time: None,
            race_control: Arc::default(),
            next_race_control_id: 0,
            identity: None,
            lifecycle: Lifecycle::Waiting,
            now: Duration::ZERO,
            last_arrival: None,
            cars: std::array::from_fn(|_| Arc::new(Car::default())),
            menu_interval: Duration::from_secs(1) / 60,
            header: None,
            session: Group::default(),
            pit_window: Group::default(),
            classification: None,
            time_trial: Group::default(),
        }
    }
}

impl SessionModel {
    /// Expected menu-rate interval (20–60 Hz); defaults to 60 Hz.
    /// The ingest caller supplies its configured or measured rate.
    pub fn new(menu_interval: Duration) -> Self {
        assert!(!menu_interval.is_zero(), "menu interval must be non-zero");
        Self {
            menu_interval,
            ..Self::default()
        }
    }
    pub fn identity(&self) -> Option<SessionId> {
        self.identity
    }
    /// Changes on a new session or a flashback. Transport/replay owns other resets.
    pub fn epoch(&self) -> u64 {
        self.epoch
    }
    pub fn lifecycle(&self) -> Lifecycle {
        self.lifecycle
    }
    pub fn final_classification(&self) -> Option<&Classification> {
        self.classification.as_deref()
    }
    pub fn car(&self, index: usize) -> Option<CarView<'_>> {
        let car = self.cars.get(index)?;
        car.valid.then_some(CarView { car, model: self })
    }
    pub fn player_car_index(&self) -> Option<usize> {
        let index = match self.session.value.as_deref() {
            Some(session) if session.is_spectating != 0 => session.spectator_car_index,
            _ => self.header.as_ref()?.player_car_index,
        };
        (usize::from(index) < MAX_CARS).then_some(usize::from(index))
    }
    /// Latest Session packet. Use `pit_window(car_index)` for a car-bound view.
    pub fn session(&self) -> Readout<&PacketSessionData> {
        self.session.read(
            self.now,
            Duration::from_millis(500),
            self.lifecycle == Lifecycle::Lost,
        )
    }
    pub fn forecast(&self) -> Readout<&[WeatherForecastSample]> {
        self.session().map(|session| {
            &session.weather_forecast_samples
                [..usize::from(session.num_weather_forecast_samples).min(64)]
        })
    }
    pub fn pit_window(&self, car_index: usize) -> Readout<&PitWindow> {
        if self.player_car_index() != Some(car_index)
            || self
                .pit_window
                .value
                .as_ref()
                .is_none_or(|window| window.car_index != car_index)
        {
            return Readout::Absent;
        }
        self.pit_window.read(
            self.now,
            Duration::from_millis(500),
            self.lifecycle == Lifecycle::Lost,
        )
    }

    /// Apply an already parsed, accepted datagram. UID 0 is ignored defensively.
    pub fn apply(&mut self, arrival_time: Duration, datagram: &ParsedDatagram) {
        if datagram.header.session_uid == 0 {
            return;
        }
        let identity = SessionId {
            uid: datagram.header.session_uid,
            format: datagram.header.packet_format,
        };
        if self.identity != Some(identity) {
            *self = Self {
                epoch: self.epoch.wrapping_add(1),
                identity: Some(identity),
                lifecycle: Lifecycle::Live,
                menu_interval: self.menu_interval,
                ..Self::default()
            };
        }
        self.advance(arrival_time);
        if self.lifecycle == Lifecycle::Ended
            && !matches!(
                datagram.packet,
                Packet::SessionHistory(_) | Packet::FinalClassification(_)
            )
        {
            return;
        }
        self.last_arrival = Some(self.now);
        self.header = Some(datagram.header.clone());
        if self.lifecycle != Lifecycle::Ended {
            self.lifecycle = Lifecycle::Live;
        }
        self.rewind_for(datagram);
        self.derive_race_control(datagram);
        self.derive_history(datagram);
        match &datagram.packet {
            Packet::TimeTrial(packet) => {
                self.time_trial
                    .set(self.now, datagram.header.session_time, packet)
            }
            Packet::FinalClassification(packet) => {
                self.classification = Some(Arc::new(Classification {
                    session_time: datagram.header.session_time,
                    data: (**packet).clone(),
                }));
                for (car, result) in self
                    .cars
                    .iter_mut()
                    .zip(&packet.classification_data)
                    .take(usize::from(packet.num_cars))
                {
                    if !car.retired
                        && let Some(result) = result
                    {
                        let car = Arc::make_mut(car);
                        if !car.valid && !matches!(result.result_status, 0 | 1) {
                            car.valid_since = Some(datagram.header.session_time);
                        }
                        car.valid = !matches!(result.result_status, 0 | 1);
                        car.retired = matches!(result.result_status, 4 | 7);
                        if car.retired {
                            car.retired_at = Some(datagram.header.session_time);
                        }
                    }
                }
            }
            Packet::Session(packet) => {
                self.session
                    .set(self.now, datagram.header.session_time, packet);
                self.pit_window = Group::default();
                if let Some(car_index) = self.player_car_index()
                    && packet.pit_stop_window_ideal_lap != 0
                    && packet.pit_stop_window_latest_lap != 0
                {
                    self.pit_window.set(
                        self.now,
                        datagram.header.session_time,
                        &PitWindow {
                            car_index,
                            ideal_lap: packet.pit_stop_window_ideal_lap,
                            latest_lap: packet.pit_stop_window_latest_lap,
                            rejoin_position: packet.pit_stop_rejoin_position,
                        },
                    );
                }
            }
            Packet::CarStatus(packet) => {
                for (car, value) in self.cars.iter_mut().zip(&packet.car_status_data) {
                    if !car.retired
                        && let Some(value) = value
                    {
                        let car = Arc::make_mut(car);
                        car.status.set(
                            self.now,
                            datagram.header.session_time,
                            &StatusData::from(value),
                        );
                        if car.public_telemetry() {
                            car.fuel.set(
                                self.now,
                                datagram.header.session_time,
                                &FuelData::from(value),
                            );
                            car.ers.set(
                                self.now,
                                datagram.header.session_time,
                                &ErsData::from(value),
                            );
                            car.front_brake_bias.set(
                                self.now,
                                datagram.header.session_time,
                                &value.front_brake_bias,
                            );
                        }
                    }
                }
            }
            Packet::CarDamage(packet) => {
                for (car, value) in self.cars.iter_mut().zip(&packet.car_damage_data) {
                    if !car.retired
                        && car.public_telemetry()
                        && let Some(value) = value
                    {
                        Arc::make_mut(car).damage.set(
                            self.now,
                            datagram.header.session_time,
                            value,
                        );
                    }
                }
            }
            Packet::TyreSets(packet) => {
                if let Some(car) = self.cars.get_mut(usize::from(packet.car_idx))
                    && !car.retired
                    && car.public_telemetry()
                {
                    Arc::make_mut(car).tyre_sets.set(
                        self.now,
                        datagram.header.session_time,
                        packet,
                    );
                }
            }
            Packet::CarTelemetry2(packet) => self.update_cars(
                &packet.car_telemetry2_data,
                datagram.header.session_time,
                |car| &mut car.aero,
            ),
            Packet::Motion(packet) => self.update_cars(
                &packet.car_motion_data,
                datagram.header.session_time,
                |car| &mut car.motion,
            ),
            Packet::CarTelemetry(packet) => self.update_cars(
                &packet.car_telemetry_data,
                datagram.header.session_time,
                |car| &mut car.telemetry,
            ),
            Packet::SessionHistory(packet) => {
                if let Some(car) = self.cars.get_mut(usize::from(packet.car_idx)) {
                    Arc::make_mut(car)
                        .history
                        .set(self.now, datagram.header.session_time, packet);
                }
            }
            Packet::Participants(packet) => {
                for (car, participant) in self
                    .cars
                    .iter_mut()
                    .zip(&packet.participants)
                    .take(usize::from(packet.num_active_cars))
                {
                    if !car.retired
                        && let Some(participant) = participant
                    {
                        let car = Arc::make_mut(car);
                        car.permission(participant.your_telemetry != 1);
                        car.participant
                            .set(self.now, datagram.header.session_time, participant);
                    }
                }
            }
            Packet::Lap(packet) => {
                for (car, lap) in self.cars.iter_mut().zip(&packet.lap_data) {
                    if !car.retired
                        && let Some(lap) = lap
                    {
                        let car = Arc::make_mut(car);
                        if !car.valid && !matches!(lap.result_status, 0 | 1) {
                            car.valid_since = Some(datagram.header.session_time);
                        }
                        car.valid = !matches!(lap.result_status, 0 | 1);
                        car.lap.set(self.now, datagram.header.session_time, lap);
                        car.retired = matches!(lap.result_status, 4 | 7);
                        if car.retired {
                            car.retired_at = Some(datagram.header.session_time);
                        }
                    }
                }
            }
            Packet::Event(event) => {
                if let EventDetails::Retirement {
                    vehicle_idx,
                    reason,
                } = event.details
                    && let Some(car) = self.cars.get_mut(usize::from(vehicle_idx))
                {
                    let car = Arc::make_mut(car);
                    car.retired = true;
                    car.retired_at = Some(datagram.header.session_time);
                    car.retirement_reason = Some(reason);
                }
            }
            _ => {}
        }
        self.derive_delta(datagram);
        if matches!(&datagram.packet, Packet::FinalClassification(_))
            || matches!(&datagram.packet, Packet::Event(event) if event.details == EventDetails::SessionEnded)
        {
            self.lifecycle = Lifecycle::Ended;
        }
    }

    fn update_cars<T: Clone>(
        &mut self,
        values: &[Option<T>; MAX_CARS],
        session_time: f32,
        group: fn(&mut Car) -> &mut Group<T>,
    ) {
        for (car, value) in self.cars.iter_mut().zip(values) {
            if !car.retired
                && let Some(value) = value
            {
                group(Arc::make_mut(car)).set(self.now, session_time, value);
            }
        }
    }

    /// Move monotonic input time forwards; backwards ticks cannot freshen data.
    pub fn advance(&mut self, time: Duration) {
        self.now = self.now.max(time);
        if self.lifecycle == Lifecycle::Live
            && self
                .last_arrival
                .is_some_and(|last| self.now.saturating_sub(last) >= Duration::from_secs(30))
        {
            self.lifecycle = Lifecycle::Lost;
        }
    }
}
