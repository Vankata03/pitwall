//! Port of dashboard/src/layout/controller.test.ts against the real layout library.
//!
//! Natively the controller talks to the in-process `LayoutLibrary` synchronously, so the
//! web tests about late HTTP replies have no counterpart (see docs/native/parity.md).
//! `TestStore` wraps a library in a temporary folder, logs mutations like the web fake
//! server, and can fail the next call.
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use pitwall_core::layout_edit::{
    ControllerOptions, DEFAULT_LAYOUT, GridItem, LayoutController, LayoutStore, LayoutSummary,
    NoticeKind, Status,
};
use pitwall_core::layouts::{Arrangements, Layout, LayoutError, LayoutLibrary, ScreenClass, Tile};
use serde_json::{Map, json};

const CLASSIC: &str = "Pitwall Classic";

fn temp_dir() -> PathBuf {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "pitwall-layout-controller-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[derive(Clone, Copy, PartialEq)]
enum Fail {
    None,
    List,
    Get,
    Save,
    Everything,
}

struct TestStore {
    library: LayoutLibrary,
    log: RefCell<Vec<String>>,
    fail: Cell<Fail>,
    lists: Cell<u32>,
}

impl TestStore {
    fn new(extra: &[Layout]) -> Self {
        let library = LayoutLibrary::open(temp_dir().join("layouts.json")).unwrap();
        for layout in extra {
            library.save(&layout.name, layout.clone()).unwrap();
        }
        Self {
            library,
            log: RefCell::new(Vec::new()),
            fail: Cell::new(Fail::None),
            lists: Cell::new(0),
        }
    }
    fn failing(&self, which: Fail) -> Result<(), LayoutError> {
        let fail = self.fail.get();
        if fail == Fail::Everything || fail == which {
            if fail != Fail::Everything {
                self.fail.set(Fail::None);
            }
            return Err(LayoutError::Io(std::io::Error::other("disk full")));
        }
        Ok(())
    }
    fn log(&self) -> Vec<String> {
        self.log.borrow().clone()
    }
}

impl LayoutStore for TestStore {
    fn revision(&self) -> u64 {
        self.library.revision()
    }
    fn list(&self) -> Result<Vec<LayoutSummary>, LayoutError> {
        self.lists.set(self.lists.get() + 1);
        self.failing(Fail::List)?;
        LayoutStore::list(&self.library)
    }
    fn get(&self, name: &str) -> Result<Option<Layout>, LayoutError> {
        self.failing(Fail::Get)?;
        LayoutStore::get(&self.library, name)
    }
    fn save(&self, name: &str, layout: Layout) -> Result<Layout, LayoutError> {
        self.log.borrow_mut().push(format!("PUT {name}"));
        self.failing(Fail::Save)?;
        LayoutStore::save(&self.library, name, layout)
    }
    fn duplicate(&self, name: &str) -> Result<Layout, LayoutError> {
        self.log.borrow_mut().push(format!("POST duplicate {name}"));
        LayoutStore::duplicate(&self.library, name)
    }
    fn rename(&self, name: &str, new_name: &str) -> Result<Layout, LayoutError> {
        self.log.borrow_mut().push(format!("PATCH {name}"));
        LayoutStore::rename(&self.library, name, new_name)
    }
    fn delete(&self, name: &str) -> Result<(), LayoutError> {
        self.log.borrow_mut().push(format!("DELETE {name}"));
        LayoutStore::delete(&self.library, name)
    }
}

fn tile(i: &str, x: u32, y: u32, w: u32, h: u32) -> Tile {
    Tile {
        i: i.into(),
        kind: i.into(),
        x,
        y,
        w,
        h,
        settings: json!({}),
        extra: Map::new(),
    }
}

/// A user layout like the web fake server's `layoutOf`: wide arranged, the others not.
fn layout_of(name: &str) -> Layout {
    Layout {
        schema_version: 1,
        name: name.into(),
        arrangements: Arrangements {
            wide: vec![tile("notice", 0, 0, 24, 1), tile("future-tile", 0, 1, 6, 2)],
            tablet: None,
            phone: None,
            extra: Map::new(),
        },
        extra: Map::new(),
    }
}

struct Setup {
    controller: LayoutController<TestStore>,
}

fn setup(extra: &[Layout], remembered: Option<&str>, fixed: Option<&str>) -> Setup {
    let store = TestStore::new(extra);
    let mut controller = LayoutController::new(
        store,
        ControllerOptions {
            fixed_layout: fixed.map(str::to_string),
            stream: false,
        },
        remembered.map(str::to_string),
    );
    controller.start();
    Setup { controller }
}

fn mine() -> Setup {
    setup(&[layout_of("Mine")], Some("Mine"), None)
}

fn current_name(c: &LayoutController<TestStore>) -> Option<String> {
    c.current().map(|layout| layout.name.clone())
}

fn kinds(c: &LayoutController<TestStore>) -> Vec<NoticeKind> {
    c.notices().iter().map(|n| n.kind).collect()
}

fn items(tiles: &[(&str, u32, u32, u32, u32)]) -> Vec<GridItem> {
    tiles
        .iter()
        .map(|&(i, x, y, w, h)| GridItem { i: i.into(), x, y, w, h })
        .collect()
}

// opening

#[test]
fn a_new_window_opens_pitwall_classic_and_remembers_nothing_yet() {
    let Setup { controller } = setup(&[], None, None);
    assert_eq!(current_name(&controller).as_deref(), Some(CLASSIC));
    assert_eq!(DEFAULT_LAYOUT, CLASSIC);
    assert_eq!(controller.remembered(), None);
    let names: Vec<_> = controller.layouts().iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, [CLASSIC, "Driver", "Race"]);
    assert!(controller.layouts().iter().all(|l| l.preset));
    assert_eq!(controller.status(), Status::Ready);
}

