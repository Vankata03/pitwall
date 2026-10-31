//! Recording files on disk: summaries, sidecars, naming, recompression and
//! retention. Pure file work; the session boundary rules live in `super`.
use crate::{
    names,
    pitrec::{Header, Reader, Writer},
    telemetry::{Packet, parse_datagram},
};
use chrono::{DateTime, SecondsFormat};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    io::{self, BufReader, BufWriter},
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

pub const MIN_SPAN_US: u64 = 60_000_000;
/// Provisional until the compression build task settles the levels.
pub const RECOMPRESS_LEVEL: i32 = 19;
const SESSION_PACKET: u8 = 1;
const LAP_PACKET: u8 = 2;
pub(super) const FINAL_CLASSIFICATION_PACKET: u8 = 8;

/// Serialises sidecar read-modify-write between recompression and pinning.
static SIDECARS: Mutex<()> = Mutex::new(());

/// What the `.json` beside each recording holds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sidecar {
    /// A string, because a u64 UID does not survive a JavaScript number.
    pub session_uid: String,
    pub packet_format: u16,
    pub started_utc: String,
    pub track: Option<String>,
    pub session_type: Option<String>,
    /// Arrival span of the records.
    pub duration_us: u64,
    pub laps: u32,
    pub pinned: bool,
    /// False while the file still has its fast-level compression.
    pub recompressed: bool,
}

/// Session facts gathered from datagrams, live or by scanning a file.
#[derive(Default)]
pub(super) struct Summary {
    uid: Option<u64>,
    track_id: Option<i8>,
    session_type: Option<u8>,
    lap_count: u32,
    classified_laps: Option<u32>,
    first_us: Option<u64>,
    last_us: u64,
}

impl Summary {
    pub fn observe(&mut self, arrival_us: u64, bytes: &[u8]) {
        self.first_us.get_or_insert(arrival_us);
        self.last_us = self.last_us.max(arrival_us);
        if self.uid.is_none() {
            self.uid = bytes
                .get(7..15)
                .map(|b| u64::from_le_bytes(b.try_into().unwrap()));
        }
        if !matches!(
            bytes.get(6),
            Some(&(SESSION_PACKET | LAP_PACKET | FINAL_CLASSIFICATION_PACKET))
        ) {
            return;
        }
        let Ok(parsed) = parse_datagram(bytes) else {
            return;
        };
        let player = usize::from(parsed.header.player_car_index);
        match parsed.packet {
            Packet::Session(s) => {
                self.track_id = Some(s.track_id);
                self.session_type = Some(s.session_type);
            }
            Packet::Lap(l) => {
                if let Some(Some(car)) = l.lap_data.get(player) {
                    self.lap_count = self.lap_count.max(car.current_lap_num.into());
                }
            }
            Packet::FinalClassification(c) => {
                if let Some(Some(car)) = c.classification_data.get(player) {
                    self.classified_laps = Some(car.num_laps.into());
                }
            }
            _ => {}
        }
    }

    pub fn span_us(&self) -> u64 {
        self.last_us - self.first_us.unwrap_or(0)
    }

    fn sidecar(&self, header: &Header) -> Sidecar {
        Sidecar {
            session_uid: self.uid.unwrap_or(0).to_string(),
            packet_format: header.packet_format,
            started_utc: header.started_utc.clone(),
            track: self.track_id.and_then(names::track_name).map(str::to_owned),
            session_type: self
                .session_type
                .and_then(names::session_name)
                .map(str::to_owned),
            duration_us: self.span_us(),
            laps: self.classified_laps.unwrap_or(self.lap_count),
            pinned: false,
            recompressed: false,
        }
    }
}

pub(super) fn rfc3339(unix_seconds: i64) -> String {
    DateTime::from_timestamp(unix_seconds, 0)
        .unwrap_or_default()
        .to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// Read every complete record of a possibly truncated file.
pub(super) fn scan(path: &Path) -> io::Result<(Header, Summary)> {
    let mut reader = Reader::new(BufReader::new(fs::File::open(path)?))?;
    let mut summary = Summary::default();
    for record in reader.by_ref() {
        match record {
            Ok(r) => summary.observe(r.arrival_us, &r.datagram),
            Err(_) => break,
        }
    }
    Ok((reader.header().clone(), summary))
}

fn clean(label: &str) -> String {
    label
        .chars()
        .filter_map(|c| match c {
            ' ' => Some('_'),
            c if c.is_alphanumeric() || c == '-' => Some(c),
            _ => None,
        })
        .collect()
}

fn sidecar_path(recording: &Path) -> PathBuf {
    recording.with_extension("json")
}

fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    fs::write(&tmp, bytes)?;
    fs::rename(tmp, path)
}

