use crate::model::LapSample;

/// Width of one lap-distance bucket, metres.
pub const BUCKET_METRES: f64 = 5.0;
/// A fall of lap distance this large is the start line, not a spin or a reverse.
const NEW_LAP_DROP_METRES: f64 = 500.0;
/// Samples further apart than this are not joined: a real break (dropped frames, a pit
/// teleport) must show as a gap, not a line through data nobody saw. The same bound the
/// model applies inside a reference lap (spec section 6).
const MAX_INTERPOLATION_METRES: f64 = 100.0;
const MAX_BUCKETS: usize = 20_000;

/// Lap samples on the 5 m bucket grid.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LapSamples {
    /// Metres.
    pub distance: Vec<f64>,
    /// km/h; `None` where the game withheld it.
    pub speed: Vec<Option<f64>>,
    /// Fractions, 0 to 1.
    pub throttle: Vec<Option<f64>>,
    pub brake: Vec<Option<f64>>,
}

#[derive(Debug, Clone, Copy)]
struct Bucket {
    speed: f64,
    throttle: f64,
    brake: f64,
}

/// The lap in progress, built from frame-rate samples (spec section 8). One slot per 5 m
/// bucket of lap distance, so memory stays flat however long the lap runs.
///
/// Each slot holds the value at the bucket's exact distance, linearly interpolated between
/// the samples either side of it. The model builds reference laps the same way, so the live
/// and reference lines share one representation whatever the telemetry rate.
#[derive(Debug, Clone)]
pub struct CurrentLap {
    version: u64,
    buckets: Vec<Option<Bucket>>,
    last_bucket: Option<usize>,
    /// The previous accepted sample, for interpolating the buckets up to the next one.
    last_distance: f64,
    last: Bucket,
    first_bucket: Option<usize>,
}

impl Default for CurrentLap {
    fn default() -> Self {
        Self {
            version: 0,
            buckets: vec![None; 2048],
            last_bucket: None,
            last_distance: f64::NAN,
            last: Bucket {
                speed: f64::NAN,
                throttle: f64::NAN,
                brake: f64::NAN,
            },
            first_bucket: None,
        }
    }
}

impl CurrentLap {
    pub fn new() -> Self {
        Self::default()
    }

    /// One sample; throttle and brake in percent, any of the three NaN when withheld.
    pub fn record(
        &mut self,
        distance: f64,
        throttle_percent: f64,
        brake_percent: f64,
        speed_kmh: f64,
    ) {
        // A negative lap distance is dropped, and so is NaN.
        if distance.is_nan() || distance < 0.0 {
            return;
        }
        let bucket = (distance / BUCKET_METRES).floor() as usize;
        if bucket >= MAX_BUCKETS {
            return;
        }
        if let Some(last) = self.last_bucket
            && (last as f64 - bucket as f64) * BUCKET_METRES > NEW_LAP_DROP_METRES
        {
            self.clear();
        }
        // NaN for the first sample of a lap.
        let gap = distance - self.last_distance;
        if gap == 0.0 {
            return; // standing still: nothing new along the lap
        }
        self.grow(bucket);
        self.first_bucket.get_or_insert(bucket);
        let sample = Bucket {
            speed: speed_kmh,
            throttle: throttle_percent / 100.0,
            brake: brake_percent / 100.0,
        };
        if gap > 0.0 && gap <= MAX_INTERPOLATION_METRES {
            // Every bucket boundary this step crossed, ending at the sample itself if it sits
            // on one. A NaN at either end interpolates to NaN, so a withheld channel breaks.
            let from = self.last_bucket.map_or(0, |b| b + 1);
            let last = self.last;
            for b in from..=bucket {
                let f = (b as f64 * BUCKET_METRES - self.last_distance) / gap;
                self.buckets[b] = Some(Bucket {
                    speed: last.speed + f * (sample.speed - last.speed),
                    throttle: last.throttle + f * (sample.throttle - last.throttle),
                    brake: last.brake + f * (sample.brake - last.brake),
                });
            }
        } else if distance == bucket as f64 * BUCKET_METRES {
            // No neighbour to interpolate from (first sample, a big jump), but it sits on a
            // boundary. A step back, as when a car spins, lands here too and rebases.
            self.buckets[bucket] = Some(sample);
        }
        self.last_distance = distance;
        self.last = sample;
        self.last_bucket = Some(bucket);
        self.version += 1;
    }

    /// Fill in what a lap-so-far trace from the model has that the live samples do not.
    pub fn seed(&mut self, trace: &[LapSample]) {
        for sample in trace {
            let bucket = (f64::from(sample.distance_m) / BUCKET_METRES).round();
            if !(0.0..MAX_BUCKETS as f64).contains(&bucket) {
                continue;
            }
            let bucket = bucket as usize;
            self.grow(bucket);
            if self.buckets[bucket].is_some() {
                continue;
            }
            let value = |v: Option<f32>| v.map_or(f64::NAN, f64::from);
            self.buckets[bucket] = Some(Bucket {
                speed: value(sample.speed_kph),
                throttle: value(sample.throttle),
                brake: value(sample.brake),
            });
        }
        self.version += 1;
    }

    /// True when the first sample of this lap was well past the line, so its start was never
    /// seen: the screen joined mid-lap and should seed the lap so far.
    pub fn joined_mid_lap(&self) -> bool {
        self.first_bucket.is_some_and(|b| b > 1)
    }

    pub fn reset(&mut self) {
        self.clear();
        self.version += 1;
    }

    /// Bumped on every change, so a render loop can tell the trace is stale.
    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn samples(&self) -> LapSamples {
        let mut out = LapSamples::default();
        let value = |v: f64| (!v.is_nan()).then_some(v);
        for (b, slot) in self.buckets.iter().enumerate() {
            let Some(slot) = slot else { continue };
            out.distance.push(b as f64 * BUCKET_METRES);
            out.speed.push(value(slot.speed));
            out.throttle.push(value(slot.throttle));
            out.brake.push(value(slot.brake));
        }
        out
    }

    fn clear(&mut self) {
        self.buckets.fill(None);
        self.last_distance = f64::NAN;
        self.last_bucket = None;
        self.first_bucket = None;
    }

    fn grow(&mut self, bucket: usize) {
        if bucket >= self.buckets.len() {
            let size = (bucket + 1).max(self.buckets.len() * 2).min(MAX_BUCKETS);
            self.buckets.resize(size, None);
        }
    }
}
