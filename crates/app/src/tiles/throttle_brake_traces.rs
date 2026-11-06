//! Throttle/brake traces (parity INPUT-01..04): a frame-rate tile following one car.
//! Port of `dashboard/src/tiles/types/throttle-brake-traces.tsx` and `traces/chart.ts`.
//!
//! The last 30 seconds of the car's inputs against session time, newest at zero, with the
//! reference lap's inputs at the same lap distance drawn dim and thin underneath, and
//! instantaneous THR/BRK meters. Every fast frame since the last render is recorded, so
//! the paint cadence never drops a sample. The reference lap trace is read from the
//! in-process model, so a completed lap is available as soon as the model has it.

use gpui::{Hsla, IntoElement, ParentElement, Styled, canvas, div, px, relative};
use pitwall_core::{
    feed::{FAST_HISTORY_SECONDS, FastFrame, Reader},
    model::{LapSample, SessionModel},
    readout::{ReadoutState, ReadoutView, readout_view},
    traces::{
        CHANNEL_ORDER, Channel, INPUT_WINDOW_SECONDS, InputHistory, InputSample, Lap,
        PEDAL_SMOOTH_SECONDS, build_window_plot, smooth_values,
    },
    units::{self, Units},
};
use serde_json::{Value, json};

use super::{Densities, Density, paint, widgets};
use crate::theme;

pub const TYPE: &str = pitwall_core::tile_catalog::THROTTLE_BRAKE_TRACES.kind;

/// Content is identical at both densities.
pub const DENSITIES: Densities = Densities {
    regular: (400.0, 200.0),
    full: None,
};
/// Space the tile's padding, title and meters take from the chart.
const CHART_INSET: (f32, f32) = (138.0, 45.0);

pub use pitwall_core::model::Reference;

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub reference: Reference,
    /// Canonical order; never empty.
    pub channels: Vec<Channel>,
}

pub(crate) fn channel_key(channel: Channel) -> &'static str {
    match channel {
        Channel::Throttle => "throttle",
        Channel::Brake => "brake",
        Channel::Speed => "speed",
    }
}

fn channel_name(channel: Channel) -> &'static str {
    match channel {
        Channel::Throttle => "Throttle",
        Channel::Brake => "Brake",
        Channel::Speed => "Speed",
    }
}

/// Unknown channel names are ignored; with no valid channel the pedals are drawn.
pub fn settings_of(settings: &Value) -> Settings {
    let saved: Vec<&str> = settings
        .get("channels")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let channels: Vec<Channel> = CHANNEL_ORDER
        .into_iter()
        .filter(|c| saved.contains(&channel_key(*c)))
        .collect();
    Settings {
        reference: Reference::best_or_previous_of(settings),
        channels: if channels.is_empty() {
            vec![Channel::Throttle, Channel::Brake]
        } else {
            channels
        },
    }
}

/// The settings with `channel` toggled, in canonical order; `None` when it is the last
/// channel, which cannot be deselected. The layout dialog uses the same channel rules.
pub fn toggle_channel(settings: &Value, channel: Channel) -> Option<Value> {
    let selected = settings_of(settings).channels;
    let next: Vec<&str> = CHANNEL_ORDER
        .into_iter()
        .filter(|&c| {
            if c == channel {
                !selected.contains(&c)
            } else {
                selected.contains(&c)
            }
        })
        .map(channel_key)
        .collect();
    if next.is_empty() {
        return None;
    }
    let mut out = settings.clone();
    if !out.is_object() {
        out = json!({});
    }
    out["channels"] = json!(next);
    Some(out)
}

/// What the tile keeps between renders: the rolling input history and the last fast
/// generation it read.
#[derive(Default)]
pub struct Memory {
    history: InputHistory,
    seen: Option<u64>,
    identity: Option<(u64, usize)>,
    break_pending: bool,
}

