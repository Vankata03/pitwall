//! Race-control feed (parity FEED-01..03): a session-wide slow tile listing race-control
//! entries. Port of `dashboard/src/tiles/types/race-control-feed.tsx`.
//!
//! The tile filters the model's race-control log by entry type and drops controller and
//! car-mode telemetry, which the log keeps. Routine headlines are held for three seconds
//! so a burst reads as one change; urgent ones (red flag, safety car, penalties) show at
//! once. A short tile is a one-line strip led by the track status.

use std::time::{Duration, Instant};

use gpui::{Div, FontWeight, Hsla, IntoElement, ParentElement, SharedString, Styled, div, px};
use pitwall_core::{
    binding::{car_label, participants_of},
    model::{RaceControlEntry, RaceControlKind, SessionModel, Source},
    race_status::race_status,
    telemetry::{Event, EventDetails},
};
use serde_json::Value;

use super::{Densities, Density, format::fixed, widgets};
use crate::theme;

pub const TYPE: &str = pitwall_core::tile_catalog::RACE_CONTROL_FEED.kind;

pub const DENSITIES: Densities = Densities {
    regular: (240.0, 184.0),
    full: Some((400.0, 304.0)),
};

/// The entry types a user can pick, in the settings order; all are on by default.
pub const ENTRY_TYPES: [(&str, &str); 8] = [
    ("Game", "Game events"),
    ("FlagChanged", "Flag"),
    ("MarshalFlagChanged", "Marshal flag"),
    ("SafetyCarChanged", "Safety car"),
    ("WeatherChanged", "Weather"),
    ("PitEntry", "Pit entry"),
    ("PitExit", "Pit exit"),
    ("Flashback", "Flashback"),
];

/// Below this height the feed is a one-line strip rather than a list.
pub const STRIP_BELOW: f32 = 140.0;
/// Routine headlines wait this long after the last change before they show.
pub const HOLD: Duration = Duration::from_secs(3);

/// The saved `entryTypes`: a list keeps its strings (unknown ones too, and empty is
/// allowed); anything else reads as every type.
pub fn entry_types_of(settings: &Value) -> Vec<String> {
    match settings.get("entryTypes") {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        _ => ENTRY_TYPES.iter().map(|(t, _)| (*t).to_owned()).collect(),
    }
}

/// The settings after ticking or clearing one type. Other keys and unknown saved types
/// stay as they were. The layout dialog uses the shared `ListSettingField` equivalent
/// and sends only the changed list through the layout controller.
#[cfg_attr(not(test), expect(dead_code))]
pub fn toggle_entry_type(settings: &Value, kind: &str, on: bool) -> Value {
    let mut types = entry_types_of(settings);
    if on {
        types.push(kind.to_owned());
    } else {
        types.retain(|t| t != kind);
    }
    let mut next = match settings {
        Value::Object(map) => map.clone(),
        _ => Default::default(),
    };
    next.insert(
        "entryTypes".into(),
        Value::Array(types.into_iter().map(Value::String).collect()),
    );
    Value::Object(next)
}

/// The entry's type as the settings name it.
pub fn type_of(kind: &RaceControlKind) -> &'static str {
    match kind {
        RaceControlKind::Game(_) => "Game",
        RaceControlKind::FlagChanged { .. } => "FlagChanged",
        RaceControlKind::MarshalFlagChanged { .. } => "MarshalFlagChanged",
        RaceControlKind::SafetyCarChanged { .. } => "SafetyCarChanged",
        RaceControlKind::WeatherChanged { .. } => "WeatherChanged",
        RaceControlKind::PitEntry => "PitEntry",
        RaceControlKind::PitExit => "PitExit",
        RaceControlKind::Flashback { .. } => "Flashback",
    }
}

/// The event's variant name, as the wire names it.
fn variant(details: &EventDetails) -> &'static str {
    use EventDetails::*;
    match details {
        SessionStarted => "SessionStarted",
        SessionEnded => "SessionEnded",
        FastestLap { .. } => "FastestLap",
        Retirement { .. } => "Retirement",
        DrsEnabled => "DrsEnabled",
        DrsDisabled { .. } => "DrsDisabled",
        TeamMateInPits { .. } => "TeamMateInPits",
        ChequeredFlag => "ChequeredFlag",
        RaceWinner { .. } => "RaceWinner",
        Penalty { .. } => "Penalty",
        SpeedTrap { .. } => "SpeedTrap",
        StartLights { .. } => "StartLights",
        LightsOut => "LightsOut",
        DriveThroughServed { .. } => "DriveThroughServed",
        StopGoServed { .. } => "StopGoServed",
        Flashback { .. } => "Flashback",
        Buttons { .. } => "Buttons",
        RedFlag => "RedFlag",
        Overtake { .. } => "Overtake",
        SafetyCar { .. } => "SafetyCar",
        Collision { .. } => "Collision",
        PartialAeroEnabled => "PartialAeroEnabled",
        PartialAeroDisabled => "PartialAeroDisabled",
        OvertakeEnabled => "OvertakeEnabled",
        OvertakeDisabled => "OvertakeDisabled",
        Unknown => "Unknown",
    }
}

/// A variant without fields: the wire sends these as a bare string.
fn is_unit(details: &EventDetails) -> bool {
    use EventDetails::*;
    matches!(
        details,
        SessionStarted
            | SessionEnded
            | DrsEnabled
            | ChequeredFlag
            | LightsOut
            | RedFlag
            | PartialAeroEnabled
            | PartialAeroDisabled
            | OvertakeEnabled
            | OvertakeDisabled
            | Unknown
    )
}

