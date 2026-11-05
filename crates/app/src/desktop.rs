//! Windows lifecycle helpers. These do not depend on the collector or its HTTP shell.
use pitwall_core::{ingest::config::Cli, paths::AppPaths};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::Path,
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

pub fn autostart(enabled: bool, cli: &Cli) -> io::Result<()> {
    let root =
        std::env::var_os("APPDATA").ok_or_else(|| io::Error::other("APPDATA unavailable"))?;
    let path = Path::new(&root).join("Microsoft/Windows/Start Menu/Programs/Startup/pitwall.vbs");
    if enabled {
        fs::create_dir_all(path.parent().unwrap())?;
        fs::write(path, startup_script(&std::env::current_exe()?, cli))
    } else {
        match fs::remove_file(path) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
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

/// Per-data-folder ownership uses the same lock as the collector. A second native
/// launch requests a window via a file, without starting another telemetry receiver.
pub struct Instance {
    _lock: File,
}
impl Instance {
    pub fn acquire(paths: &AppPaths) -> io::Result<Option<Self>> {
        fs::create_dir_all(&paths.root)?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(paths.root.join("instance"))?;
        match file.try_lock() {
            Ok(()) => {
                let _ = fs::remove_file(paths.root.join("native-open"));
                Ok(Some(Self { _lock: file }))
            }
            Err(std::fs::TryLockError::WouldBlock) => {
                fs::write(paths.root.join("native-open"), b"open")?;
                Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "pitwall is already running; requested its dashboard (it may still be starting)",
                ))
            }
            Err(std::fs::TryLockError::Error(e)) => Err(e),
        }
    }
}
