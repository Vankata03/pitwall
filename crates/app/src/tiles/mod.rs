//! Tiles. Each tile is two halves: a pure view model built from core data (tested
//! without a window) and a gpui render of that view model.

pub mod car_status;
pub mod engine_temp;
pub mod ers;
pub mod format;
pub mod fuel;
pub mod g_force;
pub mod gaps;
pub mod lap_state;
pub mod lap_timing;
pub mod live_delta;
pub mod memory;
pub mod notice;
mod paint;
pub mod race_control_feed;
mod slow;
pub mod sm;
pub mod speed_rpm_gear;
pub mod stint;
pub mod throttle_brake_traces;
pub mod timing_tower;
pub mod track_discipline;
pub mod track_map;
pub mod weather;
mod widgets;

/// How much a tile shows at its current pixel size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Density {
    Compact,
    Regular,
    Full,
}

/// Width and height thresholds (px) for Regular and Full; Compact starts at 0×0.
#[derive(Debug, Clone, Copy)]
pub struct Densities {
    pub regular: (f32, f32),
    pub full: Option<(f32, f32)>,
}

impl Densities {
    /// The last density whose width AND height thresholds the tile reaches.
    pub fn pick(&self, width: f32, height: f32) -> Density {
        let fits = |(w, h): (f32, f32)| width >= w && height >= h;
        if self.full.is_some_and(fits) {
            Density::Full
        } else if fits(self.regular) {
            Density::Regular
        } else {
            Density::Compact
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn density_needs_both_width_and_height() {
        let d = Densities {
            regular: (230.0, 150.0),
            full: Some((330.0, 370.0)),
        };
        assert_eq!(d.pick(229.0, 400.0), Density::Compact);
        assert_eq!(d.pick(230.0, 150.0), Density::Regular);
        assert_eq!(d.pick(500.0, 369.0), Density::Regular);
        assert_eq!(d.pick(330.0, 370.0), Density::Full);
    }
}

#[cfg(test)]
mod lap_timing_tests;

#[cfg(test)]
mod live_delta_tests;

#[cfg(test)]
mod remaining_tests;

#[cfg(test)]
mod timing_tests;
