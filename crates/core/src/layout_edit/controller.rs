//! Which layout a window shows and how it is edited, apart from drawing it. Ported from
//! `dashboard/src/layout/controller.ts`.
//!
//! The web controller talked to the collector over HTTP and had to discard late replies.
//! Natively it calls the in-process [`LayoutStore`] (the shared [`LayoutLibrary`])
//! synchronously, so every action finishes before it returns. Call
//! [`LayoutController::sync_library`] whenever another window may have changed the
//! library (cheap: one revision read); a change refreshes this window and raises the
//! multi-window notices. Persist [`LayoutController::remembered`] as the window's chosen
//! layout when it changes.

use crate::layouts::{Layout, LayoutError, LayoutLibrary, ScreenClass, Tile};
use serde_json::Value;

/// The preset a new window opens, and the fallback when its layout goes away.
pub const DEFAULT_LAYOUT: &str = "Pitwall Classic";

/// One entry of the layout switcher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutSummary {
    pub name: String,
    /// Presets are read-only.
    pub preset: bool,
}

/// The layout library as the controller uses it. [`LayoutLibrary`] is the real one; a
/// test can wrap it to log calls or fail them.
pub trait LayoutStore {
    /// Advances on every successful change, whichever window made it.
    fn revision(&self) -> u64;
    /// Every layout, presets first.
    fn list(&self) -> Result<Vec<LayoutSummary>, LayoutError>;
    fn get(&self, name: &str) -> Result<Option<Layout>, LayoutError>;
    /// Replace or create the layout `name`; returns it as stored.
    fn save(&self, name: &str, layout: Layout) -> Result<Layout, LayoutError>;
    /// Copy a layout as "<name> copy", numbered if taken.
    fn duplicate(&self, name: &str) -> Result<Layout, LayoutError>;
    fn rename(&self, name: &str, new_name: &str) -> Result<Layout, LayoutError>;
    fn delete(&self, name: &str) -> Result<(), LayoutError>;
}

impl LayoutStore for LayoutLibrary {
    fn revision(&self) -> u64 {
        LayoutLibrary::revision(self)
    }
    fn list(&self) -> Result<Vec<LayoutSummary>, LayoutError> {
        Ok(LayoutLibrary::list(self)
            .into_iter()
            .map(|layout| LayoutSummary {
                preset: self.is_preset(&layout.name),
                name: layout.name,
            })
            .collect())
    }
    fn get(&self, name: &str) -> Result<Option<Layout>, LayoutError> {
        Ok(LayoutLibrary::get(self, name))
    }
    fn save(&self, name: &str, layout: Layout) -> Result<Layout, LayoutError> {
        LayoutLibrary::save(self, name, layout)?;
        LayoutLibrary::get(self, name).ok_or(LayoutError::NotFound)
    }
    fn duplicate(&self, name: &str) -> Result<Layout, LayoutError> {
        LayoutLibrary::duplicate(self, name, None)
    }
    fn rename(&self, name: &str, new_name: &str) -> Result<Layout, LayoutError> {
        LayoutLibrary::rename(self, name, new_name)
    }
    fn delete(&self, name: &str) -> Result<(), LayoutError> {
        LayoutLibrary::delete(self, name)
    }
}

