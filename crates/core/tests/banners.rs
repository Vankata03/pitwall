//! Port of dashboard/src/runtime/banners.test.ts. Only the game-lifecycle banners exist
//! natively: the collector connection and reload banners went with the web stack.
use pitwall_core::banners::{BannerId, game_banner};
use pitwall_core::model::Lifecycle;

#[test]
fn nothing_while_the_game_is_sending_or_the_session_ended() {
    assert_eq!(game_banner(Lifecycle::Live, 20777), None);
    assert_eq!(game_banner(Lifecycle::Ended, 20777), None);
}

#[test]
fn waiting_shows_a_thin_banner_with_the_setup_hint() {
    let banner = game_banner(Lifecycle::Waiting, 20777).unwrap();
    assert_eq!(banner.id, BannerId::GameNotSending);
    assert_eq!(banner.text, "Waiting for the game");
    assert!(banner.hint.contains("UDP"));
    assert!(banner.hint.contains("20777"));
    assert_eq!(
        banner.hint,
        "In the game, turn UDP telemetry on, set UDP Format to 2025 or 2026, port 20777, and send to broadcast or this computer\u{2019}s IP."
    );
}

#[test]
fn lost_reads_as_the_spec_words_it() {
    let banner = game_banner(Lifecycle::Lost, 20777).unwrap();
    assert_eq!(banner.id, BannerId::GameNotSending);
    assert_eq!(banner.text, "No data: game paused or closed");
}

#[test]
fn the_hint_names_the_configured_port() {
    let banner = game_banner(Lifecycle::Waiting, 20888).unwrap();
    assert!(banner.hint.contains("port 20888,"));
}
