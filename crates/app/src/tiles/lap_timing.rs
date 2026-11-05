//! Lap timing (parity TIMING-01..04, partly): running clock, delta against the reference
//! lap, sectors, and the completed result held for five seconds of the next lap.
//! Port of `dashboard/src/tiles/types/lap-timing.tsx`. The TS declaration says
//! `rate: 'frame'`; natively the clock and delta come from the slow snapshot, which the
//! source publishes at up to 60 Hz.
//!
//! Not ported yet: sector rank cues (session/personal best), Time Trial personal-best and
//! rival references (they read as Best here).

use gpui::{IntoElement, ParentElement, Styled, div};
use pitwall_core::{
    model::{CarView, Lifecycle, Readout, Reference},
    readout::{ReadoutState, ReadoutView, readout_view, whole_tile_message},
};
use serde_json::Value;

use super::{
    Densities, Density,
    format::{clock, signed},
    widgets,
};

pub const NAME: &str = "Lap timing";
pub const DENSITIES: Densities = Densities {
    regular: (300.0, 150.0),
    full: Some((400.0, 220.0)),
};
/// Show the official result briefly, then return to the next lap's clock and delta.
pub const HOLD_RESULT_MS: u32 = 5000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LapReference {
    Best,
    Previous,
}

impl LapReference {
    /// `settings.reference`; anything unknown reads as Best and is left alone.
    pub fn of(settings: &Value) -> Self {
        match settings.get("reference").and_then(Value::as_str) {
            Some("previous") => Self::Previous,
            _ => Self::Best,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Best => "Best lap",
            Self::Previous => "Previous lap",
        }
    }
    fn model(self) -> Reference {
        match self {
            Self::Best => Reference::Best,
            Self::Previous => Reference::Previous,
        }
    }
}

/// The running lap as Lap Data reports it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LapNow {
    pub number: u8,
    pub time_ms: u32,
    /// Zero-based.
    pub sector: u8,
    pub sector1_ms: u32,
    pub sector2_ms: u32,
    pub invalid: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Completed {
    pub lap_number: u8,
    pub lap_time_ms: u32,
    pub valid: bool,
    pub delta_ms: Option<i64>,
    pub sectors_ms: [Option<u32>; 3],
    pub sector_deltas_ms: [Option<i64>; 3],
}

#[derive(Debug, Clone, PartialEq)]
pub struct RefLap {
    pub lap_number: u8,
    pub lap_time_ms: u32,
    pub sectors_ms: [Option<u32>; 3],
}

#[derive(Debug, Clone, PartialEq)]
pub struct Inputs {
    pub reference: LapReference,
    pub lap: Readout<LapNow>,
    /// Signed ms against the reference lap; negative is ahead.
    pub live_delta: Readout<f32>,
    /// The just-completed lap, while the next lap is still in sector one with no S1.
    pub completed: Option<Completed>,
    pub reference_lap: Option<RefLap>,
    pub lifecycle: Lifecycle,
    pub width: f32,
    pub height: f32,
}

