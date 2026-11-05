//! Opening the per-user data folder the way the native app does (ADR 0003).
//!
//! Files the web build wrote load as they are. Obsolete settings (`[http]` in
//! `config.toml`, `--http-port`, `--bind`) are accepted, named in [`DataFolder::obsolete`]
//! and dropped on the next save. A stored file that does not parse is renamed with a
//! `.bad` suffix, never overwritten, reported in [`DataFolder::issues`] and in the log,
//! and replaced by defaults. Recordings, track maps and the livery pack keep their own
//! loaders: [`crate::recorder`], [`crate::tracks`] and [`crate::liveries`].
use std::{
    fmt,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

use crate::{
    ingest::config::{Cli, Config},
    layouts::LayoutLibrary,
    overlay::OverlaySettings,
    paths::AppPaths,
};

/// Log file in `logs/` that load notices are appended to.
pub const LOG_FILE: &str = "collector.log";

/// A stored file the data folder loads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataFile {
    Config,
    Layouts,
    Overlay,
}

impl DataFile {
    pub fn label(self) -> &'static str {
        match self {
            DataFile::Config => "Settings (config.toml)",
            DataFile::Layouts => "Layouts (layouts.json)",
            DataFile::Overlay => "Overlay position (overlay.json)",
        }
    }
}

/// A stored file that could not be parsed. It was renamed to `kept_as` and defaults
/// were loaded in its place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadIssue {
    pub file: DataFile,
    /// Where the file was.
    pub path: PathBuf,
    /// Where the unreadable original is now.
    pub kept_as: PathBuf,
    /// The parser's message.
    pub error: String,
}

impl fmt::Display for LoadIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} could not be read ({}); kept as {} and loaded defaults",
            self.file.label(),
            self.error,
            self.kept_as.display()
        )
    }
}

/// The loaded data folder.
pub struct DataFolder {
    pub paths: AppPaths,
    /// The settings file in use: `--config PATH` or `config.toml` in the data folder.
    pub config_path: PathBuf,
    /// Settings with CLI overrides applied. The `http` part is always the default: the
    /// native app has no HTTP server.
    pub config: Config,
    /// Obsolete settings that were present and are ignored, such as `[http].port` or
    /// `--bind`: file keys sorted, then flags.
    pub obsolete: Vec<String>,
    pub layouts: LayoutLibrary,
    pub overlay: OverlaySettings,
    /// Files that could not be parsed on this open, for the status view.
    pub issues: Vec<LoadIssue>,
}

impl DataFolder {
    /// Load config, layouts and overlay settings from `paths`. Unparseable files are kept
    /// as `.bad` and replaced by defaults; notices are appended to the log once. Errors are
    /// reserved for what defaults cannot cover: an explicit `--config` file that is
    /// missing, a CLI value that fails validation, or a file that cannot be read or
    /// renamed.
    pub fn open(paths: AppPaths, cli: &Cli) -> io::Result<Self> {
        let mut issues = Vec::new();
        let mut obsolete = Vec::new();
        let config_path = cli
            .config_path
            .clone()
            .unwrap_or_else(|| paths.config.clone());

        let mut config = match fs::read_to_string(&config_path) {
            Ok(text) => match parse_config(&text) {
                Ok((config, found)) => {
                    obsolete.extend(found);
                    config
                }
                Err(error) => {
                    issues.push(quarantine(DataFile::Config, &config_path, error)?);
                    Config::default()
                }
            },
            Err(error) if error.kind() == io::ErrorKind::NotFound && cli.config_path.is_none() => {
                Config::default()
            }
            Err(error) => return Err(error),
        };
        if let Some(port) = cli.udp_port {
            config.udp.port = port;
        }
        if cli.http_port.is_some() {
            obsolete.push("--http-port".into());
        }
        if cli.bind.is_some() {
            obsolete.push("--bind".into());
        }
        config.validate().map_err(io::Error::other)?;

        let layouts = match LayoutLibrary::open(paths.layouts.clone()) {
            Ok(layouts) => layouts,
            Err(error) if error.kind() == io::ErrorKind::InvalidData => {
                issues.push(quarantine(
                    DataFile::Layouts,
                    &paths.layouts,
                    error.to_string(),
                )?);
                LayoutLibrary::open(paths.layouts.clone())?
            }
            Err(error) => return Err(error),
        };

        let overlay = match OverlaySettings::read(&paths.overlay) {
            Ok(settings) => settings.unwrap_or_default(),
            Err(error) if error.kind() == io::ErrorKind::InvalidData => {
                issues.push(quarantine(
                    DataFile::Overlay,
                    &paths.overlay,
                    error.to_string(),
                )?);
                OverlaySettings::default()
            }
            Err(error) => return Err(error),
        };

        let folder = Self {
            paths,
            config_path,
            config,
            obsolete,
            layouts,
            overlay,
            issues,
        };
        folder.log_notices();
        Ok(folder)
    }

