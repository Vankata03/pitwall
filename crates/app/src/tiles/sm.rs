//! SM (parity SM-01): the straight-mode or DRS state of one car, at frame rate.
//! Port of `dashboard/src/tiles/types/sm.tsx`.
//!
//! One byte carries the state: the active aero mode on a 2026-regulations car, the DRS
//! flag on a pre-2026 car. Without the car's regulations the byte means nothing, so the
//! readout is absent whatever the byte says.

use gpui::{IntoElement, ParentElement, Styled, div};
use pitwall_core::{
    model::{Lifecycle, Readout, Regulations, SessionModel},
    readout::{ReadoutState, ReadoutView, readout_view, whole_tile_message},
};

use super::{Densities, Density, widgets};

pub const TYPE: &str = pitwall_core::tile_catalog::SM.kind;

pub const DENSITIES: Densities = Densities {
    regular: (190.0, 72.0),
    full: None,
};

pub struct Inputs {
    pub regulations: Option<Regulations>,
    /// The aero byte: `active_aero_mode` (2026) or the DRS flag (pre-2026).
    pub state: Readout<u8>,
    /// Straight mode available (2026) or DRS allowed (pre-2026), from State.
    pub available: bool,
    pub lifecycle: Lifecycle,
    pub width: f32,
    pub height: f32,
}

