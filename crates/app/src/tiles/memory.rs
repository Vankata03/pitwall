//! What a placed tile keeps between renders. Most tiles keep nothing; Throttle/brake
//! traces keeps its rolling 30 s input history, Car status its livery pictures, the
//! Race-control feed its held headlines and Track map its fit and map. A
//! layout host keeps one per placed tile, by tile id.

use super::{car_status, live_delta, race_control_feed, stint, throttle_brake_traces, track_map};

#[derive(Default)]
pub struct Memory {
    pub delta: live_delta::Memory,
    pub traces: throttle_brake_traces::Memory,
    pub car_status: car_status::Memory,
    pub race_control_feed: race_control_feed::Memory,
    pub track_map: track_map::Memory,
    /// Which completed sets a Stint tile shows expanded.
    pub stint: stint::Memory,
}
