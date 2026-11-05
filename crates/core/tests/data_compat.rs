//! Data compatibility (ADR 0003): the native app opens files written by the web build.
//! Fixtures in `fixtures/data` are copies of real files from a user's data folder (small
//! ones verbatim; the recording is a clip of real datagrams with participant names removed).
use pitwall_core::{
    data::{DataFile, DataFolder},
    ingest::config::{Cli, Config},
    overlay::OverlaySettings,
    paths::AppPaths,
    pitrec::{Header, Reader},
    recorder::list,
    replay::Replay,
};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU32, Ordering},
};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/data")
        .join(name)
}

/// A fresh, empty data folder in the system temp directory.
fn folder() -> AppPaths {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "pitwall-data-compat-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    let paths = AppPaths::from_data_dir(&dir);
    fs::create_dir_all(&paths.root).unwrap();
    paths
}

fn place(fixture_name: &str, target: &Path) {
    fs::copy(fixture(fixture_name), target).unwrap();
}

fn open(paths: &AppPaths) -> DataFolder {
    DataFolder::open(paths.clone(), &Cli::default()).unwrap()
}

#[test]
fn real_config_with_obsolete_http_keys_loads_and_names_them() {
    let paths = folder();
    place("config-with-http.toml", &paths.config);

    let data = open(&paths);

    // The real file holds the defaults.
    assert_eq!(data.config, Config::default());
    assert_eq!(data.obsolete, ["[http].bind", "[http].port"]);
    assert!(data.issues.is_empty());
    let log = fs::read_to_string(paths.logs.join("collector.log")).unwrap();
    assert!(log.contains("[http].port"), "{log}");
}

#[test]
fn saving_settings_drops_the_obsolete_http_table_and_keeps_the_rest() {
    let paths = folder();
    place("config-with-http.toml", &paths.config);
    let data = open(&paths);

    let mut changed = data.config.clone();
    changed.recording.size_cap_gb = 30;
    data.save_config(&changed).unwrap();

    let text = fs::read_to_string(&paths.config).unwrap();
    assert!(!text.contains("[http]"), "{text}");
    let reopened = open(&paths);
    assert!(reopened.obsolete.is_empty());
    assert_eq!(reopened.config, changed);
}

#[test]
fn config_without_http_loads_every_setting() {
    let paths = folder();
    place("config-without-http.toml", &paths.config);

    let data = open(&paths);

    assert!(data.obsolete.is_empty());
    assert!(data.issues.is_empty());
    assert_eq!(data.config.forward.targets, ["192.168.1.40:20777"]);
    assert!(data.config.forward.enabled);
    assert!(!data.config.recording.enabled);
    assert_eq!(data.config.recording.size_cap_gb, 5);
    assert!(data.config.app.start_with_windows);
}

#[test]
fn http_values_the_web_build_would_reject_do_not_stop_loading() {
    let paths = folder();
    place("config-odd-http.toml", &paths.config);

    let data = open(&paths);

    assert_eq!(data.config.udp.port, 20778);
    assert_eq!(data.obsolete, ["[http].bind", "[http].port"]);
    assert!(data.issues.is_empty());
}

#[test]
fn obsolete_http_flags_are_accepted_and_named() {
    let paths = folder();
    let cli = Cli::parse([
        "--http-port",
        "9000",
        "--bind",
        "127.0.0.1",
        "--udp-port",
        "20800",
    ])
    .unwrap();

    let data = DataFolder::open(paths, &cli).unwrap();

    assert_eq!(data.config.udp.port, 20800);
    assert_eq!(data.obsolete, ["--http-port", "--bind"]);
}

#[test]
fn unparseable_config_is_kept_as_bad_and_defaults_load() {
    let paths = folder();
    place("config-malformed.toml", &paths.config);
    let original = fs::read(fixture("config-malformed.toml")).unwrap();

    let data = open(&paths);

    assert_eq!(data.config, Config::default());
    let bad = paths.root.join("config.toml.bad");
    assert_eq!(fs::read(&bad).unwrap(), original);
    assert!(!paths.config.exists(), "nothing is written in its place");
    assert_eq!(data.issues.len(), 1);
    let issue = &data.issues[0];
    assert_eq!(issue.file, DataFile::Config);
    assert_eq!(issue.path, paths.config);
    assert_eq!(issue.kept_as, bad);
    let log = fs::read_to_string(paths.logs.join("collector.log")).unwrap();
    assert!(log.contains("config.toml.bad"), "{log}");
}