    /// Lines describing what this open ignored or kept aside, as they were logged.
    pub fn notices(&self) -> Vec<String> {
        let mut lines: Vec<String> = self.issues.iter().map(ToString::to_string).collect();
        if !self.obsolete.is_empty() {
            lines.push(format!(
                "Ignored obsolete HTTP settings: {}; they are removed from {} on the next save",
                self.obsolete.join(", "),
                self.config_path.display()
            ));
        }
        lines
    }

    fn log_notices(&self) {
        let lines = self.notices();
        if lines.is_empty() {
            return;
        }
        let _ = fs::create_dir_all(&self.paths.logs);
        if let Ok(mut log) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.paths.logs.join(LOG_FILE))
        {
            let now = chrono::DateTime::<chrono::Utc>::from(std::time::SystemTime::now())
                .format("%Y-%m-%dT%H:%M:%SZ");
            for line in lines {
                let _ = writeln!(log, "{now} {line}");
            }
        }
    }

    /// Save settings to [`Self::config_path`] atomically, without the obsolete `[http]`
    /// table. Pass the user's settings, not ones carrying CLI overrides.
    pub fn save_config(&self, config: &Config) -> io::Result<()> {
        config.validate().map_err(io::Error::other)?;
        let mut table = toml::Table::try_from(config).map_err(io::Error::other)?;
        table.remove("http");
        let text = toml::to_string_pretty(&table).map_err(io::Error::other)?;
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let temporary = self.config_path.with_extension("toml.tmp");
        fs::write(&temporary, text)?;
        fs::rename(&temporary, &self.config_path)
    }
}

/// Parse `config.toml`, accepting and setting aside the obsolete `[http]` table whatever
/// it holds. Returns the obsolete keys found.
fn parse_config(text: &str) -> Result<(Config, Vec<String>), String> {
    let mut table: toml::Table = toml::from_str(text).map_err(|error| error.to_string())?;
    let mut obsolete = Vec::new();
    if let Some(http) = table.remove("http") {
        match http.as_table() {
            Some(keys) if !keys.is_empty() => {
                obsolete.extend(keys.keys().map(|key| format!("[http].{key}")));
            }
            _ => obsolete.push("[http]".into()),
        }
    }
    let config: Config = table.try_into().map_err(|error| error.to_string())?;
    config.validate()?;
    Ok((config, obsolete))
}

/// Rename an unparseable file to the first free `<name>.bad`, `<name>.2.bad`, ...
fn quarantine(file: DataFile, path: &Path, error: String) -> io::Result<LoadIssue> {
    Ok(LoadIssue {
        file,
        path: path.to_path_buf(),
        kept_as: keep_as_bad(path)?,
        error,
    })
}

/// Move an unparseable file aside to the first free `<name>.bad`, `<name>.2.bad`, ...
/// and return where it went. Never overwrites an earlier copy.
pub(crate) fn keep_as_bad(path: &Path) -> io::Result<PathBuf> {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let kept_as = (1..)
        .map(|n| match n {
            1 => path.with_file_name(format!("{name}.bad")),
            n => path.with_file_name(format!("{name}.{n}.bad")),
        })
        .find(|candidate| !candidate.exists())
        .expect("a free name exists");
    fs::rename(path, &kept_as)?;
    Ok(kept_as)
}
