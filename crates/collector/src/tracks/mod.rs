//! The track store lives in [`pitwall_core::tracks`]; this module adds its `/api/tracks` routes.
pub use pitwall_core::tracks::*;
mod routes;
pub use routes::routes;