impl Inputs {
    pub fn from_model(model: &SessionModel, index: Option<usize>, size: (f32, f32)) -> Self {
        let car = index.and_then(|i| model.car(i));
        let regulations = car.and_then(|c| c.regulations());
        let (state, available) = match (car, regulations) {
            (Some(car), Some(Regulations::Pre2026)) => (
                car.telemetry().map(|t| t.drs),
                car.status().value().is_some_and(|s| s.drs_allowed == 1),
            ),
            (Some(car), Some(Regulations::Regulations2026)) => (
                car.aero().map(|a| a.active_aero_mode),
                car.aero()
                    .value()
                    .is_some_and(|a| a.active_aero_available == 1),
            ),
            _ => (Readout::Absent, false),
        };
        Self {
            regulations,
            state,
            available,
            lifecycle: model.lifecycle(),
            width: size.0,
            height: size.1,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct View {
    pub density: Density,
    pub label: &'static str,
    pub state: ReadoutView,
    /// What the abbreviation means; regular only.
    pub detail: Option<&'static str>,
    pub message: Option<&'static str>,
}

fn state_text(regulations: Regulations, byte: u8, available: bool) -> String {
    let sm = regulations == Regulations::Regulations2026;
    match byte {
        2.. => format!("MODE {byte}"),
        1 if sm => "ACTIVE".into(),
        1 => "OPEN".into(),
        _ if available => "AVAILABLE".into(),
        _ if sm => "INACTIVE".into(),
        _ => "CLOSED".into(),
    }
}

pub fn view(inputs: &Inputs) -> View {
    let density = DENSITIES.pick(inputs.width, inputs.height);
    let (state, text) = match (inputs.regulations, inputs.state.value()) {
        (Some(r), Some(&byte)) => (inputs.state.state(), state_text(r, byte, inputs.available)),
        (Some(_), None) => (inputs.state.state(), String::new()),
        (None, _) => (ReadoutState::Absent, String::new()),
    };
    let (label, detail) = match inputs.regulations {
        Some(Regulations::Regulations2026) => ("SM", "Straight mode"),
        Some(Regulations::Pre2026) => ("DRS", "Drag reduction"),
        None => ("SM / DRS", "Aero mode"),
    };
    View {
        density,
        label,
        state: readout_view(state, &text),
        detail: (density != Density::Compact).then_some(detail),
        message: whole_tile_message([state], inputs.lifecycle, true),
    }
}

pub fn render(view: View) -> impl IntoElement {
    let mut body = div()
        .flex()
        .flex_col()
        .justify_center()
        .gap_1()
        .size_full()
        .child(widgets::label(view.label))
        .child(widgets::readout(&view.state, 28.0));
    if let Some(detail) = view.detail {
        body = body.child(widgets::label(detail));
    }
    widgets::tile_shell(body, view.message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pitwall_core::{readout::ABSENT_TEXT, tile_catalog::declaration};

    fn sm(state: Readout<u8>) -> Inputs {
        Inputs {
            regulations: Some(Regulations::Regulations2026),
            state,
            available: false,
            lifecycle: Lifecycle::Live,
            width: 220.0,
            height: 80.0,
        }
    }
    fn drs(state: Readout<u8>) -> Inputs {
        Inputs {
            regulations: Some(Regulations::Pre2026),
            ..sm(state)
        }
    }

    #[test]
    fn declares_the_catalogue_entry() {
        let d = declaration(TYPE).unwrap();
        assert_eq!(
            (d.name, d.follows_car()),
            (pitwall_core::tile_catalog::SM.name, true)
        );
        assert_eq!((d.minimum, d.default), ((2, 2), (4, 2)));
        assert_eq!(DENSITIES.pick(189.0, 72.0), Density::Compact);
        assert_eq!(DENSITIES.pick(190.0, 72.0), Density::Regular);
    }

    #[test]
    fn a_2026_regulations_car_shows_sm_the_active_aero_state() {
        let v = view(&sm(Readout::Live(1)));
        assert_eq!((v.label, v.state.text.as_str()), ("SM", "ACTIVE"));
        assert_eq!(view(&sm(Readout::Live(0))).state.text, "INACTIVE");
    }

    #[test]
    fn a_pre_2026_car_shows_drs_in_the_same_slot_from_the_same_byte() {
        let v = view(&drs(Readout::Live(1)));
        assert_eq!((v.label, v.state.text.as_str()), ("DRS", "OPEN"));
        assert_eq!(view(&drs(Readout::Live(0))).state.text, "CLOSED");
    }

    #[test]
    fn availability_shows_between_inactive_and_active() {
        let available = |i: Inputs| Inputs {
            available: true,
            ..i
        };
        assert_eq!(
            view(&available(sm(Readout::Live(0)))).state.text,
            "AVAILABLE"
        );
        assert_eq!(view(&available(sm(Readout::Live(1)))).state.text, "ACTIVE");
        assert_eq!(
            view(&available(drs(Readout::Live(0)))).state.text,
            "AVAILABLE"
        );
    }

    #[test]
    fn a_mode_the_tile_does_not_know_is_named_not_guessed() {
        assert_eq!(view(&sm(Readout::Live(2))).state.text, "MODE 2");
    }

    #[test]
    fn without_the_regulations_the_byte_cannot_be_read() {
        let v = view(&Inputs {
            regulations: None,
            ..sm(Readout::Live(1))
        });
        assert_eq!(v.state.state, ReadoutState::Absent);
        assert_eq!(v.state.text, ABSENT_TEXT);
        assert_eq!(v.label, "SM / DRS");
    }

    #[test]
    fn the_four_readout_states_value_dimmed_value_label_em_dash() {
        let pair = |r| {
            let v = view(&sm(r)).state;
            (v.state, v.text)
        };
        assert_eq!(
            pair(Readout::Live(1)),
            (ReadoutState::Live, "ACTIVE".into())
        );
        assert_eq!(
            pair(Readout::Stale(1)),
            (ReadoutState::Stale, "ACTIVE".into())
        );
        assert_eq!(
            pair(Readout::Restricted),
            (ReadoutState::Restricted, "Restricted".into())
        );
        assert_eq!(
            pair(Readout::Absent),
            (ReadoutState::Absent, ABSENT_TEXT.into())
        );
    }

    #[test]
    fn with_no_data_the_whole_tile_says_so() {
        assert_eq!(view(&sm(Readout::Absent)).message, Some("No data"));
        assert_eq!(view(&sm(Readout::Live(0))).message, None);
    }

    #[test]
    fn compact_keeps_the_state_regular_adds_what_the_abbreviation_means() {
        let compact = view(&Inputs {
            width: 100.0,
            height: 70.0,
            ..sm(Readout::Live(0))
        });
        assert_eq!(compact.detail, None);
        assert_eq!(view(&sm(Readout::Live(0))).detail, Some("Straight mode"));
        assert_eq!(view(&drs(Readout::Live(0))).detail, Some("Drag reduction"));
    }
}
