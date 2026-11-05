//! The native tile catalogue: each tile type's saved name, display name, and one entry
//! point that builds its view model from core data and renders it. A layout host looks a
//! saved `type` up here and calls [`render`] for its placed tile.

use gpui::{AnyElement, IntoElement};
use pitwall_core::{
    feed::{FastFrame, Reader},
    model::SessionModel,
    units::Units,
};
use serde_json::Value;

use super::{
    car_status, engine_temp, ers, fuel, g_force, lap_timing, live_delta, speed_rpm_gear,
    throttle_brake_traces,
};

/// A native tile type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    SpeedRpmGear,
    LapTiming,
    GForce,
    ThrottleBrakeTraces,
    LiveDelta,
    Ers,
    CarStatus,
    EngineTemp,
    Fuel,
}

impl Kind {
    pub const ALL: [Kind; 9] = [
        Kind::SpeedRpmGear,
        Kind::LapTiming,
        Kind::GForce,
        Kind::ThrottleBrakeTraces,
        Kind::LiveDelta,
        Kind::Ers,
        Kind::CarStatus,
        Kind::EngineTemp,
        Kind::Fuel,
    ];

    /// The type a layout file saves, as the web build names it.
    pub fn type_name(self) -> &'static str {
        match self {
            Kind::SpeedRpmGear => "speed-rpm-gear",
            Kind::LapTiming => "lap-timing",
            Kind::GForce => g_force::TYPE,
            Kind::ThrottleBrakeTraces => throttle_brake_traces::TYPE,
            Kind::LiveDelta => live_delta::TYPE,
            Kind::Ers => ers::TYPE,
            Kind::CarStatus => car_status::TYPE,
            Kind::EngineTemp => engine_temp::TYPE,
            Kind::Fuel => fuel::TYPE,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Kind::SpeedRpmGear => speed_rpm_gear::NAME,
            Kind::LapTiming => lap_timing::NAME,
            Kind::GForce => g_force::NAME,
            Kind::ThrottleBrakeTraces => throttle_brake_traces::NAME,
            Kind::LiveDelta => live_delta::NAME,
            Kind::Ers => ers::NAME,
            Kind::CarStatus => car_status::NAME,
            Kind::EngineTemp => engine_temp::NAME,
            Kind::Fuel => fuel::NAME,
        }
    }

    pub fn of(type_name: &str) -> Option<Kind> {
        Self::ALL.into_iter().find(|k| k.type_name() == type_name)
    }
}

/// What a placed tile draws from. `index` is the bound car after binding resolution;
/// `None` draws the tile's absent state.
pub struct Context<'a> {
    pub reader: &'a Reader,
    pub fast: &'a FastFrame,
    pub model: &'a SessionModel,
    pub index: Option<usize>,
    pub retired: bool,
    pub settings: &'a Value,
    pub units: Units,
    /// The tile body's size in pixels.
    pub size: (f32, f32),
}

/// What a placed tile keeps between renders. Most tiles keep nothing; the inputs tile
/// keeps its rolling 30 s history.
#[derive(Default)]
pub struct Memory {
    traces: throttle_brake_traces::Memory,
    car_status: car_status::Memory,
}

