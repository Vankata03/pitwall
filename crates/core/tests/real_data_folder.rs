//! Opens a copy of a real user data folder and checks that nothing is lost (issue #77,
//! ADR 0003). The source is only read: everything happens on a copy in the temp dir.
//!
//! Ignored by default because it needs a real folder and reads every recording in full:
//!
//! ```text
//! cargo test --release -p pitwall-core --test real_data_folder -- --ignored --nocapture
//! ```
//!
//! The source is `PITWALL_REAL_DATA` (a folder that contains `pitwall`'s files) or
//! `%APPDATA%\pitwall`. `webview/` (WebView2 data, obsolete natively) is not copied.
use pitwall_core::{
    data::DataFolder,
    ingest::config::{Cli, Config},
    liveries,
    paths::AppPaths,
    pitrec::Reader,
    recorder::list,
    replay::Replay,
    tracks::{Source, TrackKey, TrackStore},
};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    hash::{DefaultHasher, Hasher},
    io::{BufReader, Read},
    path::{Path, PathBuf},
};

fn source() -> PathBuf {
    std::env::var_os("PITWALL_REAL_DATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| AppPaths::resolve().unwrap().root)
}

const SKIPPED: &[&str] = &["webview"];

/// Every file under `root` (relative path → size and content hash), minus `SKIPPED`.
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
                let mut hasher = DefaultHasher::new();
                let mut file = BufReader::new(fs::File::open(&path).unwrap());
                let mut buffer = vec![0; 1 << 20];
                let mut size = 0;
                loop {
                    let n = file.read(&mut buffer).unwrap();
                    if n == 0 {
                        break;
                    }
                    hasher.write(&buffer[..n]);
                    size += n as u64;
                }
                out.insert(relative, (size, hasher.finish()));
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

fn json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

#[test]
#[ignore = "needs a real data folder; run explicitly with --ignored"]
fn a_copy_of_the_real_data_folder_opens_without_loss() {
    let source = source();
    assert!(source.is_dir(), "no data folder at {}", source.display());
    let temp = std::env::temp_dir().join(format!("pitwall-real-data-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp);
    let paths = AppPaths::from_data_dir(&temp);
    copy_tree(&source, &paths.root, &source);
    let before = snapshot(&paths.root);
    assert_eq!(before, snapshot(&source), "the copy matches the source");
    println!("copied {} files from {}", before.len(), source.display());

    let data = DataFolder::open(paths.clone(), &Cli::default()).unwrap();

    assert!(data.issues.is_empty(), "load issues: {:?}", data.issues);
    for line in data.notices() {
        println!("notice: {line}");
    }

    // Settings: the native load equals the web build's reading of the same file, minus
    // the HTTP part the native app ignores.
    if let Ok(text) = fs::read_to_string(&paths.config) {
        let web = Config::from_toml(&text).unwrap();
        assert_eq!(
            data.config,
            Config {
                http: Default::default(),
                ..web
            }
        );
        println!("config: {:?}; obsolete: {:?}", data.config, data.obsolete);
    }

    // Layouts: every stored user layout, field for field.
    if paths.layouts.exists() {
        let stored = json(&paths.layouts);
        let presets = 3;
        let user: Vec<Value> = data.layouts.list()[presets..]
            .iter()
            .map(|layout| serde_json::to_value(layout).unwrap())
            .collect();
        assert_eq!(Value::Array(user), stored["layouts"]);
        println!(
            "layouts: {} user layouts",
            stored["layouts"].as_array().unwrap().len()
        );
    }

    // Overlay position.
    if paths.overlay.exists() {
        assert_eq!(
            serde_json::to_value(data.overlay).unwrap(),
            json(&paths.overlay)
        );
        println!("overlay: {:?}", data.overlay);
    }

    // Recordings: every one listed with its sidecar unchanged, read in full, and replayable.
    let pitrecs: Vec<PathBuf> = fs::read_dir(&paths.recordings)
        .map(|dir| {
            dir.map(|e| e.unwrap().path())
                .filter(|p| p.extension().is_some_and(|e| e == "pitrec"))
                .collect()
        })
        .unwrap_or_default();
    let listed = if pitrecs.is_empty() {
        Vec::new()
    } else {
        list(&paths.recordings).unwrap()
    };
    assert_eq!(listed.len(), pitrecs.len(), "every recording is listed");
    let mut total_records = 0u64;
    for listing in &listed {
        let path = paths.recordings.join(&listing.file_name);
        assert_eq!(
            serde_json::to_value(&listing.sidecar).unwrap(),
            json(&path.with_extension("json")),
            "{} sidecar",
            listing.file_name
        );
        let mut reader = Reader::new(BufReader::new(fs::File::open(&path).unwrap())).unwrap();
        assert_eq!(reader.header().packet_format, listing.sidecar.packet_format);
        let mut count = 0u64;
        let (mut first, mut last) = (None, 0);
        for (expected_seq, record) in (0u32..).zip(reader.by_ref()) {
            let record = record.unwrap_or_else(|e| panic!("{}: {e}", listing.file_name));
            assert_eq!(record.seq, expected_seq, "{}", listing.file_name);
            first.get_or_insert(record.arrival_us);
            last = record.arrival_us;
            count += 1;
        }
        assert!(!reader.has_incomplete_tail(), "{}", listing.file_name);
        assert!(count > 0, "{}", listing.file_name);
        assert_eq!(
            last - first.unwrap(),
            listing.sidecar.duration_us,
            "{} span matches the sidecar",
            listing.file_name
        );
        Replay::load(fs::File::open(&path).unwrap(), |_| {})
            .unwrap_or_else(|e| panic!("{} replay: {e}", listing.file_name));
        total_records += count;
        println!(
            "recording {}: {count} records, format {}",
            listing.file_name, listing.sidecar.packet_format
        );
    }
    println!(
        "recordings: {} files, {total_records} records",
        listed.len()
    );

    // Track maps: every cached file is used as it is.
    let store = TrackStore::open(paths.tracks.clone());
    for entry in fs::read_dir(&paths.tracks).into_iter().flatten() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "json") {
            continue;
        }
        let file = json(&path);
        let key = TrackKey::new(
            file["trackId"].as_i64().unwrap() as i8,
            file["trackLength"].as_u64().unwrap() as u16,
        );
        let map = store.get(key).unwrap();
        assert_eq!(map.source, Source::Cached, "{}", path.display());
        assert_eq!(map.outline.len(), file["outline"].as_array().unwrap().len());
        assert_eq!(
            map.pit.as_ref().map(Vec::len),
            file["pit"].as_array().map(Vec::len)
        );
        println!("track {}: {} points", path.display(), map.outline.len());
    }

    // Livery pack: every pack image is found.
    let pack: Vec<String> = fs::read_dir(&paths.liveries)
        .map(|dir| {
            let mut names: Vec<String> = dir
                .filter_map(|e| e.ok()?.file_name().into_string().ok())
                .filter(|name| liveries::is_pack_name(name))
                .collect();
            names.sort();
            names
        })
        .unwrap_or_default();
    let mut found = liveries::pack_files(&paths.liveries);
    found.sort();
    assert_eq!(found, pack);
    println!("liveries: {} pack images", pack.len());

    // Opening wrote nothing but a log line: every file is byte-identical to the source.
    let mut after = snapshot(&paths.root);
    let mut before = before;
    for map in [&mut before, &mut after] {
        map.retain(|path, _| !path.starts_with("logs"));
    }
    assert_eq!(after, before, "opening changed or removed files");

    fs::remove_dir_all(&temp).unwrap();
}