impl Inputs {
    /// Read one car of the model; `None` for an unbound car.
    pub fn of(
        car: Option<CarView<'_>>,
        reference: LapReference,
        lifecycle: Lifecycle,
        size: (f32, f32),
    ) -> Self {
        let (width, height) = size;
        let Some(car) = car else {
            return Self {
                reference,
                lap: Readout::Absent,
                live_delta: Readout::Absent,
                completed: None,
                reference_lap: None,
                lifecycle,
                width,
                height,
            };
        };
        let lap = car.lap().map(|l| LapNow {
            number: l.current_lap_num,
            time_ms: l.current_lap_time_in_ms,
            sector: l.sector,
            sector1_ms: u32::from(l.sector1_time_minutes_part) * 60_000
                + u32::from(l.sector1_time_ms_part),
            sector2_ms: u32::from(l.sector2_time_minutes_part) * 60_000
                + u32::from(l.sector2_time_ms_part),
            invalid: l.current_lap_invalid == 1,
        });
        let in_first_sector = lap.value().is_some_and(|l| l.sector == 0);
        let laps = &car.session_history().laps;
        let completed = car
            .completed_result(reference.model())
            .filter(|_| in_first_sector)
            .map(|r| Completed {
                sectors_ms: laps
                    .iter()
                    .find(|l| l.lap_number == r.lap_number)
                    .map_or([None; 3], |l| l.sectors_ms),
                lap_number: r.lap_number,
                lap_time_ms: r.lap_time_ms,
                valid: r.valid,
                delta_ms: r.delta_ms,
                sector_deltas_ms: r.sector_deltas_ms,
            });
        let reference_lap = car.reference_lap(reference.model()).map(|l| RefLap {
            lap_number: l.lap_number,
            lap_time_ms: l.lap_time_ms,
            sectors_ms: l.sectors_ms,
        });
        Self {
            reference,
            lap,
            live_delta: car.live_delta(reference.model()),
            completed,
            reference_lap,
            lifecycle,
            width,
            height,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Sector {
    pub time: ReadoutView,
    pub delta: ReadoutView,
    pub current: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct View {
    pub density: Density,
    pub delta_label: &'static str,
    pub delta: ReadoutView,
    /// Negative is ahead.
    pub delta_sign: Option<std::cmp::Ordering>,
    pub lap_label: String,
    pub clock: ReadoutView,
    pub invalid: bool,
    pub sectors: Option<[Sector; 3]>,
    /// Full density: label and time of the reference lap.
    pub reference: Option<(String, ReadoutView)>,
    pub message: Option<&'static str>,
}

pub fn view(inputs: &Inputs) -> View {
    let density = DENSITIES.pick(inputs.width, inputs.height);
    let lap_state = inputs.lap.state();
    let lap = inputs.lap.value();
    let finished = inputs
        .completed
        .as_ref()
        .filter(|_| lap.is_none_or(|l| l.time_ms < HOLD_RESULT_MS));

    let (clock_state, clock_ms) = match (finished, lap) {
        (Some(done), _) => (ReadoutState::Live, done.lap_time_ms),
        (None, Some(l)) => (lap_state, l.time_ms),
        (None, None) => (lap_state, 0),
    };
    let (delta_state, delta_ms) = match finished {
        Some(done) => match done.delta_ms {
            Some(d) => (ReadoutState::Live, Some(d as f64)),
            None => (ReadoutState::Absent, None),
        },
        None => (
            inputs.live_delta.state(),
            inputs.live_delta.value().map(|&d| f64::from(d)),
        ),
    };

    let running: [Option<u32>; 3] = match lap {
        Some(l) => [
            (l.sector > 0 || l.sector1_ms > 0)
                .then_some(l.sector1_ms)
                .filter(|&ms| ms > 0),
            Some(l.sector2_ms).filter(|&ms| ms > 0),
            None,
        ],
        None => [None; 3],
    };
    let sector_ms = finished.map_or(running, |done| done.sectors_ms);
    let missing_state = if matches!(lap_state, ReadoutState::Restricted | ReadoutState::Absent) {
        lap_state
    } else {
        ReadoutState::Absent
    };
    let sector_states: [ReadoutState; 3] = std::array::from_fn(|s| match sector_ms[s] {
        None => missing_state,
        Some(_) if finished.is_some() => ReadoutState::Live,
        Some(_) => lap_state,
    });
    let ref_sectors = inputs
        .reference_lap
        .as_ref()
        .map_or([None; 3], |r| r.sectors_ms);
    let sectors = std::array::from_fn(|s| {
        let delta = match finished {
            Some(done) => done.sector_deltas_ms[s],
            None => sector_ms[s]
                .zip(ref_sectors[s].filter(|&r| r > 0))
                .map(|(ms, r)| i64::from(ms) - i64::from(r)),
        };
        Sector {
            time: readout_view(
                sector_states[s],
                &sector_ms[s].map_or(String::new(), |ms| clock(ms, false)),
            ),
            delta: readout_view(
                if delta.is_some() {
                    sector_states[s]
                } else {
                    ReadoutState::Absent
                },
                &signed(delta.unwrap_or(0) as f64 / 1000.0, 3),
            ),
            current: finished.is_none() && lap.is_some_and(|l| usize::from(l.sector) == s),
        }
    });

    let reference_time = inputs.reference_lap.as_ref().map(|r| r.lap_time_ms);
    let reference_state = if reference_time.is_some() {
        ReadoutState::Live
    } else {
        ReadoutState::Absent
    };
    let mut states = vec![clock_state, delta_state];
    if density != Density::Compact {
        states.extend(sector_states);
    }
    if density == Density::Full && reference_time.is_some() {
        states.push(reference_state);
    }

    View {
        density,
        delta_label: inputs.reference.label(),
        delta: readout_view(delta_state, &signed(delta_ms.unwrap_or(0.0) / 1000.0, 3)),
        delta_sign: delta_ms
            .filter(|_| delta_state.has_data())
            .map(|d| d.total_cmp(&0.0)),
        lap_label: match (finished, lap) {
            (Some(done), _) => format!("Lap {} complete", done.lap_number),
            (None, Some(l)) => format!("Lap {}", l.number),
            (None, None) => "Lap time".into(),
        },
        clock: readout_view(clock_state, &clock(clock_ms, true)),
        invalid: match finished {
            Some(done) => !done.valid,
            None => lap.is_some_and(|l| l.invalid),
        },
        sectors: (density != Density::Compact).then_some(sectors),
        reference: (density == Density::Full).then(|| {
            (
                format!(
                    "REF LAP {} \u{b7} {}",
                    inputs
                        .reference_lap
                        .as_ref()
                        .map_or("\u{2014}".into(), |r| r.lap_number.to_string()),
                    inputs.reference.label()
                ),
                readout_view(reference_state, &clock(reference_time.unwrap_or(0), true)),
            )
        }),
        message: whole_tile_message(states, inputs.lifecycle, true),
    }
}

pub fn render(view: View) -> impl IntoElement {
    use std::cmp::Ordering;
    let delta_colour = match view.delta_sign {
        _ if view.delta.state != ReadoutState::Live => None,
        Some(Ordering::Less) => Some(crate::theme::personal_best()),
        Some(Ordering::Greater) => Some(crate::theme::slower()),
        _ => None,
    };
    let delta = div()
        .flex()
        .flex_col()
        .child(widgets::label(view.delta_label))
        .child(widgets::readout_coloured(&view.delta, 30.0, delta_colour));
    let time = div()
        .flex()
        .flex_col()
        .items_end()
        .child(widgets::label(view.lap_label))
        .child(widgets::readout(&view.clock, 30.0))
        .child(widgets::label(if view.invalid { "Invalid" } else { "" }));
    let mut body = div()
        .flex()
        .flex_col()
        .gap_3()
        .size_full()
        .child(div().flex().justify_between().child(delta).child(time));
    if let Some(sectors) = &view.sectors {
        let mut row = div().flex().gap_2().w_full();
        for (i, s) in sectors.iter().enumerate() {
            let mut cell = div()
                .flex_1()
                .flex()
                .flex_col()
                .p_1()
                .rounded_sm()
                .child(widgets::label(["S1", "S2", "S3"][i]))
                .child(widgets::readout(&s.time, 18.0))
                .child(widgets::readout(&s.delta, 14.0));
            if s.current {
                cell = cell.bg(crate::theme::meter_empty());
            }
            row = row.child(cell);
        }
        body = body.child(row);
    }
    if let Some((label, time)) = &view.reference {
        body = body.child(
            div()
                .flex()
                .justify_between()
                .items_baseline()
                .child(widgets::label(label.clone()))
                .child(widgets::readout(time, 18.0)),
        );
    }
    widgets::tile_shell(body, view.message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pitwall_core::readout::ABSENT_TEXT;
    use serde_json::json;

    fn running(time_ms: u32) -> LapNow {
        LapNow {
            number: 5,
            time_ms,
            sector: 0,
            sector1_ms: 0,
            sector2_ms: 0,
            invalid: false,
        }
    }

    fn base(lap: Readout<LapNow>) -> Inputs {
        Inputs {
            reference: LapReference::Best,
            lap,
            live_delta: Readout::Live(-432.1),
            completed: None,
            reference_lap: Some(RefLap {
                lap_number: 3,
                lap_time_ms: 90_000,
                sectors_ms: [Some(30_000), Some(31_000), Some(29_000)],
            }),
            lifecycle: Lifecycle::Live,
            width: 500.0,
            height: 300.0,
        }
    }

    fn completed() -> Completed {
        Completed {
            lap_number: 4,
            lap_time_ms: 89_500,
            valid: true,
            delta_ms: Some(-500),
            sectors_ms: [Some(29_800), Some(30_900), Some(28_800)],
            sector_deltas_ms: [Some(-200), Some(-100), Some(-200)],
        }
    }

    #[test]
    fn shows_the_lap_so_far_and_the_signed_live_delta() {
        let v = view(&base(Readout::Live(running(23_456))));
        assert_eq!(v.clock.text, "0:23.456");
        assert_eq!(v.delta.text, "\u{2212}0.432");
        assert_eq!(v.delta_label, "Best lap");
        assert_eq!(v.lap_label, "Lap 5");
        assert_eq!(v.delta_sign, Some(std::cmp::Ordering::Less));
    }

    #[test]
    fn reference_setting_is_best_by_default_previous_when_set_unknown_reads_best() {
        assert_eq!(LapReference::of(&json!({})), LapReference::Best);
        assert_eq!(
            LapReference::of(&json!({"reference": "previous"})),
            LapReference::Previous
        );
        assert_eq!(
            LapReference::of(&json!({"reference": "someday"})),
            LapReference::Best
        );
    }

    #[test]
    fn the_four_readout_states_of_the_delta_and_lap_time() {
        let stale = view(&Inputs {
            live_delta: Readout::Stale(100.0),
            ..base(Readout::Stale(running(1000)))
        });
        assert_eq!(stale.clock.state, ReadoutState::Stale);
        assert_eq!(stale.delta.text, "+0.100");
        let absent = view(&Inputs {
            live_delta: Readout::Absent,
            reference_lap: None,
            ..base(Readout::Absent)
        });
        assert_eq!(absent.clock.text, ABSENT_TEXT);
        assert_eq!(absent.delta.text, ABSENT_TEXT);
        assert_eq!(absent.message, Some("No data"));
        // A known reference lap time is data, so the full tile keeps its values.
        let with_reference = view(&Inputs {
            live_delta: Readout::Absent,
            ..base(Readout::Absent)
        });
        assert_eq!(with_reference.message, None);
    }

    #[test]
    fn an_invalid_lap_keeps_its_delta_and_says_invalid() {
        let mut lap = running(40_000);
        lap.invalid = true;
        let v = view(&base(Readout::Live(lap)));
        assert!(v.invalid);
        assert_eq!(v.delta.state, ReadoutState::Live);
    }

    #[test]
    fn completed_sectors_show_time_and_delta_the_rest_wait() {
        let mut lap = running(50_000);
        lap.sector = 1;
        lap.sector1_ms = 30_250;
        let v = view(&base(Readout::Live(lap)));
        let sectors = v.sectors.unwrap();
        assert_eq!(sectors[0].time.text, "30.250");
        assert_eq!(sectors[0].delta.text, "+0.250");
        assert_eq!(sectors[1].time.text, ABSENT_TEXT);
        assert!(sectors[1].current);
    }

    #[test]
    fn the_completed_result_replaces_the_running_lap_for_five_seconds() {
        let held = view(&Inputs {
            completed: Some(completed()),
            ..base(Readout::Live(running(4_999)))
        });
        assert_eq!(held.clock.text, "1:29.500");
        assert_eq!(held.delta.text, "\u{2212}0.500");
        assert_eq!(held.lap_label, "Lap 4 complete");
        assert_eq!(held.sectors.unwrap()[2].delta.text, "\u{2212}0.200");

        let expired = view(&Inputs {
            completed: Some(completed()),
            ..base(Readout::Live(running(5_000)))
        });
        assert_eq!(expired.clock.text, "0:05.000");
        assert_eq!(expired.lap_label, "Lap 5");
        assert_eq!(expired.sectors.unwrap()[2].time.text, ABSENT_TEXT);
    }

    #[test]
    fn an_invalid_completed_lap_keeps_its_mark() {
        let v = view(&Inputs {
            completed: Some(Completed {
                valid: false,
                ..completed()
            }),
            ..base(Readout::Live(running(100)))
        });
        assert!(v.invalid);
    }

    #[test]
    fn compact_keeps_delta_and_time_regular_adds_sectors_full_adds_reference() {
        let at = |width, height| {
            view(&Inputs {
                width,
                height,
                ..base(Readout::Live(running(1)))
            })
        };
        let compact = at(200.0, 100.0);
        assert!(compact.sectors.is_none() && compact.reference.is_none());
        let regular = at(300.0, 150.0);
        assert!(regular.sectors.is_some() && regular.reference.is_none());
        let full = at(400.0, 220.0);
        let (label, time) = full.reference.unwrap();
        assert_eq!(label, "REF LAP 3 \u{b7} Best lap");
        assert_eq!(time.text, "1:30.000");
    }
}
