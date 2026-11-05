//! What the overlay shows, from one fast frame and the latest session model. UI-free so
//! the rules are tested without a window (PEDALS-01/02).

use std::sync::Arc;

use pitwall_core::{
    feed::{FastFrame, Reader},
    model::{LapTrace, Readout, Reference, SessionModel},
    traces::{InputHistory, InputSample, InputTrace, input_trace},
};

/// Shown for a value the game has not sent (the readout rule's absent text).
pub const ABSENT_TEXT: &str = "—";

/// One pedal bar.
#[derive(Debug, Clone, PartialEq)]
pub struct BarView {
    /// Whole percent, or [`ABSENT_TEXT`].
    pub text: String,
    /// Fill height, 0 to 1.
    pub fill: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OverlayView {
    /// A car is followed and its inputs are live or stale; otherwise the bars are empty and
    /// no trace is drawn.
    pub showing: bool,
    /// The inputs are stale: draw everything at half opacity.
    pub stale: bool,
    pub throttle: BarView,
    pub brake: BarView,
}

/// The car the overlay follows: the player or spectated car while it is a participant.
/// Dashboard focus and pins never apply; a missing player is not car 0.
pub fn overlay_car(model: &SessionModel) -> Option<usize> {
    let index = model.player_car_index()?;
    let car = model.car(index)?;
    matches!(car.participant(), Readout::Live(_) | Readout::Stale(_)).then_some(index)
}

/// The best lap of `car`, drawn dim behind the live lap. Only that lap, never a fallback.
pub fn best_lap(model: &SessionModel, car: usize) -> Option<Arc<LapTrace>> {
    model.car(car)?.lap_trace(Reference::Best).cloned()
}

/// The overlay's rolling input history and the rules that turn frames into a view.
#[derive(Debug, Default)]
pub struct OverlayModel {
    history: InputHistory,
}

impl OverlayModel {
    /// Take one fast frame: record the followed car's sample and return the bars.
    pub fn update(&mut self, frame: &FastFrame, model: &SessionModel) -> OverlayView {
        let car = (frame.epoch == model.epoch())
            .then(|| overlay_car(model))
            .flatten();
        let inputs = car.map_or(Readout::Absent, |index| frame.cars[index]);
        let (showing, stale, throttle, brake) = match inputs {
            Readout::Live(i) => (true, false, percent(i.throttle), percent(i.brake)),
            Readout::Stale(i) => (true, true, percent(i.throttle), percent(i.brake)),
            Readout::Restricted | Readout::Absent => (false, false, f64::NAN, f64::NAN),
        };
        if let Some(index) = car {
            let distance = match frame.lap_distances[index] {
                Readout::Live(d) | Readout::Stale(d) => f64::from(d),
                _ => f64::NAN,
            };
            self.history.record(
                (model.epoch(), index),
                InputSample {
                    session_time: f64::from(frame.session_time),
                    lap_distance: distance,
                    throttle,
                    brake,
                    speed: match inputs {
                        Readout::Live(i) | Readout::Stale(i) => f64::from(i.speed),
                        _ => f64::NAN,
                    },
                },
            );
        } else {
            self.history.clear();
        }
        let bar = |value: f64| {
            if showing {
                BarView {
                    text: format!("{value}"),
                    fill: (value.min(100.0) / 100.0) as f32,
                }
            } else {
                BarView {
                    text: if matches!(inputs, Readout::Restricted) {
                        "Restricted"
                    } else {
                        ABSENT_TEXT
                    }
                    .to_owned(),
                    fill: 0.0,
                }
            }
        };
        OverlayView {
            showing,
            stale,
            throttle: bar(throttle),
            brake: bar(brake),
        }
    }

    /// The trace for a `width` × `height` box, with the best lap aligned by lap distance.
    pub fn trace(&self, width: f64, height: f64, reference: Option<&LapTrace>) -> InputTrace {
        input_trace(
            &self.history,
            width,
            height,
            reference.map(|r| r.samples.as_slice()),
        )
    }

    pub fn history(&self) -> &InputHistory {
        &self.history
    }
}

/// Whole percent, as the web frame carried it.
fn percent(fraction: f32) -> f64 {
    (f64::from(fraction) * 100.0).round()
}

/// Reads both feed paths and requests a repaint only when either changes. Slow updates
/// can change the player or reference lap while replay is paused.
pub struct OverlayController {
    reader: Reader,
    generations: Option<(u64, u64)>,
    model: OverlayModel,
    view: Option<OverlayView>,
    reference: Option<Arc<LapTrace>>,
}

impl OverlayController {
    pub fn new(reader: Reader) -> Self {
        Self {
            reader,
            generations: None,
            model: OverlayModel::default(),
            view: None,
            reference: None,
        }
    }

    /// Refresh the presentation. Returns whether it needs drawing.
    pub fn poll(&mut self) -> bool {
        let snapshot = self.reader.snapshot();
        let fast_generation = self.reader.fast_generation();
        if self.generations == Some((fast_generation, snapshot.generation)) {
            return false;
        }
        let previous = self.generations.map_or(0, |g| g.0);
        let batch = self.reader.fast_since(previous);
        if batch.overrun {
            self.model = OverlayModel::default();
        }
        let mut generation = previous;
        for frame in batch.frames {
            generation = frame.generation;
            self.view = Some(self.model.update(&frame, &snapshot));
        }
        if generation == previous {
            self.view = Some(self.model.update(&self.reader.fast(), &snapshot));
        }
        self.generations = Some((generation, snapshot.generation));
        self.reference = overlay_car(&snapshot).and_then(|car| best_lap(&snapshot, car));
        true
    }

    pub fn view(&self) -> Option<&OverlayView> {
        self.view.as_ref()
    }

    pub fn has_reference(&self) -> bool {
        self.reference.is_some()
    }

    pub fn trace(&self, width: f64, height: f64) -> InputTrace {
        self.model.trace(width, height, self.reference.as_deref())
    }
}
