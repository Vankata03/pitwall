//! Windows lifecycle helpers. These do not depend on the collector or its HTTP shell.
use pitwall_core::{ingest::config::Cli, paths::AppPaths};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

pub fn startup_script(exe: &Path, cli: &Cli) -> String {
    let mut args = format!("\"{}\"", exe.display());
    for (flag, path) in [
        ("--config", &cli.config_path),
        ("--data-dir", &cli.data_dir),
    ] {
        if let Some(path) = path {
            args.push_str(&format!(" {flag} \"{}\"", path.display()));
        }
    }
    if let Some(port) = cli.udp_port {
        args.push_str(&format!(" --udp-port {port}"));
    }
    format!(
        "CreateObject(\"WScript.Shell\").Run \"{}\", 0, False\r\n",
        args.replace('"', "\"\"")
    )
}

/// The current user's Startup-folder entry for Start with Windows.
pub struct Startup {
    script: Option<PathBuf>,
}

impl Startup {
    /// `%APPDATA%/Microsoft/Windows/Start Menu/Programs/Startup/pitwall.vbs`.
    pub fn user() -> Self {
        Self {
            script: std::env::var_os("APPDATA").map(|root| {
                Path::new(&root).join("Microsoft/Windows/Start Menu/Programs/Startup/pitwall.vbs")
            }),
        }
    }

    pub fn at(script: PathBuf) -> Self {
        Self {
            script: Some(script),
        }
    }

    /// Create or remove the entry. Relative `--data-dir`/`--config` become absolute, since
    /// Windows starts the script from another directory.
    pub fn set(&self, enabled: bool, cli: &Cli) -> io::Result<()> {
        let path = self
            .script
            .as_ref()
            .ok_or_else(|| io::Error::other("APPDATA unavailable"))?;
        if enabled {
            let mut cli = cli.clone();
            for path in [&mut cli.data_dir, &mut cli.config_path]
                .into_iter()
                .flatten()
            {
                *path = std::path::absolute(&*path)?;
            }
            fs::create_dir_all(path.parent().unwrap_or(Path::new(".")))?;
            fs::write(path, startup_script(&std::env::current_exe()?, &cli))
        } else {
            match fs::remove_file(path) {
                Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
                _ => Ok(()),
            }
        }
    }
}

/// Quarantined files in the data folder, including window choices and legacy `.bad.N` names.
/// Directory links are not followed, so a data-folder junction cannot traverse other data.
pub fn bad_files(root: &Path) -> Vec<PathBuf> {
    fn visit(root: &Path, files: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(root).into_iter().flatten().flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                visit(&entry.path(), files);
            } else if kind.is_file() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name.ends_with(".bad")
                    || name
                        .rsplit_once(".bad.")
                        .is_some_and(|(_, n)| n.parse::<u64>().is_ok())
                {
                    files.push(entry.path());
                }
            }
        }
    }
    let mut files = Vec::new();
    visit(root, &mut files);
    files.sort();
    files
}

/// A Windows notification; failures are ignored (the log has the message).
pub fn notify(summary: &str, body: &str) {
    let _ = notify_rust::Notification::new()
        .summary(summary)
        .body(body)
        .show();
}

/// Give a GUI-subsystem build a console for `--console`/`--help`: the launching terminal's,
/// or a new one.
pub fn attach_console() {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn AttachConsole(process: u32) -> i32;
        fn AllocConsole() -> i32;
    }
    // SAFETY: plain Win32 calls without pointers; failure leaves the process unchanged.
    unsafe {
        if AttachConsole(u32::MAX) == 0 {
            AllocConsole();
        }
    }
}

pub fn open(path: &Path) -> io::Result<()> {
    std::process::Command::new("rundll32.exe")
        .arg("url.dll,FileProtocolHandler")
        .arg(path)
        .spawn()
        .map(|_| ())
}

pub fn log(paths: &AppPaths, message: &str) {
    let _ = fs::create_dir_all(&paths.logs);
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(paths.logs.join("collector.log"))
    {
        let _ = writeln!(file, "{:?} {message}", std::time::SystemTime::now());
    }
}

/// The outcome of a launch: this process owns the data folder, or another one does.
pub enum Launch {
    Primary(Instance),
    /// Another pitwall owns this data folder (it may still be starting). It was asked to
    /// open its dashboard; this process must exit without receiving telemetry.
    Existing,
}

/// Per-data-folder ownership through the collector's `instance` lock. Ownership is never
/// stolen. A second launch requests a dashboard through the `native-open` file.
pub struct Instance {
    _lock: File,
    request: PathBuf,
}

impl Instance {
    pub fn acquire(paths: &AppPaths) -> io::Result<Launch> {
        fs::create_dir_all(&paths.root)?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(paths.root.join("instance"))?;
        let request = paths.root.join("native-open");
        match file.try_lock() {
            Ok(()) => {
                let _ = fs::remove_file(&request);
                Ok(Launch::Primary(Self {
                    _lock: file,
                    request,
                }))
            }
            Err(fs::TryLockError::WouldBlock) => {
                fs::write(&request, b"open")?;
                Ok(Launch::Existing)
            }
            Err(fs::TryLockError::Error(e)) => Err(e),
        }
    }

    /// True once per second-launch request.
    pub fn take_open_request(&self) -> bool {
        fs::remove_file(&self.request).is_ok()
    }
}

/// The dashboard opens automatically only until it first opens successfully.
pub struct FirstRun {
    marker: PathBuf,
}

impl FirstRun {
    pub fn new(paths: &AppPaths) -> Self {
        Self {
            marker: paths.root.join("native-opened"),
        }
    }

    pub fn pending(&self) -> bool {
        !self.marker.exists()
    }

    /// Record a successful open. Call only after the window exists.
    pub fn opened(&self) -> io::Result<()> {
        if let Some(parent) = self.marker.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&self.marker, b"opened")
    }
}
