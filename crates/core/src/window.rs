//! One dashboard window apart from drawing it: the layout it shows (through the shared
//! [`LayoutController`]), its remembered choices and the commands its chrome and editor
//! send. A UI draws [`DashboardWindow::tiles`] for its size class and turns clicks and
//! drags into [`Command`]s.
//!
//! Each window remembers its own layout, focus car and units in its own file
//! ([`choices_path`]). Nothing is imported from the browser build (ADR 0003): a new window
//! starts at the defaults and writes nothing until a choice changes. A choice that cannot
//! be saved still applies for this run, with an error to show.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use serde_json::{Value, json};

use crate::{
    binding::{CarBinding, CarIdentity, car_binding_value},
    data::keep_as_bad,
    layout_edit::{ControllerOptions, GridItem, LayoutController, with_setting},
    layouts::{LayoutLibrary, ScreenClass, Tile},
    tile_catalog,
    units::{PressureUnit, SpeedUnit, TemperatureUnit, Units},
};

/// Version written to every choices file.
pub const CHOICES_SCHEMA_VERSION: u32 = 1;

/// Where the window `id` keeps its choices: `<root>/windows/<id>.json`.
pub fn choices_path(root: &Path, id: &str) -> PathBuf {
    root.join("windows").join(format!("{id}.json"))
}

/// What one window remembers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WindowChoices {
    /// The layout it shows; `None` until it first chooses (then Pitwall Classic).
    pub layout: Option<String>,
    /// The focus car; `None` follows the player car.
    pub focus: Option<CarIdentity>,
    pub units: Units,
}

impl WindowChoices {
    /// Read a window's choices. A missing file gives the defaults. Each choice that is
    /// missing, corrupt or unknown falls back on its own. A file that is not JSON at all is
    /// moved aside to `.bad` (returned, for the log and status) and the defaults load.
    pub fn load(path: &Path) -> io::Result<(Self, Option<PathBuf>)> {
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok((Self::default(), None)),
            Err(e) => return Err(e),
        };
        let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
            return Ok((Self::default(), Some(keep_as_bad(path)?)));
        };
        Ok((Self::from_value(&value), None))
    }

    fn from_value(value: &Value) -> Self {
        let units = &value["units"];
        let pick = |key: &str, choices: &[&str]| -> Option<usize> {
            let text = units.get(key)?.as_str()?;
            choices.iter().position(|c| *c == text)
        };
        Self {
            layout: value["layout"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(str::to_owned),
            focus: serde_json::from_value(value["focus"].clone()).ok(),
            units: Units {
                speed: match pick("speed", &["kmh", "mph"]) {
                    Some(1) => SpeedUnit::Mph,
                    _ => SpeedUnit::Kmh,
                },
                temperature: match pick("temperature", &["c", "f"]) {
                    Some(1) => TemperatureUnit::Fahrenheit,
                    _ => TemperatureUnit::Celsius,
                },
                pressure: match pick("pressure", &["psi", "bar"]) {
                    Some(1) => PressureUnit::Bar,
                    _ => PressureUnit::Psi,
                },
            },
        }
    }

    fn to_value(&self) -> Value {
        json!({
            "schemaVersion": CHOICES_SCHEMA_VERSION,
            "layout": self.layout,
            "focus": self.focus,
            "units": {
                "speed": match self.units.speed {
                    SpeedUnit::Kmh => "kmh",
                    SpeedUnit::Mph => "mph",
                },
                "temperature": match self.units.temperature {
                    TemperatureUnit::Celsius => "c",
                    TemperatureUnit::Fahrenheit => "f",
                },
                "pressure": match self.units.pressure {
                    PressureUnit::Psi => "psi",
                    PressureUnit::Bar => "bar",
                },
            },
        })
    }

    /// Write atomically (temporary file, then rename), creating the folder.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let temporary = path.with_extension("json.tmp");
        let text = serde_json::to_vec_pretty(&self.to_value()).map_err(io::Error::other)?;
        fs::write(&temporary, text)?;
        fs::rename(&temporary, path)
    }
}

/// What a window opens with.
pub struct WindowSetup {
    /// The application's one library, shared by every window.
    pub library: LayoutLibrary,
    pub choices: WindowChoices,
    /// The window's choices file. `None` keeps choices in memory only.
    pub path: Option<PathBuf>,
    pub options: ControllerOptions,
}

/// What the window's chrome and editor ask for.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Select(String),
    Duplicate(String),
    Rename(String, String),
    Delete(String),
    /// Edit the given size class of the current layout.
    Edit(ScreenClass),
    Done,
    Cancel,
    /// Add a registered tile type at its default size; unknown types are refused.
    Add(String),
    RemoveTile(String),
    MoveResize(Vec<GridItem>),
    /// Set one setting of a registered tile, keeping its others.
    Setting(String, String, Value),
    /// Set what a follows-car tile follows.
    Car(String, CarBinding),
    /// `None` follows the player car.
    Focus(Option<CarIdentity>),
    Units(Units),
    Dismiss(u64),
    /// Reload the library after a failed load and save the choices again.
    Retry,
}

