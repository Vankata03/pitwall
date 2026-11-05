//! Readout states as a UI presents them (spec section 10, Missing data; parity READ-01/02).
//!
//! Every value a tile shows is in one of four states. Live shows the value; stale keeps
//! the last value, dimmed and marked; restricted shows "Restricted"; absent shows an em
//! dash. Zero never stands in for a missing value. A whole-tile message replaces the
//! tile only when none of its readouts has data.

use crate::model::{Lifecycle, Readout};

pub const RESTRICTED_TEXT: &str = "Restricted";
pub const ABSENT_TEXT: &str = "\u{2014}";

/// The state of one readout, without its value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReadoutState {
    Live,
    Stale,
    Restricted,
    Absent,
}

impl ReadoutState {
    /// A stale value still has data to show; restricted and absent do not.
    pub fn has_data(self) -> bool {
        matches!(self, Self::Live | Self::Stale)
    }
}

impl<T> Readout<T> {
    pub fn state(&self) -> ReadoutState {
        match self {
            Self::Live(_) => ReadoutState::Live,
            Self::Stale(_) => ReadoutState::Stale,
            Self::Restricted => ReadoutState::Restricted,
            Self::Absent => ReadoutState::Absent,
        }
    }

    /// The value, for live and stale only.
    pub fn value(&self) -> Option<&T> {
        match self {
            Self::Live(value) | Self::Stale(value) => Some(value),
            Self::Restricted | Self::Absent => None,
        }
    }
}

/// What to draw for a readout: the state drives styling (stale dims and adds a marker).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadoutView {
    pub state: ReadoutState,
    pub text: String,
}

/// The single rendering rule. `value_text` is the tile's formatted value; it is shown for
/// live and stale only.
pub fn readout_view(state: ReadoutState, value_text: &str) -> ReadoutView {
    let text = match state {
        ReadoutState::Live | ReadoutState::Stale => value_text,
        ReadoutState::Restricted => RESTRICTED_TEXT,
        ReadoutState::Absent => ABSENT_TEXT,
    };
    ReadoutView {
        state,
        text: text.to_owned(),
    }
}

/// Render a model readout; `format` runs only when there is a value.
pub fn view_of<T>(readout: &Readout<T>, format: impl FnOnce(&T) -> String) -> ReadoutView {
    match readout.value() {
        Some(value) => ReadoutView {
            state: readout.state(),
            text: format(value),
        },
        None => readout_view(readout.state(), ""),
    }
}

/// While the data source is unreachable every live value is only the last one seen.
/// In-process the core is always connected; the rule stays for a future remote source.
pub fn effective_state(state: ReadoutState, connected: bool) -> ReadoutState {
    if !connected && state == ReadoutState::Live {
        ReadoutState::Stale
    } else {
        state
    }
}

pub fn has_any_data(states: impl IntoIterator<Item = ReadoutState>) -> bool {
    states.into_iter().any(ReadoutState::has_data)
}

/// The whole-tile message, or `None` while any readout has data.
pub fn whole_tile_message(
    states: impl IntoIterator<Item = ReadoutState>,
    lifecycle: Lifecycle,
    connected: bool,
) -> Option<&'static str> {
    if has_any_data(states) {
        return None;
    }
    Some(if !connected {
        "Collector unreachable"
    } else {
        match lifecycle {
            Lifecycle::Lost => "No data: game paused or closed",
            Lifecycle::Waiting => "Waiting for the game",
            Lifecycle::Live | Lifecycle::Ended => "No data",
        }
    })
}
