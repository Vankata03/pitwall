//! Stint (parity STINT-01/02): a slow tile following one car. The current tyre set
//! first, then the completed sets newest first (each expands to its detail), then the pit
//! stops. Port of `dashboard/src/tiles/types/stint.tsx`.

use std::{cell::RefCell, collections::HashSet, rc::Rc};

use gpui::{
    AnyElement, Div, FontWeight, Hsla, InteractiveElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, px, relative,
};
use pitwall_core::{
    model::{Lifecycle, PitWindow, Readout, SessionModel},
    readout::{ReadoutState, ReadoutView, readout_view, view_of},
};

use super::{Densities, Density, format::fixed, widgets};
use crate::theme;

pub const TYPE: &str = "stint";
pub const NAME: &str = "Stint";
/// Both densities show the same content.
pub const DENSITIES: Densities = Densities {
    regular: (300.0, 240.0),
    full: None,
};

/// One tyre stint as the core's history keeps it.
#[derive(Debug, Clone, PartialEq)]
pub struct StintFacts {
    pub id: u64,
    pub start_lap: u8,
    /// `None` while the set is fitted.
    pub end_lap: Option<u8>,
    pub visual_compound: u8,
    pub starting_age: Option<u8>,
    pub tyre_age_laps: Option<u8>,
    /// Last observed condition on this set, percent remaining, RL RR FL FR.
    pub tyre_condition: Readout<[f32; 4]>,
}

/// One pit stop as the core's history keeps it.
#[derive(Debug, Clone, PartialEq)]
pub struct StopFacts {
    pub id: u64,
    pub lap_number: u8,
    /// Still in the pit lane (no exit yet).
    pub in_lane: bool,
    pub lane_time_ms: Option<u32>,
    pub stopped_time_ms: Option<u32>,
}

/// The fitted set as Car Status reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tyres {
    pub visual_compound: u8,
    pub age_laps: u8,
}

pub struct Inputs {
    /// The bound car is in the session; `false` shows "No stint data".
    pub present: bool,
    pub retired: bool,
    pub current_lap: Readout<u8>,
    pub tyres: Readout<Tyres>,
    /// Current condition: once restricted, history wear is withheld too.
    pub tyre_condition: Readout<[f32; 4]>,
    pub stints: Vec<StintFacts>,
    pub stops: Vec<StopFacts>,
    /// The game's pit window: only for the player car, absent otherwise.
    pub pit_window: Readout<PitWindow>,
    pub lifecycle: Lifecycle,
    pub width: f32,
    pub height: f32,
}

impl Inputs {
    pub fn from_model(
        model: &SessionModel,
        index: Option<usize>,
        retired: bool,
        size: (f32, f32),
    ) -> Self {
        let car = index.and_then(|i| model.car(i));
        let history = car.map(|c| c.session_history());
        Self {
            present: car.is_some(),
            retired,
            current_lap: car.map_or(Readout::Absent, |c| c.lap().map(|l| l.current_lap_num)),
            tyres: car.map_or(Readout::Absent, |c| {
                c.status().map(|s| Tyres {
                    visual_compound: s.visual_tyre_compound,
                    age_laps: s.tyres_age_laps,
                })
            }),
            tyre_condition: car.map_or(Readout::Absent, |c| c.tyre_condition()),
            stints: history.map_or_else(Vec::new, |h| {
                h.stints
                    .iter()
                    .map(|s| StintFacts {
                        id: s.id,
                        start_lap: s.start_lap,
                        end_lap: s.end_lap,
                        visual_compound: s.visual_compound,
                        starting_age: s.starting_age,
                        tyre_age_laps: s.tyre_age_laps,
                        tyre_condition: s.tyre_condition,
                    })
                    .collect()
            }),
            stops: history.map_or_else(Vec::new, |h| {
                h.pit_stops
                    .iter()
                    .map(|s| StopFacts {
                        id: s.id,
                        lap_number: s.lap_number,
                        in_lane: s.exit_time.is_none(),
                        lane_time_ms: s.lane_time_ms,
                        stopped_time_ms: s.stopped_time_ms,
                    })
                    .collect()
            }),
            pit_window: index.map_or(Readout::Absent, |i| model.pit_window(i).map(|w| *w)),
            lifecycle: model.lifecycle(),
            width: size.0,
            height: size.1,
        }
    }
}

