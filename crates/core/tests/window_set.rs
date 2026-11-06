//! The app's set of windows (parity WIN-01, WIN-02, ADR 0001): every dashboard and stream
//! window has its own id, the windows open at quit come back on the next start, and each
//! one returns to its monitor and position, or somewhere visible when that monitor is gone.
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use pitwall_core::window_set::{
    Background, Display, Kind, Placement, Rect, Window, WindowSet, place, window_set_path,
};

fn temp_dir() -> PathBuf {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "pitwall-window-set-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn load(root: &std::path::Path) -> WindowSet {
    let (set, bad) = WindowSet::load(&window_set_path(root)).unwrap();
    assert_eq!(bad, None);
    set
}

#[test]
fn each_new_dashboard_window_gets_its_own_id_and_the_open_ones_come_back_after_a_restart() {
    let root = temp_dir();
    let mut set = load(&root);
    assert!(set.to_restore().is_empty());

    // The first keeps the pre-#75 choices file `windows/main.json`.
    let first = set.open_dashboard();
    let second = set.open_dashboard();
    set.save().unwrap();
    assert_eq!(first.id, "main");
    assert_eq!(second.id, "dashboard-2");
    assert_eq!(first.kind, Kind::Dashboard);

    let restored = load(&root);
    let ids: Vec<_> = restored.to_restore().iter().map(|w| w.id.clone()).collect();
    assert_eq!(ids, ["main", "dashboard-2"]);
}

#[test]
fn a_closed_window_is_not_restored_and_the_next_window_of_its_kind_reuses_its_id() {
    let root = temp_dir();
    let mut set = load(&root);
    set.open_dashboard();
    set.open_dashboard();
    set.closed("main");
    set.save().unwrap();
    let ids: Vec<_> = load(&root)
        .to_restore()
        .iter()
        .map(|w| w.id.clone())
        .collect();
    assert_eq!(ids, ["dashboard-2"]);

    // The next dashboard reuses `main` and so its choices file.
    assert_eq!(set.open_dashboard().id, "main");
    assert_eq!(set.open_dashboard().id, "dashboard-3");
}

#[test]
fn windows_that_close_while_pitwall_quits_still_come_back() {
    let root = temp_dir();
    let mut set = load(&root);
    set.open_dashboard();
    set.quitting();
    set.closed("main");
    set.save().unwrap();
    let ids: Vec<_> = load(&root)
        .to_restore()
        .iter()
        .map(|w| w.id.clone())
        .collect();
    assert_eq!(ids, ["main"]);
}

#[test]
fn a_stream_window_remembers_its_layout_and_background_and_reuses_a_closed_stream_window() {
    let root = temp_dir();
    let mut set = load(&root);
    let stream = set.open_stream("Race", Background::Green);
    assert_eq!(stream.id, "stream-1");
    assert_eq!(
        stream.kind,
        Kind::Stream {
            layout: "Race".into(),
            background: Background::Green
        }
    );
    set.moved("stream-1", placement(Some("DISPLAY2"), 100.0, 50.0));
    set.save().unwrap();
    let restored = load(&root).to_restore();
    assert_eq!(restored, [stream_window("Race", Background::Green, true)]);

    // Closed, the next stream window takes its id and position with the new choice.
    set.closed("stream-1");
    let next = set.open_stream("Quali", Background::Solid);
    assert_eq!(next, stream_window("Quali", Background::Solid, true));
    assert_eq!(set.open_stream("Race", Background::Magenta).id, "stream-2");
    // Stream and dashboard windows never share ids.
    assert_eq!(set.open_dashboard().id, "main");
}

fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

/// A primary 1920x1080 monitor (taskbar below) and a 2560x1440 one to its right.
fn displays() -> Vec<Display> {
    vec![
        Display {
            id: "DISPLAY1".into(),
            visible: rect(0.0, 0.0, 1920.0, 1040.0),
            primary: true,
        },
        Display {
            id: "DISPLAY2".into(),
            visible: rect(1920.0, 0.0, 2560.0, 1440.0),
            primary: false,
        },
    ]
}