#[test]
fn the_windows_remembered_layout_opens() {
    let Setup { controller } = mine();
    assert_eq!(current_name(&controller).as_deref(), Some("Mine"));
    assert!(controller.notices().is_empty());
}

#[test]
fn a_remembered_layout_that_no_longer_exists_costs_one_re_pick_with_a_notice() {
    let Setup { controller } = setup(&[], Some("Gone"), None);
    assert_eq!(current_name(&controller).as_deref(), Some(CLASSIC));
    assert_eq!(kinds(&controller), [NoticeKind::Moved]);
    assert_eq!(
        controller.notices()[0].text,
        "\"Gone\" no longer exists. Showing Pitwall Classic."
    );
    assert_eq!(controller.remembered(), Some(CLASSIC));
}

#[test]
fn a_fixed_layout_opens_and_leaves_the_remembered_choice_alone() {
    let Setup { mut controller } = setup(&[layout_of("Mine")], Some("Race"), Some("Mine"));
    assert_eq!(current_name(&controller).as_deref(), Some("Mine"));
    assert!(controller.fixed());
    controller.select("Driver");
    assert_eq!(current_name(&controller).as_deref(), Some("Driver"));
    assert_eq!(controller.remembered(), Some("Race"));
}

#[test]
fn an_unreadable_library_is_an_error_state_not_a_crash() {
    let store = TestStore::new(&[]);
    store.fail.set(Fail::Everything);
    let mut controller = LayoutController::new(store, ControllerOptions::default(), None);
    controller.start();
    assert_eq!(controller.status(), Status::Error);
}

// switcher actions

#[test]
fn choosing_a_layout_shows_it_and_the_window_remembers_it() {
    let Setup { mut controller } = setup(&[], None, None);
    controller.select("Race");
    assert_eq!(current_name(&controller).as_deref(), Some("Race"));
    assert_eq!(controller.remembered(), Some("Race"));
}

#[test]
fn choosing_a_layout_that_vanished_says_so_and_keeps_the_current_one() {
    let Setup { mut controller } = setup(&[], None, None);
    controller.select("Nope");
    assert_eq!(current_name(&controller).as_deref(), Some(CLASSIC));
    assert_eq!(controller.notices().last().unwrap().text, "\"Nope\" no longer exists.");
    assert_eq!(controller.notices().last().unwrap().kind, NoticeKind::Error);
}

