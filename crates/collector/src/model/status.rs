//! Split status fields before storage so withheld values cannot leak as zeroes.
use crate::telemetry::CarStatusData;

/// Status fields that the game never restricts.
#[derive(Debug, Clone, PartialEq)]
pub struct StatusData {
    pub traction_control: u8,
    pub anti_lock_brakes: u8,
    pub pit_limiter_status: u8,
    pub max_rpm: u16,
    pub idle_rpm: u16,
    pub max_gears: u8,
    pub drs_allowed: u8,
    pub drs_activation_distance: u16,
    pub actual_tyre_compound: u8,
    pub visual_tyre_compound: u8,
    pub tyres_age_laps: u8,
    pub vehicle_fia_flags: i8,
    pub network_paused: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FuelData {
    pub mix: u8,
    pub in_tank: f32,
    pub capacity: f32,
    pub remaining_laps: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ErsData {
    pub store_energy: f32,
    pub deploy_mode: u8,
    pub harvested_this_lap_mguk: f32,
    pub harvested_this_lap_mguh: f32,
    pub deployed_this_lap: f32,
    pub harvest_limit_per_lap: Option<f32>,
    pub engine_power_ice: f32,
    pub engine_power_mguk: f32,
}

impl From<&CarStatusData> for StatusData {
    fn from(s: &CarStatusData) -> Self {
        Self {
            traction_control: s.traction_control,
            anti_lock_brakes: s.anti_lock_brakes,
            pit_limiter_status: s.pit_limiter_status,
            max_rpm: s.max_rpm,
            idle_rpm: s.idle_rpm,
            max_gears: s.max_gears,
            drs_allowed: s.drs_allowed,
            drs_activation_distance: s.drs_activation_distance,
            actual_tyre_compound: s.actual_tyre_compound,
            visual_tyre_compound: s.visual_tyre_compound,
            tyres_age_laps: s.tyres_age_laps,
            vehicle_fia_flags: s.vehicle_fia_flags,
            network_paused: s.network_paused,
        }
    }
}

impl From<&CarStatusData> for FuelData {
    fn from(s: &CarStatusData) -> Self {
        Self {
            mix: s.fuel_mix,
            in_tank: s.fuel_in_tank,
            capacity: s.fuel_capacity,
            remaining_laps: s.fuel_remaining_laps,
        }
    }
}

impl From<&CarStatusData> for ErsData {
    fn from(s: &CarStatusData) -> Self {
        Self {
            store_energy: s.ers_store_energy,
            deploy_mode: s.ers_deploy_mode,
            harvested_this_lap_mguk: s.ers_harvested_this_lap_mguk,
            harvested_this_lap_mguh: s.ers_harvested_this_lap_mguh,
            deployed_this_lap: s.ers_deployed_this_lap,
            harvest_limit_per_lap: s.ers_harvest_limit_per_lap,
            engine_power_ice: s.engine_power_ice,
            engine_power_mguk: s.engine_power_mguk,
        }
    }
}
