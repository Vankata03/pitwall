//! The livery pack lookup (spec sections 8 and 13).
//!
//! The pack is a folder of images on the host, [`AppPaths::liveries`](crate::paths::AppPaths).
//! Nothing from it is ever bundled; it is read from disk on each call, so copying a file in
//! needs no restart.
//!
//! A pack image is named `my-team.png` or `<teamId>.png`. Only those names are listed or
//! resolved; any other file in the folder is ignored, and no other name can reach a file,
//! which also rules out path traversal.

use std::path::{Path, PathBuf};

/// Whether `name` is a pack image name: `my-team.png` or `<teamId>.png`, where the team
/// ID is plain decimal digits.
pub fn is_pack_name(name: &str) -> bool {
    match name.strip_suffix(".png") {
        Some("my-team") => true,
        Some(id) => !id.is_empty() && id.len() <= 5 && id.bytes().all(|byte| byte.is_ascii_digit()),
        None => false,
    }
}

/// The pack image names in `folder`, sorted. A missing or unreadable folder is an empty
/// pack.
pub fn pack_files(folder: &Path) -> Vec<String> {
    let mut files: Vec<String> = std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.path().is_file())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| is_pack_name(name))
        .collect();
    files.sort();
    files
}

/// The path of pack image `file` in `folder`, when `file` is a pack name and exists as a
/// file.
pub fn pack_image(folder: &Path, file: &str) -> Option<PathBuf> {
    if !is_pack_name(file) {
        return None;
    }
    let path = folder.join(file);
    path.is_file().then_some(path)
}
