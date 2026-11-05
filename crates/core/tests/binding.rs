//! Car identity, participants and tile car binding (spec section 5; parity BIND-01..04).
//! Ported from `dashboard/src/session/cars.test.ts`; host messages from `tiles/host.tsx`.
//! Assumed synthetic fixtures; packet-table fixtures are not capture evidence.
use pitwall_core::{
    binding::{
        BindingContext, CarBinding, CarIdentity, CarTarget, Participant, ResolvedCar,
        car_binding_of, car_label, decode_name, identity_key, identity_of, missing_car_message,
        parse_identity, participants_of, resolve_binding, resolve_identity,
    },
    model::SessionModel,
    telemetry::{Packet, ParsedDatagram, ParticipantData, parse_datagram},
};
use serde_json::json;
use std::{process::Command, sync::OnceLock, time::Duration};

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

/// A zero Participants packet, parsed once.
fn participants_packet() -> &'static ParsedDatagram {
    static PACKET: OnceLock<ParsedDatagram> = OnceLock::new();
    PACKET.get_or_init(|| packet(4, &[]))
}

fn name_bytes(text: &str) -> [u8; 32] {
    let mut out = [0; 32];
    out[..text.len()].copy_from_slice(text.as_bytes());
    out
}

/// AI driver 9, race number 44, "HAMILTON", like the TS fixture's defaults.
fn data(edit: impl FnOnce(&mut ParticipantData)) -> ParticipantData {
    let Packet::Participants(p) = &participants_packet().packet else {
        unreachable!()
    };
    let mut data = p.participants[0].clone().unwrap();
    data.ai_controlled = 1;
    data.driver_id = 9;
    data.network_id = 255;
    data.race_number = 44;
    data.name = name_bytes("HAMILTON");
    data.show_online_names = 1;
    edit(&mut data);
    data
}

/// Cars `0..cars.len()` valid, with Participants data and race positions.
fn model(cars: &[(ParticipantData, Option<u8>)], retired: &[usize]) -> SessionModel {
    let mut lap_fields = vec![];
    for (index, (_, position)) in cars.iter().enumerate() {
        let status = if retired.contains(&index) { "4" } else { "2" };
        lap_fields.push((format!("lap_data.{index}.result_status"), status.to_owned()));
        if let Some(position) = position {
            lap_fields.push((
                format!("lap_data.{index}.car_position"),
                position.to_string(),
            ));
        }
    }
    let lap_fields: Vec<(&str, &str)> = lap_fields
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let mut participants = participants_packet().clone();
    let Packet::Participants(p) = &mut participants.packet else {
        unreachable!()
    };
    p.num_active_cars = cars.len() as u8;
    for (slot, (data, _)) in p.participants.iter_mut().zip(cars) {
        *slot = Some(data.clone());
    }
    let mut model = SessionModel::default();
    model.apply(Duration::ZERO, &participants);
    model.apply(Duration::ZERO, &packet(2, &lap_fields));
    model
}

fn driver(driver_id: u16) -> CarIdentity {
    CarIdentity::Driver { driver_id }
}

fn participant(edit: impl FnOnce(&mut Participant)) -> Participant {
    let mut p = Participant {
        index: 0,
        name: "Max".into(),
        race_number: 33,
        show_online_names: true,
        identity: CarIdentity::Name { name: "Max".into() },
        retired: false,
        position: None,
    };
    edit(&mut p);
    p
}

// Reading the session model.

#[test]
fn participants_carry_index_name_race_number_and_identity() {
    let model = model(
        &[
            (
                data(|d| {
                    d.driver_id = 1;
                    d.name = name_bytes("Hülkenberg");
                }),
                None,
            ),
            (
                data(|d| {
                    d.driver_id = 5;
                    d.name = name_bytes("Alonso");
                }),
                None,
            ),
        ],
        &[],
    );
    let list = participants_of(&model);
    let summary: Vec<_> = list
        .iter()
        .map(|p| (p.index, p.name.as_str(), p.race_number))
        .collect();
    assert_eq!(summary, [(0, "Hülkenberg", 44), (1, "Alonso", 44)]);
    assert_eq!(list[0].identity, driver(1));
}

#[test]
fn an_empty_model_has_no_participants() {
    assert!(participants_of(&SessionModel::default()).is_empty());
}

#[test]
fn retired_cars_stay_listed_and_are_marked() {
    let model = model(&[(data(|_| {}), None), (data(|_| {}), None)], &[1]);
    let list = participants_of(&model);
    assert_eq!(list.len(), 2);
    assert!(!list[0].retired);
    assert!(list[1].retired);
}

#[test]
fn race_position_comes_from_lap_data() {
    let model = model(&[(data(|_| {}), Some(3))], &[]);
    assert_eq!(participants_of(&model)[0].position, Some(3));
}

