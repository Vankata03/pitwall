//! Notice (parity NOTICE-01): the short unofficial line as a tile, so it can appear in a
//! stream window, which has no chrome. Port of `dashboard/src/tiles/types/notice.tsx`.
//! It reads no telemetry and has no settings; both densities show the same line.

use gpui::{IntoElement, ParentElement, Styled, div, px};

use super::{Densities, Density};
use crate::theme;
use pitwall_native::status_view::SHORT_LINE;

pub const TYPE: &str = pitwall_core::tile_catalog::NOTICE.kind;

pub const DENSITIES: Densities = Densities {
    regular: (480.0, 0.0),
    full: None,
};

#[derive(Debug, Clone, PartialEq)]
pub struct View {
    pub density: Density,
    pub line: &'static str,
}

pub fn view(size: (f32, f32)) -> View {
    View {
        density: DENSITIES.pick(size.0, size.1),
        line: SHORT_LINE,
    }
}

pub fn render(view: View) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .size_full()
        .px_3()
        .overflow_hidden()
        .text_size(px(12.0))
        .line_height(px(15.0))
        .text_color(theme::muted())
        .child(view.line)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pitwall_core::tile_catalog::declaration;

    #[test]
    fn declares_a_session_wide_tile_with_two_densities() {
        let d = declaration(TYPE).unwrap();
        assert_eq!(
            (d.name, d.follows_car()),
            (pitwall_core::tile_catalog::NOTICE.name, false)
        );
        assert_eq!((d.minimum, d.default), ((4, 1), (6, 1)));
        assert_eq!(DENSITIES.pick(479.0, 20.0), Density::Compact);
        assert_eq!(DENSITIES.pick(480.0, 0.0), Density::Regular);
    }

    #[test]
    fn both_densities_show_the_spec_wording_word_for_word() {
        for width in [200.0, 600.0] {
            assert_eq!(
                view((width, 30.0)).line,
                "Unofficial. Not associated with the Formula 1 companies or EA."
            );
        }
    }
}
