//! The native dashboard command seam, independent of gpui and telemetry.

use super::{preferences::WindowPreferences, registry};
use pitwall_core::{
    binding::CarIdentity,
    layout_edit::{ControllerOptions, GridItem, LayoutController},
    layouts::{LayoutLibrary, ScreenClass, Tile},
    units::Units,
};
use serde_json::Value;
use std::path::PathBuf;

/// Pass the application's shared library and one window's saved preferences.
pub struct DashboardSetup {
    pub library: LayoutLibrary,
    pub preferences: WindowPreferences,
    /// A separate file per stable window ID. None disables preference writes.
    pub preferences_path: Option<PathBuf>,
    pub options: ControllerOptions,
}

#[derive(Clone)]
pub enum Command {
    Select(String),
    Duplicate(String),
    Rename(String, String),
    Delete(String),
    Edit(ScreenClass),
    Done,
    Cancel,
    Add(String),
    RemoveTile(String),
    MoveResize(Vec<GridItem>),
    Setting(String, String, Value),
    Focus(Option<CarIdentity>),
    Units(Units),
    Dismiss(u64),
    Retry,
}

pub struct DashboardModel {
    pub controller: LayoutController<LayoutLibrary>,
    pub preferences: WindowPreferences,
    path: Option<PathBuf>,
    pub error: Option<String>,
}

impl DashboardModel {
    pub fn new(setup: DashboardSetup) -> Self {
        let mut controller = LayoutController::new(
            setup.library,
            setup.options,
            setup.preferences.layout.clone(),
        );
        controller.start();
        Self {
            controller,
            preferences: setup.preferences,
            path: setup.preferences_path,
            error: None,
        }
    }
    pub fn tiles(&self, class: ScreenClass) -> Vec<Tile> {
        self.controller
            .edit()
            .map(|e| &e.draft)
            .or_else(|| self.controller.current())
            .map(|l| l.arrangement(class))
            .unwrap_or_default()
    }
    pub fn sync(&mut self) {
        self.controller.sync_library();
        self.remember();
    }
    fn remember(&mut self) {
        if let Some(layout) = self.controller.remembered()
            && self.preferences.layout.as_deref() != Some(layout)
        {
            self.preferences.layout = Some(layout.to_owned());
            self.persist();
        }
    }
    fn persist(&mut self) {
        if let Some(path) = &self.path {
            match self.preferences.save(path) {
                Ok(()) => self.error = None,
                Err(error) => {
                    self.error = Some(format!("Could not save window preferences: {error}"));
                    eprintln!(
                        "pitwall-native: {}",
                        self.error.as_deref().unwrap_or_default()
                    );
                }
            }
        }
    }
    pub fn execute(&mut self, command: Command) {
        match command {
            Command::Select(name) => self.controller.select(&name),
            Command::Duplicate(name) => self.controller.duplicate(&name),
            Command::Rename(name, next) => self.controller.rename(&name, &next),
            Command::Delete(name) => self.controller.remove(&name),
            Command::Edit(class) => self.controller.start_edit(class),
            Command::Done => self.controller.done(),
            Command::Cancel => self.controller.cancel_edit(),
            Command::Add(kind) => {
                if let Some(tile) = registry::get(&kind).filter(|t| t.render.is_some()) {
                    self.controller
                        .add_tile(&kind, tile.default.0, tile.default.1);
                }
            }
            Command::RemoveTile(id) => self.controller.remove_tile(&id),
            Command::MoveResize(items) => self.controller.move_resize(&items),
            Command::Setting(id, key, value) => {
                if let Some(edit) = self.controller.edit()
                    && let Some(tile) = edit
                        .draft
                        .arrangement(edit.class)
                        .iter()
                        .find(|t| t.i == id)
                    && registry::get(&tile.kind).is_some_and(|r| r.render.is_some())
                {
                    let mut settings = tile.settings.clone();
                    if let Some(object) = settings.as_object_mut() {
                        object.insert(key, value);
                        self.controller.set_tile_settings(&id, settings);
                    }
                }
            }
            Command::Focus(focus) => {
                self.preferences.focus = focus;
                self.persist();
            }
            Command::Units(units) => {
                self.preferences.units = units;
                self.persist();
            }
            Command::Dismiss(id) => self.controller.dismiss_notice(id),
            Command::Retry => {
                self.controller.start();
                self.persist();
            }
        }
        self.remember();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pitwall_core::layouts::{Layout, LayoutLibrary, ScreenClass};
    use serde_json::json;

    #[test]
    fn editing_unknown_tiles_keeps_data_and_only_changes_active_arrangement() {
        let root = std::env::temp_dir().join(format!("pitwall-dashboard-{}", std::process::id()));
        let library = LayoutLibrary::open(root.join("layouts.json")).unwrap();
        let layout: Layout = serde_json::from_value(json!({"schemaVersion":1,"name":"Custom","future":42,"arrangements":{
            "wide":[{"i":"future","type":"future-tile","x":0,"y":0,"w":4,"h":3,"settings":{"opaque":[1,2]},"extra":true}]
        }})).unwrap();
        library.save("Custom", layout.clone()).unwrap();
        let mut model = DashboardModel::new(DashboardSetup {
            library: library.clone(),
            preferences: WindowPreferences {
                layout: Some("Custom".into()),
                ..Default::default()
            },
            preferences_path: Some(root.join("window.json")),
            options: Default::default(),
        });
        model.execute(Command::Edit(ScreenClass::Tablet));
        model.execute(Command::MoveResize(vec![GridItem {
            i: "future".into(),
            x: 2,
            y: 3,
            w: 5,
            h: 4,
        }]));
        model.execute(Command::Done);
        let saved = library.get("Custom").unwrap();
        assert_eq!(saved.arrangements.wide, layout.arrangements.wide);
        let tile = &saved.arrangements.tablet.as_ref().unwrap()[0];
        assert_eq!((tile.x, tile.y, tile.w, tile.h), (2, 3, 5, 4));
        assert_eq!(tile.settings, json!({"opaque":[1,2]}));
        assert_eq!(tile.extra["extra"], true);
        assert_eq!(saved.extra["future"], 42);
        std::fs::remove_dir_all(root).unwrap();
    }
}