#[test]
fn a_second_unparseable_file_does_not_replace_the_first_bad_copy() {
    let paths = folder();
    fs::write(paths.root.join("config.toml.bad"), "older").unwrap();
    place("config-malformed.toml", &paths.config);

    let data = open(&paths);

    assert_eq!(
        fs::read_to_string(paths.root.join("config.toml.bad")).unwrap(),
        "older"
    );
    assert_eq!(data.issues[0].kept_as, paths.root.join("config.toml.2.bad"));
    assert!(data.issues[0].kept_as.exists());
}

#[test]
fn real_empty_layout_library_loads_with_the_presets() {
    let paths = folder();
    place("layouts-v1-empty.json", &paths.layouts);

    let data = open(&paths);

    let names: Vec<String> = data.layouts.list().into_iter().map(|l| l.name).collect();
    assert_eq!(names, ["Pitwall Classic", "Driver", "Race"]);
    assert!(data.issues.is_empty());
}

#[test]
fn version_1_layouts_load_with_every_arrangement_and_unknown_data() {
    let paths = folder();
    place("layouts-v1.json", &paths.layouts);

    let data = open(&paths);

    let race = data.layouts.get("Race engineer").unwrap();
    assert_eq!(race.arrangements.wide.len(), 3);
    assert_eq!(race.arrangements.tablet.as_ref().unwrap().len(), 1);
    assert_eq!(race.arrangements.phone, None, "phone stays generated");
    assert_eq!(race.extra["note"], "kept by builds that do not know it");
    assert_eq!(race.arrangements.wide[2].kind, "tile-from-a-newer-build");
    assert_eq!(race.arrangements.wide[2].extra["locked"], true);
    assert_eq!(
        race.arrangements.wide[0].settings["car"]["pin"]["driverId"],
        7
    );
    let quali = data.layouts.get("Quali").unwrap();
    assert_eq!(quali.arrangements.tablet, Some(vec![]));
    assert_eq!(quali.arrangements.phone.as_ref().unwrap().len(), 1);
}

#[test]
fn saving_version_1_layouts_keeps_unknown_data_and_writes_no_backup() {
    let paths = folder();
    place("layouts-v1.json", &paths.layouts);
    let data = open(&paths);

    data.layouts.rename("Quali", "Qualifying").unwrap();

    let saved: serde_json::Value =
        serde_json::from_slice(&fs::read(&paths.layouts).unwrap()).unwrap();
    assert_eq!(saved["schemaVersion"], 1);
    assert_eq!(saved["lastSelected"], "Race engineer");
    assert_eq!(
        saved["layouts"][0]["note"],
        "kept by builds that do not know it"
    );
    assert_eq!(
        saved["layouts"][0]["arrangements"]["wide"][2]["locked"],
        true
    );
    assert!(saved["layouts"][0]["arrangements"].get("phone").is_none());
    assert!(!paths.root.join("layouts.json.bak").exists());
}

#[test]
fn first_save_of_an_older_layout_file_backs_it_up_and_bumps_the_version() {
    let paths = folder();
    place("layouts-v0.json", &paths.layouts);
    let original = fs::read(&paths.layouts).unwrap();
    let data = open(&paths);
    assert_eq!(
        data.layouts
            .get("Unversioned")
            .unwrap()
            .arrangements
            .wide
            .len(),
        1
    );
    assert!(
        !paths.root.join("layouts.json.bak").exists(),
        "loading writes nothing"
    );

    data.layouts.rename("Unversioned", "Versioned").unwrap();
    data.layouts.rename("Versioned", "Versioned again").unwrap();

    let backup = paths.root.join("layouts.json.bak");
    assert_eq!(
        fs::read(&backup).unwrap(),
        original,
        "the backup is the old file"
    );
    let saved: serde_json::Value =
        serde_json::from_slice(&fs::read(&paths.layouts).unwrap()).unwrap();
    assert_eq!(saved["schemaVersion"], 1);
    assert_eq!(saved["layouts"][0]["name"], "Versioned again");
}

