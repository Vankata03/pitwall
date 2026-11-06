//! What the overlay shows for a fast frame and a session model (PEDALS-01/02), ported from
//! the bar cases of `dashboard/src/overlay/throttle-brake.test.tsx`.
//! Assumed synthetic fixtures; packet-table fixtures are not capture evidence.
use pitwall_core::{
    feed::{CarInputs, FastFrame},
    model::{Readout, SessionModel},
    telemetry::{MAX_CARS, ParsedDatagram, parse_datagram},
};
use pitwall_overlay::view::{OverlayController, OverlayModel, overlay_car};
use std::{process::Command, time::Duration};

fn packet(id: u8, fields: &[(&str, &str)]) -> ParsedDatagram {
    let mut command = Command::new(if cfg!(windows) { "python" } else { "python3" });
    command.arg(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tools/telemetry/assumed_datagram.py"
    ));
    command.args(["2025".to_owned(), id.to_string()]);
    command.args(["--set", "header.session_uid=101"]);
    for (path, value) in fields {
        command.args(["--set", &format!("{path}={value}")]);
    }
    let output = command
        .output()
        .expect("Python is required for assumed fixtures");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    parse_datagram(&output.stdout).unwrap()
}

/// A session whose player is car `player` (255 for none) and where car 1 has joined.
fn session(player: &str) -> SessionModel {
    let mut model = SessionModel::default();
    let header = ("header.player_car_index", player);
    model.apply(
        Duration::ZERO,
        &packet(2, &[header, ("lap_data.1.result_status", "2")]),
    );
    model.apply(
        Duration::ZERO,
        &packet(
            4,
            &[
                header,
                ("num_active_cars", "2"),
                ("participants.1.ai_controlled", "1"),
                ("participants.1.driver_id", "7"),
                ("participants.1.race_number", "4"),
            ],
        ),
    );
    model
}

fn inputs(throttle: f32, brake: f32) -> CarInputs {
    CarInputs {
        speed: 200,
        throttle,
        brake,
        steer: 0.0,
        clutch: 0,
        gear: 5,
        engine_rpm: 10_000,
        drs: false,
        rev_lights_percent: 0,
    }
}

fn frame(time: f32, car: Readout<CarInputs>, distance: f32) -> FastFrame {
    let mut cars = [Readout::Absent; MAX_CARS];
    cars[1] = car;
    let mut lap_distances = [Readout::Absent; MAX_CARS];
    lap_distances[1] = Readout::Live(distance);
    FastFrame {
        generation: 1,
        epoch: 1,
        session_time: time,
        player_car_index: Some(1),
        cars,
        lap_distances,
        delta_best: [Readout::Absent; MAX_CARS],
        delta_previous: [Readout::Absent; MAX_CARS],
    }
}

#[test]
fn the_overlay_follows_the_player_car_only_while_it_is_a_participant() {
    assert_eq!(overlay_car(&session("1")), Some(1));
    assert_eq!(overlay_car(&session("255")), None);
    // Car 0 has no participant entry: a player index pointing at it is not in the session.
    assert_eq!(overlay_car(&session("0")), None);
}

#[test]
fn shows_the_player_car_pedals_as_bars_with_values() {
    let model = session("1");
    let mut overlay = OverlayModel::default();
    overlay.update(&frame(0.1, Readout::Live(inputs(0.8, 0.0)), 1.0), &model);
    let view = overlay.update(&frame(0.2, Readout::Live(inputs(0.35, 0.6)), 2.0), &model);
    assert!(view.showing);
    assert!(!view.stale);
    assert_eq!(view.throttle.text, "35");
    assert_eq!(view.brake.text, "60");
    assert_eq!(view.throttle.fill, 0.35);
    assert_eq!(view.brake.fill, 0.6);
    assert_eq!(overlay.history().len(), 2);
}

#[test]
fn stale_inputs_still_show_but_dim() {
    let model = session("1");
    let mut overlay = OverlayModel::default();
    let view = overlay.update(&frame(0.1, Readout::Stale(inputs(1.0, 0.0)), 1.0), &model);
    assert!(view.showing);
    assert!(view.stale);
    assert_eq!(view.throttle.text, "100");
    assert_eq!(view.throttle.fill, 1.0);
}

