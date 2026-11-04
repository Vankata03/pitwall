use crate::{model::*, telemetry::*};
use serde::Serialize;
use ts_rs::TS;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    Live,
    Replay,
}
#[derive(Debug, Default, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Revisions {
    pub layouts: u32,
    pub tracks: u32,
}
#[derive(Debug, Default, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CollectorStatus {
    pub packet_rate_hz: u32,
    pub dropped: u32,
    pub parse_errors: u32,
    pub uid_zero: u32,
    pub closed_uid: u32,
    pub forwarding_errors: u32,
    pub last_format: Option<u16>,
}
#[derive(Debug, Default, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    pub revisions: Revisions,
    pub collector: CollectorStatus,
    pub replay: Option<ReplayState>,
}
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ReplayState {
    pub duration: f64,
    pub position: f64,
    pub speed: f64,
    pub paused: bool,
    pub banner: String,
    pub load_progress: f64,
    pub background_live: Option<String>,
}
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SessionState {
    pub track_id: i8,
    pub track_length: u16,
    pub sector2_lap_distance_start: f32,
    pub sector3_lap_distance_start: f32,
    pub session_type: u8,
    pub total_laps: u8,
    pub weather: u8,
    pub track_temperature: i8,
    pub air_temperature: i8,
    pub safety_car_status: u8,
    pub forecast: Vec<WeatherForecastSample>,
}
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CarState {
    pub participant: Readout<ParticipantData>,
    pub regulations: Option<Regulations>,
    pub retired: bool,
    pub lap: Readout<LapData>,
    pub telemetry: Readout<CarTelemetryData>,
    pub status: Readout<StatusData>,
    pub fuel: Readout<FuelData>,
    pub ers: Readout<ErsData>,
    pub damage: Readout<CarDamageData>,
    pub aero: Readout<CarTelemetry2Data>,
    pub gaps: Readout<Gaps>,
    pub tyre_condition: Readout<[f32; 4]>,
    pub front_brake_bias: Readout<u8>,
    pub penalties: Readout<PenaltyState>,
    pub best: Option<CompletedLap>,
    pub previous: Option<CompletedLap>,
    pub completed_best: Option<LapResult>,
    pub completed_previous: Option<LapResult>,
    pub personal_best: Option<TimeTrialDataSet>,
    pub rival: Option<TimeTrialDataSet>,
}
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub lifecycle: Lifecycle,
    pub session: Readout<SessionState>,
    pub player_car: Option<u8>,
    pub pit_window: Readout<PitWindow>,
    pub cars: Vec<Option<CarState>>,
    pub context: Context,
}
impl State {
    pub fn capture(model: &SessionModel, context: Context) -> Self {
        let cars = (0..24)
            .map(|index| {
                model.car(index).map(|c| CarState {
                    participant: c.participant().map(Clone::clone),
                    regulations: c.regulations(),
                    retired: c.retired(),
                    lap: c.lap().map(Clone::clone),
                    telemetry: c.telemetry().map(Clone::clone),
                    status: c.status().map(Clone::clone),
                    fuel: c.fuel().map(Clone::clone),
                    ers: c.ers().map(Clone::clone),
                    damage: c.damage().map(Clone::clone),
                    aero: c.aero().map(Clone::clone),
                    gaps: c.gaps(),
                    tyre_condition: c.tyre_condition(),
                    front_brake_bias: c.front_brake_bias().map(|v| *v),
                    penalties: c.penalty_state(),
                    best: c.reference_lap(Reference::Best).cloned(),
                    previous: c.reference_lap(Reference::Previous).cloned(),
                    completed_best: c.completed_result(Reference::Best),
                    completed_previous: c.completed_result(Reference::Previous),
                    personal_best: c.time_trial_reference(Reference::PersonalBest).cloned(),
                    rival: c.time_trial_reference(Reference::Rival).cloned(),
                })
            })
            .collect();
        let session = model.session().map(|s| SessionState {
            track_id: s.track_id,
            track_length: s.track_length,
            sector2_lap_distance_start: s.sector2_lap_distance_start,
            sector3_lap_distance_start: s.sector3_lap_distance_start,
            session_type: s.session_type,
            total_laps: s.total_laps,
            weather: s.weather,
            track_temperature: s.track_temperature,
            air_temperature: s.air_temperature,
            safety_car_status: s.safety_car_status,
            forecast: s.weather_forecast_samples
                [..usize::from(s.num_weather_forecast_samples).min(64)]
                .to_vec(),
        });
        Self {
            lifecycle: model.lifecycle(),
            session,
            player_car: model.player_car_index().map(|v| v as u8),
            pit_window: model
                .player_car_index()
                .map_or(Readout::Absent, |i| model.pit_window(i).map(Clone::clone)),
            cars,
            context,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(tag = "kind", content = "body", rename_all = "camelCase")]
pub enum HistoryBody {
    Lap(CompletedLap),
    Sector(Sector),
    Stint(Stint),
    PitStop(PitStop),
    Penalty(Penalty),
    RaceControl(RaceControlEntry),
}
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct HistoryEntry {
    pub id: String,
    pub car: Option<u8>,
    #[serde(flatten)]
    pub body: HistoryBody,
}
/// Cars plus race control. Groups `0..24` are cars; `HISTORY_GROUPS - 1` is race control.
pub const HISTORY_GROUPS: usize = 25;
pub fn history(model: &SessionModel) -> Vec<HistoryEntry> {
    (0..HISTORY_GROUPS)
        .flat_map(|group| history_group(model, group))
        .collect()
}
/// One car's history entries, or race control for the last group, in snapshot order.
pub fn history_group(model: &SessionModel, group: usize) -> Vec<HistoryEntry> {
    let mut entries = Vec::new();
    if group == HISTORY_GROUPS - 1 {
        for v in model.race_control() {
            entries.push(HistoryEntry {
                id: format!("race-control:{}", v.id),
                car: None,
                body: HistoryBody::RaceControl(v.clone()),
            });
        }
        return entries;
    }
    let index = group;
    if let Some(c) = model.car(index) {
        let h = c.session_history();
        let mut add = |kind: &str, id: String, body| {
            entries.push(HistoryEntry {
                id: format!("{index}:{kind}:{id}"),
                car: Some(index as u8),
                body,
            })
        };
        for v in &h.laps {
            add(
                "lap",
                v.lap_number.to_string(),
                HistoryBody::Lap((**v).clone()),
            );
        }
        for v in &h.sectors {
            add(
                "sector",
                format!("{}:{}", v.lap_number, v.sector),
                HistoryBody::Sector(v.clone()),
            );
        }
        for v in &h.stints {
            add("stint", v.id.to_string(), HistoryBody::Stint(v.clone()));
        }
        for v in &h.pit_stops {
            add("pit", v.id.to_string(), HistoryBody::PitStop(v.clone()));
        }
        for v in &h.penalties {
            add("penalty", v.id.to_string(), HistoryBody::Penalty(v.clone()));
        }
    }
    entries
}
#[derive(Debug, Clone, Serialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TextMessage {
    Hello {
        #[serde(rename = "buildId")]
        build_id: String,
        epoch: u32,
        source: Source,
    },
    Snapshot {
        epoch: u32,
        source: Source,
        state: State,
        history: Vec<HistoryEntry>,
    },
    State {
        epoch: u32,
        #[serde(flatten)]
        state: State,
    },
    History {
        epoch: u32,
        entries: Vec<HistoryEntry>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        removed: Option<Vec<String>>,
    },
}
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Trace {
    pub epoch: u32,
    pub lap: u8,
    pub distance: Vec<f32>,
    pub time: Vec<f32>,
    pub speed: Vec<Option<f32>>,
    pub throttle: Vec<Option<f32>>,
    pub brake: Vec<Option<f32>>,
}