/// `RedFlag` reads `Red Flag`.
fn words(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 4);
    let mut previous_lower = false;
    for c in name.chars() {
        if previous_lower && c.is_ascii_uppercase() {
            out.push(' ');
        }
        previous_lower = c.is_ascii_lowercase();
        out.push(c);
    }
    out
}

fn flag(n: i8) -> String {
    match n {
        -1 => "Unknown".into(),
        0 => "None".into(),
        1 => "G".into(),
        2 => "B".into(),
        3 => "Y".into(),
        4 => "R".into(),
        n => n.to_string(),
    }
}

fn sky(weather: u8) -> &'static str {
    pitwall_core::weather::sky(weather).unwrap_or("Unknown")
}

fn game_text(event: &Event) -> String {
    match &event.details {
        EventDetails::Unknown => format!(
            "Game event {}",
            event
                .code
                .iter()
                .map(|&b| char::from(b))
                .collect::<String>()
        ),
        EventDetails::Penalty {
            time,
            penalty_type,
            infringement_type,
            ..
        } => format!("Penalty: {time} s, type {penalty_type}, infringement {infringement_type}"),
        EventDetails::FastestLap { lap_time, .. } => {
            format!("Fastest lap: {} s", fixed(f64::from(*lap_time), 3))
        }
        EventDetails::SpeedTrap { .. } => "Speed trap".into(),
        EventDetails::SafetyCar {
            safety_car_type,
            event_type,
        } => format!("Safety car: type {safety_car_type}, event {event_type}"),
        details => words(variant(details)),
    }
}

/// An entry in words, as the list shows it.
pub fn text(kind: &RaceControlKind) -> String {
    match kind {
        RaceControlKind::PitEntry => "Pit entry".into(),
        RaceControlKind::PitExit => "Pit exit".into(),
        RaceControlKind::Game(event) => game_text(event),
        RaceControlKind::FlagChanged { flag: f, .. } => format!("Flag {}", flag(*f)),
        RaceControlKind::MarshalFlagChanged { zone, flag: f, .. } => {
            format!("Marshal zone {}: {}", u16::from(*zone) + 1, flag(*f))
        }
        RaceControlKind::SafetyCarChanged { status, .. } => match status {
            0 => "SC ended".into(),
            1 => "SC".into(),
            2 => "VSC".into(),
            3 => "Formation".into(),
            n => format!("Safety car {n}"),
        },
        RaceControlKind::WeatherChanged { weather, .. } => format!("Weather: {}", sky(*weather)),
        RaceControlKind::Flashback {
            from_time, to_time, ..
        } => format!(
            "Flashback: {} \u{2192} {} s",
            fixed(f64::from(*from_time), 1),
            fixed(f64::from(*to_time), 1)
        ),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Info,
    Good,
    Warn,
    Bad,
    Quiet,
}

impl Tone {
    fn colour(self) -> Hsla {
        match self {
            Tone::Info => theme::cold(),
            Tone::Good => theme::personal_best(),
            Tone::Warn => theme::slower(),
            Tone::Bad => theme::brake(),
            Tone::Quiet => theme::muted(),
        }
    }
}

/// A short tag, a headline and a detail for one entry, as a broadcast ticker words it.
#[derive(Debug, Clone, PartialEq)]
pub struct Headline {
    pub tag: &'static str,
    pub tone: Tone,
    pub title: String,
    pub detail: String,
}

fn flag_word(f: i8) -> Option<&'static str> {
    match f {
        1 => Some("Green flag"),
        2 => Some("Blue flag"),
        3 => Some("Yellow flag"),
        4 => Some("Red flag"),
        _ => None,
    }
}

fn lap_time(seconds: f32) -> String {
    let seconds = f64::from(seconds);
    let m = (seconds / 60.0).floor();
    let rest = seconds - m * 60.0;
    if m > 0.0 {
        format!("{m}:{:0>6}", fixed(rest, 3))
    } else {
        fixed(rest, 3)
    }
}

