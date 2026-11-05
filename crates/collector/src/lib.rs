//! The web collector: HTTP serving, the wire stream, the tray app and the overlay host,
//! built on the UI-free core in [`pitwall_core`]. Core modules are re-exported under their
//! old paths so `pitwall_collector::model` and friends keep working.

pub use pitwall_core::{model, names, paths, pitrec, recorder, telemetry};

pub mod assets;
pub mod http;
pub mod ingest;
pub mod layouts;
pub mod liveries;
pub mod notice;
pub mod overlay;
pub mod tracks;

pub mod wire;

pub mod replay;

pub mod status;

pub mod app;
pub mod discovery;