impl Memory {
    /// Record `frames` for the car at `index`, oldest first. A frame whose car has no
    /// inputs records a gap.
    pub fn record(&mut self, frames: &[FastFrame], index: Option<usize>) {
        let Some(index) = index else {
            self.history.clear();
            self.identity = None;
            return;
        };
        for frame in frames {
            let identity = (frame.epoch, index);
            if self.identity != Some(identity) {
                self.history.clear();
                self.identity = Some(identity);
                self.break_pending = false;
            }
            let inputs = frame.cars.get(index).and_then(|c| c.value());
            let distance = frame.lap_distances.get(index).and_then(|d| d.value());
            if inputs.is_none() && distance.is_none() && self.history.is_empty() {
                continue;
            }
            if self.break_pending
                && let Some(last) = self.history.samples().back()
                && f64::from(frame.session_time) > last.session_time
            {
                self.history.record(
                    identity,
                    InputSample {
                        session_time: (last.session_time + f64::from(frame.session_time)) / 2.0,
                        lap_distance: f64::NAN,
                        throttle: f64::NAN,
                        brake: f64::NAN,
                        speed: f64::NAN,
                    },
                );
                self.break_pending = false;
            }
            self.history.record(
                (frame.epoch, index),
                InputSample {
                    session_time: f64::from(frame.session_time),
                    lap_distance: distance.map_or(f64::NAN, |d| f64::from(*d)),
                    throttle: inputs.map_or(f64::NAN, |c| f64::from(c.throttle) * 100.0),
                    brake: inputs.map_or(f64::NAN, |c| f64::from(c.brake) * 100.0),
                    speed: inputs.map_or(f64::NAN, |c| f64::from(c.speed)),
                },
            );
        }
    }

    #[cfg(test)]
    pub fn history(&self) -> &InputHistory {
        &self.history
    }

    /// Catch up on every fast frame since the last render, then build the tile's inputs
    /// for the car at `index`. A late joiner starts from the retained observed tail;
    /// reference-lap samples never fill earlier current time history.
    #[allow(clippy::too_many_arguments)]
    pub fn inputs<'a>(
        &'a mut self,
        reader: &Reader,
        fast: &FastFrame,
        model: &'a SessionModel,
        index: Option<usize>,
        settings: &Value,
        units: Units,
        size: (f32, f32),
    ) -> Inputs<'a> {
        if self.identity != index.map(|i| (fast.epoch, i)) {
            self.history.clear();
            self.seen = None;
            self.identity = index.map(|i| (fast.epoch, i));
            self.break_pending = false;
        }
        let frames = match self.seen {
            None => reader.fast_since(0).frames,
            Some(seen) => {
                let batch = reader.fast_since(seen);
                self.break_pending |= batch.overrun;
                batch.frames
            }
        };
        let frames: Vec<_> = frames
            .into_iter()
            .filter(|f| {
                f.generation <= fast.generation
                    && f.epoch == fast.epoch
                    && f.session_time >= fast.session_time - FAST_HISTORY_SECONDS
            })
            .collect();
        if let Some(last) = frames.last() {
            self.seen = Some(last.generation);
        }
        self.record(&frames, index);
        let settings = settings_of(settings);
        let car = index
            .filter(|_| fast.epoch == model.epoch())
            .and_then(|i| model.car(i));
        let reference = car.and_then(|c| c.lap_trace(settings.reference));
        let inputs = index
            .filter(|_| fast.epoch == model.epoch())
            .and_then(|i| fast.cars.get(i).copied());
        Inputs {
            history: &self.history,
            reference: reference.map(|t| t.samples.as_slice()),
            state: inputs.map_or(ReadoutState::Absent, |c| c.state()),
            pedals: inputs
                .and_then(|c| c.value().copied())
                .map(|c| (c.throttle, c.brake)),
            settings,
            units,
            width: size.0,
            height: size.1,
        }
    }
}

