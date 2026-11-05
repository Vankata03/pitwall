use super::{ParseError, Reader};

/// Decoded event plus untouched details, including unknown future event codes.
#[derive(Debug, Clone, PartialEq, serde::Serialize, ts_rs::TS)]
pub struct Event {
    pub code: [u8; 4],
    pub details: EventDetails,
    pub raw_details: [u8; 12],
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, ts_rs::TS)]
pub enum EventDetails {
    SessionStarted,
    SessionEnded,
    FastestLap {
        vehicle_idx: u8,
        lap_time: f32,
    },
    Retirement {
        vehicle_idx: u8,
        reason: u8,
    },
    DrsEnabled,
    DrsDisabled {
        reason: u8,
    },
    TeamMateInPits {
        vehicle_idx: u8,
    },
    ChequeredFlag,
    RaceWinner {
        vehicle_idx: u8,
    },
    Penalty {
        penalty_type: u8,
        infringement_type: u8,
        vehicle_idx: u8,
        other_vehicle_idx: u8,
        time: u8,
        lap_num: u8,
        places_gained: u8,
    },
    SpeedTrap {
        vehicle_idx: u8,
        speed: f32,
        is_overall_fastest_in_session: u8,
        is_driver_fastest_in_session: u8,
        fastest_vehicle_idx_in_session: u8,
        fastest_speed_in_session: f32,
    },
    StartLights {
        num_lights: u8,
    },
    LightsOut,
    DriveThroughServed {
        vehicle_idx: u8,
    },
    StopGoServed {
        vehicle_idx: u8,
        stop_time: f32,
    },
    Flashback {
        frame_identifier: u32,
        session_time: f32,
    },
    Buttons {
        button_status: u32,
    },
    RedFlag,
    Overtake {
        overtaking_vehicle_idx: u8,
        being_overtaken_vehicle_idx: u8,
    },
    SafetyCar {
        safety_car_type: u8,
        event_type: u8,
    },
    Collision {
        vehicle1_idx: u8,
        vehicle2_idx: u8,
        severity: Option<u8>,
    },
    PartialAeroEnabled,
    PartialAeroDisabled,
    OvertakeEnabled,
    OvertakeDisabled,
    Unknown,
}

impl Event {
    pub(super) fn read(reader: &mut Reader<'_>, format: u16) -> Result<Self, ParseError> {
        let code = reader.take()?;
        let raw_details = reader.take()?;
        let mut r = Reader {
            bytes: &raw_details,
            offset: 0,
        };
        let details = match &code {
            b"SSTA" => EventDetails::SessionStarted,
            b"SEND" => EventDetails::SessionEnded,
            b"FTLP" => EventDetails::FastestLap {
                vehicle_idx: r.u8()?,
                lap_time: r.f32()?,
            },
            b"RTMT" => EventDetails::Retirement {
                vehicle_idx: r.u8()?,
                reason: r.u8()?,
            },
            b"DRSE" => EventDetails::DrsEnabled,
            b"DRSD" => EventDetails::DrsDisabled { reason: r.u8()? },
            b"TMPT" => EventDetails::TeamMateInPits {
                vehicle_idx: r.u8()?,
            },
            b"CHQF" => EventDetails::ChequeredFlag,
            b"RCWN" => EventDetails::RaceWinner {
                vehicle_idx: r.u8()?,
            },
            b"PENA" => EventDetails::Penalty {
                penalty_type: r.u8()?,
                infringement_type: r.u8()?,
                vehicle_idx: r.u8()?,
                other_vehicle_idx: r.u8()?,
                time: r.u8()?,
                lap_num: r.u8()?,
                places_gained: r.u8()?,
            },
            b"SPTP" => EventDetails::SpeedTrap {
                vehicle_idx: r.u8()?,
                speed: r.f32()?,
                is_overall_fastest_in_session: r.u8()?,
                is_driver_fastest_in_session: r.u8()?,
                fastest_vehicle_idx_in_session: r.u8()?,
                fastest_speed_in_session: r.f32()?,
            },
            b"STLG" => EventDetails::StartLights {
                num_lights: r.u8()?,
            },
            b"LGOT" => EventDetails::LightsOut,
            b"DTSV" => EventDetails::DriveThroughServed {
                vehicle_idx: r.u8()?,
            },
            b"SGSV" => EventDetails::StopGoServed {
                vehicle_idx: r.u8()?,
                stop_time: r.f32()?,
            },
            b"FLBK" => EventDetails::Flashback {
                frame_identifier: r.u32()?,
                session_time: r.f32()?,
            },
            b"BUTN" => EventDetails::Buttons {
                button_status: r.u32()?,
            },
            b"RDFL" => EventDetails::RedFlag,
            b"OVTK" => EventDetails::Overtake {
                overtaking_vehicle_idx: r.u8()?,
                being_overtaken_vehicle_idx: r.u8()?,
            },
            b"SCAR" => EventDetails::SafetyCar {
                safety_car_type: r.u8()?,
                event_type: r.u8()?,
            },
            b"COLL" => EventDetails::Collision {
                vehicle1_idx: r.u8()?,
                vehicle2_idx: r.u8()?,
                severity: if format == 2026 { Some(r.u8()?) } else { None },
            },
            b"PMEN" if format == 2026 => EventDetails::PartialAeroEnabled,
            b"PMDI" if format == 2026 => EventDetails::PartialAeroDisabled,
            b"OVEN" if format == 2026 => EventDetails::OvertakeEnabled,
            b"OVDI" if format == 2026 => EventDetails::OvertakeDisabled,
            _ => EventDetails::Unknown,
        };
        Ok(Self {
            code,
            details,
            raw_details,
        })
    }
}