pub fn headline(kind: &RaceControlKind, car_names: &str, name: &dyn Fn(u8) -> String) -> Headline {
    let h = |tag, tone, title: String, detail: String| Headline {
        tag,
        tone,
        title,
        detail,
    };
    match kind {
        RaceControlKind::PitEntry => h("PIT", Tone::Info, "Pit entry".into(), car_names.into()),
        RaceControlKind::PitExit => h("PIT", Tone::Info, "Pit exit".into(), car_names.into()),
        RaceControlKind::FlagChanged { flag: f, .. } => h(
            "FLAG",
            match f {
                4 => Tone::Bad,
                3 => Tone::Warn,
                1 => Tone::Good,
                _ => Tone::Info,
            },
            flag_word(*f).map_or_else(|| format!("Flag {}", flag(*f)), str::to_owned),
            if *f == 0 {
                "Flag cleared".into()
            } else {
                car_names.into()
            },
        ),
        RaceControlKind::MarshalFlagChanged { zone, flag: f, .. } => h(
            "FLAG",
            match f {
                4 => Tone::Bad,
                3 => Tone::Warn,
                _ => Tone::Info,
            },
            flag_word(*f).map_or_else(|| format!("Flag {}", flag(*f)), str::to_owned),
            format!("Marshal zone {}", u16::from(*zone) + 1),
        ),
        RaceControlKind::SafetyCarChanged { status, .. } => h(
            match status {
                2 => "VSC",
                3 => "FORM",
                _ => "SC",
            },
            if *status == 0 { Tone::Good } else { Tone::Warn },
            text(kind),
            if *status == 0 {
                "Racing resumes".into()
            } else {
                "Deployed".into()
            },
        ),
        RaceControlKind::WeatherChanged { weather, .. } => {
            h("WX", Tone::Info, "Weather".into(), sky(*weather).into())
        }
        RaceControlKind::Flashback { .. } => h(
            "FB",
            Tone::Quiet,
            "Flashback".into(),
            text(kind).replacen("Flashback: ", "", 1),
        ),
        RaceControlKind::Game(event) => {
            use EventDetails::*;
            let d = &event.details;
            if is_unit(d) {
                let tone = match d {
                    RedFlag => Tone::Bad,
                    LightsOut | SessionStarted | DrsEnabled => Tone::Good,
                    _ => Tone::Info,
                };
                let tag = match d {
                    LightsOut => "START",
                    RedFlag | ChequeredFlag => "FLAG",
                    DrsEnabled => "DRS",
                    _ => "RC",
                };
                return h(tag, tone, game_text(event), car_names.into());
            }
            match *d {
                Overtake {
                    overtaking_vehicle_idx,
                    being_overtaken_vehicle_idx,
                } => h(
                    "OVTK",
                    Tone::Info,
                    "Overtake".into(),
                    format!(
                        "{} passes {}",
                        name(overtaking_vehicle_idx),
                        name(being_overtaken_vehicle_idx)
                    ),
                ),
                FastestLap {
                    vehicle_idx,
                    lap_time: t,
                } => h(
                    "FL",
                    Tone::Good,
                    "Fastest lap".into(),
                    format!("{} \u{B7} {}", name(vehicle_idx), lap_time(t)),
                ),
                Penalty {
                    vehicle_idx, time, ..
                } => h(
                    "PEN",
                    Tone::Warn,
                    "Penalty".into(),
                    format!(
                        "{}{}",
                        name(vehicle_idx),
                        if time > 0 && time < 255 {
                            format!(" \u{B7} +{time} s")
                        } else {
                            String::new()
                        }
                    ),
                ),
                Retirement { vehicle_idx, .. } => {
                    h("DNF", Tone::Bad, "Retirement".into(), name(vehicle_idx))
                }
                Collision {
                    vehicle1_idx,
                    vehicle2_idx,
                    ..
                } => h(
                    "COL",
                    Tone::Warn,
                    "Collision".into(),
                    format!("{} & {}", name(vehicle1_idx), name(vehicle2_idx)),
                ),
                RaceWinner { vehicle_idx } => {
                    h("WIN", Tone::Good, "Race winner".into(), name(vehicle_idx))
                }
                // The game reports trap speeds in km/h; the feed keeps them so (UNIT-02).
                SpeedTrap {
                    vehicle_idx, speed, ..
                } => h(
                    "TRAP",
                    Tone::Info,
                    "Speed trap".into(),
                    format!(
                        "{} \u{B7} {} km/h",
                        name(vehicle_idx),
                        (f64::from(speed) + 0.5).floor()
                    ),
                ),
                DriveThroughServed { vehicle_idx } => h(
                    "PEN",
                    Tone::Info,
                    "Drive-through served".into(),
                    name(vehicle_idx),
                ),
                StopGoServed { vehicle_idx, .. } => h(
                    "PEN",
                    Tone::Info,
                    "Stop-go served".into(),
                    name(vehicle_idx),
                ),
                SafetyCar { .. } => h("SC", Tone::Warn, "Safety car".into(), car_names.into()),
                StartLights { num_lights } => h(
                    "START",
                    Tone::Warn,
                    "Start lights".into(),
                    format!("{num_lights} lights"),
                ),
                _ => h("RC", Tone::Info, game_text(event), car_names.into()),
            }
        }
    }
}

/// Controller and car-mode telemetry is not a race-control announcement, and neither is a
/// routine change between green and no flag.
pub fn is_announcement(entry: &RaceControlEntry) -> bool {
    match &entry.kind {
        RaceControlKind::FlagChanged { flag, previous } => *flag > 1 || *previous > 1,
        RaceControlKind::Game(event) => !matches!(
            event.details,
            EventDetails::Buttons { .. }
                | EventDetails::SpeedTrap { .. }
                | EventDetails::Unknown
                | EventDetails::PartialAeroEnabled
                | EventDetails::PartialAeroDisabled
                | EventDetails::OvertakeEnabled
                | EventDetails::OvertakeDisabled
        ),
        _ => true,
    }
}

/// Shown at once, without the routine hold.
pub fn is_urgent(entry: &RaceControlEntry) -> bool {
    match &entry.kind {
        RaceControlKind::FlagChanged { flag, .. }
        | RaceControlKind::MarshalFlagChanged { flag, .. } => *flag == 4,
        RaceControlKind::SafetyCarChanged { .. } => true,
        RaceControlKind::Game(event) => matches!(
            event.details,
            EventDetails::RedFlag
                | EventDetails::ChequeredFlag
                | EventDetails::LightsOut
                | EventDetails::Penalty { .. }
                | EventDetails::Collision { .. }
                | EventDetails::SafetyCar { .. }
                | EventDetails::Retirement { .. }
        ),
        _ => false,
    }
}