#[test]
fn a_newer_layout_file_is_backed_up_before_this_build_rewrites_it() {
    let paths = folder();
    let newer = fs::read_to_string(fixture("layouts-v1.json"))
        .unwrap()
        .replacen("\"schemaVersion\": 1", "\"schemaVersion\": 2", 1);
    fs::write(&paths.layouts, &newer).unwrap();
    let data = open(&paths);

    data.layouts.delete("Quali").unwrap();

    assert_eq!(
        fs::read_to_string(paths.root.join("layouts.json.bak")).unwrap(),
        newer
    );
}

#[test]
fn unparseable_layouts_are_kept_as_bad_and_the_library_starts_empty() {
    let paths = folder();
    place("layouts-malformed.json", &paths.layouts);
    let original = fs::read(&paths.layouts).unwrap();

    let data = open(&paths);

    assert_eq!(data.layouts.list().len(), 3, "presets only");
    let bad = paths.root.join("layouts.json.bad");
    assert_eq!(fs::read(&bad).unwrap(), original);
    assert_eq!(data.issues.len(), 1);
    assert_eq!(data.issues[0].file, DataFile::Layouts);

    data.layouts
        .duplicate("Driver", Some("Mine"))
        .expect("the library saves again");
    assert_eq!(
        fs::read(&bad).unwrap(),
        original,
        "the bad copy is untouched"
    );
}

#[test]
fn real_overlay_settings_load() {
    let paths = folder();
    place("overlay.json", &paths.overlay);

    let data = open(&paths);

    assert_eq!(
        data.overlay,
        OverlaySettings {
            visible: true,
            position: Some((17, 1200)),
            size: Some((501, 183)),
        }
    );
}

#[test]
fn overlay_settings_from_before_resizing_load_without_a_size() {
    let paths = folder();
    place("overlay-without-size.json", &paths.overlay);

    let data = open(&paths);

    assert_eq!(
        data.overlay,
        OverlaySettings {
            visible: false,
            position: Some((640, 1180)),
            size: None,
        }
    );
}

#[test]
fn unparseable_overlay_settings_are_kept_as_bad_and_start_hidden() {
    let paths = folder();
    place("overlay-malformed.json", &paths.overlay);
    let original = fs::read(&paths.overlay).unwrap();

    let data = open(&paths);

    assert_eq!(data.overlay, OverlaySettings::default());
    assert_eq!(
        fs::read(paths.root.join("overlay.json.bad")).unwrap(),
        original
    );
    assert_eq!(data.issues.len(), 1);
    assert_eq!(data.issues[0].file, DataFile::Overlay);
    assert!(
        data.issues[0].to_string().contains("overlay.json.bad"),
        "{}",
        data.issues[0]
    );
}

#[test]
fn an_empty_data_folder_loads_defaults_and_writes_nothing() {
    let paths = folder();

    let data = open(&paths);

    assert_eq!(data.config, Config::default());
    assert_eq!(data.overlay, OverlaySettings::default());
    assert!(data.issues.is_empty() && data.obsolete.is_empty());
    assert_eq!(fs::read_dir(&paths.root).unwrap().count(), 0);
}

const RACE: &str = "2026-11-01_1338_Melbourne_Race";
const UNKNOWN: &str = "2026-11-02_2128_Unknown_Session";

/// The recordings folder with the clip under both sidecars' names.
fn recordings(paths: &AppPaths) {
    fs::create_dir_all(&paths.recordings).unwrap();
    for name in [RACE, UNKNOWN] {
        place(
            &format!("recordings/{RACE}.pitrec"),
            &paths.recordings.join(format!("{name}.pitrec")),
        );
        place(
            &format!("recordings/{name}.json"),
            &paths.recordings.join(format!("{name}.json")),
        );
    }
}

