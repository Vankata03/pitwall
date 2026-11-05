//! Live ingest: the UI-free gate, config and UDP socket live in [`pitwall_core::ingest`];
//! this module adds the collector runtime that wires them to HTTP, the tray and the recorder.
pub use pitwall_core::ingest::*;
pub mod runtime;
