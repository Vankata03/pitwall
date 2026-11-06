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
        crate::storage::atomic_write(
            path,
            serde_json::to_vec_pretty(self).map_err(io::Error::other)?,
        )
    }

    /// Load overlay settings, preserving malformed originals with the shared `.bad` policy.
    pub fn load_preserved(path: &Path) -> Loaded {
        match Self::read(path) {
            Ok(Some(settings)) => Loaded::Read(settings),
            Ok(None) => Loaded::Missing(Self::default()),
            Err(error) if error.kind() == io::ErrorKind::InvalidData => {
                match crate::data::keep_as_bad(path) {
                    Ok(kept) => Loaded::Malformed {
                        settings: Self::default(),
                        kept,
                        error: error.to_string(),
                    },
                    Err(rename) => Loaded::Malformed {
                        settings: Self::default(),
                        kept: path.to_owned(),
                        error: format!("{error}; could not keep it: {rename}"),
                    },
                }
            }
            Err(error) => Loaded::Malformed {
                settings: Self::default(),
                kept: path.to_owned(),
                error: error.to_string(),
            },
        }
    }
}

/// Outcome of loading the standalone overlay's settings.
#[derive(Debug, PartialEq)]
pub enum Loaded {
    Read(OverlaySettings),
    Missing(OverlaySettings),
    Malformed {
        settings: OverlaySettings,
        kept: std::path::PathBuf,
        error: String,
    },
}
impl Loaded {
    pub fn settings(&self) -> OverlaySettings {
        match self {
            Self::Read(s) | Self::Missing(s) | Self::Malformed { settings: s, .. } => *s,
        }
    }
}
