use std::{
    fs, io,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::{Layout, SCHEMA_VERSION, Tile, presets};

#[derive(Debug)]
pub enum LayoutError {
    NotFound,
    NameTaken,
    InvalidName,
    InvalidLayout(String),
    /// Presets cannot be replaced, renamed or deleted.
    ReadOnly,
    Io(io::Error),
}

impl From<io::Error> for LayoutError {
    fn from(error: io::Error) -> Self {
        LayoutError::Io(error)
    }
}

/// The on-disk shape of `layouts.json`: only user layouts, never the presets.
#[derive(Serialize, Deserialize)]
struct Stored {
    #[serde(rename = "schemaVersion", default)]
    schema_version: u32,
    #[serde(default)]
    layouts: Vec<Layout>,
    #[serde(flatten)]
    extra: Map<String, Value>,
}

struct Inner {
    path: PathBuf,
    presets: Vec<Layout>,
    user: Vec<Layout>,
    extra: Map<String, Value>,
    revision: u64,
    /// The file's `schemaVersion` while it differs from [`SCHEMA_VERSION`]: the first
    /// write copies the file to `layouts.json.bak` before replacing it.
    needs_backup: bool,
}

/// The shared layout library. Cloning shares one library.
#[derive(Clone)]
pub struct LayoutLibrary(Arc<Mutex<Inner>>);

impl LayoutLibrary {
    /// Open the library at `path` (`layouts.json`); a missing file is an empty library.
    pub fn open(path: PathBuf) -> io::Result<Self> {
        let (user, extra, needs_backup) = match fs::read(&path) {
            Ok(bytes) => {
                let stored: Stored = serde_json::from_slice(&bytes)
                    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
                let needs_backup = stored.schema_version != SCHEMA_VERSION;
                (stored.layouts, stored.extra, needs_backup)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                (Vec::new(), Map::new(), false)
            }
            Err(error) => return Err(error),
        };
        Ok(Self(Arc::new(Mutex::new(Inner {
            path,
            presets: presets::all(),
            user,
            extra,
            revision: 0,
            needs_backup,
        }))))
    }

    /// Counter that advances on every successful change, for State's revision field.
    pub fn revision(&self) -> u64 {
        self.0.lock().unwrap().revision
    }

    /// Every layout, presets first.
    pub fn list(&self) -> Vec<Layout> {
        let inner = self.0.lock().unwrap();
        inner.presets.iter().chain(&inner.user).cloned().collect()
    }

    pub fn is_preset(&self, name: &str) -> bool {
        self.0.lock().unwrap().find_preset(name).is_some()
    }

    pub fn get(&self, name: &str) -> Option<Layout> {
        let inner = self.0.lock().unwrap();
        inner
            .find_preset(name)
            .or_else(|| inner.find_user(name))
            .cloned()
    }

    /// Replace the whole layout stored under `name`, creating it if new. Returns whether
    /// it was created.
    pub fn save(&self, name: &str, mut layout: Layout) -> Result<bool, LayoutError> {
        let name = valid_name(name)?;
        if valid_name(&layout.name)? != name {
            return Err(LayoutError::InvalidLayout(
                "layout name does not match its route".into(),
            ));
        }
        validate(&layout)?;
        layout.name = name.clone();
        let mut inner = self.0.lock().unwrap();
        if inner.find_preset(&name).is_some() {
            return Err(LayoutError::ReadOnly);
        }
        let mut user = inner.user.clone();
        let created = match user.iter().position(|saved| same_name(&saved.name, &name)) {
            Some(at) => {
                user[at] = layout;
                false
            }
            None => {
                user.push(layout);
                true
            }
        };
        inner.commit(user)?;
        Ok(created)
    }

    /// Copy a layout, preset or not. Without a name the copy is "<name> copy", numbered
    /// if that is taken.
    pub fn duplicate(&self, name: &str, new_name: Option<&str>) -> Result<Layout, LayoutError> {
        let mut inner = self.0.lock().unwrap();
        let source = inner
            .find_preset(name)
            .or_else(|| inner.find_user(name))
            .cloned()
            .ok_or(LayoutError::NotFound)?;
        let new_name = match new_name {
            Some(new_name) => {
                let new_name = valid_name(new_name)?;
                if inner.taken(&new_name) {
                    return Err(LayoutError::NameTaken);
                }
                new_name
            }
            None => {
                let mut candidate = format!("{} copy", source.name);
                let mut n = 2;
                while inner.taken(&candidate) {
                    candidate = format!("{} copy {n}", source.name);
                    n += 1;
                }
                candidate
            }
        };
        let copy = Layout {
            name: new_name,
            ..source
        };
        let mut user = inner.user.clone();
        user.push(copy.clone());
        inner.commit(user)?;
        Ok(copy)
    }

    pub fn rename(&self, name: &str, new_name: &str) -> Result<Layout, LayoutError> {
        let new_name = valid_name(new_name)?;
        let mut inner = self.0.lock().unwrap();
        if inner.find_preset(name).is_some() {
            return Err(LayoutError::ReadOnly);
        }
        let at = inner.position(name).ok_or(LayoutError::NotFound)?;
        if !same_name(&inner.user[at].name, &new_name) && inner.taken(&new_name) {
            return Err(LayoutError::NameTaken);
        }
        let mut user = inner.user.clone();
        user[at].name = new_name;
        let renamed = user[at].clone();
        inner.commit(user)?;
        Ok(renamed)
    }

    pub fn delete(&self, name: &str) -> Result<(), LayoutError> {
        let mut inner = self.0.lock().unwrap();
        if inner.find_preset(name).is_some() {
            return Err(LayoutError::ReadOnly);
        }
        let at = inner.position(name).ok_or(LayoutError::NotFound)?;
        let mut user = inner.user.clone();
        user.remove(at);
        inner.commit(user)?;
        Ok(())
    }
}

impl Inner {
    fn find_preset(&self, name: &str) -> Option<&Layout> {
        self.presets
            .iter()
            .find(|layout| same_name(&layout.name, name))
    }

    fn position(&self, name: &str) -> Option<usize> {
        self.user
            .iter()
            .position(|layout| same_name(&layout.name, name))
    }

    fn taken(&self, name: &str) -> bool {
        self.find_preset(name).is_some() || self.find_user(name).is_some()
    }

    /// Write the new user layouts to disk, then adopt them and advance the revision. A
    /// failed write leaves memory and the file as they were. The first write over a file
    /// of another `schemaVersion` copies it to `layouts.json.bak` first (ADR 0003).
    fn commit(&mut self, user: Vec<Layout>) -> io::Result<()> {
        if self.needs_backup {
            match crate::storage::preserve(&self.path, "bak") {
                Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
                _ => {}
            }
            self.needs_backup = false;
        }
        let stored = Stored {
            schema_version: SCHEMA_VERSION,
            layouts: user,
            extra: self.extra.clone(),
        };
        let bytes = serde_json::to_vec_pretty(&stored).map_err(io::Error::other)?;
        crate::storage::atomic_write(&self.path, bytes)?;
        self.user = stored.layouts;
        self.revision += 1;
        Ok(())
    }

    fn find_user(&self, name: &str) -> Option<&Layout> {
        self.user
            .iter()
            .find(|layout| same_name(&layout.name, name))
    }
}

/// Names are unique ignoring case.
fn same_name(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

/// A trimmed, non-empty name of at most 64 characters with no control characters.
fn valid_name(name: &str) -> Result<String, LayoutError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 64 || name.chars().any(char::is_control) {
        return Err(LayoutError::InvalidName);
    }
    Ok(name.to_string())
}

fn validate(layout: &Layout) -> Result<(), LayoutError> {
    let arrangements = &layout.arrangements;
    let classes = [
        ("wide", Some(&arrangements.wide)),
        ("tablet", arrangements.tablet.as_ref()),
        ("phone", arrangements.phone.as_ref()),
    ];
    for (class, tiles) in classes {
        let Some(tiles) = tiles else { continue };
        let mut seen = std::collections::HashSet::new();
        let mut stacked_height = 0u32;
        for Tile { i, x, y, w, h, .. } in tiles {
            if *w == 0 || *h == 0 {
                return Err(LayoutError::InvalidLayout(format!(
                    "{class} tile {i} has no size"
                )));
            }
            // Narrow screens may stack every tile. Both stored rectangles and
            // that generated arrangement must fit the coordinate representation.
            stacked_height = stacked_height.checked_add(*h).ok_or_else(|| {
                LayoutError::InvalidLayout(format!("{class} arrangement height overflows"))
            })?;
            if x.checked_add(*w).is_none() || y.checked_add(*h).is_none() {
                return Err(LayoutError::InvalidLayout(format!(
                    "{class} tile {i} coordinates overflow"
                )));
            }
            if !seen.insert(i) {
                return Err(LayoutError::InvalidLayout(format!(
                    "{class} tile id {i} is used twice"
                )));
            }
        }
    }
    Ok(())
}
