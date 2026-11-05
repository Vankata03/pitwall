//! Shared tile pieces: the readout element (one look for the four states), labels, the
//! tile shell with its whole-tile message.

use gpui::{
    AnyElement, Div, FontWeight, Hsla, IntoElement, ParentElement, SharedString, Styled, div, px,
};
use pitwall_core::readout::{ReadoutState, ReadoutView};

use crate::theme;

/// A readout. Stale is the muted colour plus an underline marker, so colour never carries
/// the state alone; restricted and absent show their text in the muted colour.
pub fn readout(view: &ReadoutView, size: f32) -> Div {
    readout_coloured(view, size, None)
}

/// A readout whose live value takes `colour` (for signed deltas).
pub fn readout_coloured(view: &ReadoutView, size: f32, colour: Option<Hsla>) -> Div {
    let base = div()
        .text_size(px(size))
        .font_weight(FontWeight::SEMIBOLD)
        .line_height(px(size * 1.15))
        .child(SharedString::from(view.text.clone()));
    match view.state {
        ReadoutState::Live => base.text_color(colour.unwrap_or_else(theme::text)),
        ReadoutState::Stale => base
            .text_color(theme::muted())
            .underline()
            .text_decoration_1()
            .text_decoration_color(theme::muted()),
        ReadoutState::Restricted | ReadoutState::Absent => base.text_color(theme::muted()),
    }
}

/// A label: at least 11 px (VISUAL-01).
pub fn label(text: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(12.0))
        .line_height(px(15.0))
        .text_color(theme::muted())
        .child(text.into())
}

pub fn title(text: &'static str) -> Div {
    label(text).font_weight(FontWeight::SEMIBOLD)
}

/// The tile body, or the whole-tile message over the same box so the shape stays.
pub fn tile_shell(body: Div, message: Option<&'static str>) -> AnyElement {
    let shell = div().relative().size_full().p_3();
    match message {
        None => shell.child(body).into_any_element(),
        Some(message) => shell
            .child(div().size_full().opacity(0.0).child(body))
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(16.0))
                    .text_color(theme::muted())
                    .child(message),
            )
            .into_any_element(),
    }
}
