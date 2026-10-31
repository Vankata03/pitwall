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

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};

use outline::Outline;

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

struct Entry {
    source: Source,
    outline: Outline,
    complete: bool,
    pit: Option<Vec<[f32; 2]>>,
}

struct Inner {
    dir: PathBuf,
    revision: u64,
    maps: HashMap<TrackKey, Entry>,
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
                }
            } else {
                Entry {
                    source: Source::Shipped,
                    outline: Outline::from_points(key.track_length, shipped::outline(key)?),
                    complete: true,
                    pit: None,
                }
            };
            self.maps.insert(key, entry);
        }
        self.maps.get_mut(&key)
    }

    fn feed(&mut self, key: TrackKey, sample: Sample) {
        if sample.in_pit
            || sample.lap_invalid
            || !(sample.x.is_finite() && sample.z.is_finite())
            || !(0.0..f32::from(key.track_length)).contains(&sample.lap_distance)
        {
            return;
        }
        self.entry(key);
        let entry = self.maps.entry(key).or_insert_with(|| Entry {
            source: Source::Learned,
            outline: Outline::empty(key.track_length),
            complete: false,
            pit: None,
        });
        if entry.complete {
            return;
        }
        let Some(bucket) = entry.outline.bucket_of(sample.lap_distance) else {
            return;
        };
        if entry.outline.fill(bucket, [sample.x, sample.z]) {
            entry.complete = entry.outline.is_complete();
            self.revision += 1;
            if entry.complete {
                // A failed write loses nothing: the map stays in memory and the next
                // start relearns it.
                let _ = cache::write(&self.dir, key, &entry.outline, &entry.pit);
            }
        }
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
}
