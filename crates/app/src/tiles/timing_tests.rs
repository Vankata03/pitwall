//! The timing tiles built from a model fed synthetic packets: names and privacy, the
//! window focus, the race status band, stint and stop history, and a new session
//! clearing that history.

use super::{gaps, lap_state, stint, timing_tower};
use pitwall_core::{
    model::{Lifecycle, SessionModel},
    readout::ReadoutState,
    telemetry::{Packet, ParsedDatagram, parse_datagram},
    tower::TowerSettings,
};
use std::time::Duration;

fn assumed(id: u8, uid: u64) -> ParsedDatagram {
    let output = std::process::Command::new("python")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tools/telemetry/assumed_datagram.py"
        ))
        .args([
            "2026",
            &id.to_string(),
            "--set",
            &format!("header.session_uid={uid}"),
            "--set",
            "header.player_car_index=0",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    parse_datagram(&output.stdout).unwrap()
}

struct Feed {
    session: ParsedDatagram,
    participants: ParsedDatagram,
    lap: ParsedDatagram,
    status: ParsedDatagram,
    time: f32,
}

impl Feed {
    fn new(uid: u64) -> Self {
        Self {
            session: assumed(1, uid),
            participants: assumed(4, uid),
            lap: assumed(2, uid),
            status: assumed(7, uid),
            time: 0.0,
        }
    }

    fn send(&mut self, model: &mut SessionModel, mut packet: ParsedDatagram) {
        self.time += 0.1;
        packet.header.session_time = self.time;
        model.apply(Duration::from_secs_f32(self.time), &packet);
    }
}

fn name(text: &str) -> [u8; 32] {
    let mut raw = [0u8; 32];
    raw[..text.len()].copy_from_slice(text.as_bytes());
    raw
}

/// A three-car race under the safety car: car 0 (player) P2, car 1 an AI P1 that hides
/// online names, car 2 a human P3 who hides it.
fn race(feed: &mut Feed) -> SessionModel {
    let mut model = SessionModel::default();
    let mut session = feed.session.clone();
    let Packet::Session(data) = &mut session.packet else {
        unreachable!()
    };
    data.session_type = 15;
    data.safety_car_status = 1;
    data.total_laps = 44;
    data.track_length = 5000;
    feed.send(&mut model, session);
    let mut participants = feed.participants.clone();
    let Packet::Participants(data) = &mut participants.packet else {
        unreachable!()
    };
    data.num_active_cars = 3;
    for (i, (who, ai)) in [("Player", 0), ("Bot", 1), ("private-id", 0)]
        .into_iter()
        .enumerate()
    {
        let p = data.participants[i].as_mut().unwrap();
        p.name = name(who);
        p.ai_controlled = ai;
        p.show_online_names = 0;
        p.race_number = i as u8 + 10;
        p.livery_colour[0].red = 200;
    }
    feed.send(&mut model, participants);
    let mut lap = feed.lap.clone();
    let Packet::Lap(data) = &mut lap.packet else {
        unreachable!()
    };
    for (i, position) in [2u8, 1, 3].into_iter().enumerate() {
        let car = data.lap_data[i].as_mut().unwrap();
        car.result_status = 2;
        car.car_position = position;
        car.current_lap_num = 3;
        car.sector = 1;
        car.delta_to_car_in_front_ms_part = 1250 * u16::from(position > 1);
        car.lap_distance = 100.0;
        car.total_distance = 10_100.0;
    }
    feed.send(&mut model, lap);
    model
}

#[test]
fn the_tower_names_ai_cars_hides_private_humans_and_highlights_the_focus() {
    let mut feed = Feed::new(301);
    let model = race(&mut feed);
    let v = timing_tower::view(&timing_tower::Inputs::from_model(
        &model,
        model.player_car_index(),
        TowerSettings::default(),
        (700.0, 1000.0),
    ));
    assert_eq!(v.note.text, "Safety car");
    let timing_tower::Content::Table { rows, .. } = v.content else {
        panic!("table")
    };
    let drivers: Vec<&str> = rows
        .iter()
        .map(|r| r.cells[1].value.text.as_str())
        .collect();
    assert_eq!(drivers, ["Bot", "#10", "#12"]);
    assert!(rows[1].focus);
    assert_eq!(rows[1].cells[2].value.text, "+1.250");
    assert_eq!(rows[0].team.map(|c| c.r), Some(200));
}

#[test]
fn gaps_and_lap_state_read_the_bound_car() {
    let mut feed = Feed::new(302);
    let model = race(&mut feed);
    let g = gaps::view(&gaps::Inputs::from_model(&model, Some(0), (320.0, 64.0)));
    assert_eq!(g.rows[0].value.text, "+1.250 s");
    assert_eq!(g.rows[1].value.text, "\u{2212}1.250 s");
    assert_eq!(g.notes[0].as_deref(), Some("P1 \u{b7} Bot"));
    assert_eq!(g.notes[1].as_deref(), Some("P3 \u{b7} #12"));
    let l = lap_state::view(&lap_state::Inputs::from_model(
        &model,
        Some(0),
        (320.0, 64.0),
    ));
    let texts: Vec<&str> = l.rows.iter().map(|r| r.value.text.as_str()).collect();
    assert_eq!(texts, ["3", "44", "2", "2", "On track"]);
    let none = lap_state::view(&lap_state::Inputs::from_model(&model, None, (320.0, 64.0)));
    assert_eq!(none.rows[0].value.state, ReadoutState::Absent);
}

#[test]
fn stint_reads_history_and_a_new_session_clears_it() {
    let mut feed = Feed::new(303);
    let mut model = race(&mut feed);
    let status = |feed: &Feed, compound: u8, age: u8| {
        let mut packet = feed.status.clone();
        let Packet::CarStatus(data) = &mut packet.packet else {
            unreachable!()
        };
        let car = data.car_status_data[0].as_mut().unwrap();
        car.actual_tyre_compound = compound;
        car.visual_tyre_compound = compound;
        car.tyres_age_laps = age;
        packet
    };
    let lap = |feed: &Feed, number: u8, pit: u8| {
        let mut packet = feed.lap.clone();
        let Packet::Lap(data) = &mut packet.packet else {
            unreachable!()
        };
        for (i, position) in [2u8, 1, 3].into_iter().enumerate() {
            let car = data.lap_data[i].as_mut().unwrap();
            car.result_status = 2;
            car.car_position = position;
            car.current_lap_num = number;
        }
        let car = data.lap_data[0].as_mut().unwrap();
        car.pit_status = pit;
        car.last_lap_time_in_ms = 90_000;
        packet
    };
    let packet = status(&feed, 16, 2);
    feed.send(&mut model, packet);
    let packet = lap(&feed, 5, 1);
    feed.send(&mut model, packet);
    let packet = lap(&feed, 6, 0);
    feed.send(&mut model, packet);
    let packet = status(&feed, 17, 0);
    feed.send(&mut model, packet);
    let v = stint::view(&stint::Inputs::from_model(
        &model,
        Some(0),
        false,
        (400.0, 350.0),
    ));
    let current = v.current.as_ref().unwrap();
    assert_eq!(current.name, "Medium");
    assert_eq!(current.set_label, "New set");
    assert_eq!(v.past.len(), 1);
    assert_eq!(v.past[0].name, "Soft");
    assert_eq!(v.past[0].set_label, "Used set \u{b7} starting age 2");
    assert_eq!(v.stop_count, "1 stop");
    assert_eq!(v.stops[0].label, "Stop \u{b7} lap 5");
    // A rival's stint has no player pit window.
    let rival = stint::view(&stint::Inputs::from_model(
        &model,
        Some(1),
        false,
        (400.0, 350.0),
    ));
    assert_eq!(rival.pit_window, None);

    // Another session: the old stints and stops are gone.
    let mut next = Feed::new(304);
    let mut packet = next.lap.clone();
    let Packet::Lap(data) = &mut packet.packet else {
        unreachable!()
    };
    data.lap_data[0].as_mut().unwrap().result_status = 2;
    next.send(&mut model, packet);
    assert_eq!(model.lifecycle(), Lifecycle::Live);
    let v = stint::view(&stint::Inputs::from_model(
        &model,
        Some(0),
        false,
        (400.0, 350.0),
    ));
    assert!(v.past.is_empty() && v.stops.is_empty() && v.current.is_none());
    assert_eq!(v.history_note, Some("No stint history"));
}