#[test]
fn version_1_recording_reads_every_record_in_order() {
    let file = fs::File::open(fixture(&format!("recordings/{RACE}.pitrec"))).unwrap();
    let mut reader = Reader::new(std::io::BufReader::new(file)).unwrap();

    assert_eq!(
        reader.header(),
        &Header {
            packet_format: 2026,
            game_version: "25.1.26".into(),
            pitwall_version: "0.1.0".into(),
            started_utc: "2026-11-01T13:38:09Z".into(),
        }
    );
    let records: Vec<_> = reader.by_ref().collect::<Result<_, _>>().unwrap();
    assert!(!reader.has_incomplete_tail());
    assert_eq!(records.len(), 153);
    assert!(records.iter().enumerate().all(|(n, r)| r.seq == n as u32));
    assert_eq!(records[0].arrival_us, 0);
    assert!(
        records
            .windows(2)
            .all(|p| p[0].arrival_us <= p[1].arrival_us)
    );
    assert!(
        records
            .iter()
            .all(|r| r.datagram[..2] == 2026u16.to_le_bytes())
    );
}

#[test]
fn version_1_recording_is_laid_out_as_documented() {
    let bytes = fs::read(fixture(&format!("recordings/{RACE}.pitrec"))).unwrap();

    assert_eq!(&bytes[..8], b"PITREC\x01\x00");
    let header_len = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let header: serde_json::Value = serde_json::from_slice(&bytes[12..12 + header_len]).unwrap();
    assert_eq!(header["packet_format"], 2026);
    let mut at = 12 + header_len;
    let mut seqs = Vec::new();
    while at < bytes.len() {
        let len = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
        let block = zstd::decode_all(&bytes[at + 4..at + 4 + len]).unwrap();
        let mut r = 0;
        while r < block.len() {
            seqs.push(u32::from_le_bytes(block[r..r + 4].try_into().unwrap()));
            let datagram_len = u16::from_le_bytes(block[r + 12..r + 14].try_into().unwrap());
            r += 14 + datagram_len as usize;
        }
        at += 4 + len;
    }
    seqs.sort_unstable();
    assert_eq!(seqs, (0..153).collect::<Vec<u32>>());
}

#[test]
fn recording_cut_mid_block_reads_up_to_the_cut() {
    let mut bytes = fs::read(fixture(&format!("recordings/{RACE}.pitrec"))).unwrap();
    bytes.truncate(bytes.len() - 10);

    let mut reader = Reader::new(std::io::Cursor::new(bytes)).unwrap();
    let records: Vec<_> = reader.by_ref().collect::<Result<_, _>>().unwrap();

    assert!(reader.has_incomplete_tail());
    assert!(!records.is_empty() && records.len() < 153);
}

#[test]
fn real_sidecars_list_including_unknown_track_and_session() {
    let paths = folder();
    recordings(&paths);

    let listed = list(&paths.recordings).unwrap();

    let names: Vec<&str> = listed.iter().map(|l| l.file_name.as_str()).collect();
    assert_eq!(
        names,
        [format!("{RACE}.pitrec"), format!("{UNKNOWN}.pitrec")]
    );
    let race = &listed[0].sidecar;
    assert_eq!(race.session_uid, "12931914828735337615");
    assert_eq!(race.track.as_deref(), Some("Melbourne"));
    assert_eq!(race.session_type.as_deref(), Some("Race"));
    assert_eq!(
        (race.laps, race.pinned, race.recompressed),
        (29, false, false)
    );
    let unknown = &listed[1].sidecar;
    assert_eq!(
        (unknown.track.as_ref(), unknown.session_type.as_ref()),
        (None, None)
    );
    assert!(unknown.recompressed);
}

#[test]
fn version_1_recording_loads_into_replay() {
    let file = fs::File::open(fixture(&format!("recordings/{RACE}.pitrec"))).unwrap();

    let replay = Replay::load(file, |_| {}).unwrap();

    assert!(replay.position().is_zero());
}

#[test]
fn an_explicit_config_file_that_is_missing_is_an_error() {
    let paths = folder();
    let cli = Cli::parse(["--config", "does-not-exist.toml"]).unwrap();

    assert!(DataFolder::open(paths, &cli).is_err());
}