#[test]
fn duplicate_makes_a_copy_and_switches_to_it() {
    let Setup { mut controller } = setup(&[], None, None);
    controller.duplicate("Race");
    assert_eq!(current_name(&controller).as_deref(), Some("Race copy"));
    assert!(!controller.is_preset("Race copy"));
    assert_eq!(controller.remembered(), Some("Race copy"));
    assert_eq!(controller.store().log(), ["POST duplicate Race"]);
}

#[test]
fn rename_follows_the_layout_this_window_shows() {
    let Setup { mut controller } = mine();
    controller.rename("Mine", "Mine 2");
    assert_eq!(current_name(&controller).as_deref(), Some("Mine 2"));
    assert_eq!(controller.remembered(), Some("Mine 2"));
    assert!(controller.layouts().iter().any(|l| l.name == "Mine 2"));
}

#[test]
fn a_refused_rename_reports_why_and_changes_nothing() {
    let Setup { mut controller } = setup(&[layout_of("Mine"), layout_of("Taken")], Some("Mine"), None);
    controller.rename("Mine", "Taken");
    assert_eq!(current_name(&controller).as_deref(), Some("Mine"));
    let last = controller.notices().last().unwrap();
    assert_eq!((last.kind, last.text.as_str()), (NoticeKind::Error, "name already in use"));
    assert!(controller.store().library.get("Mine").is_some());
}

#[test]
fn deleting_the_layout_this_window_shows_falls_back_to_pitwall_classic() {
    let Setup { mut controller } = mine();
    controller.remove("Mine");
    assert!(controller.store().library.get("Mine").is_none());
    assert_eq!(current_name(&controller).as_deref(), Some(CLASSIC));
    assert_eq!(controller.remembered(), Some(CLASSIC));
}

#[test]
fn presets_cannot_be_renamed_or_deleted_and_the_controller_does_not_even_ask() {
    let Setup { mut controller } = setup(&[], None, None);
    controller.remove("Race");
    controller.rename("Race", "Mine");
    assert!(controller.store().log().is_empty());
}

// several windows

#[test]
fn a_layout_deleted_in_another_window_moves_this_one_to_pitwall_classic_with_a_notice() {
    let Setup { mut controller } = mine();
    controller.store().library.delete("Mine").unwrap();
    controller.sync_library();
    assert_eq!(current_name(&controller).as_deref(), Some(CLASSIC));
    assert_eq!(controller.remembered(), Some(CLASSIC));
    assert_eq!(kinds(&controller), [NoticeKind::Moved]);
    assert_eq!(
        controller.notices()[0].text,
        "\"Mine\" was deleted or renamed on another screen. Showing Pitwall Classic."
    );
}

#[test]
fn a_layout_saved_in_another_window_updates_this_one_live_with_no_notice() {
    let Setup { mut controller } = mine();
    let mut edited = layout_of("Mine");
    edited.arrangements.wide = vec![tile("notice", 0, 4, 24, 1)];
    controller.store().library.save("Mine", edited).unwrap();
    controller.sync_library();
    assert_eq!(controller.current().unwrap().arrangements.wide[0].y, 4);
    assert!(controller.notices().is_empty());
}

#[test]
fn an_unchanged_revision_does_nothing() {
    let Setup { mut controller } = setup(&[], None, None);
    let lists = controller.store().lists.get();
    let version = controller.version();
    controller.sync_library();
    controller.sync_library();
    assert_eq!(controller.store().lists.get(), lists);
    assert_eq!(controller.version(), version);
}

