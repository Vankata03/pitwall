//! UI-free trace logic shared by the throttle/brake overlay and the inputs tile: the pedal
//! smoothing, the rolling input history, reference-lap alignment by lap distance, the
//! current-lap bucket grid and the time-window plot. Ported from
//! `dashboard/src/tiles/traces` and `dashboard/src/overlay`.

mod current_lap;
mod overlay;
mod smooth;
mod window;

/// A polyline in a drawing box: `(x, y)` with y down, so 100 % sits at the top.
pub type Polyline = Vec<(f64, f64)>;

pub use current_lap::{BUCKET_METRES, CurrentLap, LapSamples};
pub use overlay::{InputTrace, OVERLAY_WINDOW_SECONDS, ReferenceTrace, input_trace};
pub use smooth::{PEDAL_SMOOTH_SECONDS, smooth_line, smooth_values};
pub use window::{
    CHANNEL_ORDER, Channel, GAP_SECONDS, HistoryIdentity, INPUT_WINDOW_SECONDS, InputHistory,
    InputSample, Lap, PlotSeries, WindowPlot, build_window_plot, reference_value,
};
