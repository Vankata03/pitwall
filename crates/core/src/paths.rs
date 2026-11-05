use std::io;
use std::path::{Path, PathBuf};

/// Persistent paths under the OS per-user data directory. Resolving creates no files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    pub root: PathBuf,
    pub config: PathBuf,
    pub layouts: PathBuf,
    pub recordings: PathBuf,
    pub tracks: PathBuf,
    pub liveries: PathBuf,
    pub logs: PathBuf,
}

impl AppPaths {
    /// Resolve `%APPDATA%/pitwall` on Windows or
    /// `~/Library/Application Support/pitwall` on macOS.
    pub fn resolve() -> io::Result<Self> {
        let data_dir = dirs::data_dir()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no per-user data directory"))?;
        Ok(Self::from_data_dir(&data_dir))
    }

    /// Use `data_dir` when supplied (`--data-dir`), otherwise [`Self::resolve`].
    pub fn resolve_or(data_dir: Option<&Path>) -> io::Result<Self> {
        data_dir.map_or_else(Self::resolve, |dir| Ok(Self::from_data_dir(dir)))
    }

    /// Use a supplied OS data directory, for isolated hosts and tests.
    pub fn from_data_dir(data_dir: &Path) -> Self {
        let root = data_dir.join("pitwall");
        Self {
            config: root.join("config.toml"),
            layouts: root.join("layouts.json"),
            recordings: root.join("recordings"),
            tracks: root.join("tracks"),
            liveries: root.join("liveries"),
            logs: root.join("logs"),
            root,
        }
    }
}
