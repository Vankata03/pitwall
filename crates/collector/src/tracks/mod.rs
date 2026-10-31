//! The track store: shipped maps, learning and cached files (spec section 7).
//!
//! A map is keyed by [`TrackKey`]. Look one up with [`TrackStore::get`] (cached file, then
//! shipped map, then whatever the learner has so far). Feed the learner with
//! [`TrackStore::observe`] from a session model, or [`TrackStore::feed`] with raw samples.
//! [`TrackStore::revision`] advances whenever any map changes. The store sits outside the
//! session model and its checkpoints.

mod cache;
mod outline;
mod shipped;
mod watch;

use std::{
    collections::HashMap,
    io,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};

use outline::Outline;
use watch::{Drift, PitWatch};

pub use watch::MISMATCH_METRES;

/// Lap distance covered by one outline bucket.
pub const BUCKET_METRES: f32 = 8.0;

/// Identifies one layout: game track ID and track length in metres.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TrackKey {
    pub track_id: i8,
    pub track_length: u16,
}

impl TrackKey {
    pub fn new(track_id: i8, track_length: u16) -> Self {
        Self {
            track_id,
            track_length,
        }
    }
}

/// One outline point: bucket `n` sits at lap distance `n * 8` m. `x` and `z` are game world
/// coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(from = "(u32, f32, f32)", into = "(u32, f32, f32)")]
pub struct OutlinePoint {
    pub bucket: u32,
    pub x: f32,
    pub z: f32,
}

impl From<(u32, f32, f32)> for OutlinePoint {
    fn from((bucket, x, z): (u32, f32, f32)) -> Self {
        Self { bucket, x, z }
    }
}

impl From<OutlinePoint> for (u32, f32, f32) {
    fn from(point: OutlinePoint) -> Self {
        (point.bucket, point.x, point.z)
    }
}

/// Where a map's outline came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Shipped,
    Cached,
    Learned,
}

/// A track map as the store hands it out.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackMap {
    pub key: TrackKey,
    pub source: Source,
    pub complete: bool,
    pub outline: Vec<OutlinePoint>,
    pub pit: Option<Vec<[f32; 2]>>,
}

/// One car's reading, as the learner takes it. `in_pit` is a non-zero `m_pitStatus`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    pub car: usize,
    pub x: f32,
    pub z: f32,
    pub lap_distance: f32,
    pub in_pit: bool,
    pub lap_invalid: bool,
}

/// Why a store action failed.
#[derive(Debug)]
pub enum TrackError {
    /// The store knows nothing about this layout.
    NotFound,
    Io(io::Error),
}

struct Entry {
    source: Source,
    outline: Outline,
    complete: bool,
    pit: Option<Vec<[f32; 2]>>,
    drift: HashMap<usize, Drift>,
}

impl Entry {
    fn learner(key: TrackKey) -> Self {
        Self {
            source: Source::Learned,
            outline: Outline::empty(key.track_length),
            complete: false,
            pit: None,
            drift: HashMap::new(),
        }
    }
}

struct Inner {
    dir: PathBuf,
    revision: u64,
    maps: HashMap<TrackKey, Entry>,
    pit_watch: HashMap<(TrackKey, usize), PitWatch>,
}

impl Inner {
    /// Lookup order: what is already held, then a cached file, then the shipped map.
    fn entry(&mut self, key: TrackKey) -> Option<&mut Entry> {
        if !self.maps.contains_key(&key) {
            let entry = if let Some((outline, pit)) = cache::read(&self.dir, key) {
                Entry {
                    source: Source::Cached,
                    outline,
                    complete: true,
                    pit,
                    drift: HashMap::new(),
                }
            } else {
                Entry {
                    source: Source::Shipped,
                    outline: Outline::from_points(key.track_length, shipped::outline(key)?),
                    complete: true,
                    pit: None,
                    drift: HashMap::new(),
                }
            };
            self.maps.insert(key, entry);
        }
        self.maps.get_mut(&key)
    }

