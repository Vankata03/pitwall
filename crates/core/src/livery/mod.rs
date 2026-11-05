//! Livery choice and the fallback car (spec sections 8 and 13), ported from
//! `dashboard/src/liveries/choose.ts` and `fallbackCar.ts`.
//!
//! [`choose_livery`] picks the pack image for a car from the pack listing
//! ([`crate::liveries::pack_files`]); without one, the car is drawn as the fallback car,
//! [`fallback_car_svg`], recoloured with the car's livery colours.

mod fallback_car;

pub use fallback_car::{
    FALLBACK_CAR_HEIGHT, FALLBACK_CAR_WIDTH, MASK_FILLS, fallback_car_mask_svg, fallback_car_svg,
};

use crate::telemetry::ParticipantData;

/// An sRGB colour as the game sends it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

/// Livery colour slots the fallback car recolours, in the game's order.
pub const LIVERY_SLOTS: usize = 4;

/// Last resort when the game sends no colour for a slot.
pub const NEUTRAL_COLOURS: [Rgb; LIVERY_SLOTS] = [
    Rgb::new(154, 167, 173),
    Rgb::new(232, 237, 239),
    Rgb::new(90, 100, 105),
    Rgb::new(27, 31, 33),
];

/// The fields of a car's Participants entry the livery choice reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveryCar {
    pub team_id: u16,
    /// `m_myTeam`: the player's custom team.
    pub my_team: bool,
    /// `m_numColours`: how many of `colours` the game filled in.
    pub num_colours: u8,
    pub colours: [Rgb; LIVERY_SLOTS],
}

impl LiveryCar {
    pub fn from_participant(participant: &ParticipantData) -> Self {
        Self {
            team_id: participant.team_id,
            my_team: participant.my_team != 0,
            num_colours: participant.num_colours,
            colours: participant
                .livery_colour
                .clone()
                .map(|c| Rgb::new(c.red, c.green, c.blue)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiveryChoice {
    /// A pack image, by its file name in the pack folder.
    Pack {
        file: String,
        colours: [Rgb; LIVERY_SLOTS],
    },
    /// The fallback car painted with `colours`.
    Fallback { colours: [Rgb; LIVERY_SLOTS] },
}

impl LiveryChoice {
    /// The car's resolved colours for recolouring the fallback art.
    pub fn colours(&self) -> &[Rgb; LIVERY_SLOTS] {
        match self {
            LiveryChoice::Pack { colours, .. } | LiveryChoice::Fallback { colours } => colours,
        }
    }
}

/// Game colours first, in game order; a slot the game did not send is neutral.
fn resolve_colours(car: &LiveryCar) -> [Rgb; LIVERY_SLOTS] {
    let sent = usize::from(car.num_colours).min(LIVERY_SLOTS);
    std::array::from_fn(|slot| {
        if slot < sent {
            car.colours[slot]
        } else {
            NEUTRAL_COLOURS[slot]
        }
    })
}

/// Pick the picture for a car from the pack's file names: `my-team.png` for the player's
/// custom team, else `<teamId>.png`, else the fallback car. Only exact names match. A car
/// without participant data (`None`) gets the neutral fallback. An image that later fails
/// to load should be drawn as `Fallback` with the same colours.
pub fn choose_livery(car: Option<&LiveryCar>, files: &[String]) -> LiveryChoice {
    let Some(car) = car else {
        return LiveryChoice::Fallback {
            colours: NEUTRAL_COLOURS,
        };
    };
    let colours = resolve_colours(car);
    let team_file = format!("{}.png", car.team_id);
    let wanted = car
        .my_team
        .then_some("my-team.png")
        .into_iter()
        .chain([team_file.as_str()]);
    for name in wanted {
        if files.iter().any(|file| file == name) {
            return LiveryChoice::Pack {
                file: name.to_string(),
                colours,
            };
        }
    }
    LiveryChoice::Fallback { colours }
}

/// A car's team colour, as the timing tower's team marker and the track map's dots draw
/// it: the first game livery colour, read whatever `m_numColours` says, as the web build
/// does. `None` (no participant data yet) is drawn in the theme's muted text colour.
/// Team identity is game colours only (spec section 14); there is no team colour table.
pub fn team_colour(participant: Option<&ParticipantData>) -> Option<Rgb> {
    participant.map(|p| {
        let c = &p.livery_colour[0];
        Rgb::new(c.red, c.green, c.blue)
    })
}
