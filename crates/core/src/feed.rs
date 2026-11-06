//! The UI read API: how a UI thread reads the core without serialisation or a wire.
//!
//! The ingest thread owns the [`Publisher`]; any number of UI threads hold a cloned
//! [`Reader`]. There are two paths, published separately:
//!
//! - **Slow state**: a [`Snapshot`] of the whole [`SessionModel`] (session, cars, laps,
//!   stints, the race-control log). Publishing clones the model into an `Arc`; the
//!   model's own `Arc`s make that clone cheap, and a reader gets a shared handle without
//!   copying. Publish it when slow state may have changed, at most every few frames.
//! - **Fast path**: a [`FastFrame`] of frame-rate car data (inputs, speed, RPM, gear) for
//!   every car slot. It is a fixed-size frame behind a seqlock of atomic words: the writer
//!   never waits and never allocates, and a reader copies the frame without taking a lock.
//!   Publish it after every applied telemetry datagram.
//!   Trace readers can catch up through [`Reader::fast_since`], a bounded ring of
//!   independently seqlocked frames, without delaying the writer.
//!
//! Both carry a generation that advances on every publish, so a UI repaints only when
//! the generation it last drew differs. See `docs/native/fast-path.md` for the
//! measurements behind the fast-path choice.

use std::{
    hint,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering, fence},
    },
};

use crate::{
    model::{Readout, Reference, SessionModel},
    telemetry::{CarTelemetryData, MAX_CARS},
};

/// Frame-rate inputs of one car, copied out of its latest Car Telemetry packet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CarInputs {
    /// km/h.
    pub speed: u16,
    /// 0.0 to 1.0.
    pub throttle: f32,
    /// 0.0 to 1.0.
    pub brake: f32,
    /// -1.0 (full lock left) to 1.0 (full lock right).
    pub steer: f32,
    /// 0 to 100.
    pub clutch: u8,
    /// -1 reverse, 0 neutral, 1 to 8 forward.
    pub gear: i8,
    pub engine_rpm: u16,
    pub drs: bool,
    /// 0 to 100.
    pub rev_lights_percent: u8,
}

impl From<&CarTelemetryData> for CarInputs {
    fn from(data: &CarTelemetryData) -> Self {
        Self {
            speed: data.speed,
            throttle: data.throttle,
            brake: data.brake,
            steer: data.steer,
            clutch: data.clutch,
            gear: data.gear,
            engine_rpm: data.engine_rpm,
            drs: data.drs != 0,
            rev_lights_percent: data.rev_lights_percent,
        }
    }
}

/// One fast-path frame: every car slot's inputs as the model reads them at publish time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FastFrame {
    /// Advances by one on every fast publish; 0 before the first.
    pub generation: u64,
    /// The model timeline this frame belongs to. Never combine it with another epoch's
    /// snapshot when building traces.
    pub epoch: u64,
    /// The game's session clock in seconds when the frame was published.
    pub session_time: f32,
    pub player_car_index: Option<usize>,
    /// `Live` or `Stale` per the model's staleness rules, `Absent` for an empty or unseen
    /// slot.
    pub cars: [Readout<CarInputs>; MAX_CARS],
    /// Metres along the lap per car slot, from Lap Data, with that packet's own staleness.
    /// Negative before the line at a race start. Lets a trace align a live sample with a
    /// reference lap at the same circuit position.
    pub lap_distances: [Readout<f32>; MAX_CARS],
    /// Signed milliseconds against the named reference, including the official figure
    /// on the first next-lap update. Graph consumers exclude that update.
    pub delta_best: [Readout<f32>; MAX_CARS],
    pub delta_previous: [Readout<f32>; MAX_CARS],
}

/// A bounded catch-up read for traces. An overrun means some intervening publications
/// have been overwritten; callers must break their trace instead of bridging the loss.
pub struct FastBatch {
    pub frames: Vec<FastFrame>,
    pub overrun: bool,
}

/// The web frame ring's ten-second lookback, at the native 240 Hz publication budget.
/// The writer never waits for readers. Consumers also trim by the session clock.
pub const FAST_HISTORY_SECONDS: f32 = 10.0;
pub const FAST_HISTORY_CAPACITY: usize = 2400;

/// One slow-state publish: a shared, immutable session model. Dereferences to
/// [`SessionModel`], so `snapshot.car(0)` and `snapshot.race_control()` work directly.
#[derive(Debug, Clone)]
pub struct Snapshot {
    /// Advances by one on every snapshot publish; 0 before the first.
    pub generation: u64,
    pub model: Arc<SessionModel>,
}