pub struct Inputs<'a> {
    pub history: &'a InputHistory,
    /// The reference lap's distance trace, when the model holds it.
    pub reference: Option<&'a [LapSample]>,
    /// The bound car's newest inputs readout state.
    pub state: ReadoutState,
    /// Throttle and brake, 0 to 1, when the newest frame has them.
    pub pedals: Option<(f32, f32)>,
    pub settings: Settings,
    pub units: Units,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Meter {
    pub label: &'static str,
    pub value: ReadoutView,
    /// 0 to 1; empty without a value.
    pub fill: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Series {
    pub channel: Channel,
    pub lap: Lap,
    /// Pedal lines smoothed for presentation; speed exact. `None` is a gap.
    pub values: Vec<Option<f64>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Plot {
    /// Seconds before the newest sample, ascending to 0.
    pub x: Vec<f64>,
    /// Reference series first, then current.
    pub series: Vec<Series>,
    pub window_seconds: f64,
    /// Top of the speed axis: from zero, rounded up in 50-unit steps, at least 50.
    pub speed_max: f64,
    pub speed_unit: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
pub struct View {
    pub density: Density,
    /// "Throttle · Brake · Dim best lap".
    pub note: String,
    /// Why nothing (or something old) is drawn; `None` hides it.
    pub status: Option<&'static str>,
    /// Stale traces are drawn at reduced opacity.
    pub opacity: f32,
    pub meters: [Meter; 2],
    pub plot: Plot,
    pub chart: (f32, f32),
}

pub fn view(inputs: &Inputs) -> View {
    let settings = &inputs.settings;
    let unit = units::speed(0.0, inputs.units).unit;
    let raw = build_window_plot(
        inputs.history,
        inputs.reference,
        &settings.channels,
        |kmh| units::speed(kmh, inputs.units).value,
    );
    let speed_top = raw
        .series
        .iter()
        .filter(|s| s.channel == Channel::Speed)
        .flat_map(|s| s.values.iter().flatten())
        .fold(0.0f64, |a, &b| a.max(b));
    let series = raw
        .series
        .iter()
        .map(|s| Series {
            channel: s.channel,
            lap: s.lap,
            values: if s.channel == Channel::Speed {
                s.values.clone()
            } else {
                smooth_values(&raw.x, &s.values, PEDAL_SMOOTH_SECONDS)
            },
        })
        .collect();
    let state = inputs.state;
    let showing = state.has_data();
    let meter = |label, value: Option<f32>| {
        let v = value.filter(|_| showing).unwrap_or(0.0);
        Meter {
            label,
            value: readout_view(state, &format!("{}%", (v * 100.0).round())),
            fill: v.clamp(0.0, 1.0),
        }
    };
    let status = if raw.has_data && state == ReadoutState::Live {
        None
    } else if state == ReadoutState::Restricted {
        Some("Restricted")
    } else if raw.has_data && state == ReadoutState::Stale {
        Some("Stale \u{25F7}")
    } else {
        Some("No data")
    };
    let legend: Vec<&str> = settings.channels.iter().map(|c| channel_name(*c)).collect();
    View {
        density: DENSITIES.pick(inputs.width, inputs.height),
        note: format!(
            "{} \u{b7} Dim {}",
            legend.join(" \u{b7} "),
            match settings.reference {
                Reference::Previous => "previous lap",
                _ => "best lap",
            }
        ),
        status,
        opacity: if state == ReadoutState::Stale {
            0.55
        } else {
            1.0
        },
        meters: [
            meter("THR", inputs.pedals.map(|p| p.0)),
            meter("BRK", inputs.pedals.map(|p| p.1)),
        ],
        plot: Plot {
            x: raw.x.clone(),
            series,
            window_seconds: INPUT_WINDOW_SECONDS,
            speed_max: ((speed_top / 50.0).ceil() * 50.0).max(50.0),
            speed_unit: unit,
        },
        chart: (
            (inputs.width - CHART_INSET.0).max(1.0),
            (inputs.height - CHART_INSET.1).max(1.0),
        ),
    }
}

fn channel_colour(channel: Channel) -> Hsla {
    match channel {
        Channel::Throttle => theme::energy(),
        Channel::Brake => theme::brake(),
        Channel::Speed => theme::text(),
    }
}

fn render_meter(meter: &Meter, colour: Hsla) -> impl IntoElement + use<> {
    let fill = if meter.value.state == ReadoutState::Stale {
        theme::muted()
    } else {
        colour
    };
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap_1()
        .w(px(48.0))
        .h_full()
        .child(widgets::label(meter.label))
        .child(
            div()
                .relative()
                .flex_1()
                .w(px(14.0))
                .rounded(px(2.0))
                .bg(theme::meter_empty())
                .child(
                    div()
                        .absolute()
                        .bottom_0()
                        .left_0()
                        .right_0()
                        .h(relative(meter.fill))
                        .rounded(px(2.0))
                        .bg(fill),
                ),
        )
        .child(widgets::readout(&meter.value, 14.0))
}

pub fn render(view: View) -> impl IntoElement {
    let (cw, ch) = view.chart;
    let mut chart = div()
        .relative()
        .w(px(cw))
        .h(px(ch))
        .flex_none()
        .opacity(view.opacity)
        .child(render_plot(view.plot.clone(), cw, ch));
    if let Some(status) = view.status {
        chart = chart.child(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .child(widgets::label(status)),
        );
    }
    let body = div()
        .flex()
        .flex_col()
        .gap_2()
        .size_full()
        .child(
            div()
                .flex()
                .justify_between()
                .child(widgets::title("Driver inputs / last 30 s"))
                .child(widgets::label(view.note.clone())),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .flex_1()
                .min_h_0()
                .child(render_meter(&view.meters[0], theme::energy()))
                .child(render_meter(&view.meters[1], theme::brake()))
                .child(chart),
        );
    // The tile keeps its shape: the status sits over the chart, never over the tile.
    widgets::tile_shell(body, None)
}

const AXIS_LEFT: f32 = 44.0;
const AXIS_BOTTOM: f32 = 24.0;
const AXIS_RIGHT: f32 = 52.0;

/// Bright thick current lines over dim thin reference lines; percent on the left axis,
/// speed on the right, seconds below. No cursor, no selection.
fn render_plot(plot: Plot, w: f32, h: f32) -> impl IntoElement {
    let has_speed = plot.series.iter().any(|s| s.channel == Channel::Speed);
    let has_pct = plot.series.iter().any(|s| s.channel != Channel::Speed);
    let right = if has_speed { w - AXIS_RIGHT } else { w - 8.0 };
    let left = if has_pct { AXIS_LEFT } else { 8.0 };
    let bottom = h - AXIS_BOTTOM;
    let mut labels: Vec<(f32, f32, String)> = vec![];
    for i in 0..=3 {
        let s = -plot.window_seconds + 10.0 * f64::from(i);
        let x = left + (right - left) * i as f32 / 3.0;
        labels.push((x - 12.0, bottom + 6.0, format!("{s}")));
    }
    if has_pct {
        for pct in [0, 50, 100] {
            let y = 4.0 + (bottom - 4.0) * (1.0 - pct as f32 / 100.0);
            labels.push((0.0, y - 7.0, format!("{pct}%")));
        }
    }
    if has_speed {
        for k in [0.0, 0.5, 1.0] {
            let y = 4.0 + (bottom - 4.0) * (1.0 - k as f32);
            labels.push((
                right + 6.0,
                y - 7.0,
                format!("{}", (plot.speed_max * k).round()),
            ));
        }
    }
    let paint = canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            let top = 4.0;
            let x = |s: f64| {
                left + ((s + plot.window_seconds) / plot.window_seconds) as f32 * (right - left)
            };
            for i in 0..=4 {
                let y = top + (bottom - top) * i as f32 / 4.0;
                paint::polyline(window, bounds, &[(left, y), (right, y)], 1.0, theme::line());
            }
            for series in plot.series.iter() {
                let max = if series.channel == Channel::Speed {
                    plot.speed_max
                } else {
                    100.0
                };
                let y = |v: f64| bottom - (v / max).clamp(0.0, 1.0) as f32 * (bottom - top);
                let (width, alpha) = match series.lap {
                    Lap::Current => (3.5, 1.0),
                    Lap::Reference => (1.5, 0.4),
                };
                let colour = channel_colour(series.channel).opacity(alpha);
                let mut run: Vec<(f32, f32)> = vec![];
                for (i, v) in series.values.iter().enumerate() {
                    match v {
                        Some(v) => run.push((x(plot.x[i]), y(*v))),
                        None => {
                            paint::polyline(window, bounds, &run, width, colour);
                            run.clear();
                        }
                    }
                }
                paint::polyline(window, bounds, &run, width, colour);
            }
        },
    )
    .size_full();
    let mut root = div().relative().size_full().child(paint);
    for (lx, ly, text) in labels {
        root = root.child(
            div()
                .absolute()
                .left(px(lx))
                .top(px(ly))
                .text_size(px(11.0))
                .line_height(px(14.0))
                .text_color(theme::muted())
                .child(text),
        );
    }
    root
}

#[cfg(test)]
mod tests {
    use super::*;
    use pitwall_core::{feed::CarInputs, model::Readout, telemetry::MAX_CARS, units::SpeedUnit};

    fn frame(
        epoch: u64,
        time: f32,
        car: usize,
        distance: f32,
        throttle: f32,
        brake: f32,
        speed: u16,
    ) -> FastFrame {
        let mut cars = [Readout::Absent; MAX_CARS];
        let mut lap_distances = [Readout::Absent; MAX_CARS];
        cars[car] = Readout::Live(CarInputs {
            speed,
            throttle: throttle / 100.0,
            brake: brake / 100.0,
            steer: 0.0,
            clutch: 0,
            gear: 1,
            engine_rpm: 0,
            drs: false,
            rev_lights_percent: 0,
        });
        lap_distances[car] = Readout::Live(distance);
        FastFrame {
            generation: 0,
            epoch,
            session_time: time,
            player_car_index: Some(0),
            cars,
            lap_distances,
            delta_best: [Readout::Absent; MAX_CARS],
            delta_previous: [Readout::Absent; MAX_CARS],
        }
    }
    fn f(seq: u32, distance: f32, throttle: f32, brake: f32) -> FastFrame {
        frame(1, seq as f32 / 10.0, 0, distance, throttle, brake, 200)
    }
    fn inputs<'a>(
        memory: &'a Memory,
        settings: Value,
        reference: Option<&'a [LapSample]>,
    ) -> Inputs<'a> {
        Inputs {
            history: memory.history(),
            reference,
            state: ReadoutState::Live,
            pedals: Some((1.0, 0.0)),
            settings: settings_of(&settings),
            units: Units::DEFAULT,
            width: 700.0,
            height: 400.0,
        }
    }
    fn values(view: &View, channel: Channel, lap: Lap) -> Option<Vec<Option<f64>>> {
        view.plot
            .series
            .iter()
            .find(|s| s.channel == channel && s.lap == lap)
            .map(|s| s.values.clone())
    }
    fn reference_trace(throttle: f32) -> Vec<LapSample> {
        [(0.0, 1.0, 0.0), (5.0, 0.5, 0.0), (10.0, 0.0, 1.0)]
            .into_iter()
            .map(|(d, t, b)| LapSample {
                distance_m: d,
                time_ms: d * 20.0,
                session_time: 0.0,
                speed_kph: Some(100.0 + d * 10.0),
                throttle: Some(if throttle < 0.0 { t } else { throttle }),
                brake: Some(b),
            })
            .collect()
    }

    #[test]
    fn declares_the_catalogue_entry() {
        assert_eq!(
            (TYPE, pitwall_core::tile_catalog::THROTTLE_BRAKE_TRACES.name),
            ("throttle-brake-traces", "Throttle/brake traces")
        );
        assert_eq!(DENSITIES.pick(399.0, 200.0), Density::Compact);
        assert_eq!(DENSITIES.pick(400.0, 200.0), Density::Regular);
    }

    #[test]
    fn draws_recent_frames_against_time_with_throttle_and_brake_in_percent() {
        let mut memory = Memory::default();
        memory.record(
            &[
                f(1, 0.0, 100.0, 0.0),
                f(2, 10.0, 50.0, 0.0),
                f(3, 20.0, 0.0, 80.0),
            ],
            Some(0),
        );
        let v = view(&inputs(&memory, json!({}), None));
        let x: Vec<f64> = v
            .plot
            .x
            .iter()
            .map(|x| (x * 1000.0).round() / 1000.0)
            .collect();
        assert_eq!(x, [-0.2, -0.1, 0.0]);
        assert_eq!(v.plot.window_seconds, 30.0);
        let thr: Vec<f64> = values(&v, Channel::Throttle, Lap::Current)
            .unwrap()
            .into_iter()
            .map(|v| v.unwrap().round())
            .collect();
        assert_eq!(thr, [100.0, 50.0, 0.0]);
        let brk: Vec<f64> = values(&v, Channel::Brake, Lap::Current)
            .unwrap()
            .into_iter()
            .map(|v| v.unwrap().round())
            .collect();
        assert_eq!(brk, [0.0, 0.0, 80.0]);
        assert!(v.plot.series.iter().all(|s| s.channel != Channel::Speed));
    }

    #[test]
    fn draws_continuous_10_hz_samples_without_distance_bucketing() {
        let mut memory = Memory::default();
        let frames: Vec<FastFrame> = (0..=12)
            .map(|n| {
                f(
                    n + 1,
                    n as f32 * (300.0 / 3.6 / 10.0),
                    100.0 - n as f32 * 5.0,
                    n as f32 * 5.0,
                )
            })
            .collect();
        memory.record(&frames, Some(0));
        let v = view(&inputs(&memory, json!({}), None));
        assert_eq!(v.plot.x.len(), 13);
        assert!((v.plot.x[0] + 1.2).abs() < 1e-5);
        assert_eq!(*v.plot.x.last().unwrap(), 0.0);
        assert!(
            values(&v, Channel::Throttle, Lap::Current)
                .unwrap()
                .iter()
                .all(Option::is_some)
        );
        assert!(
            values(&v, Channel::Brake, Lap::Current)
                .unwrap()
                .iter()
                .all(Option::is_some)
        );
    }

    #[test]
    fn overlays_the_reference_lap_at_the_same_distance_in_percent() {
        let mut memory = Memory::default();
        memory.record(&[f(1, 0.0, 100.0, 0.0)], Some(0));
        let best = reference_trace(-1.0);
        let v = view(&inputs(&memory, json!({}), Some(&best)));
        assert_eq!(
            values(&v, Channel::Throttle, Lap::Reference),
            Some(vec![Some(100.0)])
        );
        assert_eq!(
            values(&v, Channel::Brake, Lap::Reference),
            Some(vec![Some(0.0)])
        );
        let previous = reference_trace(0.2);
        let v = view(&inputs(
            &memory,
            json!({"reference": "previous"}),
            Some(&previous),
        ));
        let thr = values(&v, Channel::Throttle, Lap::Reference).unwrap();
        assert!((thr[0].unwrap() - 20.0).abs() < 1e-4);
        assert!(v.note.ends_with("Dim previous lap"));
        let none = view(&inputs(&memory, json!({}), None));
        assert_eq!(values(&none, Channel::Throttle, Lap::Reference), None);
    }

    #[test]
    fn a_late_joiner_starts_with_observed_inputs_only() {
        let mut memory = Memory::default();
        memory.record(&[f(1, 10.0, 40.0, 0.0)], Some(0));
        let v = view(&inputs(&memory, json!({}), None));
        assert_eq!(v.plot.x, [0.0]);
        assert_eq!(
            values(&v, Channel::Throttle, Lap::Current).unwrap()[0].map(f64::round),
            Some(40.0)
        );
    }

    #[test]
    fn a_new_epoch_clears_the_history() {
        let mut memory = Memory::default();
        memory.record(&[f(1, 0.0, 100.0, 0.0), f(2, 100.0, 100.0, 0.0)], Some(0));
        assert_eq!(memory.history().len(), 2);
        memory.record(&[frame(2, 0.1, 0, 5.0, 70.0, 0.0, 200)], Some(0));
        let v = view(&inputs(&memory, json!({}), None));
        assert_eq!(v.plot.x, [0.0]);
        assert_eq!(
            values(&v, Channel::Throttle, Lap::Current).unwrap()[0].map(f64::round),
            Some(70.0)
        );
    }

    #[test]
    fn following_another_car_starts_a_fresh_history_for_that_car() {
        let mut memory = Memory::default();
        memory.record(&[f(1, 0.0, 100.0, 0.0)], Some(0));
        memory.record(&[frame(1, 9.0, 1, 50.0, 30.0, 0.0, 100)], Some(1));
        let v = view(&inputs(&memory, json!({}), None));
        assert_eq!(v.plot.x, [0.0]);
        assert_eq!(
            values(&v, Channel::Throttle, Lap::Current).unwrap()[0].map(f64::round),
            Some(30.0)
        );
    }

    #[test]
    fn the_channels_setting_chooses_what_is_plotted_with_speed_in_the_devices_unit() {
        let mut memory = Memory::default();
        memory.record(&[frame(1, 0.1, 0, 0.0, 100.0, 0.0, 100)], Some(0));
        let v = view(&Inputs {
            units: Units {
                speed: SpeedUnit::Mph,
                ..Units::DEFAULT
            },
            ..inputs(&memory, json!({"channels": ["speed"]}), None)
        });
        let current: Vec<Channel> = v
            .plot
            .series
            .iter()
            .filter(|s| s.lap == Lap::Current)
            .map(|s| s.channel)
            .collect();
        assert_eq!(current, [Channel::Speed]);
        let speed = values(&v, Channel::Speed, Lap::Current).unwrap()[0].unwrap();
        assert!((speed - 62.14).abs() < 0.005);
        assert_eq!(v.plot.speed_unit, "mph");
        assert_eq!(v.plot.speed_max, 100.0);
    }

    #[test]
    fn channels_follow_canonical_order_ignore_unknowns_and_fall_back_to_pedals() {
        assert_eq!(
            settings_of(&json!({"channels": ["speed", "brake", "nonsense"]})).channels,
            [Channel::Brake, Channel::Speed]
        );
        assert_eq!(
            settings_of(&json!({"channels": ["nonsense"]})).channels,
            [Channel::Throttle, Channel::Brake]
        );
        assert_eq!(settings_of(&json!({})).reference, Reference::Best);
    }

    #[test]
    fn the_channel_setting_toggles_in_order_keeps_unknown_keys_and_keeps_the_last_channel() {
        assert_eq!(
            toggle_channel(&json!({"other": 1}), Channel::Speed),
            Some(json!({"other": 1, "channels": ["throttle", "brake", "speed"]}))
        );
        // The last channel stays.
        assert_eq!(
            toggle_channel(&json!({"channels": ["throttle"]}), Channel::Throttle),
            None
        );
    }

    #[test]
    fn says_why_there_is_nothing_to_draw_until_the_car_has_data() {
        let mut memory = Memory::default();
        let empty = view(&Inputs {
            state: ReadoutState::Absent,
            pedals: None,
            ..inputs(&memory, json!({}), None)
        });
        assert_eq!(empty.status, Some("No data"));
        assert_eq!(empty.meters[0].value.text, "\u{2014}");
        memory.record(&[f(1, 0.0, 100.0, 0.0)], Some(0));
        let drawn = view(&inputs(&memory, json!({}), None));
        assert_eq!(drawn.status, None);
        assert_eq!(drawn.meters[0].value.text, "100%");
        assert_eq!(drawn.meters[0].fill, 1.0);
        let restricted = view(&Inputs {
            state: ReadoutState::Restricted,
            pedals: None,
            ..inputs(&memory, json!({}), None)
        });
        assert_eq!(restricted.status, Some("Restricted"));
        assert_eq!(restricted.meters[1].fill, 0.0);
    }

    #[test]
    fn crossing_the_line_preserves_recent_input_history() {
        let mut memory = Memory::default();
        memory.record(&[f(1, 0.0, 100.0, 0.0), f(2, 5000.0, 80.0, 0.0)], Some(0));
        memory.record(&[f(3, 0.0, 30.0, 0.0)], Some(0));
        let v = view(&inputs(&memory, json!({}), None));
        let thr: Vec<f64> = values(&v, Channel::Throttle, Lap::Current)
            .unwrap()
            .into_iter()
            .map(|v| v.unwrap())
            .collect();
        assert_eq!(thr.len(), 3);
        assert_eq!(
            (thr[0].round(), thr[2].round()),
            (100.0, 30.0),
            "ends stay exact"
        );
    }

    #[test]
    fn stale_dims_the_last_trace_and_marks_it_without_new_telemetry() {
        let mut memory = Memory::default();
        memory.record(&[f(1, 0.0, 100.0, 0.0)], Some(0));
        let v = view(&Inputs {
            state: ReadoutState::Stale,
            ..inputs(&memory, json!({}), None)
        });
        assert_eq!(v.status, Some("Stale \u{25F7}"));
        assert_eq!(v.opacity, 0.55);
        assert_eq!(
            values(&v, Channel::Throttle, Lap::Current).unwrap()[0].map(f64::round),
            Some(100.0)
        );
    }
}
