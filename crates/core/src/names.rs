//! Plain display names for both grids (2025 IDs 0–9 and 2026 IDs 476–486).
//!
//! IDs follow the repository's F1 25 telemetry research. Missing mappings were
//! checked against EA's *Data Output from F1 25 2026 Season Pack*, appendices
//! (PDF pages 25, 27 and 29). Team labels omit the year suffix.
//! [EA publication](https://forums.ea.com/blog/f1-games-game-info-hub-en/ea-sports%E2%84%A2-f1%C2%AE25-2026-season-pack-udp-specification/12187347).
//! EA's PDF download was unavailable; the published document was read from this
//! [pinned PDF mirror](https://github.com/bobbythehuman/Race-Telemetry-Package/blob/45aea575c8cee234ad6f76a5821528f654f9ff94/Supporting_Docs/Data%20Output%20from%20F1%2025%202026%20Season%20Pack.pdf).
//! PDF SHA-256: `a260c02d3562f8be2d0a6efba9bfba6dd713d8a3a752794de06c74cfaee340fa`.
//!
//! Callers choose their own missing-label presentation from `None`. These
//! helpers neither invent names nor identify My Team from a sentinel ID.

/// Short team name for either grid. Unknown IDs have no label.
///
/// Pass the participant's `m_myTeam` flag, never infer it from a sentinel ID.
pub fn team_name(team_id: u16, my_team: bool) -> Option<&'static str> {
    if my_team {
        return Some("My Team");
    }
    Some(match team_id {
        0 | 476 => "Mercedes",
        1 | 477 => "Ferrari",
        2 | 478 => "Red Bull Racing",
        3 | 479 => "Williams",
        4 | 480 => "Aston Martin",
        5 | 481 => "Alpine",
        6 | 482 => "RB",
        7 | 483 => "Haas",
        8 | 484 => "McLaren",
        9 => "Sauber",
        485 => "Audi",
        486 => "Cadillac",
        _ => return None,
    })
}

/// Appendix place name for `m_trackId`, including distinct reverse layouts.
/// Unknown and unlisted IDs have no label.
pub fn track_name(track_id: i8) -> Option<&'static str> {
    Some(match track_id {
        0 => "Melbourne",
        2 => "Shanghai",
        3 => "Sakhir (Bahrain)",
        4 => "Catalunya",
        5 => "Monaco",
        6 => "Montreal",
        7 => "Silverstone",
        9 => "Hungaroring",
        10 => "Spa",
        11 => "Monza",
        12 => "Singapore",
        13 => "Suzuka",
        14 => "Abu Dhabi",
        15 => "Texas",
        16 => "Brazil",
        17 => "Austria",
        19 => "Mexico",
        20 => "Baku (Azerbaijan)",
        26 => "Zandvoort",
        27 => "Imola",
        29 => "Jeddah",
        30 => "Miami",
        31 => "Las Vegas",
        32 => "Losail",
        39 => "Silverstone (Reverse)",
        40 => "Austria (Reverse)",
        41 => "Zandvoort (Reverse)",
        42 => "Madrid",
        _ => return None,
    })
}

/// Plain session name for `m_sessionType`, not a game-mode or event title.
/// Zero (unknown) and unlisted IDs have no label.
pub fn session_name(session_type: u8) -> Option<&'static str> {
    Some(match session_type {
        1 => "Practice 1",
        2 => "Practice 2",
        3 => "Practice 3",
        4 => "Short Practice",
        5 => "Qualifying 1",
        6 => "Qualifying 2",
        7 => "Qualifying 3",
        8 => "Short Qualifying",
        9 => "One-Shot Qualifying",
        10 => "Sprint Shootout 1",
        11 => "Sprint Shootout 2",
        12 => "Sprint Shootout 3",
        13 => "Short Sprint Shootout",
        14 => "One-Shot Sprint Shootout",
        15 => "Race",
        16 => "Race 2",
        17 => "Race 3",
        18 => "Time Trial",
        _ => return None,
    })
}
