//! Slow tiles: rows of quiet labels over large numerals, and the whole-tile message when
//! no row has data. Port of `dashboard/src/tiles/slow.tsx`.

use gpui::{AnyElement, Div, Hsla, ParentElement, Styled, div, px};
use pitwall_core::{
    model::Lifecycle,
    readout::{ReadoutState, ReadoutView, whole_tile_message},
};

use super::widgets;
use crate::theme;

/// Accent for a live value; colour only repeats what the text or sign already says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Good,
    Bad,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub label: &'static str,
    pub value: ReadoutView,
    pub tone: Option<Tone>,
}

impl Row {
    pub fn new(label: &'static str, value: ReadoutView) -> Self {
        Self {
            label,
            value,
            tone: None,
        }
    }
}

/// The whole-tile message for a slow tile's rows.
pub fn message(rows: &[Row], lifecycle: Lifecycle) -> Option<&'static str> {
    whole_tile_message(rows.iter().map(|r| r.value.state), lifecycle, true)
}

fn tone_colour(tone: Option<Tone>) -> Option<Hsla> {
    tone.map(|t| match t {
        Tone::Good => theme::personal_best(),
        Tone::Bad => theme::brake(),
    })
}

/// Numerals grow with the tile: `clamp(18px, min(5.6cqw, 36cqh), 56px)` in the web build.
pub fn value_size(width: f32, height: f32) -> f32 {
    (width * 0.056).min(height * 0.36).clamp(18.0, 56.0)
}

/// One row: label above its value.
pub fn row(r: &Row, size: f32) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .min_w_0()
        .child(widgets::label(r.label))
        .child(widgets::readout_coloured(
            &r.value,
            size,
            if r.value.state == ReadoutState::Live {
                tone_colour(r.tone)
            } else {
                None
            },
        ))
}

/// Rows spread across the tile; the message replaces them when none has data.
pub fn render(rows: &[Row], message: Option<&'static str>, size: (f32, f32)) -> AnyElement {
    let numerals = value_size(size.0, size.1);
    let mut line = div().flex().justify_between().gap_4().w_full();
    for (i, r) in rows.iter().enumerate() {
        let mut cell = row(r, numerals);
        if i > 0 && i == rows.len() - 1 {
            cell = cell.items_end();
        }
        line = line.child(cell);
    }
    widgets::tile_shell(
        div()
            .flex()
            .flex_col()
            .justify_center()
            .size_full()
            .child(line)
            .text_size(px(14.0)),
        message,
    )
}