impl std::ops::Deref for Snapshot {
    type Target = SessionModel;
    fn deref(&self) -> &SessionModel {
        &self.model
    }
}

/// Create a connected publisher and reader. Both start empty, at generation 0.
pub fn channel() -> (Publisher, Reader) {
    let shared = Arc::new(Shared {
        snapshot: Mutex::new(Snapshot {
            generation: 0,
            model: Arc::new(SessionModel::default()),
        }),
        frame_snapshot: Mutex::new(Snapshot {
            generation: 0,
            model: Arc::new(SessionModel::default()),
        }),
        fast: SeqLock::new(encode(&EMPTY_FRAME)),
        history: (0..FAST_HISTORY_CAPACITY)
            .map(|_| SeqLock::new(encode(&EMPTY_FRAME)))
            .collect(),
    });
    (
        Publisher {
            shared: shared.clone(),
            snapshot_generation: 0,
        },
        Reader { shared },
    )
}

/// The writing end, owned by the ingest thread. Not `Clone`: the fast path has exactly
/// one writer.
pub struct Publisher {
    shared: Arc<Shared>,
    snapshot_generation: u64,
}

impl Publisher {
    /// Separate the single fast writer from slow snapshot publication.
    pub fn split(self) -> (SnapshotPublisher, FastPublisher) {
        (
            SnapshotPublisher {
                shared: self.shared.clone(),
                generation: self.snapshot_generation,
            },
            FastPublisher {
                shared: self.shared,
            },
        )
    }

    /// Publish the frame-rate car data of `model`. Never blocks and never allocates.
    pub fn publish_fast(&mut self, model: &SessionModel) {
        self.shared.publish_fast(model);
    }

    /// Publish `model` as the new slow-state snapshot. Clones the model; readers holding
    /// an older snapshot keep it until they drop it.
    pub fn publish_snapshot(&mut self, model: &SessionModel) {
        self.snapshot_generation += 1;
        let snapshot = Snapshot {
            generation: self.snapshot_generation,
            model: Arc::new(model.clone()),
        };
        *self.shared.snapshot.lock().unwrap() = snapshot;
    }
}

/// The slow writer can clone models without delaying the fast writer.
pub struct SnapshotPublisher {
    shared: Arc<Shared>,
    generation: u64,
}
impl SnapshotPublisher {
    pub fn publish_snapshot(&mut self, model: &SessionModel) {
        self.generation += 1;
        let snapshot = Snapshot {
            generation: self.generation,
            model: Arc::new(model.clone()),
        };
        *self.shared.snapshot.lock().unwrap() = snapshot;
    }
}

/// Exactly one fast writer, independently owned from the slow writer.
pub struct FastPublisher {
    shared: Arc<Shared>,
}
impl FastPublisher {
    /// Publish the shared model used by complex frame tiles, at most once per display
    /// frame. Prepare the copy outside source switching; the store only swaps an Arc.
    pub fn publish_frame_model(&mut self, model: Arc<SessionModel>) {
        let mut snapshot = self.shared.frame_snapshot.lock().unwrap();
        let generation = snapshot.generation + 1;
        *snapshot = Snapshot { generation, model };
    }

    /// Publish the frame-rate car data of `model`. Never blocks and never allocates.
    pub fn publish_fast(&mut self, model: &SessionModel) {
        self.shared.publish_fast(model);
    }
}

/// The reading end. Clone one per UI thread or window; reads never block the publisher.
#[derive(Clone)]
pub struct Reader {
    shared: Arc<Shared>,
}

impl Reader {
    /// Model data for frame tiles (lap timing, delta, map and other complex readouts).
    /// Kept separate from the 5 Hz snapshot consumed by slow tiles.
    pub fn frame_snapshot(&self) -> Snapshot {
        self.shared.frame_snapshot.lock().unwrap().clone()
    }

    /// The latest slow-state snapshot. Holds the lock only to clone an `Arc`.
    pub fn snapshot(&self) -> Snapshot {
        self.shared.snapshot.lock().unwrap().clone()
    }

    /// The latest fast-path frame, copied out without a lock.
    pub fn fast(&self) -> FastFrame {
        let (sequence, words) = self.shared.fast.read();
        decode(sequence / 2, &words)
    }

