//! The UI-free pitwall core: telemetry parsing, ingest, the session model, the recorder,
//! replay, the track store, layout storage, per-user paths and the livery pack lookup.
//! Nothing here serves HTTP or draws a window; the collector and native UIs build on it.

pub mod bands;
pub mod banners;
pub mod car_names;
pub mod data;
pub mod feed;
pub mod field;
pub mod gaps;
pub mod ingest;
pub mod lap_traces;
pub mod layout_edit;
pub mod layouts;
pub mod liveries;
pub mod livery;
pub mod model;
pub mod names;
pub mod overlay;
pub mod paths;
pub mod pitrec;
pub mod race_status;
pub mod recorder;
pub mod replay;
pub mod screen;
pub mod telemetry;
pub mod tower;
pub mod tracks;
