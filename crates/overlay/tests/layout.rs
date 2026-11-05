//! Panel geometry, move-mode grips and drag/resize (OVERLAY-03), in the overlay page's CSS
//! pixels (`dashboard/src/overlay/overlay.css`).
use pitwall_overlay::layout::{Edge, Frame, Hit, drag, hit_test, layout};

#[test]
fn the_trace_fills_the_panel_left_of_two_30_px_bars() {
    let l = layout(360.0, 110.0);
    // Padding 8 × 10, a 1 px left and bottom border, a 10 px gap, then 30 + 6 + 30.
    assert_eq!(
        (l.trace.left, l.trace.top, l.trace.right, l.trace.bottom),
        (11.0, 8.0, 274.0, 101.0)
    );
    assert_eq!(l.bars[0].column.left, 284.0);
    assert_eq!(l.bars[1].column.right, 350.0);
    // The 14 px track is centred in its column.
    assert_eq!(l.bars[0].track.left, 292.0);
    assert_eq!(l.bars[0].track.right, 306.0);
    assert!(l.bars[0].track.top > l.bars[0].column.top);
    assert!(l.bars[0].track.bottom < l.bars[0].column.bottom);
}

#[test]
fn a_tiny_panel_never_lays_out_negative_boxes() {
    let l = layout(10.0, 5.0);
    assert!(l.trace.right >= l.trace.left);
    assert!(l.trace.bottom >= l.trace.top);
    assert!(l.bars[0].track.bottom >= l.bars[0].track.top);
}

#[test]
fn move_mode_grips_match_the_page() {
    let (w, h) = (360.0, 110.0);
    assert_eq!(hit_test(180.0, 55.0, w, h), Hit::Move);
    assert_eq!(hit_test(180.0, 2.0, w, h), Hit::Resize(Edge::North));
    assert_eq!(hit_test(180.0, 108.0, w, h), Hit::Resize(Edge::South));
    assert_eq!(hit_test(358.0, 55.0, w, h), Hit::Resize(Edge::East));
    assert_eq!(hit_test(1.0, 55.0, w, h), Hit::Resize(Edge::West));
    assert_eq!(hit_test(2.0, 2.0, w, h), Hit::Resize(Edge::NorthWest));
    assert_eq!(hit_test(355.0, 2.0, w, h), Hit::Resize(Edge::NorthEast));
    assert_eq!(hit_test(2.0, 105.0, w, h), Hit::Resize(Edge::SouthWest));
    // The visible bottom-right grip is 14 px.
    assert_eq!(hit_test(348.0, 98.0, w, h), Hit::Resize(Edge::SouthEast));
    // The edge strips stop 8 px short of the corners, where 10 px corners take over.
    assert_eq!(hit_test(9.0, 2.0, w, h), Hit::Resize(Edge::NorthWest));
    assert_eq!(hit_test(11.0, 7.0, w, h), Hit::Move);
}

const START: Frame = Frame {
    x: 100,
    y: 200,
    width: 360,
    height: 110,
};

#[test]
fn dragging_the_body_moves_without_resizing() {
    assert_eq!(
        drag(Hit::Move, START, (-30, 45), (180, 70)),
        Frame {
            x: 70,
            y: 245,
            ..START
        }
    );
}

#[test]
fn dragging_an_edge_resizes_from_that_edge_and_keeps_the_opposite_one() {
    assert_eq!(
        drag(Hit::Resize(Edge::SouthEast), START, (40, 10), (180, 70)),
        Frame {
            width: 400,
            height: 120,
            ..START
        }
    );
    assert_eq!(
        drag(Hit::Resize(Edge::NorthWest), START, (-20, -10), (180, 70)),
        Frame {
            x: 80,
            y: 190,
            width: 380,
            height: 120
        }
    );
    assert_eq!(
        drag(Hit::Resize(Edge::East), START, (40, 99), (180, 70)),
        Frame {
            width: 400,
            ..START
        }
    );
}

#[test]
fn resizing_stops_at_the_minimum_size_without_moving_the_opposite_edge() {
    let shrunk = drag(Hit::Resize(Edge::NorthWest), START, (500, 500), (180, 70));
    assert_eq!((shrunk.width, shrunk.height), (180, 70));
    assert_eq!(shrunk.x + shrunk.width as i32, START.x + START.width as i32);
    assert_eq!(
        shrunk.y + shrunk.height as i32,
        START.y + START.height as i32
    );
}
