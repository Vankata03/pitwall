//! The UI-free pitwall core: telemetry parsing, ingest, the session model, the recorder,
//! replay, the track store, layout storage, per-user paths and the livery pack lookup.
//! Nothing here serves HTTP or draws a window; the collector and native UIs build on it.

pub mod data;
pub mod feed;
pub mod ingest;
pub mod layouts;
pub mod liveries;
pub mod model;
pub mod names;
pub mod overlay;
pub mod paths;
pub mod pitrec;
pub mod recorder;
pub mod replay;
pub mod telemetry;
pub mod tracks;
