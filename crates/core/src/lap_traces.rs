//! Lap traces held per car and lap (spec section 8, Lap traces), ported from
//! `dashboard/src/tiles/traces/lap-traces.ts` and the request rules of
//! `use-reference-trace.ts`.
//!
//! Traces are held only for the epoch they were read in: a flashback or new session bumps
//! the model epoch and everything held is forgotten, so a tile never shows a lap from the
//! old timeline. A lap the source cannot supply is remembered as unavailable and not asked
//! for again until [`LapTraceCache::invalidate_unavailable`] (a lap just completed).
//!
//! The web cache fetched over HTTP; natively [`LapTraceCache::request`] reads from a
//! source synchronously, normally [`model_source`] over the current snapshot. The cache
//! is generic so tests can hold plain values; the app holds `Arc<LapTrace>`.
//!
//! How a tile keeps its reference lap loaded (the web `useReferenceTrace`):
//! - each draw, `request(epoch, car, reference_lap, ..)` for the reference lap number
//!   (`CarView::reference_lap(Reference::Best|Previous)`); a no-op once held;
//! - when a lap completes, `invalidate_unavailable(epoch)`, then for that car
//!   `invalidate(epoch, car, completed_lap)` so a partial current-lap trace is replaced by
//!   the complete one, and request it plus the current previous and best lap numbers.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::model::{LapTrace, Reference, SessionModel};

/// A trace as a source supplies it, stamped with the epoch and lap it belongs to.
#[derive(Debug, Clone, PartialEq)]
pub struct SourcedTrace<T> {
    pub epoch: u64,
    pub lap: u8,
    pub trace: T,
}

/// Traces held per `(car, lap)` for one window or tile host.
#[derive(Debug, Clone)]
pub struct LapTraceCache<T> {
    held: HashMap<(usize, u8), T>,
    unavailable: HashSet<(usize, u8)>,
    held_epoch: Option<u64>,
    version: u64,
}

impl<T> Default for LapTraceCache<T> {
    fn default() -> Self {
        Self {
            held: HashMap::new(),
            unavailable: HashSet::new(),
            held_epoch: None,
            version: 0,
        }
    }
}

impl<T> LapTraceCache<T> {
    /// Bumped whenever a lap arrives or held laps are forgotten; redraw when it moves.
    pub fn version(&self) -> u64 {
        self.version
    }

    /// Forget everything when the epoch moved. `None` means no snapshot yet.
    fn sync(&mut self, epoch: Option<u64>) {
        if epoch != self.held_epoch {
            let had_data = !self.held.is_empty();
            self.held.clear();
            self.unavailable.clear();
            self.held_epoch = epoch;
            if had_data {
                self.version += 1;
            }
        }
    }

    /// Read lap `lap` of car `car` from `source` unless it is held or known unavailable.
    /// A no-op before the first snapshot (`epoch` None). A reply for another epoch or lap,
    /// or none at all, marks the lap unavailable.
    pub fn request(
        &mut self,
        epoch: Option<u64>,
        car: usize,
        lap: u8,
        source: impl FnOnce(usize, u8) -> Option<SourcedTrace<T>>,
    ) {
        self.sync(epoch);
        let Some(epoch) = epoch else { return };
        let key = (car, lap);
        if self.held.contains_key(&key) || self.unavailable.contains(&key) {
            return;
        }
        match source(car, lap) {
            Some(reply) if reply.epoch == epoch && reply.lap == lap => {
                self.held.insert(key, reply.trace);
                self.version += 1;
            }
            _ => {
                self.unavailable.insert(key);
            }
        }
    }

    pub fn get(&mut self, epoch: Option<u64>, car: usize, lap: u8) -> Option<&T> {
        self.sync(epoch);
        self.held.get(&(car, lap))
    }

    /// Allow laps that were unavailable to be requested again (a lap just completed).
    pub fn invalidate_unavailable(&mut self, epoch: Option<u64>) {
        self.sync(epoch);
        self.unavailable.clear();
    }

    /// Also drop the held trace of one lap, so the next request reads it again: a partial
    /// current-lap trace is replaced by the complete one.
    pub fn invalidate(&mut self, epoch: Option<u64>, car: usize, lap: u8) {
        self.invalidate_unavailable(epoch);
        if self.held.remove(&(car, lap)).is_some() {
            self.version += 1;
        }
    }
}

/// The trace of `lap` for `car` from a model snapshot: the current lap's partial trace,
/// or a completed lap the model retains as a named reference (best, previous, personal
/// best, rival). Use as the `source` of [`LapTraceCache::request`].
pub fn model_source(
    model: &SessionModel,
    car: usize,
    lap: u8,
) -> Option<SourcedTrace<Arc<LapTrace>>> {
    let view = model.car(car)?;
    let current = view.current_lap_trace().filter(|t| t.lap_number == lap);
    let trace = current.or_else(|| {
        [
            Reference::Best,
            Reference::Previous,
            Reference::PersonalBest,
            Reference::Rival,
        ]
        .into_iter()
        .filter_map(|reference| view.lap_trace(reference))
        .find(|t| t.lap_number == lap)
    })?;
    Some(SourcedTrace {
        epoch: model.epoch(),
        lap,
        trace: Arc::clone(trace),
    })
}