fn write_sidecar(recording: &Path, sidecar: &Sidecar) -> io::Result<()> {
    write_atomic(
        &sidecar_path(recording),
        &serde_json::to_vec_pretty(sidecar)?,
    )
}

pub fn read_sidecar(recording: &Path) -> io::Result<Sidecar> {
    Ok(serde_json::from_slice(&fs::read(sidecar_path(recording))?)?)
}

fn update_sidecar(recording: &Path, change: impl FnOnce(&mut Sidecar)) -> io::Result<()> {
    let _guard = SIDECARS.lock().unwrap_or_else(|e| e.into_inner());
    let mut sidecar = read_sidecar(recording)?;
    change(&mut sidecar);
    write_sidecar(recording, &sidecar)
}

/// Pin or unpin a recording by file name. Pinned recordings are never deleted by retention.
pub fn set_pinned(dir: &Path, file_name: &str, pinned: bool) -> io::Result<()> {
    update_sidecar(&dir.join(file_name), |s| s.pinned = pinned)
}

/// Names look like `2026-10-31_1430_Monza_Race`, plus the UID only on a collision.
fn free_name(dir: &Path, sidecar: &Sidecar) -> String {
    let when = DateTime::parse_from_rfc3339(&sidecar.started_utc)
        .map(|t| t.format("%Y-%m-%d_%H%M").to_string())
        .unwrap_or_else(|_| "unknown-time".into());
    let base = format!(
        "{when}_{}_{}",
        clean(sidecar.track.as_deref().unwrap_or("Unknown")),
        clean(sidecar.session_type.as_deref().unwrap_or("Session")),
    );
    let taken = |name: &str| {
        dir.join(format!("{name}.pitrec")).exists() || dir.join(format!("{name}.json")).exists()
    };
    if !taken(&base) {
        return base;
    }
    let with_uid = format!("{base}_{}", sidecar.session_uid);
    (1..)
        .map(|n| match n {
            1 => with_uid.clone(),
            n => format!("{with_uid}-{n}"),
        })
        .find(|name| !taken(name))
        .unwrap()
}

/// Close step 2: drop a short file, or write its sidecar and rename it to its
/// final name. The sidecar goes first, so a final file never lacks one.
pub(super) fn publish(
    dir: &Path,
    part: &Path,
    header: &Header,
    summary: &Summary,
) -> io::Result<Option<PathBuf>> {
    if summary.first_us.is_none() || summary.span_us() < MIN_SPAN_US {
        fs::remove_file(part)?;
        return Ok(None);
    }
    let sidecar = summary.sidecar(header);
    let name = free_name(dir, &sidecar);
    let recording = dir.join(format!("{name}.pitrec"));
    write_sidecar(&recording, &sidecar)?;
    fs::rename(part, &recording)?;
    Ok(Some(recording))
}

/// Recover a `.part` left by a crash: same steps as a normal close.
pub(super) fn recover(dir: &Path, part: &Path) -> io::Result<Option<PathBuf>> {
    match scan(part) {
        Ok((header, summary)) => publish(dir, part, &header, &summary),
        Err(_) => {
            // No readable header: nothing to keep.
            fs::remove_file(part)?;
            Ok(None)
        }
    }
}

/// A `.pitrec` without a sidecar gets one rebuilt from its records.
pub(super) fn rebuild_sidecar(recording: &Path) -> io::Result<()> {
    let (header, summary) = scan(recording)?;
    let mut sidecar = summary.sidecar(&header);
    sidecar.recompressed = true;
    write_sidecar(recording, &sidecar)
}

fn extension_is(path: &Path, extension: &str) -> bool {
    path.extension().is_some_and(|e| e == extension)
}

fn entries(dir: &Path) -> io::Result<impl Iterator<Item = PathBuf>> {
    Ok(fs::read_dir(dir)?.filter_map(|e| e.ok().map(|e| e.path())))
}

