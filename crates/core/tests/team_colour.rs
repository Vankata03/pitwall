//! Team colours for the timing tower's team marker and the track map's dots.
use pitwall_core::livery::Rgb;
use pitwall_core::livery::team_colour;
use pitwall_core::telemetry::{LiveryColour, ParticipantData};

fn participant(team_id: u16, num_colours: u8, first: (u8, u8, u8)) -> ParticipantData {
    let grey = LiveryColour {
        red: 50,
        green: 50,
        blue: 50,
    };
    ParticipantData {
        ai_controlled: 1,
        driver_id: 9,
        network_id: 255,
        team_id,
        my_team: 0,
        race_number: 44,
        nationality: 0,
        name: [0; 32],
        your_telemetry: 0,
        show_online_names: 1,
        tech_level: 0,
        platform: 0,
        num_colours,
        livery_colour: [
            LiveryColour {
                red: first.0,
                green: first.1,
                blue: first.2,
            },
            grey.clone(),
            grey.clone(),
            grey,
        ],
    }
}

#[test]
fn a_car_wears_its_first_game_livery_colour() {
    for team_id in 0..=u16::MAX {
        let car = participant(team_id, 4, (200, 30, 10));
        assert_eq!(team_colour(Some(&car)), Some(Rgb::new(200, 30, 10)));
    }
}

#[test]
fn the_first_colour_is_used_whatever_num_colours_says_as_on_the_web() {
    // tower and track map read `livery_colour[0]` without consulting `m_numColours`.
    assert_eq!(
        team_colour(Some(&participant(3, 0, (1, 2, 3)))),
        Some(Rgb::new(1, 2, 3))
    );
}

#[test]
fn a_car_without_participant_data_has_no_colour_and_is_drawn_muted() {
    assert_eq!(team_colour(None), None);
}
