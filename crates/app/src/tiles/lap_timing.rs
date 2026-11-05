//! Lap timing (parity TIMING-01..04, partly): running clock, delta against the reference
//! lap, sectors, and the completed result held for five seconds of the next lap.
//! Port of `dashboard/src/tiles/types/lap-timing.tsx`. The TS declaration says
//! `rate: 'frame'`; natively the clock and delta come from the slow snapshot, which the
//! source publishes at up to 60 Hz.
//!

use gpui::{IntoElement, ParentElement, Styled, div};
use pitwall_core::{
    model::{CarView, Lifecycle, Readout, Reference, SessionModel},
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
    PersonalBest,
    Rival,
}

impl LapReference {
    /// `settings.reference`; anything unknown reads as Best and is left alone.
    pub fn of(settings: &Value) -> Self {
        match settings.get("reference").and_then(Value::as_str) {
            Some("previous") => Self::Previous,
            Some("personal-best") => Self::PersonalBest,
            Some("rival") => Self::Rival,
            _ => Self::Best,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Best => "Best lap",
            Self::Previous => "Previous lap",
            Self::PersonalBest => "Personal best",
            Self::Rival => "Rival",
        }
    }
    fn extra(self) -> bool {
        matches!(self, Self::PersonalBest | Self::Rival)
    }
    fn model(self) -> Reference {
        match self {
            Self::Best => Reference::Best,
            Self::Previous => Reference::Previous,
            Self::PersonalBest => Reference::PersonalBest,
            Self::Rival => Reference::Rival,
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
    pub lap_number: Option<u8>,
    pub lap_time_ms: u32,
    pub sectors_ms: [Option<u32>; 3],
}

#[derive(Debug, Clone, PartialEq)]
pub struct Inputs {
    pub reference: LapReference,
    pub sector_ranks: [Option<SectorRank>; 3],
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
    /// Resolve all session histories for sector ranks, excluding the displayed lap.
    pub fn from_model(
        model: &SessionModel,
        index: Option<usize>,
        reference: LapReference,
        size: (f32, f32),
    ) -> Self {
        let mut inputs = Self::of(
            index.and_then(|i| model.car(i)),
            reference,
            model.lifecycle(),
            size,
        );
        let Some((index, lap)) = index.zip(inputs.lap.value()) else {
            return inputs;
        };
        let held = inputs
            .completed
            .as_ref()
            .filter(|_| lap.time_ms < HOLD_RESULT_MS);
        let number = held.map_or(lap.number, |c| c.lap_number);
        let times = held.map_or(
            [
                Some(lap.sector1_ms).filter(|m| *m > 0),
                Some(lap.sector2_ms).filter(|m| *m > 0),
                None,
            ],
            |c| c.sectors_ms,
        );
        let history = model.car(index).unwrap().session_history();
        inputs.sector_ranks = std::array::from_fn(|sector| {
            let ms = times[sector]?;
            let valid = if let Some(done) = held {
                history
                    .laps
                    .iter()
                    .find(|l| l.lap_number == number)
                    .map_or(done.valid, |l| l.sector_valid[sector])
            } else {
                history
                    .sectors
                    .iter()
                    .find(|s| s.lap_number == number && usize::from(s.sector) == sector)
                    .map_or(!lap.invalid, |s| s.valid)
            };
            if !valid {
                return Some(SectorRank::Slower);
            }
            let mut personal = u32::MAX;
            let mut session = u32::MAX;
            for other in 0..pitwall_core::telemetry::MAX_CARS {
                let Some(car) = model.car(other) else {
                    continue;
                };
                let history = car.session_history();
                let candidates = history
                    .sectors
                    .iter()
                    .filter(|s| usize::from(s.sector) == sector && s.valid)
                    .map(|s| (s.lap_number, s.time_ms))
                    .chain(
                        history
                            .laps
                            .iter()
                            .filter(|l| l.sector_valid[sector])
                            .filter_map(|l| l.sectors_ms[sector].map(|ms| (l.lap_number, ms))),
                    );
                for (lap_number, candidate) in candidates {
                    if candidate == 0 || (other == index && lap_number >= number) {
                        continue;
                    }
                    session = session.min(candidate);
                    if other == index {
                        personal = personal.min(candidate);
                    }
                }
            }
            Some(if ms < session {
                SectorRank::SessionBest
            } else if ms <= personal {
                SectorRank::PersonalBest
            } else {
                SectorRank::Slower
            })
        });
        inputs
    }

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
                sector_ranks: [None; 3],
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
        let result = car
            .completed_result(reference.model())
            .filter(|_| in_first_sector);
        let completed = result.as_ref().map(|r| Completed {
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
        // A held result compares with the prior reference, even when that result
        // has just become the new best/previous lap.
        let held = result
            .as_ref()
            .filter(|_| lap.value().is_some_and(|l| l.time_ms < HOLD_RESULT_MS));
        let reference_lap = if reference.extra() {
            car.time_trial_reference(reference.model())
                .map(|tt| RefLap {
                    lap_number: None,
                    lap_time_ms: tt.lap_time_in_ms,
                    sectors_ms: [
                        tt.sector1_time_in_ms,
                        tt.sector2_time_in_ms,
                        tt.sector3_time_in_ms,
                    ]
                    .map(|ms| (ms > 0).then_some(ms)),
                })
        } else {
            match held {
                Some(result) => result.reference_lap_number.and_then(|number| {
                    laps.iter()
                        .find(|l| l.lap_number == number)
                        .map(|l| l.as_ref())
                }),
                None => car.reference_lap(reference.model()),
            }
            .map(|l| RefLap {
                lap_number: Some(l.lap_number),
                lap_time_ms: l.lap_time_ms,
                sectors_ms: l.sectors_ms,
            })
        };
        Self {
            reference,
            sector_ranks: [None; 3],
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectorRank {
    SessionBest,
    PersonalBest,
    Slower,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Sector {
    pub rank: Option<SectorRank>,
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
    pub reference_sectors: Option<[ReadoutView; 3]>,
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
        let rank = inputs.sector_ranks[s].filter(|_| sector_states[s].has_data());
        let glyph = match rank {
            Some(SectorRank::SessionBest) => " \u{25c6}",
            Some(SectorRank::PersonalBest) => " \u{25cf}",
            _ => "",
        };
        Sector {
            rank,
            time: readout_view(
                sector_states[s],
                &sector_ms[s].map_or(String::new(), |ms| format!("{}{}", clock(ms, false), glyph)),
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
                        .and_then(|r| r.lap_number)
                        .map_or("\u{2014}".into(), |n| n.to_string()),
                    inputs.reference.label()
                ),
                readout_view(reference_state, &clock(reference_time.unwrap_or(0), true)),
            )
        }),
        reference_sectors: (density == Density::Full && inputs.reference.extra()).then(|| {
            ref_sectors.map(|ms| {
                readout_view(
                    if ms.is_some() {
                        ReadoutState::Live
                    } else {
                        ReadoutState::Absent
                    },
                    &clock(ms.unwrap_or(0), false),
                )
            })
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
                .child(widgets::readout_coloured(
                    &s.time,
                    18.0,
                    s.rank.map(|rank| match rank {
                        SectorRank::SessionBest => crate::theme::session_best(),
                        SectorRank::PersonalBest => crate::theme::personal_best(),
                        SectorRank::Slower => crate::theme::slower(),
                    }),
                ))
                .child(widgets::readout(&s.delta, 14.0));
            if s.current {
                cell = cell.bg(crate::theme::meter_empty());
            }
            row = row.child(cell);
        }
        body = body.child(row);
    }
    if let Some(sectors) = &view.reference_sectors {
        let mut row = div().flex().gap_2().w_full();
        for (i, sector) in sectors.iter().enumerate() {
            row = row.child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .child(widgets::label(format!("REF S{}", i + 1)))
                    .child(widgets::readout(sector, 18.0)),
            );
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
            sector_ranks: [None; 3],
            lap,
            live_delta: Readout::Live(-432.1),
            completed: None,
            reference_lap: Some(RefLap {
                lap_number: Some(3),
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
    fn a_new_best_result_shows_the_prior_reference_used_for_its_delta() {
        use pitwall_core::{
            model::SessionModel,
            telemetry::{Packet, parse_datagram},
        };
        use std::{process::Command, time::Duration};

        // Assumed telemetry, not a game capture: lap 2 beats lap 1 by 200 ms.
        let output = Command::new("python")
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tools/telemetry/assumed_datagram.py"
            ))
            .args(["2026", "2", "--set", "header.session_uid=101"])
            .output()
            .unwrap();
        assert!(output.status.success());
        let mut packet = parse_datagram(&output.stdout).unwrap();
        let mut model = SessionModel::default();
        for (number, last_ms, at) in [(1, 0, 0.0), (2, 2000, 2.0), (3, 1800, 3.8)] {
            packet.header.session_time = at;
            let Packet::Lap(data) = &mut packet.packet else {
                unreachable!()
            };
            let lap = data.lap_data[0].as_mut().unwrap();
            lap.result_status = 2;
            lap.current_lap_num = number;
            lap.last_lap_time_in_ms = last_ms;
            lap.current_lap_time_in_ms = 100;
            model.apply(Duration::from_secs_f32(at), &packet);
        }
        let v = view(&Inputs::of(
            model.car(0),
            LapReference::Best,
            model.lifecycle(),
            (500.0, 300.0),
        ));
        assert_eq!(v.clock.text, "0:01.800");
        assert_eq!(v.delta.text, "\u{2212}0.200");
        let (label, time) = v.reference.unwrap();
        assert_eq!(label, "REF LAP 1 \u{b7} Best lap");
        assert_eq!(time.text, "0:02.000");
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