#[test]
fn restricted_pedals_have_no_fill_and_keep_the_restricted_readout() {
    let mut overlay = OverlayModel::default();
    let view = overlay.update(&frame(0.1, Readout::Restricted, 1.0), &session("1"));
    assert!(!view.showing);
    assert_eq!(view.throttle.text, "Restricted");
    assert_eq!(view.brake.text, "Restricted");
    assert_eq!(view.throttle.fill, 0.0);
    assert_eq!(view.brake.fill, 0.0);
}

#[test]
fn empties_the_bars_without_a_player_car_or_its_inputs() {
    let mut overlay = OverlayModel::default();
    let view = overlay.update(
        &frame(0.1, Readout::Live(inputs(0.8, 0.2)), 1.0),
        &session("255"),
    );
    assert!(!view.showing);
    assert_eq!(view.throttle.text, "—");
    assert_eq!(view.throttle.fill, 0.0);
    assert_eq!(view.brake.fill, 0.0);
    let view = overlay.update(&frame(0.2, Readout::Absent, 1.0), &session("1"));
    assert!(!view.showing);
    assert_eq!(view.brake.text, "—");
}

#[test]
fn history_restarts_when_the_session_time_steps_back() {
    let model = session("1");
    let mut overlay = OverlayModel::default();
    for time in [1.0, 1.1, 1.2] {
        overlay.update(&frame(time, Readout::Live(inputs(0.5, 0.0)), 1.0), &model);
    }
    overlay.update(&frame(0.5, Readout::Live(inputs(0.5, 0.0)), 1.0), &model);
    assert_eq!(overlay.history().len(), 1);
}

#[test]
fn a_slow_snapshot_change_updates_the_overlay_without_a_new_fast_frame() {
    let (mut publisher, reader) = pitwall_core::feed::channel();
    publisher.publish_snapshot(&session("1"));
    let mut overlay = OverlayController::new(reader);
    assert!(overlay.poll());
    assert!(!overlay.poll(), "unchanged feed must not request a repaint");
    publisher.publish_snapshot(&session("255"));
    assert!(
        overlay.poll(),
        "a player or reference change needs a repaint even while paused"
    );
    assert!(!overlay.view().unwrap().showing);
    assert!(overlay.trace(100.0, 50.0).throttle.is_empty());
    assert!(!overlay.poll());
}

#[test]
fn a_delayed_draw_keeps_every_intervening_pedal_sample() {
    let (mut publisher, reader) = pitwall_core::feed::channel();
    let mut model = session("1");
    for (time, throttle) in [("1.0", "0.0"), ("1.1", "1.0"), ("1.2", "0.0")] {
        model.apply(
            Duration::from_secs_f64(time.parse().unwrap()),
            &packet(
                6,
                &[
                    ("header.player_car_index", "1"),
                    ("header.session_time", time),
                    ("car_telemetry_data.1.throttle", throttle),
                ],
            ),
        );
        publisher.publish_fast(&model);
    }
    publisher.publish_snapshot(&model);
    let mut overlay = OverlayController::new(reader);
    assert!(overlay.poll());
    let trace = overlay.trace(100.0, 50.0);
    let values: Vec<_> = trace.throttle[0]
        .iter()
        .map(|p| p.1.round() as i32)
        .collect();
    assert_eq!(
        values,
        [50, 0, 50],
        "the throttle stab between draws must remain visible"
    );
}

#[test]
fn a_new_epoch_snapshot_does_not_show_retained_inputs_from_the_old_epoch() {
    let (mut publisher, reader) = pitwall_core::feed::channel();
    let mut model = session("1");
    model.apply(
        Duration::ZERO,
        &packet(
            6,
            &[
                ("header.player_car_index", "1"),
                ("car_telemetry_data.1.throttle", "1.0"),
            ],
        ),
    );
    publisher.publish_fast(&model);
    model.apply(
        Duration::from_secs(1),
        &packet(
            2,
            &[
                ("header.session_uid", "202"),
                ("header.player_car_index", "1"),
                ("lap_data.1.result_status", "2"),
            ],
        ),
    );
    model.apply(
        Duration::from_secs(1),
        &packet(
            4,
            &[
                ("header.session_uid", "202"),
                ("header.player_car_index", "1"),
                ("num_active_cars", "2"),
                ("participants.1.driver_id", "7"),
            ],
        ),
    );
    assert_eq!(overlay_car(&model), Some(1));
    publisher.publish_snapshot(&model);
    let mut overlay = OverlayController::new(reader);
    overlay.poll();
    assert!(!overlay.view().unwrap().showing);
    assert!(overlay.trace(100.0, 50.0).throttle.is_empty());
}