/// What a failed library call tells the user; the same words the web routes used.
fn message_of(error: &LayoutError) -> String {
    match error {
        LayoutError::NotFound => "no such layout".into(),
        LayoutError::NameTaken => "name already in use".into(),
        LayoutError::InvalidName => "invalid layout name".into(),
        LayoutError::InvalidLayout(reason) => reason.clone(),
        LayoutError::ReadOnly => "presets are read-only".into(),
        LayoutError::Io(error) => error.to_string(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Loading,
    Ready,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeKind {
    /// This window was moved to another layout.
    Moved,
    /// Another window changed the layout being edited here.
    Conflict,
    /// An action failed.
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub id: u64,
    pub kind: NoticeKind,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EditState {
    /// The screen class being edited: only this class changes.
    pub class: ScreenClass,
    /// What the editor works on. The grid shows this, not the saved layout.
    pub draft: Layout,
    /// The layout as it was when editing began.
    pub base: Layout,
    pub base_preset: bool,
    pub dirty: bool,
    /// Why the last Done failed.
    pub error: Option<String>,
    /// Where Done will save: the layout itself, or for a changed preset, the copy it will
    /// make.
    pub target_name: String,
}

/// Grid positions reported by the editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridItem {
    pub i: String,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ControllerOptions {
    /// Show this layout and never touch the remembered choice (the web `?layout=`).
    pub fixed_layout: Option<String>,
    /// A stream window: shows `fixed_layout` (or the default), blank while it is missing,
    /// never edited, no notices.
    pub stream: bool,
}

/// The name the library gives a copy: "<name> copy", then "<name> copy 2", ignoring case.
fn copy_name_for(name: &str, taken: &[LayoutSummary]) -> String {
    let used = |candidate: &str| {
        taken
            .iter()
            .any(|l| l.name.to_lowercase() == candidate.to_lowercase())
    };
    let mut candidate = format!("{name} copy");
    let mut n = 2;
    while used(&candidate) {
        candidate = format!("{name} copy {n}");
        n += 1;
    }
    candidate
}

fn sorted_by_id(mut tiles: Vec<Tile>) -> Vec<Tile> {
    tiles.sort_by(|a, b| a.i.cmp(&b.i));
    tiles
}

pub struct LayoutController<S: LayoutStore> {
    store: S,
    options: ControllerOptions,
    remembered: Option<String>,
    status: Status,
    layouts: Vec<LayoutSummary>,
    current: Option<Layout>,
    edit: Option<EditState>,
    notices: Vec<Notice>,
    next_notice: u64,
    /// Library revision this window last read in full.
    known: Option<u64>,
    /// A copy made for a preset whose save then failed; a retry reuses it.
    pending_copy: Option<String>,
    missing_stream_layout: Option<String>,
    version: u64,
}

impl<S: LayoutStore> LayoutController<S> {
    /// `remembered` is the window's saved layout choice, if any. Nothing is shown until
    /// [`start`](Self::start).
    pub fn new(store: S, options: ControllerOptions, remembered: Option<String>) -> Self {
        Self {
            store,
            options,
            remembered,
            status: Status::Loading,
            layouts: Vec::new(),
            current: None,
            edit: None,
            notices: Vec::new(),
            next_notice: 1,
            known: None,
            pending_copy: None,
            missing_stream_layout: None,
            version: 0,
        }
    }

    pub fn store(&self) -> &S {
        &self.store
    }
    pub fn status(&self) -> Status {
        self.status
    }
    pub fn layouts(&self) -> &[LayoutSummary] {
        &self.layouts
    }
    /// The layout this window shows when not editing; while editing, draw `edit().draft`.
    pub fn current(&self) -> Option<&Layout> {
        self.current.as_ref()
    }
    pub fn edit(&self) -> Option<&EditState> {
        self.edit.as_ref()
    }
    pub fn is_editing(&self) -> bool {
        self.edit.is_some()
    }
    pub fn notices(&self) -> &[Notice] {
        &self.notices
    }
    /// The window shows a fixed layout and leaves the remembered choice alone.
    pub fn fixed(&self) -> bool {
        self.options.fixed_layout.is_some() || self.options.stream
    }
    /// The layout this window remembers; persist it when it changes. `None` until the
    /// window first chooses, so a new window writes nothing.
    pub fn remembered(&self) -> Option<&str> {
        self.remembered.as_deref()
    }
    /// The stream window's named layout while it is missing, for status and logging.
    pub fn missing_stream_layout(&self) -> Option<&str> {
        self.missing_stream_layout.as_deref()
    }
    pub fn is_preset(&self, name: &str) -> bool {
        self.layouts.iter().any(|l| l.name == name && l.preset)
    }
    /// Bumped on every visible change; redraw when it moves.
    pub fn version(&self) -> u64 {
        self.version
    }

    fn changed(&mut self) {
        self.version += 1;
    }

    fn notify(&mut self, kind: NoticeKind, text: String) {
        self.notices.retain(|n| !(n.kind == kind && n.text == text));
        self.notices.push(Notice {
            id: self.next_notice,
            kind,
            text,
        });
        self.next_notice += 1;
        self.changed();
    }

    fn remember(&mut self, name: &str) {
        if !self.fixed() {
            self.remembered = Some(name.to_string());
        }
    }

    fn load_layouts(&mut self) -> Result<(), LayoutError> {
        let revision = self.store.revision();
        self.layouts = self.store.list()?;
        self.known = Some(revision);
        self.refresh_target();
        self.changed();
        Ok(())
    }

    /// Show `name`; false when it does not exist.
    fn show(&mut self, name: &str) -> Result<bool, LayoutError> {
        let Some(found) = self.store.get(name)? else {
            return Ok(false);
        };
        self.current = Some(found);
        self.status = Status::Ready;
        self.changed();
        Ok(true)
    }

    fn fall_back(&mut self, text: String) -> Result<(), LayoutError> {
        match self.store.get(DEFAULT_LAYOUT)? {
            None => {
                self.status = Status::Error;
                self.current = None;
                self.changed();
            }
            Some(classic) => {
                self.remember(DEFAULT_LAYOUT);
                self.current = Some(classic);
                self.status = Status::Ready;
                self.notify(NoticeKind::Moved, text);
            }
        }
        Ok(())
    }

    fn show_stream(&mut self) -> Result<(), LayoutError> {
        let wanted = self
            .options
            .fixed_layout
            .clone()
            .unwrap_or_else(|| DEFAULT_LAYOUT.to_string());
        let found = self.store.get(&wanted)?;
        self.missing_stream_layout = found.is_none().then_some(wanted);
        self.current = found;
        self.status = Status::Ready;
        self.changed();
        Ok(())
    }

    /// Load the library and show the fixed, remembered or default layout. A remembered
    /// layout that is gone falls back to the default with a notice.
    pub fn start(&mut self) {
        let result = (|| {
            self.load_layouts()?;
            if self.options.stream {
                return self.show_stream();
            }
            let wanted = self
                .options
                .fixed_layout
                .clone()
                .or_else(|| self.remembered.clone())
                .unwrap_or_else(|| DEFAULT_LAYOUT.to_string());
            if !self.show(&wanted)? {
                self.fall_back(format!(
                    "\"{wanted}\" no longer exists. Showing {DEFAULT_LAYOUT}."
                ))?;
            }
            Ok(())
        })();
        if result.is_err() {
            self.status = Status::Error;
            self.changed();
        }
    }

    /// Pick up changes other windows made to the library. Nothing happens while the
    /// revision is the one last read; a failed refresh is retried on the next call.
    pub fn sync_library(&mut self) {
        let Some(known) = self.known else { return };
        let revision = self.store.revision();
        if revision == known {
            return;
        }
        let _ = self.refresh(revision);
    }

    fn refresh(&mut self, revision: u64) -> Result<(), LayoutError> {
        self.layouts = self.store.list()?;
        self.refresh_target();
        self.changed();
        if self.options.stream {
            self.show_stream()?;
            self.known = Some(revision);
            return Ok(());
        }
        let Some(here) = self.current.clone() else {
            self.known = Some(revision);
            return Ok(());
        };
        if !self.layouts.iter().any(|l| l.name == here.name) {
            if self.is_editing() {
                self.notify(
                    NoticeKind::Conflict,
                    format!(
                        "\"{}\" was deleted or renamed on another screen. Done will save it again under this name.",
                        here.name
                    ),
                );
            } else {
                self.fall_back(format!(
                    "\"{}\" was deleted or renamed on another screen. Showing {DEFAULT_LAYOUT}.",
                    here.name
                ))?;
            }
            self.known = Some(revision);
            return Ok(());
        }
        let Some(latest) = self.store.get(&here.name)? else {
            return Ok(()); // vanished between list and get: retry next time
        };
        self.known = Some(revision);
        if latest == here {
            return Ok(());
        }
        self.current = Some(latest);
        self.changed();
        if self.is_editing() {
            self.notify(
                NoticeKind::Conflict,
                format!(
                    "\"{}\" was saved on another screen. Done will replace it with your edits.",
                    here.name
                ),
            );
        }
        Ok(())
    }

    /// Show a layout and remember it. Ignored while editing.
    pub fn select(&mut self, name: &str) {
        if self.is_editing() {
            return;
        }
        match self.show(name) {
            Ok(true) => self.remember(name),
            Ok(false) => {
                self.notify(NoticeKind::Error, format!("\"{name}\" no longer exists."));
                let _ = self.load_layouts();
            }
            Err(error) => self.notify(NoticeKind::Error, message_of(&error)),
        }
    }

    /// Copy a layout and switch to the copy. Ignored while editing.
    pub fn duplicate(&mut self, name: &str) {
        if self.is_editing() {
            return;
        }
        let result = self.store.duplicate(name).and_then(|copy| {
            self.load_layouts()?;
            Ok(copy)
        });
        match result {
            Ok(copy) => {
                self.remember(&copy.name);
                self.current = Some(copy);
                self.changed();
            }
            Err(error) => self.notify(NoticeKind::Error, message_of(&error)),
        }
    }

    /// Rename a user layout; the window follows it. Presets are never sent.
    pub fn rename(&mut self, name: &str, new_name: &str) {
        if self.is_editing() || self.is_preset(name) {
            return;
        }
        let result = self.store.rename(name, new_name).and_then(|renamed| {
            self.load_layouts()?;
            Ok(renamed)
        });
        match result {
            Ok(renamed) => {
                if self.current.as_ref().is_some_and(|c| c.name == name) {
                    self.remember(&renamed.name);
                    self.current = Some(renamed);
                    self.changed();
                }
            }
            Err(error) => self.notify(NoticeKind::Error, message_of(&error)),
        }
    }

    /// Delete a user layout; a window showing it moves to the default. Presets are never
    /// sent.
    pub fn remove(&mut self, name: &str) {
        if self.is_editing() || self.is_preset(name) {
            return;
        }
        let result = (|| {
            self.store.delete(name)?;
            self.load_layouts()?;
            if self.current.as_ref().is_some_and(|c| c.name == name)
                && let Some(classic) = self.store.get(DEFAULT_LAYOUT)?
            {
                self.current = Some(classic);
                self.remember(DEFAULT_LAYOUT);
                self.changed();
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.notify(NoticeKind::Error, message_of(&error));
        }
    }

    pub fn dismiss_notice(&mut self, id: u64) {
        self.notices.retain(|n| n.id != id);
        self.changed();
    }

    /// Start editing `class` of the current layout. Not for stream windows.
    pub fn start_edit(&mut self, class: ScreenClass) {
        if self.options.stream || self.edit.is_some() {
            return;
        }
        let Some(current) = self.current.clone() else {
            return;
        };
        self.edit = Some(EditState {
            class,
            draft: current.clone(),
            base_preset: self.is_preset(&current.name),
            target_name: current.name.clone(),
            base: current,
            dirty: false,
            error: None,
        });
        self.changed();
    }

    fn leave_edit(&mut self) {
        self.pending_copy = None;
        self.edit = None;
        self.changed();
    }

    /// Leave edit mode, discarding the draft.
    pub fn cancel_edit(&mut self) {
        if self.edit.is_some() {
            self.leave_edit();
        }
    }

    fn refresh_target(&mut self) {
        let layouts = &self.layouts;
        if let Some(edit) = &mut self.edit {
            edit.target_name = if edit.base_preset && edit.dirty {
                copy_name_for(&edit.base.name, layouts)
            } else {
                edit.base.name.clone()
            };
        }
    }

    /// Save the draft and leave edit mode. Without a change nothing is saved. The first
    /// change to a preset is saved as a copy, which the window then shows. A failure keeps
    /// the editor and draft open with the reason in `edit().error`; a retry reuses a copy
    /// already made.
    pub fn done(&mut self) {
        let Some(edit) = self.edit.clone() else {
            return;
        };
        if !edit.dirty {
            self.leave_edit();
            return;
        }
        let result = (|| {
            let mut target = edit.base.name.clone();
            if edit.base_preset {
                if self.pending_copy.is_none() {
                    self.pending_copy = Some(self.store.duplicate(&edit.base.name)?.name);
                }
                target = self.pending_copy.clone().expect("copy made");
            }
            let body = Layout {
                name: target.clone(),
                ..edit.draft.clone()
            };
            self.store.save(&target, body)
        })();
        match result {
            Ok(saved) => {
                // The saved layout is what matters; the list refreshes on the next sync.
                let _ = self.load_layouts();
                self.remember(&saved.name);
                self.current = Some(saved);
                self.leave_edit();
            }
            Err(error) => {
                if let Some(edit) = &mut self.edit {
                    edit.error = Some(message_of(&error));
                }
                self.changed();
            }
        }
    }

    fn change_draft(&mut self, change: impl FnOnce(&Layout, ScreenClass) -> Layout) {
        let Some(edit) = &self.edit else { return };
        let draft = change(&edit.draft, edit.class);
        let dirty = sorted_by_id(draft.arrangement(edit.class))
            != sorted_by_id(edit.base.arrangement(edit.class));
        if draft == edit.draft && dirty == edit.dirty {
            return;
        }
        let edit = self.edit.as_mut().expect("editing");
        edit.draft = draft;
        edit.dirty = dirty;
        edit.error = None;
        self.refresh_target();
        self.changed();
    }

    /// Positions from the grid. Only a real change makes the draft dirty.
    pub fn move_resize(&mut self, items: &[GridItem]) {
        self.change_draft(|draft, class| {
            let tiles = draft
                .arrangement(class)
                .into_iter()
                .map(|tile| match items.iter().find(|item| item.i == tile.i) {
                    Some(item) => Tile {
                        x: item.x,
                        y: item.y,
                        w: item.w,
                        h: item.h,
                        ..tile
                    },
                    None => tile,
                })
                .collect();
            super::replace_arrangement(draft, class, tiles)
        });
    }

    /// Add a tile of `kind` at its default size (`w` by `h` cells) under the others.
    pub fn add_tile(&mut self, kind: &str, w: u32, h: u32) {
        self.change_draft(|draft, class| {
            let (tiles, _) = super::add_tile(&draft.arrangement(class), kind, w, h, class.columns());
            super::replace_arrangement(draft, class, tiles)
        });
    }

    pub fn remove_tile(&mut self, id: &str) {
        self.change_draft(|draft, class| {
            super::replace_arrangement(draft, class, super::remove_tile(&draft.arrangement(class), id))
        });
    }

    pub fn set_tile_settings(&mut self, id: &str, settings: Value) {
        self.change_draft(|draft, class| {
            super::replace_arrangement(
                draft,
                class,
                super::set_tile_settings(&draft.arrangement(class), id, settings),
            )
        });
    }
}
