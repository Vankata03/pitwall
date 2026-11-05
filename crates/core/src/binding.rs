//! Cars as a UI needs them: who is in the session, which car is the focus, and which car
//! a tile follows (spec section 5; parity BIND-01..04).
//!
//! A car is recognised across sessions by identity, never by array slot: AI and offline
//! humans by driver id, online humans by name (the network id is session-scoped). A
//! binding that cannot be met says so; a retired car keeps its binding and is reported
//! as retired, with no silent fallback to another car.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    car_names::{decode_name, label_from, shows_name},
    model::SessionModel,
    telemetry::ParticipantData,
};

/// The game sends 255 as the network id of a car that is not an online human.
const NOT_NETWORKED: u16 = 255;

/// How a car is recognised across sessions and devices. Serialises as the browser
/// dashboard stored it: `{"kind":"driver","driverId":9}` or `{"kind":"name","name":"Max"}`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "lowercase",
    rename_all_fields = "camelCase"
)]
pub enum CarIdentity {
    Driver { driver_id: u16 },
    Name { name: String },
}

/// One car with a participant record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Participant {
    /// Vehicle index: identifies the car within the session only.
    pub index: usize,
    pub name: String,
    pub race_number: u8,
    /// Whether the name may be displayed; hidden online-human names can hold platform IDs.
    pub show_online_names: bool,
    pub identity: CarIdentity,
    pub retired: bool,
    /// Race position, when Lap Data has it.
    pub position: Option<u8>,
}

pub fn identity_of(data: &ParticipantData) -> CarIdentity {
    if data.ai_controlled == 0 && data.network_id != NOT_NETWORKED {
        CarIdentity::Name {
            name: decode_name(&data.name),
        }
    } else {
        CarIdentity::Driver {
            driver_id: data.driver_id,
        }
    }
}

/// Every valid car with a live or stale participant record, in vehicle-index order.
pub fn participants_of(model: &SessionModel) -> Vec<Participant> {
    (0..crate::telemetry::MAX_CARS)
        .filter_map(|index| {
            let car = model.car(index)?;
            let data = *car.participant().value()?;
            Some(Participant {
                index,
                name: decode_name(&data.name),
                race_number: data.race_number,
                show_online_names: shows_name(data),
                identity: identity_of(data),
                retired: car.retired(),
                position: car.lap().value().map(|lap| lap.car_position),
            })
        })
        .collect()
}

/// The shown name, by the `car_names` rule: the name as received, or `#<race number>`
/// when hidden or blank.
pub fn car_label(p: &Participant) -> String {
    label_from(&p.name, p.race_number, p.show_online_names)
}

/// The stored form of an identity (JSON, as the browser stored it).
pub fn identity_key(identity: &CarIdentity) -> String {
    serde_json::to_string(identity).expect("an identity always serialises")
}

pub fn parse_identity(text: &str) -> Option<CarIdentity> {
    serde_json::from_str(text).ok()
}

/// Where a binding landed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedCar {
    Ok(usize),
    /// The bound car, retired: the tile draws it and says so.
    Retired(usize),
    NotInSession,
    /// Car ahead or behind with nobody there.
    NoNeighbour,
}

impl ResolvedCar {
    /// The car to draw, for `Ok` and `Retired`.
    pub fn index(self) -> Option<usize> {
        match self {
            Self::Ok(index) | Self::Retired(index) => Some(index),
            Self::NotInSession | Self::NoNeighbour => None,
        }
    }

    fn of(p: &Participant) -> Self {
        if p.retired {
            Self::Retired(p.index)
        } else {
            Self::Ok(p.index)
        }
    }
}

pub fn resolve_identity(identity: &CarIdentity, participants: &[Participant]) -> ResolvedCar {
    participants
        .iter()
        .find(|p| p.identity == *identity)
        .map_or(ResolvedCar::NotInSession, ResolvedCar::of)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CarTarget {
    #[default]
    Focus,
    Pinned,
    CarAhead,
    CarBehind,
}

/// A follows-car tile's car setting, stored as `settings.car`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CarBinding {
    pub target: CarTarget,
    pub pin: Option<CarIdentity>,
}

#[derive(Debug, Clone, Copy)]
pub struct BindingContext<'a> {
    pub participants: &'a [Participant],
    pub player_car: Option<usize>,
    /// The window's chosen focus car; `None` follows the player car.
    pub focus: Option<&'a CarIdentity>,
}

