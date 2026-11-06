use std::collections::VecDeque;

use crate::model::LapSample;

/// Seconds of history the inputs tile plots.
pub const INPUT_WINDOW_SECONDS: f64 = 30.0;
/// A pause in samples longer than this breaks a time-axis line instead of joining across it.
pub const GAP_SECONDS: f64 = 0.5;
/// Reference samples further apart than this are never joined (spec section 6).
const MAX_INTERPOLATION_METRES: f64 = 100.0;

/// A plotted quantity. Throttle and brake are percent in [`InputSample`] and fractions in a
/// [`LapSample`]; speed is km/h in both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Throttle,
    Brake,
    Speed,
}

/// The order series are plotted in, whatever order a setting lists them.
pub const CHANNEL_ORDER: [Channel; 3] = [Channel::Throttle, Channel::Brake, Channel::Speed];

/// One frame-rate sample of a car's inputs. Any value may be NaN when the game withheld it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InputSample {
    /// The game's session clock, seconds.
    pub session_time: f64,
    /// Metres along the lap.
    pub lap_distance: f64,
    /// Percent, 0 to 100.
    pub throttle: f64,
    /// Percent, 0 to 100.
    pub brake: f64,
    /// km/h.
    pub speed: f64,
}

impl InputSample {
    pub fn value(&self, channel: Channel) -> f64 {
        match channel {
            Channel::Throttle => self.throttle,
            Channel::Brake => self.brake,
            Channel::Speed => self.speed,
        }
    }
}

/// Which car's timeline a history belongs to: `(epoch, car index)`.
pub type HistoryIdentity = (u64, usize);

/// The rolling input history a trace is drawn from: the last 30 s of publications.
/// A new identity (epoch or car) or a step back in session time starts it again; a sample
/// at the same session time as the newest replaces it, so a paused replay that keeps
/// publishing the same instant never evicts what was received.
#[derive(Debug, Clone, Default)]
pub struct InputHistory {
    identity: Option<HistoryIdentity>,
    samples: VecDeque<InputSample>,
}

impl InputHistory {
    /// A safety bound above 30 seconds at the native feed's 240 Hz budget. Unlike the
    /// web's 1801 frames, native publications include multiple packet groups per frame.
    pub const CAPACITY: usize = 20_000;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, identity: HistoryIdentity, sample: InputSample) {
        if self.identity != Some(identity) {
            self.samples.clear();
            self.identity = Some(identity);
        }
        match self.samples.back_mut() {
            Some(last) if sample.session_time == last.session_time => *last = sample,
            Some(last) if sample.session_time < last.session_time => {
                self.samples.clear();
                self.samples.push_back(sample);
            }
            _ => {
                if self.samples.len() == Self::CAPACITY {
                    self.samples.pop_front();
                }
                self.samples.push_back(sample);
            }
        }
        let cutoff = sample.session_time - INPUT_WINDOW_SECONDS;
        while self
            .samples
            .front()
            .is_some_and(|s| s.session_time < cutoff)
        {
            self.samples.pop_front();
        }
    }

    /// Forget everything, as when the overlay loses its car.
    pub fn clear(&mut self) {
        self.samples.clear();
        self.identity = None;
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Oldest first.
    pub fn samples(&self) -> &VecDeque<InputSample> {
        &self.samples
    }
}

/// The reference lap's value at the same circuit position as a live sample, linearly
/// interpolated between its neighbouring 5 m samples. Throttle and brake are fractions.
/// `None` outside the reference, across a missing value or a gap over 100 m: never
/// extrapolated.
pub fn reference_value(trace: &[LapSample], distance: f64, channel: Channel) -> Option<f64> {
    if !distance.is_finite() {
        return None;
    }
    let value = |s: &LapSample| {
        match channel {
            Channel::Throttle => s.throttle,
            Channel::Brake => s.brake,
            Channel::Speed => s.speed_kph,
        }
        .map(f64::from)
    };
    let index = trace.partition_point(|s| f64::from(s.distance_m) < distance);
    let right = trace.get(index);
    if let Some(right) = right
        && f64::from(right.distance_m) == distance
    {
        return value(right);
    }
    let left = trace.get(index.checked_sub(1)?)?;
    let right = right?;
    let (start, end) = (f64::from(left.distance_m), f64::from(right.distance_m));
    if end - start > MAX_INTERPOLATION_METRES {
        return None;
    }
    let (a, b) = (value(left)?, value(right)?);
    Some(a + (b - a) * (distance - start) / (end - start))
}

/// Whether a series is the live lap or the dim reference lap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lap {
    Current,
    Reference,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlotSeries {
    pub channel: Channel,
    pub lap: Lap,
    /// Percent for throttle and brake, the caller's speed unit for speed; `None` is a gap.
    pub values: Vec<Option<f64>>,
}

/// A rolling time plot: `x` is seconds relative to the newest sample, from
/// `-window_seconds` to 0. Reference series come before current ones.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowPlot {
    pub x: Vec<f64>,
    pub series: Vec<PlotSeries>,
    pub window_seconds: f64,
    pub has_data: bool,
}

/// Recent telemetry in session time: the axis never grows, and crossing the line keeps the
/// inputs. `speed` converts km/h to the display unit.
pub fn build_window_plot(
    history: &InputHistory,
    reference: Option<&[LapSample]>,
    channels: &[Channel],
    speed: impl Fn(f64) -> f64,
) -> WindowPlot {
    let chosen: Vec<Channel> = CHANNEL_ORDER
        .into_iter()
        .filter(|c| channels.contains(c))
        .collect();
    let mut series = vec![];
    if reference.is_some() {
        series.extend(chosen.iter().map(|&channel| PlotSeries {
            channel,
            lap: Lap::Reference,
            values: vec![],
        }));
    }
    series.extend(chosen.iter().map(|&channel| PlotSeries {
        channel,
        lap: Lap::Current,
        values: vec![],
    }));
    let mut plot = WindowPlot {
        x: vec![],
        series,
        window_seconds: INPUT_WINDOW_SECONDS,
        has_data: false,
    };
    let samples = history.samples();
    let Some(now) = samples.back().map(|s| s.session_time) else {
        return plot;
    };
    let mut first = samples.len() - 1;
    while first > 0
        && samples[first - 1].session_time >= now - INPUT_WINDOW_SECONDS
        && samples[first - 1].session_time <= samples[first].session_time
    {
        first -= 1;
    }
    let mut previous = f64::NEG_INFINITY;
    for sample in samples.range(first..) {
        let time = sample.session_time;
        if !time.is_finite() || time <= previous {
            continue;
        }
        if previous.is_finite() && time - previous > GAP_SECONDS {
            plot.x.push((previous + time) / 2.0 - now);
            for s in &mut plot.series {
                s.values.push(None);
            }
        }
        plot.x.push(time - now);
        for s in &mut plot.series {
            let raw = match s.lap {
                Lap::Current => Some(sample.value(s.channel)),
                Lap::Reference => {
                    reference_value(reference.unwrap_or(&[]), sample.lap_distance, s.channel)
                }
            };
            let value = raw
                .filter(|v| v.is_finite())
                .map(|v| match (s.channel, s.lap) {
                    (Channel::Speed, _) => speed(v),
                    (_, Lap::Reference) => v * 100.0,
                    _ => v,
                });
            s.values.push(value);
            if s.lap == Lap::Current && value.is_some() {
                plot.has_data = true;
            }
        }
        previous = time;
    }
    plot
}
