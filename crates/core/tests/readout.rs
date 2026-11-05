//! One rendering rule for the four readout states (parity READ-01/02). Ported from
//! `dashboard/src/runtime/readout.test.ts`. The DOM-write test (`writeReadout`) and the
//! wire's frame-field group bits have no native counterpart: a native tile reads a typed
//! `Readout` per value, so `Readout::state` replaces `frameReadoutState`.
use pitwall_core::{
    model::{Lifecycle, Readout},
    readout::{
        ABSENT_TEXT, RESTRICTED_TEXT, ReadoutState, ReadoutView, effective_state, has_any_data,
        readout_view, view_of, whole_tile_message,
    },
};

fn view(state: ReadoutState, text: &str) -> ReadoutView {
    ReadoutView {
        state,
        text: text.to_owned(),
    }
}

#[test]
fn live_shows_the_value() {
    assert_eq!(
        readout_view(ReadoutState::Live, "312"),
        view(ReadoutState::Live, "312")
    );
}

#[test]
fn stale_keeps_the_last_value_and_is_flagged_for_dimming() {
    assert_eq!(
        readout_view(ReadoutState::Stale, "312"),
        view(ReadoutState::Stale, "312")
    );
}

#[test]
fn restricted_shows_a_label_never_the_value() {
    assert_eq!(RESTRICTED_TEXT, "Restricted");
    assert_eq!(
        readout_view(ReadoutState::Restricted, "0"),
        view(ReadoutState::Restricted, "Restricted")
    );
}

#[test]
fn absent_shows_an_em_dash_never_zero() {
    assert_eq!(ABSENT_TEXT, "\u{2014}");
    assert_eq!(
        readout_view(ReadoutState::Absent, "0"),
        view(ReadoutState::Absent, "\u{2014}")
    );
}

#[test]
fn a_live_readout_reads_stale_while_disconnected() {
    assert_eq!(
        effective_state(ReadoutState::Live, false),
        ReadoutState::Stale
    );
    assert_eq!(
        effective_state(ReadoutState::Live, true),
        ReadoutState::Live
    );
}

#[test]
fn restricted_and_absent_stay_as_they_are_while_disconnected() {
    assert_eq!(
        effective_state(ReadoutState::Restricted, false),
        ReadoutState::Restricted
    );
    assert_eq!(
        effective_state(ReadoutState::Absent, false),
        ReadoutState::Absent
    );
}

#[test]
fn stale_counts_as_data_restricted_and_absent_do_not() {
    use ReadoutState::*;
    assert!(!has_any_data([Absent, Restricted]));
    assert!(has_any_data([Absent, Stale]));
    assert!(has_any_data([Live]));
    assert!(!has_any_data([]));
}

#[test]
fn no_whole_tile_message_while_any_readout_has_data() {
    assert_eq!(
        whole_tile_message([ReadoutState::Live], Lifecycle::Live, true),
        None
    );
}

#[test]
fn whole_tile_message_wording_follows_why_there_is_no_data() {
    let none = [ReadoutState::Absent];
    assert_eq!(
        whole_tile_message(none, Lifecycle::Live, false),
        Some("Collector unreachable")
    );
    assert_eq!(
        whole_tile_message(none, Lifecycle::Lost, true),
        Some("No data: game paused or closed")
    );
    assert_eq!(
        whole_tile_message(none, Lifecycle::Waiting, true),
        Some("Waiting for the game")
    );
    assert_eq!(
        whole_tile_message(none, Lifecycle::Live, true),
        Some("No data")
    );
    assert_eq!(
        whole_tile_message([ReadoutState::Restricted], Lifecycle::Live, true),
        Some("No data")
    );
}

#[test]
fn model_readouts_carry_their_state() {
    assert_eq!(Readout::Live(1).state(), ReadoutState::Live);
    assert_eq!(Readout::Stale(1).state(), ReadoutState::Stale);
    assert_eq!(Readout::<u8>::Restricted.state(), ReadoutState::Restricted);
    assert_eq!(Readout::<u8>::Absent.state(), ReadoutState::Absent);
    assert_eq!(Readout::Stale(7).value(), Some(&7));
    assert_eq!(Readout::<u8>::Restricted.value(), None);
}

#[test]
fn model_readouts_render_values_and_never_format_missing_placeholders() {
    let format = |ms: &u32| format!("{ms} ms");
    assert_eq!(
        view_of(&Readout::Stale(1234), format),
        view(ReadoutState::Stale, "1234 ms")
    );
    assert_eq!(
        view_of(&Readout::Restricted, format),
        view(ReadoutState::Restricted, "Restricted")
    );
    assert_eq!(
        view_of(&Readout::Absent, format),
        view(ReadoutState::Absent, "\u{2014}")
    );
}

#[test]
fn only_live_and_stale_have_data() {
    assert!(ReadoutState::Live.has_data());
    assert!(ReadoutState::Stale.has_data());
    assert!(!ReadoutState::Restricted.has_data());
    assert!(!ReadoutState::Absent.has_data());
}
