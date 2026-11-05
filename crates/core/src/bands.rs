//! Temperature bands and the other unit-free rules of the Car status tile (spec section
//! 10, Colour), ported from `dashboard/src/tiles/car-status/bands.ts` and `view.ts`.
//!
//! PROVISIONAL. The band numbers are placeholders chosen to make the tile usable. The real
//! values come from recordings in the calibration task (#52), which replaces both tables
//! and clears [`BANDS_ARE_PROVISIONAL`]. Nothing else holds a temperature threshold.
//!
//! Every band and column fill is computed in °C, whatever the display unit.

use crate::telemetry::CarDamageData;

pub const BANDS_ARE_PROVISIONAL: bool = true;

/// Below `cold`, cold; above `hot`, hot; between the two, inclusive, in window. °C.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Band {
    pub cold: f32,
    pub hot: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BandName {
    Cold,
    Window,
    Hot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemperatureKind {
    /// Tyre surface or inner temperature; banded by actual compound.
    Tyre,
    /// Brake disc temperature.
    Brake,
}

/// Brake discs, °C.
pub const BRAKE_BAND: Band = Band {
    cold: 300.0,
    hot: 850.0,
};

/// Tyre band by actual compound ID (`m_actualTyreCompound`), for surface and inner.
pub fn tyre_band(actual_compound: u8) -> Option<Band> {
    let (cold, hot) = match actual_compound {
        22 => (78.0, 98.0),  // C6
        16 => (80.0, 100.0), // C5
        17 => (82.0, 102.0), // C4
        18 => (84.0, 104.0), // C3
        19 => (86.0, 106.0), // C2
        20 => (88.0, 108.0), // C1
        21 => (90.0, 110.0), // C0
        7 => (60.0, 85.0),   // intermediate
        8 => (50.0, 75.0),   // wet
        _ => return None,
    };
    Some(Band { cold, hot })
}

/// Band of a temperature in °C. A tyre with an unknown or missing compound has no band:
/// the tile shows the number without colour or glyph.
pub fn band_of(
    kind: TemperatureKind,
    celsius: f32,
    actual_compound: Option<u8>,
) -> Option<BandName> {
    let band = match kind {
        TemperatureKind::Brake => BRAKE_BAND,
        TemperatureKind::Tyre => tyre_band(actual_compound?)?,
    };
    Some(if celsius < band.cold {
        BandName::Cold
    } else if celsius > band.hot {
        BandName::Hot
    } else {
        BandName::Window
    })
}

pub const BAND_GLYPH_COLD: &str = "\u{25BC}";
pub const BAND_GLYPH_HOT: &str = "\u{25B2}";

/// Cold and hot show an arrow beside the number, so colour never carries it alone.
pub fn band_glyph(band: Option<BandName>) -> &'static str {
    match band {
        Some(BandName::Cold) => BAND_GLYPH_COLD,
        Some(BandName::Hot) => BAND_GLYPH_HOT,
        _ => "",
    }
}

/// Full scale of a thermal column, °C. Provisional too.
pub const TYRE_COLUMN_MAX: f32 = 140.0;
pub const BRAKE_COLUMN_MAX: f32 = 1000.0;

/// Column height, 0 to 1 of the column's scale, from °C.
pub fn column_fill(kind: TemperatureKind, celsius: f32) -> f32 {
    let max = match kind {
        TemperatureKind::Tyre => TYRE_COLUMN_MAX,
        TemperatureKind::Brake => BRAKE_COLUMN_MAX,
    };
    (celsius / max).clamp(0.0, 1.0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionLevel {
    Healthy,
    Mid,
    Low,
}

/// Tyre condition level from percent remaining (clamped 0 to 100 by the caller). Not in
/// the spec; colour only repeats the number.
pub fn condition_level(percent: f32) -> ConditionLevel {
    if percent < 40.0 {
        ConditionLevel::Low
    } else if percent < 70.0 {
        ConditionLevel::Mid
    } else {
        ConditionLevel::Healthy
    }
}

/// Name of a visual compound (`m_visualTyreCompound`).
pub fn visual_compound_name(visual_compound: u8) -> &'static str {
    match visual_compound {
        16 => "Soft",
        17 => "Medium",
        18 => "Hard",
        7 => "Inter",
        8 => "Wet",
        _ => "Unknown",
    }
}

/// One damage row of the Car status tile.
#[derive(Debug, Clone, Copy)]
pub struct DamagePart {
    pub key: &'static str,
    pub label: &'static str,
    /// The part's damage percent in the Car Damage packet.
    pub read: fn(&CarDamageData) -> u8,
}

/// Damage rows in display order.
pub const DAMAGE_PARTS: [DamagePart; 8] = [
    DamagePart {
        key: "front-left-wing",
        label: "Front left wing",
        read: |d| d.front_left_wing_damage,
    },
    DamagePart {
        key: "front-right-wing",
        label: "Front right wing",
        read: |d| d.front_right_wing_damage,
    },
    DamagePart {
        key: "rear-wing",
        label: "Rear wing",
        read: |d| d.rear_wing_damage,
    },
    DamagePart {
        key: "floor",
        label: "Floor",
        read: |d| d.floor_damage,
    },
    DamagePart {
        key: "diffuser",
        label: "Diffuser",
        read: |d| d.diffuser_damage,
    },
    DamagePart {
        key: "sidepod",
        label: "Sidepod",
        read: |d| d.sidepod_damage,
    },
    DamagePart {
        key: "gearbox",
        label: "Gearbox",
        read: |d| d.gear_box_damage,
    },
    DamagePart {
        key: "engine",
        label: "Engine",
        read: |d| d.engine_damage,
    },
];
