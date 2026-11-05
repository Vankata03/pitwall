//! Game-lifecycle banners (spec section 2), ported from `dashboard/src/runtime/banners.ts`.
//!
//! The web build also had collector-connection and reload banners; the native app runs
//! the collector in-process and has no page to reload, so only the game banner remains.

use crate::model::Lifecycle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BannerId {
    /// The game is not sending: waiting for the first packet, or packets stopped.
    GameNotSending,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Banner {
    pub id: BannerId,
    pub text: &'static str,
    /// Second line, for setup guidance.
    pub hint: String,
}

fn setup_hint(udp_port: u16) -> String {
    format!(
        "In the game, turn UDP telemetry on, set UDP Format to 2025 or 2026, port {udp_port}, and send to broadcast or this computer\u{2019}s IP."
    )
}

/// The banner for the game lifecycle, if any. `udp_port` is the port the collector listens
/// on, named in the setup hint (20777 by default).
pub fn game_banner(lifecycle: Lifecycle, udp_port: u16) -> Option<Banner> {
    let text = match lifecycle {
        Lifecycle::Waiting => "Waiting for the game",
        Lifecycle::Lost => "No data: game paused or closed",
        Lifecycle::Live | Lifecycle::Ended => return None,
    };
    Some(Banner {
        id: BannerId::GameNotSending,
        text,
        hint: setup_hint(udp_port),
    })
}