/// Track status first in the strip, in words: red flag, safety car or track clear. It
/// reads every non-rewound entry, whatever the filter or the hold.
pub fn status_of(session: Option<Option<u8>>, log: &[RaceControlEntry]) -> (&'static str, Tone) {
    let Some(safety_car_status) = session else {
        return ("No session", Tone::Quiet);
    };
    let band = race_status(safety_car_status, log);
    if !band.is_empty() {
        return (
            band,
            if band == "Red flag" {
                Tone::Bad
            } else {
                Tone::Warn
            },
        );
    }
    let mut zones: Vec<(u8, i8)> = Vec::new();
    for entry in log.iter().filter(|e| !e.rewound) {
        if let RaceControlKind::MarshalFlagChanged { zone, flag, .. } = entry.kind {
            match zones.iter_mut().find(|(z, _)| *z == zone) {
                Some(slot) => slot.1 = flag,
                None => zones.push((zone, flag)),
            }
        }
    }
    if zones.iter().any(|(_, f)| *f == 4) {
        ("Red flag", Tone::Bad)
    } else if zones.iter().any(|(_, f)| *f == 3) {
        ("Yellow flag", Tone::Warn)
    } else {
        ("Track clear", Tone::Good)
    }
}

/// What the feed shows, held between renders.
struct Shown {
    entries: Vec<RaceControlEntry>,
    epoch: u64,
    filter: Vec<String>,
    at: Duration,
}

/// The feed's hold: routine headlines wait three seconds after the last published
/// change; urgent ones, a new epoch, a filter change or a rewritten log show at once.
#[derive(Default)]
pub struct Memory {
    shown: Option<Shown>,
    origin: Option<Instant>,
}

impl Memory {
    /// The entries to show now, given the filtered `candidates`; `now` is any steady clock.
    pub fn announce(
        &mut self,
        candidates: Vec<RaceControlEntry>,
        epoch: u64,
        filter: &[String],
        now: Duration,
    ) -> Vec<RaceControlEntry> {
        let publish = |entries: Vec<RaceControlEntry>| Shown {
            entries,
            epoch,
            filter: filter.to_vec(),
            at: now,
        };
        let Some(shown) = &self.shown else {
            self.shown = Some(publish(candidates.clone()));
            return candidates;
        };
        // A seek or restore rewrites the log: the shown entries no longer lead it.
        let rewritten = shown.entries.len() > candidates.len()
            || shown
                .entries
                .iter()
                .zip(&candidates)
                .any(|(a, b)| a.id != b.id);
        let reset = shown.epoch != epoch || shown.filter != filter || rewritten;
        if !reset && shown.entries == candidates {
            return candidates;
        }
        let added = &candidates[shown.entries.len().min(candidates.len())..];
        let immediate = reset || shown.entries.is_empty() || added.iter().any(is_urgent);
        if !immediate && now < shown.at + HOLD {
            return shown.entries.clone();
        }
        self.shown = Some(publish(candidates.clone()));
        candidates
    }

    /// [`Memory::announce`] on the wall clock.
    pub fn announce_now(
        &mut self,
        candidates: Vec<RaceControlEntry>,
        epoch: u64,
        filter: &[String],
    ) -> Vec<RaceControlEntry> {
        let now = self.origin.get_or_insert_with(Instant::now).elapsed();
        self.announce(candidates, epoch, filter, now)
    }
}

pub struct Inputs {
    /// The whole race-control log, in arrival order.
    pub log: Vec<RaceControlEntry>,
    /// Shown names by vehicle index.
    pub names: Vec<(usize, String)>,
    /// `Some(safety_car_status)` while the session readout has a value.
    pub session: Option<Option<u8>>,
    pub epoch: u64,
    pub entry_types: Vec<String>,
    pub width: f32,
    pub height: f32,
}

impl Inputs {
    pub fn from_model(model: &SessionModel, settings: &Value, size: (f32, f32)) -> Self {
        Self {
            log: model.race_control().to_vec(),
            names: participants_of(model)
                .iter()
                .map(|p| (p.index, car_label(p)))
                .collect(),
            session: model.session().value().map(|s| Some(s.safety_car_status)),
            epoch: model.epoch(),
            entry_types: entry_types_of(settings),
            width: size.0,
            height: size.1,
        }
    }

