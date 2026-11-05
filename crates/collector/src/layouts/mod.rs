//! The layout library lives in [`pitwall_core::layouts`]; this module adds its `/api/layouts` routes.
pub use pitwall_core::layouts::*;
mod routes;
pub use routes::routes;
