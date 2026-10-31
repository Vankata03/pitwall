//! Track store tests. Every scenario is synthetic and `assumed`: laps are generated
//! circles and lines, not captures. Shipped-map coordinates are checked in #52.
use pitwall_collector::tracks::{Sample, Source, TrackKey, TrackStore};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

fn temp_dir() -> PathBuf {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "pitwall-tracks-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The 22 spec layouts as (track ID, length), from the source project's file names.
const SHIPPED: [(i8, u16); 22] = [
    (14, 5280),
    (17, 4323),
    (20, 5994),
    (16, 4294),
    (4, 4664),
    (9, 4378),
    (27, 4910),
    (29, 6175),
    (32, 5417),
    (0, 5276),
    (19, 4525),
    (5, 3323),
    (6, 4371),
    (11, 5798),
    (3, 5408),
    (2, 5441),
    (7, 5890),
    (12, 4928),
    (10, 7007),
    (13, 5809),
    (15, 5514),
    (26, 4258),
];

#[test]
fn all_22_shipped_layouts_are_complete_without_any_cached_file() {
    let store = TrackStore::open(temp_dir());
    for (track_id, track_length) in SHIPPED {
        let map = store
            .get(TrackKey::new(track_id, track_length))
            .unwrap_or_else(|| panic!("shipped map {track_id}-{track_length} missing"));
        assert_eq!(map.source, Source::Shipped);
        assert!(map.complete);
        assert!(map.pit.is_none(), "no pit lanes ship");
        assert!(map.outline.len() > 400);
    }
}

#[test]
fn unknown_layout_has_no_map() {
    let store = TrackStore::open(temp_dir());
    assert!(store.get(TrackKey::new(19, 4526)).is_none());
    assert!(store.get(TrackKey::new(10, 7000)).is_none());
}

#[test]
fn shipped_file_imports_only_id_length_and_points() {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/tracks.json"))
        .unwrap();
    let maps: Vec<serde_json::Value> = serde_json::from_str(&text).unwrap();
    assert_eq!(maps.len(), 22);
    for map in maps {
        let mut keys: Vec<&String> = map.as_object().unwrap().keys().collect();
        keys.sort();
        assert_eq!(keys, ["points", "trackId", "trackLength"]);
    }
}

/// A layout pitwall does not ship, so it must be learned.
const UNSHIPPED: (i8, u16) = (31, 6200);

fn unshipped() -> TrackKey {
    TrackKey::new(UNSHIPPED.0, UNSHIPPED.1)
}

/// A car on a circle of the layout's length at lap distance `d`, centred on (`cx`, 0).
fn at(key: TrackKey, car: usize, d: f32, cx: f32) -> Sample {
    let length = f32::from(key.track_length);
    let radius = length / std::f32::consts::TAU;
    let angle = d / length * std::f32::consts::TAU;
    Sample {
        car,
        x: cx + radius * angle.cos(),
        z: radius * angle.sin(),
        lap_distance: d,
        in_pit: false,
        lap_invalid: false,
    }
}

/// Drive lap distances `from..to` in `step` metre samples.
fn drive(store: &TrackStore, key: TrackKey, car: usize, from: f32, to: f32, step: f32) {
    let mut d = from;
    while d < to {
        store.feed(key, at(key, car, d, 0.0));
        d += step;
    }
}

#[test]
fn learner_grows_the_line_as_samples_arrive_and_is_incomplete_until_covered() {
    let store = TrackStore::open(temp_dir());
    let key = unshipped();
    assert!(store.get(key).is_none());
    let before = store.revision();

    drive(&store, key, 0, 0.0, 3000.0, 8.0);
    let half = store.get(key).unwrap();
    assert_eq!(half.source, Source::Learned);
    assert!(!half.complete);
    assert_eq!(half.outline.len(), 375);
    assert!(store.revision() > before);

    drive(&store, key, 0, 3000.0, 6200.0, 8.0);
    assert!(store.get(key).unwrap().complete);
}

#[test]
fn a_gap_of_40_m_is_complete_and_a_gap_over_40_m_is_not() {
    let key = unshipped();
    // Buckets are 8 m wide: filled buckets 99 and 104 are 40 m apart.
    let mut ok = TrackStore::open(temp_dir());
    drive(&ok, key, 0, 0.0, 800.0, 8.0);
    drive(&ok, key, 0, 832.0, 6200.0, 8.0);
    assert!(ok.get(key).unwrap().complete);

    // Buckets 99 and 105: 48 m apart.
    ok = TrackStore::open(temp_dir());
    drive(&ok, key, 0, 0.0, 800.0, 8.0);
    drive(&ok, key, 0, 840.0, 6200.0, 8.0);
    assert!(!ok.get(key).unwrap().complete);
}

#[test]
fn a_gap_across_the_start_line_counts_too() {
    let key = unshipped();
    let store = TrackStore::open(temp_dir());
    // Nothing between 6160 m and 40 m is 80 m of empty line across the finish.
    drive(&store, key, 0, 40.0, 6160.0, 8.0);
    assert!(!store.get(key).unwrap().complete);
}

#[test]
fn pit_lane_invalid_lap_and_negative_distance_samples_are_rejected() {
    let key = unshipped();
    let store = TrackStore::open(temp_dir());
    for d in [0.0, 8.0, 16.0] {
        store.feed(key, Sample { in_pit: true, ..at(key, 0, d, 0.0) });
        store.feed(key, Sample { lap_invalid: true, ..at(key, 0, d, 0.0) });
    }
    store.feed(key, at(key, 0, -8.0, 0.0));
    assert!(store.get(key).is_none(), "nothing accepted, so no map yet");

    // One clean lap except a rejected stretch: incomplete until a second pass covers it.
    drive(&store, key, 0, 0.0, 2000.0, 8.0);
    for d in (2000..2200).step_by(8) {
        store.feed(key, Sample { lap_invalid: true, ..at(key, 0, d as f32, 0.0) });
    }
    drive(&store, key, 0, 2200.0, 6200.0, 8.0);
    assert!(!store.get(key).unwrap().complete);
    drive(&store, key, 1, 2000.0, 2200.0, 8.0);
    assert!(store.get(key).unwrap().complete);
}

#[test]
fn the_first_complete_map_wins_and_freezes() {
    let key = unshipped();
    let store = TrackStore::open(temp_dir());
    drive(&store, key, 0, 0.0, 6200.0, 8.0);
    let done = store.get(key).unwrap();
    assert!(done.complete);
    let revision = store.revision();

    // A second car on a slightly different line, within tolerance, changes nothing.
    for d in (0..6200).step_by(8) {
        store.feed(key, at(key, 1, d as f32, 3.0));
    }
    assert_eq!(store.get(key).unwrap(), done);
    assert_eq!(store.revision(), revision);
}

fn cached_path(dir: &std::path::Path, key: TrackKey) -> PathBuf {
    dir.join(format!("{}-{}.json", key.track_id, key.track_length))
}

/// A hand-written cached file whose outline covers every 8 m bucket on a line.
fn cached_file(key: TrackKey, schema: u64) -> serde_json::Value {
    let buckets = (f32::from(key.track_length) / 8.0).ceil() as u32;
    let outline: Vec<_> = (0..buckets).map(|b| serde_json::json!([b, b as f32, 7.5])).collect();
    serde_json::json!({
        "schemaVersion": schema,
        "trackId": key.track_id,
        "trackLength": key.track_length,
        "outline": outline,
        "pit": null,
    })
}

#[test]
fn a_map_is_written_to_its_cached_file_the_moment_it_completes_and_reloads() {
    let dir = temp_dir().join("tracks");
    let key = unshipped();
    let store = TrackStore::open(dir.clone());
    drive(&store, key, 0, 0.0, 6000.0, 8.0);
    assert!(!cached_path(&dir, key).exists(), "nothing is written before completion");
    drive(&store, key, 0, 6000.0, 6200.0, 8.0);
    let learned = store.get(key).unwrap();
    assert!(cached_path(&dir, key).is_file());
    assert_eq!(dir.read_dir().unwrap().count(), 1, "no manifest, no temp files");

    let reopened = TrackStore::open(dir).get(key).unwrap();
    assert_eq!(reopened.source, Source::Cached);
    assert!(reopened.complete);
    assert_eq!(reopened.outline, learned.outline);
}

#[test]
fn a_cached_file_wins_over_the_shipped_map() {
    let dir = temp_dir();
    let key = TrackKey::new(10, 7007);
    std::fs::write(cached_path(&dir, key), cached_file(key, 1).to_string()).unwrap();
    let map = TrackStore::open(dir).get(key).unwrap();
    assert_eq!(map.source, Source::Cached);
    assert_eq!(map.outline[3].z, 7.5);
}

#[test]
fn unreadable_old_schema_or_incomplete_cached_files_are_ignored_and_relearned() {
    let key = unshipped();
    let mut incomplete = cached_file(key, 1);
    incomplete["outline"].as_array_mut().unwrap().truncate(100);
    let mut other_layout = cached_file(key, 1);
    other_layout["trackLength"] = 6201.into();
    let bad = [
        "{ not json".to_string(),
        cached_file(key, 0).to_string(),
        cached_file(key, 2).to_string(),
        incomplete.to_string(),
        other_layout.to_string(),
    ];
    for contents in bad {
        let dir = temp_dir();
        std::fs::write(cached_path(&dir, key), contents).unwrap();
        let store = TrackStore::open(dir.clone());
        assert!(store.get(key).is_none(), "ignored, so unknown until learned");
        drive(&store, key, 0, 0.0, 6200.0, 8.0);
        assert_eq!(store.get(key).unwrap().source, Source::Learned);
        let rewritten: serde_json::Value =
            serde_json::from_slice(&std::fs::read(cached_path(&dir, key)).unwrap()).unwrap();
        assert_eq!(rewritten["schemaVersion"], 1);
    }
}

#[test]
fn an_unreadable_cached_file_for_a_shipped_layout_falls_back_to_the_shipped_map() {
    let dir = temp_dir();
    let key = TrackKey::new(5, 3323);
    std::fs::write(cached_path(&dir, key), "garbage").unwrap();
    assert_eq!(TrackStore::open(dir).get(key).unwrap().source, Source::Shipped);
}