pub struct DashboardWindow {
    controller: LayoutController<LayoutLibrary>,
    choices: WindowChoices,
    path: Option<PathBuf>,
    error: Option<String>,
}

impl DashboardWindow {
    /// Load the library and show the remembered (or default) layout.
    pub fn open(setup: WindowSetup) -> Self {
        let mut controller =
            LayoutController::new(setup.library, setup.options, setup.choices.layout.clone());
        controller.start();
        let mut window = Self {
            controller,
            choices: setup.choices,
            path: setup.path,
            error: None,
        };
        window.remember();
        window
    }

    pub fn controller(&self) -> &LayoutController<LayoutLibrary> {
        &self.controller
    }
    pub fn choices(&self) -> &WindowChoices {
        &self.choices
    }
    /// Why the last choice could not be saved.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
    /// Show an error that is not the window's own, such as a data folder that failed to
    /// open.
    pub fn set_error(&mut self, error: Option<String>) {
        self.error = error;
    }

    /// The name shown in the switcher: the layout, or while editing a changed preset, the
    /// copy Done will make.
    pub fn current_name(&self) -> Option<&str> {
        self.controller
            .edit()
            .map(|e| e.target_name.as_str())
            .or_else(|| self.controller.current().map(|l| l.name.as_str()))
    }

    /// The tiles to draw for `class`: the draft while editing, else the saved layout.
    /// While editing, only the class being edited is drawn.
    pub fn tiles(&self, class: ScreenClass) -> Vec<Tile> {
        match self.controller.edit() {
            Some(edit) => edit.draft.arrangement(edit.class),
            None => self
                .controller
                .current()
                .map(|l| l.arrangement(class))
                .unwrap_or_default(),
        }
    }

    /// Pick up other windows' library changes; call once per frame.
    pub fn sync(&mut self) {
        self.controller.sync_library();
        self.remember();
    }

    pub fn apply(&mut self, command: Command) {
        match command {
            Command::Select(name) => self.controller.select(&name),
            Command::Duplicate(name) => self.controller.duplicate(&name),
            Command::Rename(name, next) => self.controller.rename(&name, &next),
            Command::Delete(name) => self.controller.remove(&name),
            Command::Edit(class) => self.controller.start_edit(class),
            Command::Done => self.controller.done(),
            Command::Cancel => self.controller.cancel_edit(),
            Command::Add(kind) => {
                if let Some(d) = tile_catalog::declaration(&kind) {
                    self.controller.add_tile(&kind, d.default.0, d.default.1);
                }
            }
            Command::RemoveTile(id) => self.controller.remove_tile(&id),
            Command::MoveResize(items) => self.controller.move_resize(&items),
            Command::Setting(id, key, value) => {
                if let Some(tile) = self.registered_draft_tile(&id) {
                    let settings = with_setting(&tile.settings, &key, value);
                    self.controller.set_tile_settings(&id, settings);
                }
            }
            Command::Car(id, binding) => {
                if let Some(tile) = self
                    .registered_draft_tile(&id)
                    .filter(|t| tile_catalog::declaration(&t.kind).is_some_and(|d| d.follows_car()))
                {
                    let settings = with_setting(&tile.settings, "car", car_binding_value(&binding));
                    self.controller.set_tile_settings(&id, settings);
                }
            }
            Command::Focus(focus) => {
                if self.choices.focus != focus {
                    self.choices.focus = focus;
                    self.persist();
                }
            }
            Command::Units(units) => {
                if self.choices.units != units {
                    self.choices.units = units;
                    self.persist();
                }
            }
            Command::Dismiss(id) => self.controller.dismiss_notice(id),
            Command::Retry => {
                self.controller.start();
                self.persist();
            }
        }
        self.remember();
    }

    /// A tile of a registered type in the draft's edited class; unknown types have no
    /// settings.
    fn registered_draft_tile(&self, id: &str) -> Option<Tile> {
        let edit = self.controller.edit()?;
        edit.draft
            .arrangement(edit.class)
            .into_iter()
            .find(|t| t.i == id)
            .filter(|t| tile_catalog::declaration(&t.kind).is_some())
    }

    fn remember(&mut self) {
        if let Some(layout) = self.controller.remembered()
            && self.choices.layout.as_deref() != Some(layout)
        {
            self.choices.layout = Some(layout.to_owned());
            self.persist();
        }
    }

    fn persist(&mut self) {
        let Some(path) = &self.path else { return };
        self.error = match self.choices.save(path) {
            Ok(()) => None,
            Err(error) => Some(format!(
                "Could not save this window's choices to {}: {error}. They apply until pitwall closes.",
                path.display()
            )),
        };
    }
}
