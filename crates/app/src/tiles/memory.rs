//! What a placed tile keeps between renders. Most tiles keep nothing; Throttle/brake
//! traces keeps its rolling 30 s input history and Car status its livery pictures. A
//! layout host keeps one per placed tile, by tile id.

use super::{car_status, stint, throttle_brake_traces};

#[derive(Default)]
pub struct Memory {
    pub traces: throttle_brake_traces::Memory,
    pub car_status: car_status::Memory,
    /// Which completed sets a Stint tile shows expanded.
    pub stint: stint::Memory,
}