    /// The fast-path generation alone: one atomic load, for a cheap "anything new?" check
    /// before copying a frame.
    pub fn fast_generation(&self) -> u64 {
        self.shared.fast.sequence.load(Ordering::Acquire) / 2
    }

    /// Copy publications newer than `generation` in order. Each retained frame has its
    /// own seqlock, so a reader cannot stall ingest or observe a torn frame. A writer that
    /// laps this read causes an overrun; only matching generations are returned.
    pub fn fast_since(&self, generation: u64) -> FastBatch {
        let latest = self.fast_generation();
        let first = generation
            .saturating_add(1)
            .max(latest.saturating_sub(FAST_HISTORY_CAPACITY as u64 - 1));
        let mut batch = FastBatch {
            frames: Vec::new(),
            overrun: latest.saturating_sub(generation) > FAST_HISTORY_CAPACITY as u64,
        };
        for expected in first..=latest {
            let (sequence, words) =
                self.shared.history[expected as usize % FAST_HISTORY_CAPACITY].read();
            if sequence / 2 == expected {
                batch.frames.push(decode(expected, &words));
            } else {
                batch.overrun = true;
                // Return only the contiguous tail after the last overwritten slot.
                batch.frames.clear();
            }
        }
        batch
    }
}

struct Shared {
    frame_snapshot: Mutex<Snapshot>,
    snapshot: Mutex<Snapshot>,
    fast: SeqLock,
    history: Box<[SeqLock]>,
}

impl Shared {
    fn publish_fast(&self, model: &SessionModel) {
        let mut frame = EMPTY_FRAME;
        frame.epoch = model.epoch();
        frame.session_time = model.session_time();
        frame.player_car_index = model.player_car_index();
        for (index, slot) in frame.cars.iter_mut().enumerate() {
            if let Some(car) = model.car(index) {
                *slot = car.telemetry().map(CarInputs::from);
                frame.lap_distances[index] = car.lap().map(|lap| lap.lap_distance);
                frame.delta_best[index] = car.live_delta(Reference::Best);
                frame.delta_previous[index] = car.live_delta(Reference::Previous);
            }
        }
        let words = encode(&frame);
        let generation = self.fast.sequence.load(Ordering::Relaxed) / 2 + 1;
        self.history[generation as usize % FAST_HISTORY_CAPACITY]
            .write_generation(&words, generation);
        self.fast.write(&words);
    }
}

const EMPTY_FRAME: FastFrame = FastFrame {
    generation: 0,
    epoch: 0,
    session_time: 0.0,
    player_car_index: None,
    cars: [Readout::Absent; MAX_CARS],
    lap_distances: [Readout::Absent; MAX_CARS],
    delta_best: [Readout::Absent; MAX_CARS],
    delta_previous: [Readout::Absent; MAX_CARS],
};

/// One header word, three input words and three distance/delta words per car, then epoch.
const WORDS: usize = 2 + 6 * MAX_CARS;
const LAP_BASE: usize = 1 + 3 * MAX_CARS;
const BEST_BASE: usize = LAP_BASE + MAX_CARS;
const PREVIOUS_BASE: usize = BEST_BASE + MAX_CARS;
const EPOCH_WORD: usize = WORDS - 1;

/// A single-writer sequence lock over plain atomic words, so a racing read is never a
/// data race: at worst it sees a mix of two frames, which the sequence check rejects.
/// The sequence is odd while a write is in progress; it advances by two per write.
struct SeqLock {
    sequence: AtomicU64,
    words: [AtomicU64; WORDS],
}

impl SeqLock {
    fn new(initial: [u64; WORDS]) -> Self {
        Self {
            sequence: AtomicU64::new(0),
            words: initial.map(AtomicU64::new),
        }
    }

    /// Callers guarantee a single writer (`Publisher` is not `Clone` and takes `&mut`).
    fn write(&self, words: &[u64; WORDS]) {
        let generation = self.sequence.load(Ordering::Relaxed) / 2 + 1;
        self.write_generation(words, generation);
    }

    fn write_generation(&self, words: &[u64; WORDS], generation: u64) {
        self.sequence.store(generation * 2 - 1, Ordering::Relaxed);
        fence(Ordering::Release);
        for (slot, word) in self.words.iter().zip(words) {
            slot.store(*word, Ordering::Relaxed);
        }
        self.sequence.store(generation * 2, Ordering::Release);
    }

