use pitwall_collector::overlay::{Edge, OverlayEvent, OverlaySettings, default_position, page_url};

#[test]
fn page_url_adds_move_mode_only_while_moving() {
    assert_eq!(
        page_url("http://127.0.0.1:8777/", false),
        "http://127.0.0.1:8777/overlay/throttle-brake"
    );
    assert_eq!(
        page_url("http://127.0.0.1:8777", true),
        "http://127.0.0.1:8777/overlay/throttle-brake?move=1"
    );
}

#[test]
fn default_position_centres_near_the_bottom_of_the_monitor() {
    assert_eq!(default_position((0, 0, 1920, 1080), (360, 110)), (780, 754));
    assert_eq!(
        default_position((1920, 0, 2560, 1440), (720, 220)),
        (1920 + 920, 932)
    );
}

#[test]
fn settings_round_trip_and_fall_back_to_hidden() {
    let dir = std::env::temp_dir().join(format!("pitwall-overlay-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("overlay.json");
    assert_eq!(OverlaySettings::load(&path), OverlaySettings::default());
    let saved = OverlaySettings {
        visible: true,
        position: Some((-1200, 40)),
        size: Some((540, 160)),
    };
    saved.save(&path).unwrap();
    assert_eq!(OverlaySettings::load(&path), saved);
    // Files saved before the overlay could be resized carry no size.
    std::fs::write(&path, r#"{"visible":true,"position":[36,1252]}"#).unwrap();
    assert_eq!(
        OverlaySettings::load(&path),
        OverlaySettings {
            visible: true,
            position: Some((36, 1252)),
            size: None,
        }
    );
    std::fs::write(&path, "not json").unwrap();
    assert_eq!(OverlaySettings::load(&path), OverlaySettings::default());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn page_messages_parse_into_moves_and_resizes() {
    assert_eq!(OverlayEvent::parse("drag"), Some(OverlayEvent::Drag));
    assert_eq!(
        OverlayEvent::parse("resize:se"),
        Some(OverlayEvent::Resize(Edge::SouthEast))
    );
    assert_eq!(
        OverlayEvent::parse("resize:w"),
        Some(OverlayEvent::Resize(Edge::West))
    );
    for body in ["", "resize", "resize:x", "drag:se", "close"] {
        assert_eq!(OverlayEvent::parse(body), None, "{body}");
    }
}