    fn feed(&mut self, key: TrackKey, sample: Sample) {
        if !(sample.x.is_finite() && sample.z.is_finite()) {
            return;
        }
        let at = [sample.x, sample.z];
        let trace = self
            .pit_watch
            .entry((key, sample.car))
            .or_default()
            .observe(at, sample.in_pit);
        if let Some(trace) = trace {
            self.add_pit(key, trace);
        }
        if sample.in_pit
            || sample.lap_invalid
            || !(0.0..f32::from(key.track_length)).contains(&sample.lap_distance)
        {
            return;
        }
        let length = f32::from(key.track_length);
        if let Some(entry) = self.entry(key)
            && entry.complete
        {
            // Frozen: only a whole lap far off the line replaces the map.
            let bucket = entry.outline.bucket_of(sample.lap_distance);
            let off_line = bucket
                .and_then(|bucket| entry.outline.distance_to_line(bucket, at))
                .map(|distance| distance > MISMATCH_METRES);
            let replace = off_line.is_some_and(|off_line| {
                entry
                    .drift
                    .entry(sample.car)
                    .or_default()
                    .observe(sample.lap_distance, off_line, length)
            });
            if !replace {
                return;
            }
            self.maps.insert(key, Entry::learner(key));
            self.revision += 1;
        }
        let entry = self
            .maps
            .entry(key)
            .or_insert_with(|| Entry::learner(key));
        let Some(bucket) = entry.outline.bucket_of(sample.lap_distance) else {
            return;
        };
        if entry.outline.fill(bucket, at) {
            entry.complete = entry.outline.is_complete();
            self.revision += 1;
            if entry.complete {
                self.save(key);
            }
        }
    }

    /// Keep the first pit trace a map sees. Later passes change nothing.
    fn add_pit(&mut self, key: TrackKey, trace: Vec<[f32; 2]>) {
        let entry = match self.entry(key) {
            Some(entry) => entry,
            None => self.maps.entry(key).or_insert_with(|| Entry::learner(key)),
        };
        if entry.pit.is_some() {
            return;
        }
        entry.pit = Some(trace);
        let complete = entry.complete;
        self.revision += 1;
        if complete {
            self.save(key);
        }
    }

    /// Write the map's cached file. A failed write loses nothing: the map stays in memory
    /// and the next start relearns it.
    fn save(&self, key: TrackKey) {
        if let Some(entry) = self.maps.get(&key) {
            let _ = cache::write(&self.dir, key, &entry.outline, &entry.pit);
        }
    }

    fn relearn(&mut self, key: TrackKey) -> Result<(), TrackError> {
        let removed = cache::remove(&self.dir, key).map_err(TrackError::Io)?;
        let dropped = self.maps.remove(&key);
        if !removed && dropped.is_none() && shipped::outline(key).is_none() {
            return Err(TrackError::NotFound);
        }
        if removed || dropped.is_some_and(|entry| entry.source != Source::Shipped) {
            self.revision += 1;
        }
        Ok(())
    }
}

/// The shared track store. Cloning shares one store. `dir` is the `tracks/` folder.
#[derive(Clone)]
pub struct TrackStore(Arc<Mutex<Inner>>);

impl TrackStore {
    pub fn open(dir: PathBuf) -> Self {
        Self(Arc::new(Mutex::new(Inner {
            dir,
            revision: 0,
            maps: HashMap::new(),
            pit_watch: HashMap::new(),
        })))
    }

    /// Counter that advances whenever any map changes.
    pub fn revision(&self) -> u64 {
        self.0.lock().unwrap().revision
    }

    pub fn get(&self, key: TrackKey) -> Option<TrackMap> {
        let mut inner = self.0.lock().unwrap();
        let entry = inner.entry(key)?;
        Some(TrackMap {
            key,
            source: entry.source,
            complete: entry.complete,
            outline: entry.outline.points(),
            pit: entry.pit.clone(),
        })
    }

    /// Offer one car's reading to the learner. Rejected samples are dropped silently.
    pub fn feed(&self, key: TrackKey, sample: Sample) {
        self.0.lock().unwrap().feed(key, sample);
    }

    /// The host's Relearn map action: drop the cached file and any map being learned, and
    /// fall back to the shipped map if there is one.
    pub fn relearn(&self, key: TrackKey) -> Result<(), TrackError> {
        self.0.lock().unwrap().relearn(key)
    }
}
