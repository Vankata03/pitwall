//! Per-car watchers: drift away from a frozen line and passes through the pit lane.

/// How far a car may sit from the stored line before it counts as off it, in metres.
/// Provisional until calibration against recordings (#52).
pub const MISMATCH_METRES: f32 = 30.0;

/// The longest step between two samples of one run, in metres. A longer jump is a
/// flashback, a teleport or lost packets, and breaks the run.
const MAX_STEP_METRES: f32 = 100.0;

/// The shortest stretch of driving that can be a whole pit lane, in metres.
const MIN_PIT_LANE_METRES: f32 = 100.0;

/// Pit trace points are kept at least this far apart, in metres.
const PIT_SPACING_METRES: f32 = 2.0;

fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// One car's run off a frozen line.
#[derive(Debug, Default, Clone)]
pub(super) struct Drift {
    last_distance: Option<f32>,
    off_metres: f32,
}

impl Drift {
    /// Take one accepted sample. Returns true once the car has sat off the line across a
    /// whole lap.
    pub(super) fn observe(&mut self, lap_distance: f32, off_line: bool, track_length: f32) -> bool {
        let mut step = self.last_distance.map(|last| lap_distance - last);
        if let Some(delta) = step.as_mut().filter(|delta| **delta < -track_length / 2.0) {
            *delta += track_length;
        }
        self.last_distance = Some(lap_distance);
        match step {
            Some(delta) if off_line && (0.0..=MAX_STEP_METRES).contains(&delta) => {
                self.off_metres += delta;
            }
            _ => self.off_metres = 0.0,
        }
        self.off_metres >= track_length
    }
}

/// One car's trip through the pit lane.
#[derive(Debug, Default, Clone)]
pub(super) struct PitWatch {
    on_track: bool,
    pass: Option<Vec<[f32; 2]>>,
    length: f32,
}

impl PitWatch {
    /// Take one sample. Returns the pit trace when the car has just driven the whole lane,
    /// entry to exit. Passes that start inside the lane or break off are discarded.
    pub(super) fn observe(&mut self, at: [f32; 2], in_pit: bool) -> Option<Vec<[f32; 2]>> {
        if !in_pit {
            self.on_track = true;
            let pass = self.pass.take()?;
            return (self.length >= MIN_PIT_LANE_METRES && pass.len() >= 2).then_some(pass);
        }
        match &mut self.pass {
            None if self.on_track => {
                self.pass = Some(vec![at]);
                self.length = 0.0;
            }
            None => {}
            Some(points) => {
                let last = *points.last().expect("a pass has a first point");
                let step = distance(last, at);
                if step > MAX_STEP_METRES {
                    self.pass = None;
                } else if step >= PIT_SPACING_METRES {
                    self.length += step;
                    points.push(at);
                }
            }
        }
        self.on_track = false;
        None
    }
}