    /// The log entries this tile's filter lets through.
    pub fn candidates(&self) -> Vec<RaceControlEntry> {
        self.log
            .iter()
            .filter(|e| {
                self.entry_types.iter().any(|t| t == type_of(&e.kind)) && is_announcement(e)
            })
            .cloned()
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StripEntry {
    pub headline: Headline,
    /// `L2 · 00:10`, with `· Rewound` when a flashback abandoned it.
    pub time: String,
    pub rewound: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ListEntry {
    /// `10.0 s · Lap 2`, with `· Rewound` when a flashback abandoned it.
    pub when: String,
    pub text: String,
    /// Full density only.
    pub source: Option<&'static str>,
    pub rewound: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum View {
    Strip {
        status: (&'static str, Tone),
        /// Newest first, at most six.
        entries: Vec<StripEntry>,
        empty: Option<&'static str>,
        width: f32,
    },
    List {
        density: Density,
        /// Newest first, as many as fit.
        entries: Vec<ListEntry>,
        empty: Option<&'static str>,
    },
}

fn clock(seconds: f32) -> String {
    let t = seconds.max(0.0).floor() as u64;
    format!("{:02}:{:02}", t / 60, t % 60)
}

/// The view, from the entries the hold lets through (`shown`).
pub fn view(inputs: &Inputs, shown: &[RaceControlEntry]) -> View {
    let name = |index: usize| {
        inputs
            .names
            .iter()
            .find(|(i, _)| *i == index)
            .map_or_else(|| format!("Car {}", index + 1), |(_, n)| n.clone())
    };
    let names = |e: &RaceControlEntry| {
        e.cars
            .iter()
            .map(|&i| name(i))
            .collect::<Vec<_>>()
            .join(", ")
    };
    if inputs.height < STRIP_BELOW {
        let entries: Vec<_> = shown
            .iter()
            .rev()
            .take(6)
            .map(|e| StripEntry {
                headline: headline(&e.kind, &names(e), &|i| name(usize::from(i))),
                time: format!(
                    "{}{}{}",
                    e.lap_number
                        .map_or(String::new(), |l| format!("L{l} \u{B7} ")),
                    clock(e.session_time),
                    if e.rewound { " \u{B7} Rewound" } else { "" }
                ),
                rewound: e.rewound,
            })
            .collect();
        return View::Strip {
            status: status_of(inputs.session, &inputs.log),
            empty: entries.is_empty().then_some("No race-control events yet"),
            entries,
            width: inputs.width,
        };
    }
    let density = DENSITIES.pick(inputs.width, inputs.height);
    let row = if density == Density::Compact {
        44.0
    } else {
        64.0
    };
    let fit = (((inputs.height - 24.0) / row).floor() as usize).max(1);
    let entries: Vec<_> = shown
        .iter()
        .rev()
        .take(fit)
        .map(|e| {
            let cars = names(e);
            ListEntry {
                when: format!(
                    "{} s{}{}",
                    fixed(f64::from(e.session_time), 1),
                    e.lap_number
                        .map_or(String::new(), |l| format!(" \u{B7} Lap {l}")),
                    if e.rewound { " \u{B7} Rewound" } else { "" }
                ),
                text: if cars.is_empty() {
                    text(&e.kind)
                } else {
                    format!("{} \u{B7} {cars}", text(&e.kind))
                },
                source: (density == Density::Full).then_some(match e.source {
                    Source::Game => "Game",
                    Source::Derived => "Derived",
                }),
                rewound: e.rewound,
            }
        })
        .collect();
    View::List {
        density,
        empty: entries
            .is_empty()
            .then_some(if inputs.entry_types.is_empty() {
                "No entry types selected"
            } else {
                "No race-control entries"
            }),
        entries,
    }
}

fn tag(text: &'static str, tone: Tone) -> Div {
    div()
        .flex_none()
        .px(px(6.0))
        .py(px(2.0))
        .rounded(px(3.0))
        .bg(tone.colour())
        .text_color(theme::background())
        .text_size(px(11.0))
        .font_weight(FontWeight::BOLD)
        .child(text)
}

pub fn render(view: View) -> impl IntoElement {
    match view {
        View::Strip {
            status,
            entries,
            empty,
            width,
        } => {
            let wide = width > 640.0;
            let mut head = div().flex().flex_col().flex_none().gap(px(2.0)).child(
                div()
                    .text_size(px(13.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme::hot())
                    .child("RACE CONTROL"),
            );
            if wide {
                head = head
                    .pr(px(12.0))
                    .border_r_1()
                    .border_color(theme::line())
                    .child(widgets::label("LIVE EVENT STREAM"));
            }
            let chip = div()
                .flex_none()
                .px(px(if wide { 10.0 } else { 6.0 }))
                .py(px(if wide { 4.0 } else { 2.0 }))
                .border_1()
                .border_color(status.1.colour())
                .rounded(px(3.0))
                .text_color(status.1.colour())
                .text_size(px(14.0))
                .font_weight(FontWeight::BOLD)
                .child(status.0.to_uppercase());
            // Entries that do not fit wrap onto a hidden second line, so only whole
            // entries show.
            let mut ticker = div().flex().flex_wrap().w_full();
            for (i, e) in entries.iter().enumerate() {
                let mut item = div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap_2()
                    .h(px(28.0))
                    .px(px(12.0))
                    .text_size(px(14.0))
                    .child(tag(e.headline.tag, e.headline.tone))
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .child(e.headline.title.to_uppercase()),
                    );
                if i == 0 {
                    item = item.pl_0();
                    if !e.headline.detail.is_empty() {
                        item = item.child(
                            div()
                                .flex_none()
                                .text_color(theme::window())
                                .child(SharedString::from(e.headline.detail.clone())),
                        );
                    }
                    if wide {
                        item = item.child(widgets::label(e.time.clone()));
                    }
                } else {
                    item = item.border_l_1().border_color(theme::line());
                }
                let fade = if e.rewound {
                    0.5
                } else if i >= 2 {
                    0.72
                } else {
                    1.0
                };
                ticker = ticker.child(item.opacity(fade));
            }
            let mut strip = div()
                .flex()
                .items_center()
                .gap(px(if wide { 12.0 } else { 10.0 }))
                .size_full()
                .px(px(if wide { 16.0 } else { 10.0 }))
                .overflow_hidden()
                .whitespace_nowrap()
                .border_l(px(3.0))
                .border_color(theme::hot())
                .bg(theme::raised())
                .text_color(theme::text())
                .child(head)
                .child(chip)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .h(px(28.0))
                        .overflow_hidden()
                        .child(ticker),
                );
            if let Some(empty) = empty {
                strip = strip.child(widgets::label(empty));
            }
            strip.into_any_element()
        }
        View::List {
            density,
            entries,
            empty,
        } => {
            let mut list = div().flex().flex_col().overflow_hidden();
            for e in entries {
                let mut item = div()
                    .flex()
                    .flex_col()
                    .py(px(4.0))
                    .border_b_1()
                    .border_color(theme::line())
                    .child(widgets::label(e.when))
                    .child(
                        div()
                            .text_size(px(14.0))
                            .text_color(if e.rewound {
                                theme::muted()
                            } else {
                                theme::text()
                            })
                            .child(e.text),
                    );
                if let Some(source) = e.source {
                    item = item.child(widgets::label(source));
                }
                list = list.child(item);
            }
            let mut body = div()
                .flex()
                .flex_col()
                .gap_1()
                .size_full()
                .overflow_hidden()
                .child(widgets::label("Race control"))
                .child(list);
            if let Some(empty) = empty {
                body = body.child(
                    div()
                        .text_size(px(14.0))
                        .text_color(theme::muted())
                        .child(empty),
                );
            }
            let _ = density;
            widgets::tile_shell(body, None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn entry(id: u64, kind: RaceControlKind) -> RaceControlEntry {
        RaceControlEntry {
            id,
            session_time: id as f32 * 10.0,
            lap_number: Some(2),
            cars: vec![],
            source: Source::Derived,
            kind,
            rewound: false,
        }
    }
    fn game(code: &[u8; 4], details: EventDetails) -> RaceControlKind {
        RaceControlKind::Game(Event {
            code: *code,
            details,
            raw_details: [0; 12],
        })
    }
    fn inputs(log: Vec<RaceControlEntry>, settings: Value, height: f32) -> Inputs {
        Inputs {
            log,
            names: vec![],
            session: Some(Some(0)),
            epoch: 1,
            entry_types: entry_types_of(&settings),
            width: 320.0,
            height,
        }
    }
    fn strip_entries(v: &View) -> &[StripEntry] {
        match v {
            View::Strip { entries, .. } => entries,
            View::List { .. } => panic!("a list"),
        }
    }
    fn list_entries(v: &View) -> &[ListEntry] {
        match v {
            View::List { entries, .. } => entries,
            View::Strip { .. } => panic!("a strip"),
        }
    }
    const SEC: Duration = Duration::from_secs(1);

    #[test]
    fn declares_the_catalogue_entry() {
        assert_eq!(
            (TYPE, pitwall_core::tile_catalog::RACE_CONTROL_FEED.name),
            ("race-control-feed", "Race-control feed")
        );
        assert_eq!(DENSITIES.pick(239.0, 400.0), Density::Compact);
        assert_eq!(DENSITIES.pick(240.0, 184.0), Density::Regular);
        assert_eq!(DENSITIES.pick(400.0, 304.0), Density::Full);
    }

    #[test]
    fn entry_types_default_to_all_and_an_empty_list_is_kept() {
        assert_eq!(entry_types_of(&json!({})).len(), 8);
        assert!(entry_types_of(&json!({ "entryTypes": [] })).is_empty());
        assert_eq!(
            entry_types_of(&json!({ "entryTypes": ["FutureKind", 3, "PitEntry"] })),
            ["FutureKind", "PitEntry"]
        );
    }

    #[test]
    fn the_filter_preserves_unknown_settings_and_unknown_entry_types() {
        let settings =
            json!({ "extra": { "keep": true }, "entryTypes": ["FutureKind", "PitEntry"] });
        assert_eq!(
            toggle_entry_type(&settings, "PitEntry", false),
            json!({ "extra": { "keep": true }, "entryTypes": ["FutureKind"] })
        );
    }

    #[test]
    fn filters_entry_types_names_hidden_drivers_by_race_number_and_marks_rewound() {
        let mut pit = entry(1, RaceControlKind::PitEntry);
        pit.cars = vec![0];
        pit.rewound = true;
        let weather = entry(
            2,
            RaceControlKind::WeatherChanged {
                previous: 0,
                weather: 4,
            },
        );
        let mut i = inputs(
            vec![pit, weather],
            json!({ "entryTypes": ["PitEntry"] }),
            160.0,
        );
        i.names = vec![(0, "#27".into())];
        let v = view(&i, &i.candidates());
        let all: String = list_entries(&v)
            .iter()
            .map(|e| format!("{} {}", e.when, e.text))
            .collect();
        assert!(all.contains("#27"));
        assert!(all.contains("Rewound"));
        assert!(!all.contains("Weather"));
    }

    #[test]
    fn hidden_names_use_the_race_number_from_the_model() {
        use pitwall_core::car_names::label_from;
        assert_eq!(label_from("private-id", 27, false), "#27");
    }

    #[test]
    fn uses_words_for_flags_and_safety_car_states_latest_first() {
        let log = vec![
            entry(
                1,
                RaceControlKind::FlagChanged {
                    previous: 0,
                    flag: 3,
                },
            ),
            entry(
                2,
                RaceControlKind::SafetyCarChanged {
                    previous: 0,
                    status: 2,
                },
            ),
            entry(3, game(b"REDF", EventDetails::RedFlag)),
        ];
        let i = inputs(log, json!({}), 600.0);
        let v = view(&i, &i.candidates());
        let items = list_entries(&v);
        assert!(items[0].text.contains("Red Flag"));
        assert!(items[1].text.contains("VSC"));
        assert!(items[2].text.contains("Flag Y"));
    }

    #[test]
    fn ignores_controller_and_mode_telemetry_at_both_heights() {
        let log = vec![
            entry(1, game(b"LGOT", EventDetails::LightsOut)),
            entry(2, game(b"BUTN", EventDetails::Buttons { button_status: 1 })),
            entry(3, game(b"BUTN", EventDetails::PartialAeroEnabled)),
            entry(4, game(b"BUTN", EventDetails::OvertakeEnabled)),
            entry(5, game(b"BUTN", EventDetails::Unknown)),
            entry(
                6,
                game(
                    b"SPTP",
                    EventDetails::SpeedTrap {
                        vehicle_idx: 0,
                        speed: 250.0,
                        is_overall_fastest_in_session: 0,
                        is_driver_fastest_in_session: 0,
                        fastest_vehicle_idx_in_session: 0,
                        fastest_speed_in_session: 0.0,
                    },
                ),
            ),
            entry(
                7,
                RaceControlKind::FlagChanged {
                    previous: 0,
                    flag: 1,
                },
            ),
        ];
        for height in [74.0, 320.0] {
            let i = inputs(log.clone(), json!({}), height);
            assert_eq!(i.candidates().len(), 1, "{height}");
        }
    }

    #[test]
    fn routine_headlines_wait_three_seconds_and_epoch_resets_show_at_once() {
        let mut memory = Memory::default();
        let filter = entry_types_of(&json!({}));
        let pit = entry(1, RaceControlKind::PitEntry);
        let exit = entry(2, RaceControlKind::PitExit);
        assert_eq!(memory.announce(vec![pit.clone()], 1, &filter, SEC).len(), 1);
        let both = vec![pit.clone(), exit.clone()];
        assert_eq!(
            memory
                .announce(both.clone(), 1, &filter, SEC + SEC / 10)
                .len(),
            1
        );
        assert_eq!(memory.announce(both.clone(), 1, &filter, SEC * 4).len(), 2);
        // A new epoch shows what the log holds now, at once.
        assert!(memory.announce(vec![], 2, &filter, SEC * 4).is_empty());
    }

    #[test]
    fn safety_announcements_bypass_the_hold_and_a_seek_clears_pending_headlines() {
        let mut memory = Memory::default();
        let filter = entry_types_of(&json!({}));
        let pit = entry(1, RaceControlKind::PitEntry);
        let exit = entry(2, RaceControlKind::PitExit);
        let red = entry(3, game(b"REDF", EventDetails::RedFlag));
        let rain = entry(
            4,
            RaceControlKind::WeatherChanged {
                previous: 0,
                weather: 4,
            },
        );
        memory.announce(vec![pit.clone()], 1, &filter, SEC);
        let held = memory.announce(vec![pit.clone(), exit.clone()], 1, &filter, SEC);
        assert_eq!(held.len(), 1);
        let urgent = memory.announce(
            vec![pit.clone(), exit.clone(), red.clone()],
            1,
            &filter,
            SEC,
        );
        assert_eq!(urgent.last(), Some(&red));
        let held = memory.announce(vec![pit.clone(), exit, red, rain], 1, &filter, SEC);
        assert_eq!(held.len(), 3);
        // A seek back rewrites the log: the pending weather never shows.
        assert_eq!(memory.announce(vec![], 1, &filter, SEC).len(), 0);
        assert_eq!(memory.announce(vec![], 1, &filter, SEC * 9).len(), 0);
    }

    #[test]
    fn a_filter_change_shows_at_once() {
        let mut memory = Memory::default();
        let all = entry_types_of(&json!({}));
        let pit = entry(1, RaceControlKind::PitEntry);
        let exit = entry(2, RaceControlKind::PitExit);
        memory.announce(vec![pit.clone()], 1, &all, SEC);
        let only = vec!["PitEntry".to_owned(), "PitExit".to_owned()];
        assert_eq!(memory.announce(vec![pit, exit], 1, &only, SEC).len(), 2);
    }

    #[test]
    fn per_car_flag_bursts_are_batched_while_the_track_status_updates_immediately() {
        let marshal = |id, previous, flag| {
            entry(
                id,
                RaceControlKind::MarshalFlagChanged {
                    zone: 1,
                    previous: Some(previous),
                    flag,
                },
            )
        };
        let mut log = vec![entry(1, RaceControlKind::PitEntry), marshal(2, 1, 3)];
        assert_eq!(status_of(Some(Some(0)), &log), ("Yellow flag", Tone::Warn));
        let mut memory = Memory::default();
        let filter = entry_types_of(&json!({}));
        memory.announce(log[..1].to_vec(), 1, &filter, SEC);
        let i = inputs(log.clone(), json!({}), 74.0);
        assert_eq!(memory.announce(i.candidates(), 1, &filter, SEC).len(), 1);
        for id in 3..15 {
            let mut flag = entry(
                id,
                RaceControlKind::FlagChanged {
                    previous: 1,
                    flag: 3,
                },
            );
            flag.cars = vec![id as usize];
            log.push(flag);
        }
        let i = inputs(log.clone(), json!({}), 74.0);
        assert_eq!(
            memory
                .announce(i.candidates(), 1, &filter, SEC + SEC / 2)
                .len(),
            1
        );
        let shown = memory.announce(i.candidates(), 1, &filter, SEC * 5);
        let v = view(&i, &shown);
        assert_eq!(strip_entries(&v)[0].headline.title, "Yellow flag");
        log.push(marshal(15, 3, 1));
        assert_eq!(status_of(Some(Some(0)), &log), ("Track clear", Tone::Good));
    }

    #[test]
    fn red_flags_and_safety_cars_lead_the_track_status() {
        let red = vec![entry(1, game(b"REDF", EventDetails::RedFlag))];
        assert_eq!(status_of(Some(Some(0)), &red), ("Red flag", Tone::Bad));
        assert_eq!(status_of(Some(Some(1)), &[]), ("Safety car", Tone::Warn));
        assert_eq!(status_of(None, &[]), ("No session", Tone::Quiet));
        let mut rewound = entry(
            2,
            RaceControlKind::MarshalFlagChanged {
                zone: 0,
                previous: None,
                flag: 4,
            },
        );
        rewound.rewound = true;
        assert_eq!(status_of(Some(Some(0)), &[rewound]).0, "Track clear");
    }

    #[test]
    fn a_short_wide_tile_is_a_one_line_strip_newest_first_with_names() {
        let log = vec![
            entry(1, game(b"LGOT", EventDetails::LightsOut)),
            entry(
                2,
                game(
                    b"OVTK",
                    EventDetails::Overtake {
                        overtaking_vehicle_idx: 0,
                        being_overtaken_vehicle_idx: 1,
                    },
                ),
            ),
        ];
        let mut i = inputs(log, json!({}), 74.0);
        i.width = 940.0;
        i.names = vec![(0, "Alpha".into()), (1, "Bravo".into())];
        let v = view(&i, &i.candidates());
        let View::Strip { status, .. } = &v else {
            panic!("a strip")
        };
        assert_eq!(status.0, "Track clear");
        let items = strip_entries(&v);
        assert_eq!(items[0].headline.tag, "OVTK");
        assert_eq!(items[0].headline.detail, "Alpha passes Bravo");
        assert_eq!(items[1].headline.title, "Lights Out");
        assert_eq!(items[0].time, "L2 \u{B7} 00:20");
    }

    #[test]
    fn the_strip_keeps_six_entries_and_the_list_fits_by_density() {
        let log: Vec<_> = (1..=20)
            .map(|id| entry(id, RaceControlKind::PitEntry))
            .collect();
        let strip = inputs(log.clone(), json!({}), 74.0);
        assert_eq!(strip_entries(&view(&strip, &log)).len(), 6);
        let compact = Inputs {
            width: 200.0,
            ..inputs(log.clone(), json!({}), 200.0)
        };
        // (200 - 24) / 44 = 4 rows compact; (304 - 24) / 64 = 4 rows full, with source.
        assert_eq!(list_entries(&view(&compact, &log)).len(), 4);
        let full = Inputs {
            width: 400.0,
            ..inputs(log.clone(), json!({}), 304.0)
        };
        let v = view(&full, &log);
        assert_eq!(list_entries(&v).len(), 4);
        assert_eq!(list_entries(&v)[0].source, Some("Derived"));
        assert_eq!(list_entries(&v)[0].when, "200.0 s \u{B7} Lap 2");
    }

    #[test]
    fn empty_states_stay_distinct() {
        let none = inputs(vec![], json!({ "entryTypes": [] }), 200.0);
        let View::List { empty, .. } = view(&none, &[]) else {
            panic!("a list")
        };
        assert_eq!(empty, Some("No entry types selected"));
        let quiet = inputs(vec![], json!({}), 200.0);
        let View::List { empty, .. } = view(&quiet, &[]) else {
            panic!("a list")
        };
        assert_eq!(empty, Some("No race-control entries"));
        let View::Strip { empty, .. } = view(&inputs(vec![], json!({}), 74.0), &[]) else {
            panic!("a strip")
        };
        assert_eq!(empty, Some("No race-control events yet"));
    }

    #[test]
    fn headlines_word_game_events_like_a_broadcast_ticker() {
        let name = |i: u8| format!("Car {}", i + 1);
        let pen = game(
            b"PENA",
            EventDetails::Penalty {
                penalty_type: 0,
                infringement_type: 7,
                vehicle_idx: 2,
                other_vehicle_idx: 255,
                time: 5,
                lap_num: 3,
                places_gained: 0,
            },
        );
        let h = headline(&pen, "", &name);
        assert_eq!(
            (h.tag, h.title.as_str(), h.detail.as_str()),
            ("PEN", "Penalty", "Car 3 \u{B7} +5 s")
        );
        let fl = game(
            b"FTLP",
            EventDetails::FastestLap {
                vehicle_idx: 0,
                lap_time: 83.4567,
            },
        );
        assert_eq!(headline(&fl, "", &name).detail, "Car 1 \u{B7} 1:23.457");
        assert_eq!(text(&fl), "Fastest lap: 83.457 s");
        assert_eq!(
            text(&game(b"ABCD", EventDetails::Unknown)),
            "Game event ABCD"
        );
        assert_eq!(
            text(&game(b"DRSD", EventDetails::DrsDisabled { reason: 1 })),
            "Drs Disabled"
        );
        assert_eq!(
            text(&RaceControlKind::Flashback {
                from_time: 12.25,
                to_time: 8.0,
                event: None
            }),
            "Flashback: 12.3 \u{2192} 8.0 s"
        );
        assert_eq!(
            text(&RaceControlKind::MarshalFlagChanged {
                zone: 0,
                previous: None,
                flag: 3
            }),
            "Marshal zone 1: Y"
        );
    }
}
