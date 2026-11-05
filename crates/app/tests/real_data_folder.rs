//! The native host opens a copy of a real user data folder without loss (issue #77
//! done-condition, ADR 0003). The source is only read; everything runs on a temp copy and
//! Start with Windows points into the copy, never the user's Startup folder.
//!
//! ```text
//! cargo test --release -p pitwall-native --test real_data_folder -- --ignored --nocapture
//! ```
//!
//! The source is `PITWALL_REAL_DATA` or `%APPDATA%\pitwall`. `webview/` is not copied.
use pitwall_core::{data::DataFolder, ingest::config::Cli, paths::AppPaths};
use pitwall_native::{
    desktop::Startup,
    host::{Command, Desktop, Host},
};
use std::{
    collections::BTreeMap,
    fs,
    hash::{DefaultHasher, Hasher},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const SKIPPED: &[&str] = &["webview"];

fn snapshot(root: &Path) -> BTreeMap<PathBuf, (u64, u64)> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, (u64, u64)>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let relative = path.strip_prefix(root).unwrap().to_path_buf();
            if SKIPPED.iter().any(|s| relative == Path::new(s)) {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                let bytes = fs::read(&path).unwrap();
                let mut hasher = DefaultHasher::new();
                hasher.write(&bytes);
                out.insert(relative, (bytes.len() as u64, hasher.finish()));
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

fn copy_tree(from: &Path, to: &Path, root: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let path = entry.unwrap().path();
        let relative = path.strip_prefix(root).unwrap();
        if SKIPPED.iter().any(|s| relative == Path::new(s)) {
            continue;
        }
        let target = to.join(path.file_name().unwrap());
        if path.is_dir() {
            copy_tree(&path, &target, root);
        } else {
            fs::copy(&path, &target).unwrap();
        }
    }
}

#[test]
#[ignore = "needs a real data folder; run explicitly with --ignored"]
fn the_native_host_opens_a_copy_of_the_real_data_folder_without_loss() {
    let source = std::env::var_os("PITWALL_REAL_DATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| AppPaths::resolve().unwrap().root);
    assert!(source.is_dir(), "no data folder at {}", source.display());
    let temp = std::env::temp_dir().join(format!("pitwall-native-real-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp);
    let paths = AppPaths::from_data_dir(&temp);
    copy_tree(&source, &paths.root, &source);
    let before = snapshot(&paths.root);
    println!("copied {} files from {}", before.len(), source.display());

    let cli = Cli {
        data_dir: Some(temp.clone()),
        ..Cli::default()
    };
    let folder = DataFolder::open(paths.clone(), &cli).unwrap();
    assert!(folder.issues.is_empty(), "load issues: {:?}", folder.issues);
    let mut host = Host::open(
        folder,
        cli,
        Desktop {
            startup: Startup::at(temp.join("Startup/pitwall.vbs")),
            notifications: false,
        },
    )
    .unwrap();
    let status = host.status();
    for line in &status.notices {
        println!("notice: {line}");
    }
    assert!(status.bad_files.is_empty(), "{:?}", status.bad_files);
    let pitrecs = before
        .keys()
        .filter(|p| p.starts_with("recordings") && p.extension().is_some_and(|e| e == "pitrec"))
        .count();
    assert_eq!(
        status.recordings.len(),
        pitrecs,
        "every recording is listed"
    );
    println!("config: {:?}", status.config);

    // Replay the newest recording through the host, as the status window does.
    if let Some(newest) = status.recordings.last() {
        host.command(Command::Start(newest.file_name.clone()))
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(120);
        while host.status().loading.is_some() {
            host.tick(Duration::ZERO).unwrap();
            assert!(Instant::now() < deadline, "replay never loaded");
            std::thread::sleep(Duration::from_millis(5));
        }
        host.command(Command::Seek(5.0)).unwrap();
        host.tick(Duration::ZERO).unwrap();
        let replay = host.status().replay.unwrap();
        println!("replayed {} ({:.0} s)", newest.file_name, replay.duration);
        host.command(Command::Stop).unwrap();
    }
    drop(host);

    // Nothing but the log may differ, except that recorder maintenance may finish
    // pending recompression in place (same name, new bytes).
    let after = snapshot(&paths.root);
    let mut changed = Vec::new();
    for (path, value) in &before {
        if path.starts_with("logs") {
            continue;
        }
        match after.get(path) {
            None => panic!("lost {}", path.display()),
            Some(v) if v != value => changed.push(path.clone()),
            _ => {}
        }
    }
    let added: Vec<_> = after
        .keys()
        .filter(|p| !before.contains_key(*p) && !p.starts_with("logs"))
        .collect();
    println!("changed: {changed:?}; added: {added:?}");
    assert!(
        changed
            .iter()
            .all(|p| p.starts_with("recordings") || p.starts_with("tracks")),
        "only maintenance may rewrite files: {changed:?}"
    );
    assert!(added.is_empty(), "unexpected new files: {added:?}");
    let _ = fs::remove_dir_all(&temp);
}