pub fn render(kind: Kind, ctx: &Context, memory: &mut Memory) -> AnyElement {
    match kind {
        Kind::SpeedRpmGear => {
            speed_rpm_gear::render(speed_rpm_gear::view(&speed_rpm_gear::Inputs::from_source(
                ctx.fast, ctx.model, ctx.index, ctx.units, ctx.size,
            )))
            .into_any_element()
        }
        Kind::LapTiming => lap_timing::render(lap_timing::view(&lap_timing::Inputs::from_model(
            ctx.model,
            ctx.index,
            lap_timing::LapReference::of(ctx.settings),
            ctx.size,
        )))
        .into_any_element(),
        Kind::GForce => g_force::render(g_force::view(&g_force::Inputs::from_model(
            ctx.model, ctx.index, ctx.size,
        )))
        .into_any_element(),
        Kind::ThrottleBrakeTraces => {
            throttle_brake_traces::render(throttle_brake_traces::view(&memory.traces.inputs(ctx)))
                .into_any_element()
        }
        Kind::LiveDelta => live_delta::render(live_delta::view(&live_delta::Inputs::from_model(
            ctx.model,
            ctx.index,
            live_delta::Reference::of(ctx.settings),
            ctx.size,
        )))
        .into_any_element(),
        Kind::Ers => ers::render(ers::view(&ers::Inputs::from_model(
            ctx.model, ctx.index, ctx.size,
        )))
        .into_any_element(),
        Kind::CarStatus => car_status::render(
            car_status::view(&car_status::Inputs::from_model(
                ctx.model,
                ctx.index,
                ctx.retired,
                ctx.units,
                ctx.size,
            )),
            &mut memory.car_status,
        )
        .into_any_element(),
        Kind::EngineTemp => engine_temp::render(engine_temp::view(
            &engine_temp::Inputs::from_model(ctx.model, ctx.index, ctx.units, ctx.size),
        ))
        .into_any_element(),
        Kind::Fuel => fuel::render(fuel::view(&fuel::Inputs::from_model(
            ctx.model, ctx.index, ctx.size,
        )))
        .into_any_element(),
    }
}

/// The tile's comparison lap, for tiles that have one.
pub fn reference_label(kind: Kind, settings: &Value) -> Option<&'static str> {
    match kind {
        Kind::LapTiming => Some(lap_timing::LapReference::of(settings).label()),
        Kind::LiveDelta => Some(live_delta::Reference::of(settings).label()),
        Kind::ThrottleBrakeTraces => Some(
            match throttle_brake_traces::settings_of(settings).reference {
                throttle_brake_traces::Reference::Best => "Best lap",
                throttle_brake_traces::Reference::Previous => "Previous lap",
            },
        ),
        _ => None,
    }
}

/// Step the comparison lap to the next choice, keeping every other setting. Time Trial
/// choices exist for Lap timing only and are skipped outside Time Trial.
pub fn cycle_reference(kind: Kind, settings: &mut Value, time_trial: bool) {
    match kind {
        Kind::LapTiming => lap_timing::LapReference::cycle(settings, time_trial),
        Kind::LiveDelta => {
            *settings = live_delta::with_reference(
                settings,
                match live_delta::Reference::of(settings) {
                    live_delta::Reference::Best => live_delta::Reference::Previous,
                    live_delta::Reference::Previous => live_delta::Reference::Best,
                },
            )
        }
        Kind::ThrottleBrakeTraces => {
            let next = match throttle_brake_traces::settings_of(settings).reference {
                throttle_brake_traces::Reference::Best => {
                    throttle_brake_traces::Reference::Previous
                }
                throttle_brake_traces::Reference::Previous => {
                    throttle_brake_traces::Reference::Best
                }
            };
            *settings = throttle_brake_traces::with_reference(settings, next);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_type_round_trips_by_its_saved_name() {
        for kind in Kind::ALL {
            assert_eq!(Kind::of(kind.type_name()), Some(kind));
        }
        assert_eq!(Kind::of("no-such-tile"), None);
        assert_eq!(Kind::of("ers").map(Kind::name), Some("ERS"));
        assert_eq!(
            Kind::of("throttle-brake-traces").map(Kind::name),
            Some("Throttle/brake traces")
        );
    }

    #[test]
    fn the_comparison_lap_cycles_best_and_previous_keeping_other_settings() {
        use serde_json::json;
        for kind in [Kind::LiveDelta, Kind::ThrottleBrakeTraces] {
            let mut settings = json!({ "car": { "target": "car-ahead" }, "keep": 1 });
            assert_eq!(reference_label(kind, &settings), Some("Best lap"));
            cycle_reference(kind, &mut settings, false);
            assert_eq!(reference_label(kind, &settings), Some("Previous lap"));
            assert_eq!(settings["keep"], 1);
            assert_eq!(settings["car"]["target"], "car-ahead");
            cycle_reference(kind, &mut settings, true);
            assert_eq!(settings["reference"], "best");
        }
        assert_eq!(reference_label(Kind::GForce, &json!({})), None);
    }
}