pub(super) fn part_files(dir: &Path) -> io::Result<Vec<PathBuf>> {
    Ok(entries(dir)?
        .filter(|p| p.to_string_lossy().ends_with(".pitrec.part"))
        .collect())
}

/// Delete leftovers of interrupted writes, rebuild missing sidecars, drop
/// sidecars whose recording is gone.
pub(super) fn repair(dir: &Path) -> io::Result<()> {
    for path in entries(dir)?.collect::<Vec<_>>() {
        let text = path.to_string_lossy();
        if text.ends_with(".pitrec.tmp") || text.ends_with(".json.tmp") {
            let _ = fs::remove_file(&path);
        } else if extension_is(&path, "pitrec") && !sidecar_path(&path).exists() {
            let _ = rebuild_sidecar(&path);
        } else if extension_is(&path, "json") && !path.with_extension("pitrec").exists() {
            let _ = fs::remove_file(&path);
        }
    }
    Ok(())
}

/// One listed recording, from its sidecar.
#[derive(Clone, Debug, PartialEq)]
pub struct Listing {
    pub file_name: String,
    pub size: u64,
    pub sidecar: Sidecar,
}

/// Recordings with a readable sidecar, oldest first. `.part` files are not listed.
pub fn list(dir: &Path) -> io::Result<Vec<Listing>> {
    let mut listed: Vec<Listing> = entries(dir)?
        .filter(|p| extension_is(p, "pitrec"))
        .filter_map(|p| {
            Some(Listing {
                file_name: p.file_name()?.to_str()?.to_owned(),
                size: fs::metadata(&p).ok()?.len(),
                sidecar: read_sidecar(&p).ok()?,
            })
        })
        .collect();
    listed.sort_by(|a, b| {
        (&a.sidecar.started_utc, &a.file_name).cmp(&(&b.sidecar.started_utc, &b.file_name))
    });
    Ok(listed)
}

/// Delete the oldest recordings until the total fits `cap_bytes`. Pinned and
/// loaded recordings are never deleted, even when the cap stays exceeded.
pub(super) fn sweep(
    dir: &Path,
    cap_bytes: &AtomicU64,
    loaded: &Mutex<HashMap<String, usize>>,
) -> io::Result<()> {
    let listed = list(dir)?;
    let cap_bytes = cap_bytes.load(Ordering::Relaxed);
    let mut total: u64 = listed.iter().map(|l| l.size).sum();
    for entry in listed {
        if total <= cap_bytes {
            break;
        }
        let in_replay = loaded
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&entry.file_name)
            .is_some_and(|&n| n > 0);
        if entry.sidecar.pinned || in_replay {
            continue;
        }
        let path = dir.join(&entry.file_name);
        // Sidecar first: a recording without one is rebuilt, a sidecar without one is cleaned.
        let _ = fs::remove_file(sidecar_path(&path));
        if fs::remove_file(&path).is_ok() {
            total -= entry.size;
        }
    }
    Ok(())
}

/// Close step 3: recompress to a temporary file, then swap it in atomically.
/// Replay output is identical because records, order and times are preserved.
pub(super) fn recompress(recording: &Path, level: i32, stop: &AtomicBool) -> io::Result<()> {
    let mut tmp = recording.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    let result = (|| {
        let mut reader = Reader::new(BufReader::new(fs::File::open(recording)?))?;
        let file = fs::File::create(&tmp)?;
        let mut writer = Writer::new(BufWriter::new(file), reader.header())?;
        writer.set_compression_level(level);
        for record in reader.by_ref() {
            if stop.load(Ordering::Relaxed) {
                return Err(io::Error::other("recompression stopped"));
            }
            let record = record?;
            writer.write(record.arrival_us, &record.datagram)?;
            if record.seq % 512 == 0 {
                std::thread::yield_now();
            }
        }
        // An incomplete tail (a crash's last frame) is dropped by the swap; a
        // corrupt complete block is an error above and keeps the original.
        let file = writer.finish()?.into_inner().map_err(|e| e.into_error())?;
        file.sync_all()?;
        fs::rename(&tmp, recording)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    } else {
        update_sidecar(recording, |s| s.recompressed = true)?;
    }
    result
}
