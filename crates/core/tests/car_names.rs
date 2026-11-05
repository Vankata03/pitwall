//! Port of the "labels" part of dashboard/src/session/cars.test.ts. Identity, participant
//! listing and binding resolution are #67's (`pitwall_core::binding`).
use pitwall_core::car_names::{car_label, decode_name, label_from, shows_name};
use pitwall_core::telemetry::{LiveryColour, ParticipantData};

fn bytes(text: &str) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[..text.len()].copy_from_slice(text.as_bytes());
    out
}

fn participant(ai_controlled: u8, network_id: u16, show_online_names: u8, name: &str) -> ParticipantData {
    let colour = LiveryColour {
        red: 0,
        green: 0,
        blue: 0,
    };
    ParticipantData {
        ai_controlled,
        driver_id: 9,
        network_id,
        team_id: 3,
        my_team: 0,
        race_number: 44,
        nationality: 0,
        name: bytes(name),
        your_telemetry: 0,
        show_online_names,
        tech_level: 0,
        platform: 0,
        num_colours: 0,
        livery_colour: [colour.clone(), colour.clone(), colour.clone(), colour],
    }
}

#[test]
fn names_decode_up_to_the_first_nul_as_utf8() {
    assert_eq!(decode_name(&bytes("H\u{fc}lkenberg")), "H\u{fc}lkenberg");
    let mut trailing = bytes("Alonso");
    trailing[10] = b'x'; // bytes after the terminator are ignored
    assert_eq!(decode_name(&trailing), "Alonso");
    assert_eq!(decode_name(&[b'A'; 32]), "A".repeat(32));
}

#[test]
fn name_as_received() {
    assert_eq!(label_from("Max", 33, true), "Max");
    assert_eq!(car_label(&participant(1, 255, 1, "HAMILTON")), "HAMILTON");
}

#[test]
fn ai_names_remain_visible_when_the_online_name_flag_is_off() {
    let ai = participant(1, 65535, 0, "P\u{c9}REZ");
    assert!(shows_name(&ai));
    assert_eq!(car_label(&ai), "P\u{c9}REZ");
}

#[test]
fn hidden_online_human_names_remain_private_and_empty_names_fall_back_to_race_number() {
    let human = participant(0, 17, 0, "private-platform-id");
    assert!(!shows_name(&human));
    assert_eq!(car_label(&human), "#44");
    assert_eq!(label_from("  ", 33, true), "#33");
}

#[test]
fn a_human_who_hides_their_online_name_is_shown_by_race_number() {
    assert_eq!(label_from("Max", 33, false), "#33");
}