    /// A consistent copy and the even sequence it was taken at.
    fn read(&self) -> (u64, [u64; WORDS]) {
        loop {
            let before = self.sequence.load(Ordering::Acquire);
            if before % 2 == 1 {
                hint::spin_loop();
                continue;
            }
            let words = std::array::from_fn(|i| self.words[i].load(Ordering::Relaxed));
            fence(Ordering::Acquire);
            if self.sequence.load(Ordering::Relaxed) == before {
                return (before, words);
            }
            hint::spin_loop();
        }
    }
}

const NO_PLAYER: u64 = 0xFF;
const ABSENT: u64 = 0;
const LIVE: u64 = 1;
const STALE: u64 = 2;
const RESTRICTED: u64 = 3;

fn encode(frame: &FastFrame) -> [u64; WORDS] {
    let mut words = [0; WORDS];
    let player = frame
        .player_car_index
        .map_or(NO_PLAYER, |index| index as u64);
    words[0] = u64::from(frame.session_time.to_bits()) | player << 32;
    words[EPOCH_WORD] = frame.epoch;
    for (index, car) in frame.cars.iter().enumerate() {
        let (state, inputs) = match car {
            Readout::Live(inputs) => (LIVE, Some(inputs)),
            Readout::Stale(inputs) => (STALE, Some(inputs)),
            Readout::Restricted => (RESTRICTED, None),
            Readout::Absent => (ABSENT, None),
        };
        let base = 1 + 3 * index;
        if let Some(c) = inputs {
            words[base] = u64::from(c.throttle.to_bits()) | u64::from(c.brake.to_bits()) << 32;
            words[base + 1] = u64::from(c.steer.to_bits())
                | u64::from(c.speed) << 32
                | u64::from(c.engine_rpm) << 48;
            words[base + 2] = u64::from(c.clutch)
                | u64::from(c.gear as u8) << 8
                | u64::from(c.drs) << 16
                | u64::from(c.rev_lights_percent) << 24;
        }
        words[base + 2] |= state << 56;
    }
    for (base, values) in [
        (LAP_BASE, &frame.lap_distances),
        (BEST_BASE, &frame.delta_best),
        (PREVIOUS_BASE, &frame.delta_previous),
    ] {
        for (index, value) in values.iter().enumerate() {
            words[base + index] = match value {
                Readout::Live(value) => u64::from(value.to_bits()) | LIVE << 56,
                Readout::Stale(value) => u64::from(value.to_bits()) | STALE << 56,
                Readout::Restricted => RESTRICTED << 56,
                Readout::Absent => ABSENT << 56,
            };
        }
    }
    words
}

fn decode(generation: u64, words: &[u64; WORDS]) -> FastFrame {
    let player = (words[0] >> 32) & 0xFF;
    FastFrame {
        generation,
        epoch: words[EPOCH_WORD],
        session_time: f32::from_bits(words[0] as u32),
        player_car_index: (player != NO_PLAYER).then_some(player as usize),
        cars: std::array::from_fn(|index| {
            let base = 1 + 3 * index;
            let [a, b, c] = [words[base], words[base + 1], words[base + 2]];
            let inputs = CarInputs {
                throttle: f32::from_bits(a as u32),
                brake: f32::from_bits((a >> 32) as u32),
                steer: f32::from_bits(b as u32),
                speed: (b >> 32) as u16,
                engine_rpm: (b >> 48) as u16,
                clutch: c as u8,
                gear: (c >> 8) as u8 as i8,
                drs: (c >> 16) & 1 == 1,
                rev_lights_percent: (c >> 24) as u8,
            };
            match c >> 56 {
                LIVE => Readout::Live(inputs),
                STALE => Readout::Stale(inputs),
                RESTRICTED => Readout::Restricted,
                _ => Readout::Absent,
            }
        }),
        lap_distances: decode_values(words, LAP_BASE),
        delta_best: decode_values(words, BEST_BASE),
        delta_previous: decode_values(words, PREVIOUS_BASE),
    }
}

fn decode_values(words: &[u64; WORDS], base: usize) -> [Readout<f32>; MAX_CARS] {
    std::array::from_fn(|index| {
        let word = words[base + index];
        let value = f32::from_bits(word as u32);
        match word >> 56 {
            LIVE => Readout::Live(value),
            STALE => Readout::Stale(value),
            RESTRICTED => Readout::Restricted,
            _ => Readout::Absent,
        }
    })
}
