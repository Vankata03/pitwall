//! overlay.json and placement (OVERLAY-02, MIG-04), ported from
//! `crates/collector/tests/overlay.rs`.
use pitwall_overlay::settings::{
    DEFAULT_SIZE, Loaded, MIN_SIZE, OverlaySettings, Rect, default_position, place,
};

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("pitwall-overlay-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
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
    let dir = temp_dir("round-trip");
    let path = dir.join("overlay.json");
    assert_eq!(
        OverlaySettings::load(&path),
        Loaded::Missing(OverlaySettings::default())
    );
    let saved = OverlaySettings {
        visible: true,
        position: Some((-1200, 40)),
        size: Some((540, 160)),
    };
    saved.save(&path).unwrap();
    assert_eq!(OverlaySettings::load(&path), Loaded::Read(saved));
    // Files saved before the overlay could be resized carry no size.
    std::fs::write(&path, r#"{"visible":true,"position":[36,1252]}"#).unwrap();
    assert_eq!(
        OverlaySettings::load(&path).settings(),
        OverlaySettings {
            visible: true,
            position: Some((36, 1252)),
            size: None,
        }
    );
    // The real file the web overlay wrote on this machine.
    std::fs::write(
        &path,
        "{\n  \"visible\": true,\n  \"position\": [\n    17,\n    1200\n  ],\n  \"size\": [\n    501,\n    183\n  ]\n}",
    )
    .unwrap();
    assert_eq!(
        OverlaySettings::load(&path).settings(),
        OverlaySettings {
            visible: true,
            position: Some((17, 1200)),
            size: Some((501, 183)),
        }
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_malformed_file_is_kept_under_bad_and_defaults_load() {
    let dir = temp_dir("bad");
    let path = dir.join("overlay.json");
    std::fs::write(&path, "not json").unwrap();
    let Loaded::Malformed {
        settings,
        kept,
        error: _,
    } = OverlaySettings::load(&path)
    else {
        panic!("malformed file not reported");
    };
    assert_eq!(settings, OverlaySettings::default());
    assert_eq!(kept, dir.join("overlay.json.bad"));
    assert_eq!(std::fs::read_to_string(&kept).unwrap(), "not json");
    assert!(!path.exists());
    // A second bad file never replaces the first one kept.
    std::fs::write(&path, "{").unwrap();
    let Loaded::Malformed { kept, .. } = OverlaySettings::load(&path) else {
        panic!("malformed file not reported");
    };
    assert_eq!(kept, dir.join("overlay.json.bad.2"));
    assert_eq!(
        std::fs::read_to_string(dir.join("overlay.json.bad")).unwrap(),
        "not json"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

const PRIMARY: Rect = Rect {
    left: 0,
    top: 0,
    right: 2560,
    bottom: 1440,
};
const SECOND: Rect = Rect {
    left: -1920,
    top: 0,
    right: 0,
    bottom: 1080,
};

#[test]
fn a_new_overlay_takes_the_default_size_scaled_to_the_monitor_low_in_the_centre() {
    let settings = OverlaySettings::default();
    let placed = place(&settings, &[PRIMARY, SECOND], PRIMARY, 1.5);
    assert_eq!(placed.size, (540, 165));
    assert_eq!(
        placed.position,
        default_position((0, 0, 2560, 1440), (540, 165))
    );
    assert_eq!(DEFAULT_SIZE, (360.0, 110.0));
    assert_eq!(MIN_SIZE, (180.0, 70.0));
}

#[test]
fn a_saved_spot_is_kept_only_while_some_monitor_still_shows_it() {
    let settings = OverlaySettings {
        visible: true,
        position: Some((-1200, 40)),
        size: Some((501, 183)),
    };
    let placed = place(&settings, &[PRIMARY, SECOND], PRIMARY, 1.0);
    assert_eq!(placed.position, (-1200, 40));
    assert_eq!(placed.size, (501, 183));
    // The second monitor is gone: back to the default spot, keeping the saved size.
    let placed = place(&settings, &[PRIMARY], PRIMARY, 1.0);
    assert_eq!(
        placed.position,
        default_position((0, 0, 2560, 1440), (360, 110))
    );
    assert_eq!(placed.size, (501, 183));
}