/// Colour family of a visual compound; the compound is always named in text too.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    Soft,
    Medium,
    Hard,
    Inter,
    Wet,
    Unknown,
}

fn family(compound: u8) -> Family {
    match compound {
        16 | 19 | 20 => Family::Soft,
        17 | 21 => Family::Medium,
        18 | 22 => Family::Hard,
        7 => Family::Inter,
        8 => Family::Wet,
        _ => Family::Unknown,
    }
}

fn symbol(family: Family) -> &'static str {
    match family {
        Family::Soft => "S",
        Family::Medium => "M",
        Family::Hard => "H",
        Family::Inter => "I",
        Family::Wet => "W",
        Family::Unknown => "?",
    }
}

fn compound_name(compound: u8) -> String {
    match compound {
        16 | 20 => "Soft",
        17 | 21 => "Medium",
        18 | 22 => "Hard",
        7 => "Intermediate",
        8 => "Wet",
        19 => "Super soft",
        other => return format!("Compound {other}"),
    }
    .to_string()
}

fn set_label(starting_age: Option<u8>) -> String {
    match starting_age {
        None => String::new(),
        Some(age) if age > 0 => format!("Used set \u{b7} starting age {age}"),
        Some(_) => "New set".to_string(),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Wheel {
    pub wheel: &'static str,
    /// Wear percent, `40%`.
    pub wear: ReadoutView,
    /// The bar's fill, 0 to 1.
    pub fill: f32,
}

/// Per-wheel wear in FL FR RL RR order, from condition in RL RR FL FR order.
fn wheels(condition: &[f32; 4], state: ReadoutState) -> Vec<Wheel> {
    ["FL", "FR", "RL", "RR"]
        .into_iter()
        .zip([2, 3, 0, 1])
        .map(|(wheel, at)| {
            let wear = (100.0 - f64::from(condition[at])).round().clamp(0.0, 100.0);
            Wheel {
                wheel,
                wear: readout_view(state, &format!("{wear}%")),
                fill: wear as f32 / 100.0,
            }
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
pub enum Wear {
    Wheels(Vec<Wheel>),
    /// Restricted or absent: the state's text instead of the wheels.
    Missing(ReadoutView),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Current {
    pub family: Family,
    pub symbol: &'static str,
    pub name: String,
    pub set_label: String,
    /// Completed laps on this set, two digits; an en dash without lap data.
    pub laps_done: String,
    pub tyre_age: ReadoutView,
    /// `Laps 11–14 · current`.
    pub caption: String,
    pub wear: Wear,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Past {
    pub id: u64,
    pub family: Family,
    pub symbol: &'static str,
    pub name: String,
    /// `Laps 1–10 · 10 laps`.
    pub range: String,
    /// `50% max wear`.
    pub max_wear: ReadoutView,
    pub set_label: String,
    /// `Final tyre age 12`.
    pub final_age: String,
    /// Shown when expanded and the wear has a value.
    pub wheels: Option<Vec<Wheel>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stop {
    pub id: u64,
    /// `Stop · lap 10`.
    pub label: String,
    pub in_lane: bool,
    pub stopped: ReadoutView,
    pub lane: ReadoutView,
}

#[derive(Debug, Clone, PartialEq)]
pub struct View {
    pub density: Density,
    /// `Current stint`, with `· retired` for a retired car.
    pub title: String,
    /// `1 stop` / `2 stops`.
    pub stop_count: String,
    pub current: Option<Current>,
    pub pit_window: Option<ReadoutView>,
    /// Completed sets, newest first.
    pub past: Vec<Past>,
    /// "No stint history" or "No previous sets recorded".
    pub history_note: Option<&'static str>,
    pub stops: Vec<Stop>,
    /// "No stint data" when the bound car is not in the session.
    pub message: Option<&'static str>,
    pub size: (f32, f32),
}

fn seconds(ms: Option<u32>, stale: bool) -> ReadoutView {
    match ms {
        Some(ms) => readout_view(
            if stale {
                ReadoutState::Stale
            } else {
                ReadoutState::Live
            },
            &format!("{} s", fixed(f64::from(ms) / 1000.0, 3)),
        ),
        None => readout_view(ReadoutState::Absent, ""),
    }
}

fn dim(mut view: ReadoutView, stale: bool) -> ReadoutView {
    if stale && view.state == ReadoutState::Live {
        view.state = ReadoutState::Stale;
    }
    view
}

pub fn view(inputs: &Inputs) -> View {
    let stale = inputs.lifecycle == Lifecycle::Lost;
    let mut stints = inputs.stints.clone();
    stints.sort_by_key(|s| s.start_lap);
    let mut stops = inputs.stops.clone();
    stops.sort_by_key(|s| s.lap_number);
    // Never expose a historical value after the car switches telemetry to restricted.
    let condition = |r: &Readout<[f32; 4]>| {
        if inputs.tyre_condition == Readout::Restricted {
            Readout::Restricted
        } else {
            *r
        }
    };
    let lap = inputs.current_lap.value().copied();
    let laps_of = |s: &StintFacts| {
        let end = s
            .end_lap
            .map(i32::from)
            .unwrap_or_else(|| lap.map_or(i32::from(s.start_lap) - 1, |l| i32::from(l) - 1));
        (end - i32::from(s.start_lap) + 1).max(0)
    };
    let current = stints.iter().find(|s| s.end_lap.is_none()).map(|running| {
        let compound = inputs
            .tyres
            .value()
            .map_or(running.visual_compound, |t| t.visual_compound);
        let fam = family(compound);
        Current {
            family: fam,
            symbol: symbol(fam),
            name: compound_name(compound),
            set_label: set_label(running.starting_age),
            laps_done: lap.map_or("\u{2013}".to_string(), |_| {
                format!("{:02}", laps_of(running))
            }),
            tyre_age: dim(view_of(&inputs.tyres, |t| t.age_laps.to_string()), stale),
            caption: format!(
                "Laps {}\u{2013}{} \u{b7} current",
                running.start_lap,
                lap.map_or("\u{2014}".to_string(), |l| l.to_string())
            ),
            wear: match &condition(&inputs.tyre_condition) {
                Readout::Live(c) => Wear::Wheels(wheels(c, dim_state(ReadoutState::Live, stale))),
                Readout::Stale(c) => Wear::Wheels(wheels(c, ReadoutState::Stale)),
                other => Wear::Missing(view_of(other, |_| String::new())),
            },
        }
    });
    let past: Vec<Past> = stints
        .iter()
        .filter(|s| s.end_lap.is_some())
        .rev()
        .map(|s| {
            let wear = condition(&s.tyre_condition);
            let fam = family(s.visual_compound);
            let laps = laps_of(s);
            Past {
                id: s.id,
                family: fam,
                symbol: symbol(fam),
                name: compound_name(s.visual_compound),
                range: format!(
                    "Laps {}\u{2013}{} \u{b7} {laps} {}",
                    s.start_lap,
                    s.end_lap.unwrap_or_default(),
                    if laps == 1 { "lap" } else { "laps" }
                ),
                max_wear: dim(
                    view_of(&wear, |c| {
                        let worst = c.iter().copied().fold(f32::INFINITY, f32::min);
                        format!("{}% max wear", (100.0 - f64::from(worst)).round())
                    }),
                    stale,
                ),
                set_label: set_label(s.starting_age),
                final_age: format!(
                    "Final tyre age {}",
                    s.tyre_age_laps
                        .map_or("\u{2014}".to_string(), |a| a.to_string())
                ),
                wheels: wear
                    .value()
                    .map(|c| wheels(c, dim_state(wear.state(), stale))),
            }
        })
        .collect();
    let history_note = if stints.is_empty() {
        Some("No stint history")
    } else if past.is_empty() {
        Some("No previous sets recorded")
    } else {
        None
    };
    let stop_count = format!(
        "{} {}",
        stops.len(),
        if stops.len() == 1 { "stop" } else { "stops" }
    );
    View {
        density: DENSITIES.pick(inputs.width, inputs.height),
        title: format!(
            "Current stint{}",
            if inputs.retired {
                " \u{b7} retired"
            } else {
                ""
            }
        ),
        stop_count,
        current,
        pit_window: (inputs.pit_window != Readout::Absent).then(|| {
            dim(
                view_of(&inputs.pit_window, |w| {
                    format!(
                        "Laps {}\u{2013}{} \u{b7} rejoin P{}",
                        w.ideal_lap, w.latest_lap, w.rejoin_position
                    )
                }),
                stale,
            )
        }),
        past,
        history_note,
        stops: stops
            .iter()
            .map(|s| Stop {
                id: s.id,
                label: format!("Stop \u{b7} lap {}", s.lap_number),
                in_lane: s.in_lane,
                stopped: seconds(s.stopped_time_ms, stale),
                lane: seconds(s.lane_time_ms, stale),
            })
            .collect(),
        message: (!inputs.present).then_some("No stint data"),
        size: (inputs.width, inputs.height),
    }
}

fn dim_state(state: ReadoutState, stale: bool) -> ReadoutState {
    if stale && state == ReadoutState::Live {
        ReadoutState::Stale
    } else {
        state
    }
}

/// Which completed sets this placed tile shows expanded, by stint id.
#[derive(Default)]
pub struct Memory {
    open: Rc<RefCell<HashSet<u64>>>,
}

impl Memory {
    /// Expand a collapsed set, collapse an expanded one.
    pub fn toggle(&self, id: u64) {
        let mut open = self.open.borrow_mut();
        if !open.remove(&id) {
            open.insert(id);
        }
    }

    pub fn is_open(&self, id: u64) -> bool {
        self.open.borrow().contains(&id)
    }

    fn handle(&self) -> Self {
        Self {
            open: self.open.clone(),
        }
    }
}

fn family_colour(family: Family) -> Hsla {
    match family {
        Family::Soft => theme::brake(),
        Family::Medium => theme::slower(),
        Family::Hard => theme::window(),
        Family::Inter => theme::personal_best(),
        Family::Wet => theme::cold(),
        Family::Unknown => theme::muted(),
    }
}

fn ring(symbol: &'static str, colour: Hsla, size: f32) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(size))
        .rounded_full()
        .border(px(if size > 26.0 { 3.0 } else { 2.0 }))
        .border_color(colour)
        .text_color(colour)
        .text_size(px(if size > 26.0 { 18.0 } else { 14.0 }))
        .font_weight(FontWeight::BOLD)
        .child(symbol)
}

fn wheels_row(wheels: &[Wheel], colour: Hsla) -> Div {
    let mut row = div().flex().gap_3().w_full();
    for w in wheels {
        row = row.child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .gap(px(3.0))
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .items_baseline()
                        .child(widgets::label(w.wheel))
                        .child(widgets::readout(&w.wear, 14.0)),
                )
                .child(
                    div()
                        .h(px(3.0))
                        .w_full()
                        .rounded_sm()
                        .bg(theme::meter_empty())
                        .child(div().h_full().w(relative(w.fill)).bg(colour)),
                ),
        );
    }
    row
}

fn metric(value: Div, label: &'static str) -> Div {
    div()
        .flex()
        .flex_col()
        .items_end()
        .gap(px(2.0))
        .child(value)
        .child(widgets::label(label))
}

fn line_top(body: Div) -> Div {
    body.border_t_1().border_color(theme::line()).py_1()
}

pub fn render(view: View, memory: &mut Memory) -> AnyElement {
    if let Some(message) = view.message {
        return widgets::tile_shell(div(), Some(message));
    }
    let compact = view.size.1 < 280.0;
    let mut body = div()
        .flex()
        .flex_col()
        .gap_2()
        .size_full()
        .overflow_hidden()
        .text_size(px(14.0))
        .text_color(theme::text())
        .child(
            div()
                .flex()
                .justify_between()
                .items_baseline()
                .child(widgets::label(view.title.clone()).font_weight(FontWeight::SEMIBOLD))
                .child(widgets::label(view.stop_count.clone())),
        );
    if let Some(current) = &view.current {
        let colour = family_colour(current.family);
        let mut active = div()
            .flex()
            .flex_none()
            .flex_col()
            .gap(px(if compact { 4.0 } else { 8.0 }))
            .p(px(if compact { 6.0 } else { 10.0 }))
            .rounded_md()
            .bg(theme::raised())
            .border_l(px(3.0))
            .border_color(colour)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(ring(current.symbol, colour, 32.0))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .child(SharedString::from(current.name.clone())),
                            )
                            .child(widgets::label(current.set_label.clone())),
                    )
                    .child(metric(
                        div()
                            .text_size(px(if compact { 20.0 } else { 24.0 }))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(SharedString::from(current.laps_done.clone())),
                        "Laps done",
                    ))
                    .child(metric(
                        widgets::readout(&current.tyre_age, if compact { 20.0 } else { 24.0 }),
                        "Tyre age",
                    )),
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .gap_2()
                    .child(widgets::label(current.caption.clone()))
                    .child(widgets::label("Tyre wear")),
            );
        active = active.child(match &current.wear {
            Wear::Wheels(w) => wheels_row(w, colour),
            Wear::Missing(v) => widgets::readout(v, 14.0),
        });
        body = body.child(active);
    }
    if let Some(window) = &view.pit_window {
        body = body.child(
            div()
                .flex()
                .flex_none()
                .justify_between()
                .items_baseline()
                .gap_2()
                .child(widgets::label("Game pit window"))
                .child(widgets::readout(window, 14.0)),
        );
    }
    let mut detail = div()
        .id("stint-detail")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .overflow_y_scroll();
    for past in &view.past {
        let colour = family_colour(past.family);
        let expanded = memory.is_open(past.id);
        let toggle = memory.handle();
        let id = past.id;
        let mut item = line_top(div().flex().flex_col()).child(
            div()
                .id(("stint-past", past.id))
                .flex()
                .items_center()
                .gap(px(10.0))
                .min_h(px(if compact { 26.0 } else { 28.0 }))
                .cursor_pointer()
                .on_click(move |_, window, _| {
                    toggle.toggle(id);
                    window.refresh();
                })
                .child(ring(past.symbol, colour, 24.0))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .flex_1()
                        .min_w_0()
                        .items_baseline()
                        .gap_x(px(10.0))
                        .child(
                            div()
                                .font_weight(FontWeight::BOLD)
                                .child(SharedString::from(past.name.clone())),
                        )
                        .child(widgets::label(past.range.clone())),
                )
                .child(widgets::readout(&past.max_wear, 12.0).font_weight(FontWeight::NORMAL))
                .child(widgets::label(if expanded {
                    "\u{25B4}"
                } else {
                    "\u{25BE}"
                })),
        );
        if expanded {
            let mut more = div()
                .flex()
                .flex_col()
                .gap_2()
                .pl(px(34.0))
                .pt_2()
                .pb_1()
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .gap_2()
                        .child(widgets::label(past.set_label.clone()))
                        .child(widgets::label(past.final_age.clone())),
                );
            if let Some(w) = &past.wheels {
                more = more.child(wheels_row(w, colour).opacity(0.8));
            }
            item = item.child(more);
        }
        detail = detail.child(item);
    }
    if let Some(note) = view.history_note {
        detail = detail.child(widgets::label(note).my_1());
    }
    for stop in &view.stops {
        let mut name = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .gap(px(2.0))
            .child(SharedString::from(stop.label.clone()));
        if stop.in_lane {
            name = name.child(widgets::label("In pit lane"));
        }
        let timing = |label: &'static str, value: &ReadoutView| {
            div()
                .flex()
                .flex_col()
                .items_end()
                .gap(px(2.0))
                .child(widgets::label(label))
                .child(widgets::readout(value, 14.0))
        };
        detail = detail.child(
            line_top(div().flex().items_center().gap_4())
                .child(name)
                .child(timing("Stopped", &stop.stopped))
                .child(timing("Pit lane", &stop.lane)),
        );
    }
    body = body.child(detail);
    widgets::tile_shell(body, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pitwall_core::readout::ABSENT_TEXT;

    /// Port of the web fixture: a used Soft set for laps 1–10, a new Medium set since
    /// lap 11, one stop on lap 10, at lap 14, with the player's pit window.
    fn inputs(restricted: bool, player: bool) -> Inputs {
        let condition = if restricted {
            Readout::Restricted
        } else {
            Readout::Live([80.0, 70.0, 60.0, 50.0])
        };
        Inputs {
            present: true,
            retired: false,
            current_lap: Readout::Live(14),
            tyres: Readout::Live(Tyres {
                visual_compound: 17,
                age_laps: 4,
            }),
            tyre_condition: condition,
            stints: vec![
                StintFacts {
                    id: 2,
                    start_lap: 11,
                    end_lap: None,
                    visual_compound: 17,
                    starting_age: Some(0),
                    tyre_age_laps: Some(4),
                    tyre_condition: condition,
                },
                StintFacts {
                    id: 1,
                    start_lap: 1,
                    end_lap: Some(10),
                    visual_compound: 16,
                    starting_age: Some(2),
                    tyre_age_laps: Some(12),
                    tyre_condition: condition,
                },
            ],
            stops: vec![StopFacts {
                id: 1,
                lap_number: 10,
                in_lane: false,
                lane_time_ms: Some(22_000),
                stopped_time_ms: Some(2_300),
            }],
            pit_window: if player {
                Readout::Live(PitWindow {
                    car_index: 0,
                    ideal_lap: 18,
                    latest_lap: 22,
                    rejoin_position: 8,
                })
            } else {
                Readout::Absent
            },
            lifecycle: Lifecycle::Live,
            width: 400.0,
            height: 350.0,
        }
    }

    #[test]
    fn declares_the_catalogue_entry_with_the_same_content_at_both_densities() {
        assert_eq!((TYPE, NAME), ("stint", "Stint"));
        assert_eq!(DENSITIES.pick(299.0, 240.0), Density::Compact);
        assert_eq!(DENSITIES.pick(300.0, 240.0), Density::Regular);
        let regular = view(&inputs(false, true));
        let compact = view(&Inputs {
            width: 200.0,
            ..inputs(false, true)
        });
        assert_eq!(compact.density, Density::Compact);
        assert_eq!(
            (compact.current, compact.past, compact.stops),
            (regular.current, regular.past, regular.stops)
        );
    }

    #[test]
    fn shows_completed_and_current_sets_used_starting_age_stops_and_player_pit_window() {
        let v = view(&inputs(false, true));
        assert_eq!(v.past[0].range, "Laps 1\u{2013}10 \u{b7} 10 laps");
        let current = v.current.as_ref().unwrap();
        assert_eq!(current.caption, "Laps 11\u{2013}14 \u{b7} current");
        assert_eq!(current.set_label, "New set");
        assert_eq!(v.past[0].set_label, "Used set \u{b7} starting age 2");
        assert_eq!(v.past[0].final_age, "Final tyre age 12");
        assert_eq!(v.stops[0].label, "Stop \u{b7} lap 10");
        assert_eq!(v.stops[0].stopped.text, "2.300 s");
        assert_eq!(v.stops[0].lane.text, "22.000 s");
        assert_eq!(
            v.pit_window.as_ref().unwrap().text,
            "Laps 18\u{2013}22 \u{b7} rejoin P8"
        );
        assert_eq!(v.stop_count, "1 stop");
        assert_eq!(v.title, "Current stint");
        assert_eq!(v.history_note, None);
    }

    #[test]
    fn restricted_rival_keeps_stint_history_but_no_wear_values_or_player_pit_window() {
        let v = view(&inputs(true, false));
        let current = v.current.as_ref().unwrap();
        let mut restricted = 0;
        if let Wear::Missing(w) = &current.wear {
            assert_eq!(w.text, "Restricted");
            restricted += 1;
        }
        restricted += v
            .past
            .iter()
            .filter(|p| p.max_wear.text == "Restricted")
            .count();
        assert_eq!(restricted, 2);
        assert_eq!(v.past[0].range, "Laps 1\u{2013}10 \u{b7} 10 laps");
        assert_eq!(v.past[0].wheels, None);
        assert_eq!(v.pit_window, None);
    }

    #[test]
    fn a_later_restriction_withholds_the_wear_already_recorded_on_completed_sets() {
        let mut later = inputs(false, true);
        later.tyre_condition = Readout::Restricted;
        let v = view(&later);
        assert_eq!(v.past[0].max_wear.state, ReadoutState::Restricted);
        assert_eq!(v.past[0].wheels, None);
        assert!(matches!(v.current.unwrap().wear, Wear::Missing(_)));
    }

    #[test]
    fn current_set_comes_first_with_labelled_wheel_wear_and_distinct_pit_timing_values() {
        let v = view(&inputs(false, true));
        let current = v.current.as_ref().unwrap();
        assert_eq!(current.name, "Medium");
        assert_eq!(current.family, Family::Medium);
        assert_eq!(current.laps_done, "03");
        assert_eq!(current.tyre_age.text, "4");
        let Wear::Wheels(wheels) = &current.wear else {
            panic!("wheels")
        };
        let read: Vec<(&str, &str)> = wheels
            .iter()
            .map(|w| (w.wheel, w.wear.text.as_str()))
            .collect();
        assert_eq!(
            read,
            [("FL", "40%"), ("FR", "50%"), ("RL", "20%"), ("RR", "30%")]
        );
        assert_eq!(v.past[0].name, "Soft");
        assert_eq!(v.past[0].max_wear.text, "50% max wear");
        assert_ne!(v.stops[0].stopped, v.stops[0].lane);
    }

    #[test]
    fn completed_sets_list_newest_first_and_stops_in_lap_order() {
        let mut more = inputs(false, true);
        more.stints.push(StintFacts {
            id: 3,
            start_lap: 1,
            end_lap: Some(1),
            visual_compound: 7,
            starting_age: None,
            tyre_age_laps: None,
            tyre_condition: Readout::Absent,
        });
        more.stints[1].start_lap = 2;
        more.stops.insert(
            0,
            StopFacts {
                id: 2,
                lap_number: 13,
                in_lane: true,
                lane_time_ms: None,
                stopped_time_ms: None,
            },
        );
        let v = view(&more);
        let ranges: Vec<&str> = v.past.iter().map(|p| p.range.as_str()).collect();
        assert_eq!(
            ranges,
            [
                "Laps 2\u{2013}10 \u{b7} 9 laps",
                "Laps 1\u{2013}1 \u{b7} 1 lap"
            ]
        );
        assert_eq!(v.past[1].name, "Intermediate");
        assert_eq!(v.past[1].final_age, "Final tyre age \u{2014}");
        assert_eq!(v.past[1].set_label, "");
        assert_eq!(v.past[1].max_wear.text, ABSENT_TEXT);
        let labels: Vec<&str> = v.stops.iter().map(|s| s.label.as_str()).collect();
        assert_eq!(labels, ["Stop \u{b7} lap 10", "Stop \u{b7} lap 13"]);
        assert!(v.stops[1].in_lane);
        assert_eq!(v.stops[1].stopped.state, ReadoutState::Absent);
        assert_eq!(v.stop_count, "2 stops");
    }

    #[test]
    fn history_notes_tell_no_history_from_no_previous_sets_and_a_missing_car_from_both() {
        let empty = view(&Inputs {
            stints: Vec::new(),
            stops: Vec::new(),
            ..inputs(false, true)
        });
        assert_eq!(empty.history_note, Some("No stint history"));
        assert_eq!(empty.current, None);
        assert_eq!(empty.stop_count, "0 stops");
        let first = view(&Inputs {
            stints: inputs(false, true).stints[..1].to_vec(),
            ..inputs(false, true)
        });
        assert_eq!(first.history_note, Some("No previous sets recorded"));
        let missing = view(&Inputs {
            present: false,
            ..inputs(false, true)
        });
        assert_eq!(missing.message, Some("No stint data"));
    }

    #[test]
    fn a_completed_set_expands_and_collapses_on_its_own() {
        let memory = Memory::default();
        assert!(!memory.is_open(1));
        memory.toggle(1);
        assert!(memory.is_open(1) && !memory.is_open(2));
        memory.handle().toggle(1);
        assert!(!memory.is_open(1));
        // Expanded detail: starting and final age and each wheel.
        let past = &view(&inputs(false, true)).past[0];
        let wheels = past.wheels.as_ref().unwrap();
        assert_eq!(wheels.len(), 4);
        assert_eq!(wheels[0].wheel, "FL");
    }

    #[test]
    fn a_retired_car_says_so_and_a_lost_game_dims_live_values() {
        let v = view(&Inputs {
            retired: true,
            lifecycle: Lifecycle::Lost,
            ..inputs(false, true)
        });
        assert_eq!(v.title, "Current stint \u{b7} retired");
        assert_eq!(v.stops[0].stopped.state, ReadoutState::Stale);
        let current = v.current.unwrap();
        assert_eq!(current.tyre_age.state, ReadoutState::Stale);
        let Wear::Wheels(wheels) = current.wear else {
            panic!("wheels")
        };
        assert!(wheels.iter().all(|w| w.wear.state == ReadoutState::Stale));
    }
}
