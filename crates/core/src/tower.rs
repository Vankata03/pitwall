//! The Timing tower as data (spec TOWER-01..05), ported from
//! `dashboard/src/tiles/types/timing-tower.tsx`: which cars are listed in which order,
//! which rows fit, the gap column, and the text of every cell. Drawing is the tile's.
//!
//! The title note is [`crate::race_status::race_status`], or "Racing" when it is empty.
//! The team marker is the participant's first livery colour.

use serde_json::Value;

use crate::field::{FieldCar, signed_seconds};
use crate::model::{Gaps, PenaltyState, Readout};
use crate::screen::{Density, DensityRule};

/// The tower's declared densities: Regular and Full need 500 px of width.
pub const TOWER_DENSITIES: [DensityRule; 3] = [
    DensityRule {
        density: Density::Compact,
        min_width: 0.0,
        min_height: 0.0,
    },
    DensityRule {
        density: Density::Regular,
        min_width: 500.0,
        min_height: 240.0,
    },
    DensityRule {
        density: Density::Full,
        min_width: 500.0,
        min_height: 480.0,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GapMode {
    /// To the car one position ahead (default).
    Interval,
    Leader,
    /// Relative to the window's focus car.
    Focus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowsMode {
    /// The density decides: Compact shows the cars around focus.
    Auto,
    Full,
    Around,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TowerSettings {
    pub gap: GapMode,
    pub rows: RowsMode,
    /// Cars each side of focus, 0 to 12.
    pub radius: usize,
}

impl Default for TowerSettings {
    fn default() -> Self {
        Self {
            gap: GapMode::Interval,
            rows: RowsMode::Auto,
            radius: 3,
        }
    }
}

impl TowerSettings {
    /// Read the tile's saved settings; anything unknown or malformed takes the default.
    /// Write back only the key that changed, so unrelated keys survive.
    pub fn from_json(settings: &Value) -> Self {
        let gap = match settings.get("gap").and_then(Value::as_str) {
            Some("leader") => GapMode::Leader,
            Some("focus") => GapMode::Focus,
            _ => GapMode::Interval,
        };
        let rows = match settings.get("rows").and_then(Value::as_str) {
            Some("full") => RowsMode::Full,
            Some("around") => RowsMode::Around,
            _ => RowsMode::Auto,
        };
        let radius = settings
            .get("radius")
            .and_then(Value::as_f64)
            .filter(|r| r.is_finite())
            .map_or(3, |r| r.floor().clamp(0.0, 12.0) as usize);
        Self { gap, rows, radius }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TowerRow {
    /// Vehicle index of the car on this row.
    pub index: usize,
    /// The gap column: its text, with the state the cell shows.
    pub gap: Readout<String>,
    /// The window's focus car.
    pub focus: bool,
    /// Retired, did not finish, disqualified or not classified; drawn sunk.
    pub retired: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TowerTable {
    /// Header of the gap column.
    pub gap_label: String,
    pub rows: Vec<TowerRow>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TowerContent {
    /// A whole-tile message instead of the table.
    Message(&'static str),
    Table(TowerTable),
}

/// `m_resultStatus` values that sink a car: did not finish, disqualified, not classified,
/// retired.
const OUT: [u8; 4] = [4, 5, 6, 7];
const DSQ: u8 = 5;
const NOT_CLASSIFIED: u8 = 6;

fn result_status(car: &FieldCar) -> Option<u8> {
    match car.lap {
        Readout::Live(lap) | Readout::Stale(lap) => Some(lap.result_status),
        _ => None,
    }
}

fn is_out(car: &FieldCar) -> bool {
    car.retired || result_status(car).is_some_and(|status| OUT.contains(&status))
}

/// Rows that fit under the header at this density and height.
pub fn tower_capacity(density: Density, height: f32) -> usize {
    let (header, row) = match density {
        Density::Full => (120.0, 72.0),
        Density::Regular => (76.0, 32.0),
        Density::Compact => (80.0, 28.0),
    };
    ((height - header) / row).floor().max(0.0) as usize
}

fn gaps_value(readout: &Readout<Gaps>) -> Option<Gaps> {
    match readout {
        Readout::Live(g) | Readout::Stale(g) => Some(*g),
        _ => None,
    }
}

/// Text with a readout's state: values keep live or stale, missing data keeps its state.
fn with_state<T>(state: &Readout<T>, text: String) -> Readout<String> {
    match state {
        Readout::Live(_) => Readout::Live(text),
        Readout::Stale(_) => Readout::Stale(text),
        Readout::Restricted => Readout::Restricted,
        Readout::Absent => Readout::Absent,
    }
}

fn stale(text: Readout<String>) -> Readout<String> {
    match text {
        Readout::Live(text) => Readout::Stale(text),
        other => other,
    }
}

/// The timing tower for a field of cars. `session_type` is the session's
/// `m_sessionType` (15–17 race, 18 Time Trial); `focus` is the window's focus car (the
/// player car when following the player), `None` when it is not in the session.
pub fn timing_tower(
    field: &[FieldCar],
    session_type: Option<u8>,
    focus: Option<usize>,
    settings: &TowerSettings,
    density: Density,
    height: f32,
) -> TowerContent {
    let mut all: Vec<&FieldCar> = field
        .iter()
        .filter(|car| car.label.is_some() && !matches!(result_status(car), Some(0 | 1)))
        .collect();
    all.sort_by_key(|car| (is_out(car), car.position().unwrap_or(99)));
    if session_type == Some(18) {
        return TowerContent::Message("Time Trial \u{2014} only the player car runs");
    }
    if all.is_empty() {
        return TowerContent::Message("No timing data");
    }
    let race = matches!(session_type, Some(15..=17));
    let regular = density != Density::Compact;
    let capacity = tower_capacity(density, height);
    let around = settings.rows == RowsMode::Around
        || (settings.rows == RowsMode::Auto && !regular)
        || all.len() > capacity;

    let mut rows: Vec<usize> = (0..all.len()).collect(); // positions in `all`
    if around {
        let center = all
            .iter()
            .position(|car| Some(car.index) == focus)
            .unwrap_or(0);
        let start = center.saturating_sub(settings.radius);
        let end = (center + settings.radius + 1).min(all.len());
        rows = (start..end).collect();
        let pinned_leader = !regular && !rows.contains(&0);
        let available = capacity.saturating_sub(usize::from(pinned_leader));
        if rows.len() > available {
            // Keep the nearest cars, retaining their classification order.
            let mut nearest = rows.clone();
            nearest.sort_by_key(|&at| at.abs_diff(center));
            nearest.truncate(available);
            rows.retain(|at| nearest.contains(at));
        }
        if pinned_leader && capacity > 0 {
            rows.insert(0, 0);
        }
    }

    let leader = *all
        .iter()
        .find(|car| car.position() == Some(1))
        .unwrap_or(&all[0]);
    let focus_car = focus.and_then(|index| field.iter().find(|car| car.index == index));
    let focus_listed = focus.and_then(|index| all.iter().find(|car| car.index == index));
    let reference: Option<&FieldCar> = match settings.gap {
        GapMode::Focus if race => focus_car,
        GapMode::Focus if focus_car.is_some_and(|c| c.best_lap_ms.is_some()) => focus_car,
        _ => Some(leader),
    };
    let reference_is_focus =
        focus_car.is_some() && reference.map(|r| r.index) == focus_car.map(|f| f.index);
    let gap_label = match settings.gap {
        GapMode::Focus => {
            let to = if reference_is_focus {
                focus_listed
                    .and_then(|car| car.label.clone())
                    .unwrap_or_else(|| "focus".to_string())
            } else if race {
                "focus".to_string()
            } else {
                "session best".to_string()
            };
            format!("Gap to {to}")
        }
        _ if race && settings.gap == GapMode::Leader => "Leader".to_string(),
        _ if race => "Interval".to_string(),
        _ => "Session best".to_string(),
    };

    let rows = rows
        .into_iter()
        .map(|at| {
            let car = all[at];
            let position = car.position();
            let mut gap = with_state(&car.gaps, "\u{2014}".to_string());
            match (race, gaps_value(&car.gaps)) {
                (true, Some(mine)) => {
                    let compare = if settings.gap == GapMode::Interval {
                        let ahead = position.and_then(|p| p.checked_sub(1));
                        Some(
                            *all.iter()
                                .find(|other| ahead.is_some() && other.position() == ahead)
                                .unwrap_or(&leader),
                        )
                    } else {
                        reference
                    };
                    if let Some(other) = compare.and_then(|c| gaps_value(&c.gaps)) {
                        let laps = mine
                            .laps_behind
                            .zip(other.laps_behind)
                            .map(|(a, b)| a.abs_diff(b));
                        let text = match laps {
                            Some(laps) if laps > 0 => format!("+{laps} LAP"),
                            _ if position == Some(1) && settings.gap != GapMode::Focus => {
                                "Leader".to_string()
                            }
                            _ if settings.gap == GapMode::Interval => mine
                                .to_car_in_front_ms
                                .map_or("\u{2014}".to_string(), |ms| signed_seconds(i64::from(ms))),
                            _ => signed_seconds(
                                i64::from(mine.to_leader_ms) - i64::from(other.to_leader_ms),
                            ),
                        };
                        gap = with_state(&car.gaps, text);
                        if matches!(compare.map(|c| &c.gaps), Some(Readout::Stale(_))) {
                            gap = stale(gap);
                        }
                    }
                }
                (false, _) if car.best_lap_ms.is_some_and(|ms| ms > 0) => {
                    let best = car.best_lap_ms.unwrap_or_default();
                    let text = if position == Some(1) {
                        lap_time_text(Some(best))
                    } else {
                        match reference.and_then(|r| r.best_lap_ms).filter(|&ms| ms > 0) {
                            Some(reference_best) => {
                                signed_seconds(i64::from(best) - i64::from(reference_best))
                            }
                            None => "\u{2014}".to_string(),
                        }
                    };
                    gap = with_state(&car.lap, text);
                }
                _ => {}
            }
            TowerRow {
                index: car.index,
                gap,
                focus: Some(car.index) == focus,
                retired: is_out(car),
            }
        })
        .collect();
    TowerContent::Table(TowerTable { gap_label, rows })
}

/// `m:ss.mmm`; no time, or zero, is an em dash.
pub fn lap_time_text(ms: Option<u32>) -> String {
    match ms {
        Some(ms) if ms > 0 => format!("{}:{:06.3}", ms / 60_000, f64::from(ms % 60_000) / 1000.0),
        _ => "\u{2014}".to_string(),
    }
}

/// The tyre badge letter for a visual compound; unknown compounds show their number.
pub fn compound_letter(visual_compound: u8) -> String {
    match visual_compound {
        16 | 20 => "S",
        17 | 21 => "M",
        18 | 22 => "H",
        7 => "I",
        8 => "W",
        19 => "SS",
        other => return other.to_string(),
    }
    .to_string()
}

/// Accessible name of a badge letter, "<name> tyres" in the tile.
pub fn compound_name(letter: &str) -> String {
    match letter {
        "S" => "Soft".into(),
        "M" => "Medium".into(),
        "H" => "Hard".into(),
        "I" => "Intermediate".into(),
        "W" => "Wet".into(),
        "SS" => "Super soft".into(),
        other => format!("Compound {other}"),
    }
}

/// The Status column. `retired` is the model's retired flag.
pub fn status_text(result_status: u8, retired: bool, pit_status: u8) -> &'static str {
    if result_status == DSQ {
        "DSQ"
    } else if result_status == NOT_CLASSIFIED {
        "NC"
    } else if retired || OUT.contains(&result_status) {
        "Retired"
    } else if pit_status != 0 {
        "PIT"
    } else {
        "Running"
    }
}

/// `5s / 2 TL`, plus drive-throughs and stop-gos when there are any.
pub fn penalties_text(p: &PenaltyState) -> String {
    let mut text = format!("{}s / {} TL", p.seconds, p.corner_cutting_warnings);
    if p.drive_throughs > 0 {
        text.push_str(&format!(" / {} DT", p.drive_throughs));
    }
    if p.stop_gos > 0 {
        text.push_str(&format!(" / {} SG", p.stop_gos));
    }
    text
}

/// Highest wheel wear from the tyre condition (percent remaining, RL RR FL FR).
pub fn wear_text(condition: [f32; 4]) -> String {
    let worst = condition.into_iter().fold(f32::INFINITY, f32::min);
    format!("{}%", (100.0 - worst).round())
}

/// Places gained since the grid, signed; no grid slot is an em dash.
pub fn gained_text(grid_position: u8, position: u8) -> String {
    if grid_position == 0 {
        return "\u{2014}".to_string();
    }
    let gained = i32::from(grid_position) - i32::from(position);
    if gained >= 0 {
        format!("+{gained}")
    } else {
        gained.to_string()
    }
}