#[test]
fn while_editing_a_save_elsewhere_leaves_the_draft_and_warns_that_done_replaces_it() {
    let Setup { mut controller } = mine();
    controller.start_edit(ScreenClass::Wide);
    controller.move_resize(&items(&[("future-tile", 6, 1, 6, 2)]));
    let mut theirs = layout_of("Mine");
    theirs.arrangements.wide = vec![tile("notice", 0, 9, 24, 1)];
    controller.store().library.save("Mine", theirs).unwrap();
    controller.sync_library();
    let edit = controller.edit().unwrap();
    let moved = edit.draft.arrangements.wide.iter().find(|t| t.i == "future-tile").unwrap();
    assert_eq!(moved.x, 6);
    let last = controller.notices().last().unwrap();
    assert_eq!(last.kind, NoticeKind::Conflict);
    assert!(last.text.contains("Done will replace"));
}

#[test]
fn while_editing_a_layout_deleted_elsewhere_keeps_the_draft_and_says_so() {
    let Setup { mut controller } = mine();
    controller.start_edit(ScreenClass::Wide);
    controller.store().library.delete("Mine").unwrap();
    controller.sync_library();
    assert!(controller.edit().is_some());
    let last = controller.notices().last().unwrap();
    assert_eq!(last.kind, NoticeKind::Conflict);
    assert!(last.text.contains("deleted or renamed"));
}

#[test]
fn this_windows_own_save_does_not_warn_itself() {
    let Setup { mut controller } = mine();
    controller.start_edit(ScreenClass::Wide);
    controller.move_resize(&items(&[("future-tile", 6, 1, 6, 2)]));
    controller.done();
    controller.sync_library();
    assert!(controller.notices().is_empty());
    let current = controller.current().unwrap();
    assert_eq!(current.arrangements.wide.iter().find(|t| t.i == "future-tile").unwrap().x, 6);
}

#[test]
fn a_failed_refresh_is_retried_on_the_next_sync() {
    for which in [Fail::List, Fail::Get] {
        let Setup { mut controller } = mine();
        let mut changed = layout_of("Mine");
        changed.arrangements.wide = vec![tile("notice", 0, 7, 24, 1)];
        controller.store().library.save("Mine", changed).unwrap();
        controller.store().fail.set(which);
        controller.sync_library();
        assert_eq!(controller.current().unwrap().arrangements.wide[0].y, 0);
        controller.sync_library();
        assert_eq!(controller.current().unwrap().arrangements.wide[0].y, 7);
    }
}

// edit mode

#[test]
fn edit_mode_is_reported_and_done_and_cancel_leave_it() {
    let Setup { mut controller } = mine();
    controller.start_edit(ScreenClass::Wide);
    assert!(controller.is_editing());
    controller.cancel_edit();
    assert!(!controller.is_editing());
    controller.start_edit(ScreenClass::Wide);
    controller.done();
    assert!(!controller.is_editing());
}

#[test]
fn cancel_discards_every_change_and_saves_nothing() {
    let Setup { mut controller } = mine();
    controller.start_edit(ScreenClass::Wide);
    controller.move_resize(&items(&[("future-tile", 12, 1, 6, 2)]));
    controller.remove_tile("notice");
    controller.cancel_edit();
    assert!(controller.edit().is_none());
    assert_eq!(controller.current().unwrap().arrangements.wide.len(), 2);
    assert!(controller.store().log().is_empty());
}

#[test]
fn done_without_a_change_saves_nothing_and_makes_no_copy_of_a_preset() {
    let Setup { mut controller } = setup(&[], None, None);
    controller.start_edit(ScreenClass::Wide);
    let same: Vec<GridItem> = controller
        .current()
        .unwrap()
        .arrangements
        .wide
        .iter()
        .map(|t| GridItem { i: t.i.clone(), x: t.x, y: t.y, w: t.w, h: t.h })
        .collect();
    controller.move_resize(&same);
    assert!(!controller.edit().unwrap().dirty);
    controller.done();
    assert!(controller.store().log().is_empty());
    assert!(controller.edit().is_none());
}

