//! Car names as the dashboard shows them, ported from the label rules of
//! `dashboard/src/session/cars.ts`. Identity and binding resolution live in #67's
//! `binding` module; this is only how a car is printed.

use crate::telemetry::ParticipantData;

/// The game's 32-byte NUL-padded UTF-8 name, cut at the first NUL. Invalid UTF-8 is
/// replaced, never dropped.
pub fn decode_name(raw: &[u8; 32]) -> String {
    let end = raw.iter().position(|&byte| byte == 0).unwrap_or(raw.len());
    String::from_utf8_lossy(&raw[..end]).into_owned()
}

/// Whether the name may be displayed. AI names always show; a hidden online-human name
/// may contain a platform ID and stays private.
pub fn shows_name(participant: &ParticipantData) -> bool {
    participant.ai_controlled == 1 || participant.show_online_names != 0
}

/// The label rule on plain parts: the name as received when it may be shown and is not
/// blank, else `#<race number>`.
pub fn label_from(name: &str, race_number: u8, shows_name: bool) -> String {
    if shows_name && !name.trim().is_empty() {
        name.to_string()
    } else {
        format!("#{race_number}")
    }
}

/// Shown name of a car: `m_name` as received, or `#<race number>` for a human who hides it.
pub fn car_label(participant: &ParticipantData) -> String {
    label_from(
        &decode_name(&participant.name),
        participant.race_number,
        shows_name(participant),
    )
}