#[test]
fn names_are_the_received_utf8_cut_at_the_first_nul() {
    assert_eq!(decode_name(&name_bytes("Hülkenberg")), "Hülkenberg");
    assert_eq!(decode_name(b"AB\0CD"), "AB");
    assert_eq!(decode_name(b""), "");
}

// Identity: one rule for focus and pins (spec section 5).

#[test]
fn ai_and_offline_humans_are_identified_by_driver_id() {
    assert_eq!(
        identity_of(&data(|d| {
            d.ai_controlled = 1;
            d.driver_id = 12;
        })),
        driver(12)
    );
    assert_eq!(
        identity_of(&data(|d| {
            d.ai_controlled = 0;
            d.network_id = 255;
            d.driver_id = 0;
        })),
        driver(0)
    );
}

#[test]
fn online_humans_are_identified_by_name_because_network_id_is_session_scoped() {
    assert_eq!(
        identity_of(&data(|d| {
            d.ai_controlled = 0;
            d.network_id = 17;
            d.name = name_bytes("Max");
        })),
        CarIdentity::Name { name: "Max".into() }
    );
}

#[test]
fn a_pin_survives_into_another_session_by_driver_id() {
    let next = participants_of(&model(
        &[
            (data(|d| d.driver_id = 20), None),
            (data(|d| d.driver_id = 9), None),
        ],
        &[],
    ));
    assert_eq!(resolve_identity(&driver(9), &next), ResolvedCar::Ok(1));
}

#[test]
fn a_pin_absent_from_the_session_says_so_and_is_not_replaced() {
    let list = participants_of(&model(&[(data(|_| {}), None)], &[]));
    assert_eq!(
        resolve_identity(&driver(99), &list),
        ResolvedCar::NotInSession
    );
}

#[test]
fn a_retired_car_keeps_the_focus_and_reports_retired_with_no_fallback() {
    let list = participants_of(&model(&[(data(|d| d.driver_id = 9), None)], &[0]));
    assert_eq!(resolve_identity(&driver(9), &list), ResolvedCar::Retired(0));
}