#[test]
fn moving_and_resizing_change_the_draft_and_done_saves_it_over_the_layout() {
    let Setup { mut controller } = mine();
    controller.start_edit(ScreenClass::Wide);
    controller.move_resize(&items(&[("notice", 0, 0, 24, 1), ("future-tile", 18, 1, 6, 4)]));
    assert!(controller.edit().unwrap().dirty);
    controller.done();
    assert_eq!(controller.store().log(), ["PUT Mine"]);
    let saved = controller.store().library.get("Mine").unwrap();
    let moved = saved.arrangements.wide.iter().find(|t| t.i == "future-tile").unwrap();
    assert_eq!((moved.x, moved.h), (18, 4));
    assert!(controller.edit().is_none());
}

#[test]
fn editing_changes_only_the_current_screen_class_and_that_class_stops_being_generated() {
    let Setup { mut controller } = mine();
    controller.start_edit(ScreenClass::Tablet);
    controller.move_resize(&items(&[("notice", 0, 0, 12, 1), ("future-tile", 6, 1, 6, 2)]));
    controller.done();
    let saved = controller.store().library.get("Mine").unwrap();
    let original = layout_of("Mine");
    assert_eq!(saved.arrangements.wide, original.arrangements.wide);
    assert_eq!(saved.arrangements.phone, None);
    let tablet = saved.arrangements.tablet.unwrap();
    assert_eq!(tablet.iter().find(|t| t.i == "future-tile").unwrap().x, 6);
}

#[test]
fn the_first_change_to_a_preset_is_saved_as_a_copy_and_the_window_moves_to_it() {
    let Setup { mut controller } = setup(&[], None, None);
    let preset_wide = controller.current().unwrap().arrangements.wide.clone();
    controller.start_edit(ScreenClass::Wide);
    assert_eq!(controller.edit().unwrap().target_name, CLASSIC);
    controller.remove_tile("notice");
    assert_eq!(controller.edit().unwrap().target_name, "Pitwall Classic copy");
    controller.done();
    assert_eq!(
        controller.store().log(),
        ["POST duplicate Pitwall Classic", "PUT Pitwall Classic copy"]
    );
    let library = &controller.store().library;
    assert_eq!(library.get(CLASSIC).unwrap().arrangements.wide, preset_wide);
    let copy = library.get("Pitwall Classic copy").unwrap();
    assert!(copy.arrangements.wide.iter().all(|t| t.i != "notice"));
    assert_eq!(copy.arrangements.wide.len(), preset_wide.len() - 1);
    assert_eq!(current_name(&controller).as_deref(), Some("Pitwall Classic copy"));
    assert_eq!(controller.remembered(), Some("Pitwall Classic copy"));
}

#[test]
fn the_predicted_copy_name_skips_names_already_taken() {
    let Setup { mut controller } = setup(&[layout_of("pitwall classic COPY")], None, None);
    controller.start_edit(ScreenClass::Wide);
    controller.remove_tile("notice");
    assert_eq!(controller.edit().unwrap().target_name, "Pitwall Classic copy 2");
}

#[test]
fn add_puts_a_tile_under_the_others_at_its_default_size_with_its_own_id() {
    let Setup { mut controller } = mine();
    controller.start_edit(ScreenClass::Wide);
    controller.add_tile("future-tile", 6, 2);
    let edit = controller.edit().unwrap();
    let added = edit.draft.arrangements.wide.last().unwrap();
    assert_eq!(
        (added.i.as_str(), added.kind.as_str(), added.x, added.y, added.w, added.h),
        ("future-tile-2", "future-tile", 0, 3, 6, 2)
    );
    assert!(edit.dirty);
}

#[test]
fn tile_settings_are_saved_with_the_tile_instance() {
    let Setup { mut controller } = mine();
    controller.start_edit(ScreenClass::Wide);
    controller.set_tile_settings("future-tile", json!({ "car": { "target": "focus" } }));
    controller.done();
    let saved = controller.store().library.get("Mine").unwrap();
    let tile = saved.arrangements.wide.iter().find(|t| t.i == "future-tile").unwrap();
    assert_eq!(tile.settings, json!({ "car": { "target": "focus" } }));
}

