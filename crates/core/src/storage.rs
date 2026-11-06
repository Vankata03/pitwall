//! Shared writes and preservation for the native data files.
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

/// Replace a file only after its complete new contents have been written.
pub fn atomic_write(path: &Path, bytes: impl AsRef<[u8]>) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let temporary = path.with_file_name(format!(
        "{name}.{}.{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let result = (|| {
        file.write_all(bytes.as_ref())?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

/// Copy to the first free `<name>.<suffix>`, `<name>.2.<suffix>`, ... without replacing backups.
pub fn preserve(path: &Path, suffix: &str) -> io::Result<PathBuf> {
    let mut source = fs::File::open(path)?;
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    for n in 1u64.. {
        let target = path.with_file_name(if n == 1 {
            format!("{name}.{suffix}")
        } else {
            format!("{name}.{n}.{suffix}")
        });
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
        {
            Ok(mut file) => {
                io::copy(&mut source, &mut file)?;
                file.sync_all()?;
                return Ok(target);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    unreachable!()
}