#[test]
fn identities_store_and_parse_round_trip_and_reject_junk() {
    for id in [driver(4), CarIdentity::Name { name: "Max".into() }] {
        assert_eq!(parse_identity(&identity_key(&id)), Some(id));
    }
    // The browser's stored form stays readable.
    assert_eq!(
        parse_identity(r#"{"kind":"driver","driverId":4}"#),
        Some(driver(4))
    );
    assert_eq!(parse_identity("garbage"), None);
    assert_eq!(parse_identity(r#"{"kind":"driver","driverId":"x"}"#), None);
}

// Labels.

#[test]
fn label_is_the_name_as_received() {
    assert_eq!(car_label(&participant(|_| {})), "Max");
}

#[test]
fn ai_names_remain_visible_when_the_online_name_flag_is_off() {
    let list = participants_of(&model(
        &[(
            data(|d| {
                d.ai_controlled = 1;
                d.network_id = 65535;
                d.show_online_names = 0;
                d.name = name_bytes("PÉREZ");
            }),
            None,
        )],
        &[],
    ));
    assert_eq!(car_label(&list[0]), "PÉREZ");
}

#[test]
fn hidden_online_names_stay_private_and_empty_names_fall_back_to_race_number() {
    let list = participants_of(&model(
        &[(
            data(|d| {
                d.ai_controlled = 0;
                d.network_id = 17;
                d.show_online_names = 0;
                d.name = name_bytes("private-platform-id");
            }),
            None,
        )],
        &[],
    ));
    assert_eq!(car_label(&list[0]), "#44");
    assert_eq!(car_label(&participant(|p| p.name = "  ".into())), "#33");
}

#[test]
fn a_human_who_hides_their_online_name_is_shown_by_race_number() {
    assert_eq!(
        car_label(&participant(|p| p.show_online_names = false)),
        "#33"
    );
}

// Binding a tile to a car.

fn grid() -> Vec<Participant> {
    participants_of(&model(
        &[
            (data(|d| d.driver_id = 1), Some(1)),
            (data(|d| d.driver_id = 2), Some(2)),
            (data(|d| d.driver_id = 3), Some(3)),
        ],
        &[],
    ))
}

fn target(target: CarTarget) -> CarBinding {
    CarBinding { target, pin: None }
}

#[test]
fn focus_follows_the_player_car_by_default() {
    let list = grid();
    let ctx = BindingContext {
        participants: &list,
        player_car: Some(1),
        focus: None,
    };
    assert_eq!(
        resolve_binding(&target(CarTarget::Focus), &ctx),
        ResolvedCar::Ok(1)
    );
}

#[test]
fn focus_follows_a_stored_focus_identity_even_one_absent_from_the_session() {
    let list = grid();
    let three = driver(3);
    let missing = driver(77);
    let ctx = |focus| BindingContext {
        participants: &list,
        player_car: Some(1),
        focus,
    };
    assert_eq!(
        resolve_binding(&target(CarTarget::Focus), &ctx(Some(&three))),
        ResolvedCar::Ok(2)
    );
    assert_eq!(
        resolve_binding(&target(CarTarget::Focus), &ctx(Some(&missing))),
        ResolvedCar::NotInSession
    );
}

#[test]
fn no_player_car_yet_resolves_to_nothing_not_to_car_zero() {
    let list = grid();
    let ctx = BindingContext {
        participants: &list,
        player_car: None,
        focus: None,
    };
    assert_eq!(
        resolve_binding(&target(CarTarget::Focus), &ctx),
        ResolvedCar::NotInSession
    );
}

#[test]
fn pinned_driver_ignores_the_focus() {
    let list = grid();
    let ctx = BindingContext {
        participants: &list,
        player_car: Some(1),
        focus: None,
    };
    let binding = CarBinding {
        target: CarTarget::Pinned,
        pin: Some(driver(3)),
    };
    assert_eq!(resolve_binding(&binding, &ctx), ResolvedCar::Ok(2));
}

#[test]
fn pinned_driver_with_no_pin_chosen_yet_is_not_in_session() {
    let list = grid();
    let ctx = BindingContext {
        participants: &list,
        player_car: Some(1),
        focus: None,
    };
    assert_eq!(
        resolve_binding(&target(CarTarget::Pinned), &ctx),
        ResolvedCar::NotInSession
    );
}

#[test]
fn car_ahead_and_behind_are_the_race_position_neighbours_of_the_focus_car() {
    let list = grid();
    let ctx = BindingContext {
        participants: &list,
        player_car: Some(1),
        focus: None,
    };
    assert_eq!(
        resolve_binding(&target(CarTarget::CarAhead), &ctx),
        ResolvedCar::Ok(0)
    );
    assert_eq!(
        resolve_binding(&target(CarTarget::CarBehind), &ctx),
        ResolvedCar::Ok(2)
    );
}

#[test]
fn the_leader_has_no_car_ahead() {
    let list = grid();
    let leader = driver(1);
    let ctx = BindingContext {
        participants: &list,
        player_car: Some(1),
        focus: Some(&leader),
    };
    assert_eq!(
        resolve_binding(&target(CarTarget::CarAhead), &ctx),
        ResolvedCar::NoNeighbour
    );
}

#[test]
fn neighbours_of_a_focus_not_in_session_are_not_in_session_either() {
    let list = grid();
    let ctx = BindingContext {
        participants: &list,
        player_car: None,
        focus: None,
    };
    assert_eq!(
        resolve_binding(&target(CarTarget::CarBehind), &ctx),
        ResolvedCar::NotInSession
    );
}

// Tile settings and host messages.

#[test]
fn car_setting_defaults_to_focus_and_keeps_valid_pins_only() {
    assert_eq!(car_binding_of(&json!({})), target(CarTarget::Focus));
    assert_eq!(
        car_binding_of(&json!({"car": {"target": "sideways"}})),
        target(CarTarget::Focus)
    );
    assert_eq!(
        car_binding_of(&json!({"car": {"target": "car-ahead"}})),
        target(CarTarget::CarAhead)
    );
    assert_eq!(
        car_binding_of(
            &json!({"car": {"target": "pinned", "pin": {"kind": "driver", "driverId": 7}}})
        ),
        CarBinding {
            target: CarTarget::Pinned,
            pin: Some(driver(7))
        }
    );
    assert_eq!(
        car_binding_of(&json!({"car": {"target": "pinned", "pin": {"kind": "nobody"}}})),
        target(CarTarget::Pinned)
    );
}

#[test]
fn a_missing_car_says_why_and_a_retired_car_still_draws() {
    let focus = target(CarTarget::Focus);
    assert_eq!(
        missing_car_message(&focus, ResolvedCar::NotInSession),
        Some("Car not in session")
    );
    assert_eq!(
        missing_car_message(&target(CarTarget::CarAhead), ResolvedCar::NoNeighbour),
        Some("No car ahead")
    );
    assert_eq!(
        missing_car_message(&target(CarTarget::CarBehind), ResolvedCar::NoNeighbour),
        Some("No car behind")
    );
    assert_eq!(missing_car_message(&focus, ResolvedCar::Retired(3)), None);
    assert_eq!(missing_car_message(&focus, ResolvedCar::Ok(3)), None);
    assert_eq!(ResolvedCar::Retired(3).index(), Some(3));
    assert_eq!(ResolvedCar::NotInSession.index(), None);
}