/// The car the window focuses on. A missing player car is not car 0.
pub fn resolve_focus(ctx: &BindingContext) -> ResolvedCar {
    if let Some(focus) = ctx.focus {
        return resolve_identity(focus, ctx.participants);
    }
    ctx.player_car
        .and_then(|player| ctx.participants.iter().find(|p| p.index == player))
        .map_or(ResolvedCar::NotInSession, ResolvedCar::of)
}

pub fn resolve_binding(binding: &CarBinding, ctx: &BindingContext) -> ResolvedCar {
    match binding.target {
        CarTarget::Pinned => binding
            .pin
            .as_ref()
            .map_or(ResolvedCar::NotInSession, |pin| {
                resolve_identity(pin, ctx.participants)
            }),
        CarTarget::Focus => resolve_focus(ctx),
        CarTarget::CarAhead | CarTarget::CarBehind => {
            let focus = resolve_focus(ctx);
            let Some(index) = focus.index() else {
                return focus;
            };
            let Some(here) = ctx
                .participants
                .iter()
                .find(|p| p.index == index)
                .and_then(|p| p.position)
            else {
                return ResolvedCar::NoNeighbour;
            };
            let wanted = if binding.target == CarTarget::CarAhead {
                here.checked_sub(1)
            } else {
                here.checked_add(1)
            };
            wanted
                .and_then(|wanted| ctx.participants.iter().find(|p| p.position == Some(wanted)))
                .map_or(ResolvedCar::NoNeighbour, ResolvedCar::of)
        }
    }
}

/// A tile's car setting from its settings object; anything invalid reads as Focus, and
/// a pin is kept only when it parses as an identity.
pub fn car_binding_of(settings: &Value) -> CarBinding {
    let Some(car) = settings.get("car").filter(|c| c.is_object()) else {
        return CarBinding::default();
    };
    let target = match car.get("target").and_then(Value::as_str) {
        Some("focus") => CarTarget::Focus,
        Some("pinned") => CarTarget::Pinned,
        Some("car-ahead") => CarTarget::CarAhead,
        Some("car-behind") => CarTarget::CarBehind,
        _ => return CarBinding::default(),
    };
    let pin = car
        .get("pin")
        .and_then(|pin| serde_json::from_value(pin.clone()).ok());
    CarBinding { target, pin }
}

impl CarTarget {
    /// Every target, in the order the settings list them.
    pub const ALL: [CarTarget; 4] = [
        CarTarget::Focus,
        CarTarget::Pinned,
        CarTarget::CarAhead,
        CarTarget::CarBehind,
    ];

    /// The stored value.
    pub fn key(self) -> &'static str {
        match self {
            CarTarget::Focus => "focus",
            CarTarget::Pinned => "pinned",
            CarTarget::CarAhead => "car-ahead",
            CarTarget::CarBehind => "car-behind",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            CarTarget::Focus => "Focus car",
            CarTarget::Pinned => "Pinned driver",
            CarTarget::CarAhead => "Car ahead",
            CarTarget::CarBehind => "Car behind",
        }
    }
}

/// A car setting as stored in `settings.car`: `{"target": ..., "pin": ...}`, the pin only
/// when there is one. A pin stays saved while another target is chosen.
pub fn car_binding_value(binding: &CarBinding) -> Value {
    let mut car = serde_json::Map::new();
    car.insert("target".into(), Value::from(binding.target.key()));
    if let Some(pin) = &binding.pin {
        car.insert(
            "pin".into(),
            serde_json::to_value(pin).expect("an identity always serialises"),
        );
    }
    Value::Object(car)
}

/// What a tile shows instead of its content when its car is missing. A retired car
/// still draws (with a Retired label), so it has no message.
pub fn missing_car_message(binding: &CarBinding, resolved: ResolvedCar) -> Option<&'static str> {
    match resolved {
        ResolvedCar::Ok(_) | ResolvedCar::Retired(_) => None,
        ResolvedCar::NotInSession => Some("Car not in session"),
        ResolvedCar::NoNeighbour => Some(match binding.target {
            CarTarget::CarBehind => "No car behind",
            _ => "No car ahead",
        }),
    }
}