#[test]
fn a_window_returns_to_its_monitor_and_position() {
    let saved = Placement {
        display: Some("DISPLAY2".into()),
        bounds: rect(2000.0, 100.0, 1200.0, 800.0),
        maximized: true,
    };
    let placed = place(Some(&saved), &displays()).unwrap();
    assert_eq!(placed, saved);
}

#[test]
fn a_window_whose_monitor_is_gone_opens_centred_on_the_primary_monitor_and_fits_it() {
    let saved = Placement {
        display: Some("DISPLAY3".into()),
        bounds: rect(5000.0, 100.0, 2400.0, 800.0),
        maximized: false,
    };
    let placed = place(Some(&saved), &displays()).unwrap();
    assert_eq!(
        placed,
        Placement {
            display: Some("DISPLAY1".into()),
            bounds: rect(0.0, 120.0, 1920.0, 800.0),
            maximized: false,
        }
    );
}

#[test]
fn a_window_left_off_its_shrunken_monitor_is_moved_back_onto_it() {
    // DISPLAY2 is now 1280x720; the saved window sits off its right edge and above it.
    let mut monitors = displays();
    monitors[1].visible = rect(1920.0, 0.0, 1280.0, 720.0);
    let saved = Placement {
        display: Some("DISPLAY2".into()),
        bounds: rect(3000.0, -50.0, 800.0, 900.0),
        maximized: false,
    };
    let placed = place(Some(&saved), &monitors).unwrap();
    assert_eq!(placed.display.as_deref(), Some("DISPLAY2"));
    assert_eq!(placed.bounds, rect(2400.0, 0.0, 800.0, 720.0));
}

#[test]
fn a_window_never_placed_or_with_no_monitor_reported_opens_at_the_default() {
    assert_eq!(place(None, &displays()), None);
    let saved = placement(Some("DISPLAY2"), 0.0, 0.0);
    assert_eq!(place(Some(&saved), &[]), None);
}

fn placement(display: Option<&str>, x: f32, y: f32) -> Placement {
    Placement {
        display: display.map(str::to_owned),
        bounds: Rect {
            x,
            y,
            width: 800.0,
            height: 450.0,
        },
        maximized: false,
    }
}

fn stream_window(layout: &str, background: Background, open: bool) -> Window {
    Window {
        id: "stream-1".into(),
        kind: Kind::Stream {
            layout: layout.into(),
            background,
        },
        placement: Some(placement(Some("DISPLAY2"), 100.0, 50.0)),
        open,
    }
}

#[test]
fn an_unreadable_window_set_is_kept_as_bad_and_unreadable_entries_are_skipped() {
    let root = temp_dir();
    let path = window_set_path(&root);
    std::fs::write(&path, b"not json").unwrap();
    let (set, bad) = WindowSet::load(&path).unwrap();
    assert!(set.windows().is_empty());
    assert_eq!(bad, Some(root.join("windows.json.bad")));
    assert!(!path.exists());

    std::fs::write(
        &path,
        br#"{"schemaVersion":1,"windows":[
            {"id":"main","kind":"dashboard","open":true},
            {"id":"main","kind":"dashboard","open":true},
            {"id":"x","kind":"future"},
            {"id":"stream-1","kind":"stream","layout":"Race","background":"teal","open":true},
            {"id":"stream-2","kind":"stream","layout":"Air","open":true}
        ]}"#,
    )
    .unwrap();
    let restored = load(&root).to_restore();
    let ids: Vec<_> = restored.iter().map(|w| w.id.clone()).collect();
    assert_eq!(ids, ["main", "stream-1", "stream-2"]);
    // An unknown background falls back to solid on its own.
    assert_eq!(
        restored[1].kind,
        Kind::Stream {
            layout: "Race".into(),
            background: Background::Solid
        }
    );
}
