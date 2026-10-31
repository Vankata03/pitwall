//! Recorder boundary tests. Input is synthetic multi-session traffic built from
//! the packet tables, labelled `assumed`: no capture has confirmed these
//! boundaries. Output is checked through the files and sidecars the recorder
//! leaves in its directory.
use pitwall_collector::{
    ingest::{DatagramTap, Ingest},
    pitrec::{Header, Reader, Record, Writer},
    recorder::{Recorder, RecorderConfig, list, set_pinned},
};
use std::{
    collections::HashMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc, Mutex,
        atomic::{AtomicI64, Ordering},
    },
};

const S: u64 = 1_000_000;
const MONZA: &str = "11";
const RACE: &str = "15";

fn build(format: u16, id: u8, uid: u64, extra: &[(&str, &str)]) -> Vec<u8> {
    static CACHE: Mutex<Option<HashMap<String, Vec<u8>>>> = Mutex::new(None);
    let key = format!("{format}/{id}/{uid}/{extra:?}");
    if let Some(hit) = CACHE.lock().unwrap().get_or_insert_default().get(&key) {
        return hit.clone();
    }
    let mut cmd = Command::new(if cfg!(windows) { "python" } else { "python3" });
    cmd.arg(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tools/telemetry/assumed_datagram.py"
    ))
    .args([format.to_string(), id.to_string()])
    .args(["--set", &format!("header.session_uid={uid}")]);
    for (field, value) in extra {
        cmd.args(["--set", &format!("{field}={value}")]);
    }
    let output = cmd.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    CACHE
        .lock()
        .unwrap()
        .get_or_insert_default()
        .insert(key, output.stdout.clone());
    output.stdout
}

/// Same datagram with a distinct overall frame number, so order is checkable.
fn frame(mut bytes: Vec<u8>, n: u32) -> Vec<u8> {
    bytes[23..27].copy_from_slice(&n.to_le_bytes());
    bytes
}

fn session_packet(format: u16, uid: u64) -> Vec<u8> {
    build(
        format,
        1,
        uid,
        &[("track_id", MONZA), ("session_type", RACE)],
    )
}
fn telemetry(format: u16, uid: u64, n: u32) -> Vec<u8> {
    frame(build(format, 6, uid, &[]), n)
}
fn lap_packet(format: u16, uid: u64, lap: u32) -> Vec<u8> {
    build(
        format,
        2,
        uid,
        &[("lap_data.0.current_lap_num", &lap.to_string())],
    )
}
fn classification(format: u16, uid: u64, laps: u32) -> Vec<u8> {
    build(
        format,
        8,
        uid,
        &[
            ("num_cars", "1"),
            ("classification_data.0.num_laps", &laps.to_string()),
        ],
    )
}
fn session_ended(format: u16, uid: u64) -> Vec<u8> {
    let codes: Vec<(String, String)> = b"SEND"
        .iter()
        .enumerate()
        .map(|(i, c)| (format!("event_string_code.{i}"), c.to_string()))
        .collect();
    let extra: Vec<(&str, &str)> = codes
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    build(format, 3, uid, &extra)
}

fn start_clock() -> i64 {
    chrono::NaiveDate::from_ymd_opt(2026, 10, 31)
        .unwrap()
        .and_hms_opt(14, 30, 0)
        .unwrap()
        .and_utc()
        .timestamp()
}

fn fresh_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("pitwall-recorder-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn start(dir: &Path) -> Recorder {
    start_with(RecorderConfig::new(dir.to_owned(), u64::MAX))
}
fn start_with(config: RecorderConfig) -> Recorder {
    Recorder::start(config, start_clock).unwrap()
}

fn files(dir: &Path, extension: &str) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|n| n.ends_with(extension))
        .collect();
    names.sort();
    names
}
fn recordings(dir: &Path) -> Vec<String> {
    files(dir, ".pitrec")
}

