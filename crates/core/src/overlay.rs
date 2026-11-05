//! What the in-game overlay remembers between runs: `overlay.json` in the data folder.
use serde::{Deserialize, Serialize};
use std::{fs, io, path::Path};

/// The overlay's stored visibility, position and size.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverlaySettings {
    pub visible: bool,
    /// Physical screen position of the window's top-left corner; `None` until first placed.
    pub position: Option<(i32, i32)>,
    /// Physical inner size; `None` until first resized. Files from before sizing lack it.
    #[serde(default)]
    pub size: Option<(u32, u32)>,
}

impl OverlaySettings {
    /// Missing or unreadable settings start hidden at the default position. The native
    /// app loads through [`crate::data::DataFolder`] instead, which keeps unreadable files.
    pub fn load(path: &Path) -> Self {
        Self::read(path).ok().flatten().unwrap_or_default()
    }

    /// Read `overlay.json`: `Ok(None)` when it does not exist, `InvalidData` when it does
    /// not parse.
    pub fn read(path: &Path) -> io::Result<Option<Self>> {
        match fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// Write pretty-printed JSON through a temporary file.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        let tmp = path.with_extension("json.tmp");
        fs::write(
            &tmp,
            serde_json::to_string_pretty(self).map_err(io::Error::other)?,
        )?;
        fs::rename(tmp, path)
    }
}