#[test]
fn an_unknown_tile_type_survives_moving_resizing_and_saving_untouched() {
    let mut odd = layout_of("Odd");
    let mut warp = tile("future-1", 0, 3, 4, 2);
    warp.kind = "warp-core".into();
    warp.settings = json!({ "flux": 3 });
    warp.extra.insert("colour".into(), json!("red"));
    odd.arrangements.wide.push(warp);
    let Setup { mut controller } = setup(&[odd], Some("Odd"), None);
    controller.start_edit(ScreenClass::Wide);
    controller.move_resize(&items(&[("future-1", 4, 3, 4, 2)]));
    controller.done();
    let saved = controller.store().library.get("Odd").unwrap();
    let tile = saved.arrangements.wide.iter().find(|t| t.i == "future-1").unwrap();
    assert_eq!(
        serde_json::to_value(tile).unwrap(),
        json!({ "i": "future-1", "type": "warp-core", "x": 4, "y": 3, "w": 4, "h": 2, "settings": { "flux": 3 }, "colour": "red" })
    );
}

#[test]
fn a_failed_save_keeps_the_editor_open_with_the_draft_and_says_why() {
    let Setup { mut controller } = mine();
    controller.start_edit(ScreenClass::Wide);
    controller.remove_tile("notice");
    controller.store().fail.set(Fail::Save);
    controller.done();
    let edit = controller.edit().unwrap();
    assert_eq!(edit.error.as_deref(), Some("disk full"));
    assert!(edit.draft.arrangements.wide.iter().all(|t| t.i != "notice"));
    controller.done();
    assert!(controller.edit().is_none());
}

#[test]
fn a_retry_after_the_copy_was_made_but_the_save_failed_reuses_the_copy() {
    let Setup { mut controller } = setup(&[], None, None);
    controller.start_edit(ScreenClass::Wide);
    controller.remove_tile("notice");
    controller.store().fail.set(Fail::Save);
    controller.done();
    assert!(controller.edit().is_some());
    controller.done();
    let duplicates = controller
        .store()
        .log()
        .iter()
        .filter(|l| l.starts_with("POST duplicate"))
        .count();
    assert_eq!(duplicates, 1);
    assert!(controller.edit().is_none());
    assert_eq!(current_name(&controller).as_deref(), Some("Pitwall Classic copy"));
}

#[test]
fn the_layout_cannot_be_switched_or_renamed_mid_edit() {
    let Setup { mut controller } = mine();
    controller.start_edit(ScreenClass::Wide);
    controller.select("Race");
    controller.rename("Mine", "Other");
    controller.duplicate("Mine");
    controller.remove("Mine");
    assert_eq!(current_name(&controller).as_deref(), Some("Mine"));
    assert!(controller.store().log().is_empty());
}

#[test]
fn dismissing_a_notice_removes_only_it() {
    let Setup { mut controller } = setup(&[], Some("Gone"), None);
    controller.select("Nope");
    let first = controller.notices()[0].id;
    controller.dismiss_notice(first);
    assert_eq!(kinds(&controller), [NoticeKind::Error]);
}

#[test]
fn stream_view_stays_blank_for_a_missing_named_layout_and_recovers_on_a_library_change() {
    let store = TestStore::new(&[]);
    let mut controller = LayoutController::new(
        store,
        ControllerOptions {
            fixed_layout: Some("Air".into()),
            stream: true,
        },
        Some("Race".into()),
    );
    controller.start();
    assert!(controller.current().is_none());
    assert!(controller.notices().is_empty());
    assert_eq!(controller.missing_stream_layout(), Some("Air"));
    assert_eq!(controller.remembered(), Some("Race"));
    controller.store().library.save("Air", layout_of("Air")).unwrap();
    controller.sync_library();
    assert_eq!(current_name(&controller).as_deref(), Some("Air"));
    assert_eq!(controller.missing_stream_layout(), None);
    controller.store().library.delete("Air").unwrap();
    controller.sync_library();
    assert!(controller.current().is_none());
    assert_eq!(controller.missing_stream_layout(), Some("Air"));
    // A stream window is never edited.
    controller.start_edit(ScreenClass::Wide);
    assert!(controller.edit().is_none());
}