fn read(path: &Path) -> Vec<Record> {
    Reader::new(fs::File::open(path).unwrap())
        .unwrap()
        .map(Result::unwrap)
        .collect()
}
fn sidecar(dir: &Path, recording: &str) -> serde_json::Value {
    let path = dir.join(recording).with_extension("json");
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

/// One session of `seconds` seconds of Car Telemetry from `t0`, plus a Session packet first.
fn feed_session(rec: &mut Recorder, format: u16, uid: u64, t0: u64, seconds: u64) {
    rec.datagram(t0, &session_packet(format, uid));
    for s in 1..=seconds {
        rec.datagram(t0 + s * S, &telemetry(format, uid, s as u32));
    }
}

#[test]
fn assumed_final_classification_closes_and_names_the_recording() {
    let dir = fresh_dir("close");
    let mut rec = start(&dir);
    let sent = [
        (S, session_packet(2025, 10)),
        (30 * S, telemetry(2025, 10, 1)),
        (85 * S, lap_packet(2025, 10, 5)),
        (91 * S, classification(2025, 10, 5)),
    ];
    for (t, bytes) in &sent {
        rec.datagram(*t, bytes);
    }
    rec.sync();

    assert_eq!(recordings(&dir), ["2026-10-31_1430_Monza_Race.pitrec"]);
    assert!(files(&dir, ".part").is_empty());
    let records = read(&dir.join("2026-10-31_1430_Monza_Race.pitrec"));
    let got: Vec<_> = records
        .iter()
        .map(|r| (r.seq, r.arrival_us, r.datagram.clone()))
        .collect();
    let want: Vec<_> = sent
        .iter()
        .enumerate()
        .map(|(i, (t, b))| (i as u32, t - S, b.clone()))
        .collect();
    assert_eq!(got, want);

    let side = sidecar(&dir, "2026-10-31_1430_Monza_Race.pitrec");
    assert_eq!(side["track"], "Monza");
    assert_eq!(side["session_type"], "Race");
    assert_eq!(side["duration_us"], 90 * S);
    assert_eq!(side["laps"], 5);
    assert_eq!(side["pinned"], false);
    assert_eq!(side["recompressed"], true);
}

#[test]
fn assumed_open_recording_is_a_part_file_until_it_closes() {
    let dir = fresh_dir("part");
    let mut rec = start(&dir);
    feed_session(&mut rec, 2025, 10, S, 70);
    rec.sync();
    assert_eq!(files(&dir, ".part"), ["10-2025.pitrec.part"]);
    assert!(recordings(&dir).is_empty());
    rec.shutdown();
    assert!(files(&dir, ".part").is_empty());
    assert_eq!(recordings(&dir).len(), 1);
}

#[test]
fn assumed_session_under_sixty_seconds_is_dropped() {
    let dir = fresh_dir("short");
    let mut rec = start(&dir);
    feed_session(&mut rec, 2025, 10, S, 59);
    rec.datagram(60 * S, &classification(2025, 10, 1));
    rec.sync();
    assert!(recordings(&dir).is_empty());
    assert!(files(&dir, ".json").is_empty());
    assert!(files(&dir, ".part").is_empty());
}

#[test]
fn assumed_session_of_exactly_sixty_seconds_is_kept() {
    let dir = fresh_dir("sixty");
    let mut rec = start(&dir);
    feed_session(&mut rec, 2025, 10, S, 60);
    rec.shutdown();
    assert_eq!(recordings(&dir).len(), 1);
}

#[test]
fn assumed_uid_zero_and_unsupported_format_are_never_recorded() {
    let dir = fresh_dir("rejected");
    let mut rec = start(&dir);
    feed_session(&mut rec, 2025, 0, S, 100);
    // The generator only knows 2025 and 2026, so relabel the format bytes.
    for s in 0..100 {
        let mut bytes = telemetry(2025, 7, s as u32);
        bytes[..2].copy_from_slice(&2024u16.to_le_bytes());
        rec.datagram(S + s * S, &bytes);
    }
    rec.datagram(0, &[1, 2, 3]);
    rec.shutdown();
    assert!(recordings(&dir).is_empty());
    assert!(files(&dir, ".part").is_empty());
}

#[test]
fn assumed_new_uid_closes_previous_and_closed_identity_never_reopens() {
    let dir = fresh_dir("newuid");
    let mut rec = start(&dir);
    feed_session(&mut rec, 2025, 10, S, 70);
    feed_session(&mut rec, 2025, 20, 100 * S, 70);
    // Late packets of the closed session go nowhere.
    rec.datagram(200 * S, &telemetry(2025, 10, 999));
    rec.datagram(201 * S, &classification(2025, 20, 3));
    rec.sync();

    let names = recordings(&dir);
    assert_eq!(names.len(), 2, "{names:?}");
    // Same start minute, so the second needs a UID suffix.
    assert_eq!(
        names,
        [
            "2026-10-31_1430_Monza_Race.pitrec",
            "2026-10-31_1430_Monza_Race_20.pitrec"
        ]
    );
    assert_eq!(read(&dir.join(&names[0])).len(), 71);
    assert_eq!(read(&dir.join(&names[1])).len(), 72);
}

#[test]
fn assumed_format_change_on_one_uid_is_a_new_recording() {
    let dir = fresh_dir("format");
    let mut rec = start(&dir);
    feed_session(&mut rec, 2025, 10, S, 70);
    feed_session(&mut rec, 2026, 10, 100 * S, 70);
    rec.shutdown();
    let names = recordings(&dir);
    assert_eq!(names.len(), 2, "{names:?}");
    let formats: Vec<u64> = names
        .iter()
        .map(|n| sidecar(&dir, n)["packet_format"].as_u64().unwrap())
        .collect();
    assert_eq!(formats, [2025, 2026]);
}

#[test]
fn assumed_silence_and_session_end_do_not_close_the_recording() {
    let dir = fresh_dir("silence");
    let mut rec = start(&dir);
    feed_session(&mut rec, 2025, 10, S, 70);
    rec.datagram(72 * S, &session_ended(2025, 10));
    // Over an hour of silence, then the same UID resumes.
    rec.datagram(5_000 * S, &telemetry(2025, 10, 1));
    rec.sync();
    assert!(recordings(&dir).is_empty(), "still open");
    rec.datagram(5_001 * S, &classification(2025, 10, 3));
    rec.sync();
    let names = recordings(&dir);
    assert_eq!(names.len(), 1);
    assert_eq!(read(&dir.join(&names[0])).len(), 71 + 3);
}

#[test]
fn assumed_without_classification_the_file_stays_open_until_exit() {
    let dir = fresh_dir("exit");
    let mut rec = start(&dir);
    feed_session(&mut rec, 2025, 10, S, 70);
    rec.sync();
    assert!(recordings(&dir).is_empty());
    rec.shutdown();
    assert_eq!(recordings(&dir).len(), 1);
}

#[test]
fn assumed_recorder_works_as_the_ingest_tap_and_sees_filtered_packets() {
    let dir = fresh_dir("ingest");
    let mut rec = start(&dir);
    let mut gate = Ingest::default();
    let dup = telemetry(2025, 10, 5);
    gate.ingest(S, &session_packet(2025, 10), &mut rec).unwrap();
    // Frame duplicates are dropped by the gate yet recorded.
    for s in 1..=70 {
        gate.ingest(S + s * S, &dup, &mut rec).unwrap();
    }
    rec.shutdown();
    let names = recordings(&dir);
    assert_eq!(read(&dir.join(&names[0])).len(), 71);
}

/// A `.part` as a crashed collector leaves it: complete blocks, then a torn one.
fn write_part(dir: &Path, uid: u64, seconds: u64, torn_tail: bool) -> Vec<Vec<u8>> {
    let header = Header {
        packet_format: 2025,
        game_version: "25.0.0".into(),
        pitwall_version: "test".into(),
        started_utc: "2026-10-30T09:05:00Z".into(),
    };
    let path = dir.join(format!("{uid}-2025.pitrec.part"));
    let mut writer = Writer::new(fs::File::create(&path).unwrap(), &header).unwrap();
    let mut sent = vec![session_packet(2025, uid)];
    writer.write(0, &sent[0]).unwrap();
    for s in 1..=seconds {
        let bytes = telemetry(2025, uid, s as u32);
        writer.write(s * S, &bytes).unwrap();
        sent.push(bytes);
    }
    let mut file = writer.finish().unwrap();
    if torn_tail {
        file.write_all(&500u32.to_le_bytes()).unwrap();
        file.write_all(&[0x28, 0xb5, 0x2f]).unwrap();
    }
    sent
}

#[test]
fn assumed_crash_leftovers_are_recovered_like_a_normal_close() {
    let dir = fresh_dir("recover");
    let sent = write_part(&dir, 77, 70, true);
    write_part(&dir, 78, 10, false); // under 60 s: dropped
    fs::write(dir.join("79-2025.pitrec.part"), b"not a pitrec").unwrap();
    fs::write(dir.join("stale.pitrec.tmp"), b"half a swap").unwrap();

    let rec = start(&dir);
    rec.sync();

    assert_eq!(recordings(&dir), ["2026-10-30_0905_Monza_Race.pitrec"]);
    assert!(files(&dir, ".part").is_empty());
    assert!(files(&dir, ".tmp").is_empty());
    assert_eq!(files(&dir, ".json").len(), 1);
    let got: Vec<_> = read(&dir.join("2026-10-30_0905_Monza_Race.pitrec"))
        .into_iter()
        .map(|r| r.datagram)
        .collect();
    assert_eq!(got, sent);
    let side = sidecar(&dir, "2026-10-30_0905_Monza_Race.pitrec");
    assert_eq!(side["session_uid"], "77");
    assert_eq!(side["duration_us"], 70 * S);
    assert_eq!(side["recompressed"], true);
}

#[test]
fn assumed_missing_sidecar_is_rebuilt_at_start_and_on_rescan() {
    let dir = fresh_dir("rebuild");
    let mut rec = start(&dir);
    rec.datagram(S, &session_packet(2025, 10));
    rec.datagram(30 * S, &lap_packet(2025, 10, 4));
    rec.datagram(80 * S, &telemetry(2025, 10, 1));
    rec.shutdown();
    let name = recordings(&dir).remove(0);
    let json = dir.join(&name).with_extension("json");
    let before = sidecar(&dir, &name);

    fs::remove_file(&json).unwrap();
    let rec = start(&dir);
    rec.sync();
    let mut after = sidecar(&dir, &name);
    assert_eq!(after["track"], "Monza");
    assert_eq!(after["laps"], 4);
    assert_eq!(after["duration_us"], 79 * S);
    assert_eq!(after["session_uid"], before["session_uid"]);

    // A file copied into the folder shows up after a rescan.
    fs::copy(dir.join(&name), dir.join("copied.pitrec")).unwrap();
    assert_eq!(list(&dir).unwrap().len(), 1);
    rec.rescan();
    rec.sync();
    assert_eq!(list(&dir).unwrap().len(), 2);
    after = sidecar(&dir, "copied.pitrec");
    assert_eq!(after["track"], "Monza");
}

/// Three closed recordings on three days, oldest first, each a little larger.
fn three_days(name: &str) -> (PathBuf, Vec<String>) {
    let dir = fresh_dir(name);
    let clock = Arc::new(AtomicI64::new(start_clock()));
    let mut rec = {
        let clock = clock.clone();
        Recorder::start(RecorderConfig::new(dir.clone(), u64::MAX), move || {
            clock.load(Ordering::Relaxed)
        })
        .unwrap()
    };
    for (day, uid) in [(0, 1), (1, 2), (2, 3)] {
        clock.store(start_clock() + day * 86_400, Ordering::Relaxed);
        feed_session(&mut rec, 2025, uid, S, 70 + day as u64);
        rec.datagram(200 * S, &classification(2025, uid, 1));
    }
    rec.shutdown();
    let names: Vec<String> = list(&dir)
        .unwrap()
        .into_iter()
        .map(|l| l.file_name)
        .collect();
    assert_eq!(names.len(), 3, "{names:?}");
    (dir, names)
}
fn size(dir: &Path, name: &str) -> u64 {
    fs::metadata(dir.join(name)).unwrap().len()
}
fn cap_start(dir: &Path, cap: u64) -> Recorder {
    let rec = start_with(RecorderConfig::new(dir.to_owned(), cap));
    rec.sync();
    rec
}

#[test]
fn assumed_retention_deletes_the_oldest_first_with_their_sidecars() {
    let (dir, n) = three_days("retention");
    let cap = size(&dir, &n[1]) + size(&dir, &n[2]);
    cap_start(&dir, cap).shutdown();
    assert_eq!(recordings(&dir), [n[1].clone(), n[2].clone()]);
    assert_eq!(files(&dir, ".json").len(), 2);
}

#[test]
fn assumed_retention_spares_pinned_recordings() {
    let (dir, n) = three_days("pinned");
    set_pinned(&dir, &n[0], true).unwrap();
    let cap = size(&dir, &n[0]) + size(&dir, &n[2]);
    cap_start(&dir, cap).shutdown();
    assert_eq!(recordings(&dir), [n[0].clone(), n[2].clone()]);
    assert_eq!(sidecar(&dir, &n[0])["pinned"], true);
}

#[test]
fn assumed_retention_spares_the_recording_loaded_in_replay() {
    let (dir, n) = three_days("loaded");
    let rec = cap_start(&dir, u64::MAX);
    let guard = rec.mark_loaded(&n[0]);
    rec.set_size_cap(1);
    rec.sync();
    assert_eq!(
        recordings(&dir),
        [n[0].clone()],
        "only the loaded one stays"
    );
    drop(guard);
    rec.rescan();
    rec.sync();
    assert!(recordings(&dir).is_empty());
}

#[test]
fn assumed_a_stalled_writer_never_blocks_the_tap() {
    let dir = fresh_dir("backpressure");
    let mut config = RecorderConfig::new(dir.clone(), u64::MAX);
    config.queue_capacity = 1;
    let mut rec = start_with(config);
    let bytes = telemetry(2025, 10, 1);
    // Anchors with an empty queue are accepted, so the span always exceeds 60 s.
    rec.datagram(S, &bytes);
    rec.sync();
    let flood = 20_000u64;
    for i in 0..flood {
        rec.datagram(2 * S + i * 5_000, &bytes);
    }
    rec.sync();
    rec.datagram(200 * S, &bytes);
    rec.sync();
    let sent = flood + 2;
    let dropped = rec.dropped();
    rec.shutdown();
    let names = recordings(&dir);
    let kept = read(&dir.join(&names[0])).len() as u64;
    assert_eq!(kept + dropped, sent);
}
