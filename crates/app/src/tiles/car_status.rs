//! Car status (parity CAR-01/02, THERMAL-01/02, LIVERY): a slow tile following one car.
//! Port of `dashboard/src/tiles/types/car-status.tsx` and `car-status/view.ts`.
//!
//! Wheel arrays are in the game's order: rear-left, rear-right, front-left, front-right.
//! On screen the rear axle sits above the front and the driver's left is on the right.

use std::{
    path::PathBuf,
    sync::{Arc, OnceLock},
};

use gpui::{
    Div, Hsla, ImageSource, IntoElement, ObjectFit, ParentElement, RenderImage, Styled,
    StyledImage, div, img, px, relative,
};
use pitwall_core::{
    bands::{
        BandName, ConditionLevel, DAMAGE_PARTS, TemperatureKind, band_glyph, band_of, column_fill,
        condition_level, visual_compound_name,
    },
    livery::LiveryCar,
    model::{Lifecycle, Readout, SessionModel, StatusData},
    readout::{ReadoutState, ReadoutView, view_of, whole_tile_message},
    telemetry::{CarDamageData, CarTelemetryData},
    units::{self, PressureUnit, Units},
};
use pitwall_livery_art::{LiveryImages, RgbaImage};

use super::{Densities, Density, widgets};
use crate::theme;

pub const TYPE: &str = pitwall_core::tile_catalog::CAR_STATUS.kind;

pub const DENSITIES: Densities = Densities {
    regular: (560.0, 380.0),
    full: Some((800.0, 600.0)),
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WheelId {
    RL,
    RR,
    FL,
    FR,
}

impl WheelId {
    pub const GAME_ORDER: [WheelId; 4] = [WheelId::RL, WheelId::RR, WheelId::FL, WheelId::FR];
    pub fn label(self) -> &'static str {
        match self {
            WheelId::RL => "RL",
            WheelId::RR => "RR",
            WheelId::FL => "FL",
            WheelId::FR => "FR",
        }
    }
}

pub struct Inputs {
    pub telemetry: Readout<CarTelemetryData>,
    pub status: Readout<StatusData>,
    pub damage: Readout<CarDamageData>,
    /// Percent remaining per wheel, game order.
    pub condition: Readout<[f32; 4]>,
    /// The followed car's livery facts, when its participant entry is known.
    pub livery: Option<LiveryCar>,
    pub retired: bool,
    pub units: Units,
    pub lifecycle: Lifecycle,
    pub width: f32,
    pub height: f32,
}

impl Inputs {
    pub fn from_model(
        model: &SessionModel,
        index: Option<usize>,
        retired: bool,
        units: Units,
        size: (f32, f32),
    ) -> Self {
        let car = index.and_then(|i| model.car(i));
        Self {
            telemetry: car.map_or(Readout::Absent, |c| c.telemetry().map(Clone::clone)),
            status: car.map_or(Readout::Absent, |c| c.status().map(Clone::clone)),
            damage: car.map_or(Readout::Absent, |c| c.damage().map(Clone::clone)),
            condition: car.map_or(Readout::Absent, |c| c.tyre_condition()),
            livery: car.and_then(|c| {
                c.participant()
                    .value()
                    .map(|p| LiveryCar::from_participant(p))
            }),
            retired,
            units,
            lifecycle: model.lifecycle(),
            width: size.0,
            height: size.1,
        }
    }
}

/// A temperature cell: `band` is `None` for an unknown compound or a missing value.
#[derive(Debug, Clone, PartialEq)]
pub struct TemperatureCell {
    pub value: ReadoutView,
    pub band: Option<BandName>,
    /// Arrow beside the number for cold and hot; empty otherwise.
    pub glyph: &'static str,
    /// Column height, 0 to 1 of the column's °C scale; `None` without a value.
    pub fill: Option<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConditionCell {
    pub value: ReadoutView,
    /// 0 to 100, or `None` unless live or stale: it sets the bar length.
    pub percent: Option<f32>,
    pub level: Option<ConditionLevel>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WheelView {
    pub id: WheelId,
    pub condition: ConditionCell,
    pub brake: TemperatureCell,
    pub surface: TemperatureCell,
    pub inner: TemperatureCell,
    pub pressure: ReadoutView,
    pub brake_damage: ReadoutView,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DamageView {
    pub key: &'static str,
    pub label: &'static str,
    pub value: ReadoutView,
    /// 0 to 100, or `None` unless live or stale: it sets the bar length.
    pub percent: Option<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct View {
    pub density: Density,
    /// Game order: RL, RR, FL, FR.
    pub wheels: Vec<WheelView>,
    /// The visual compound's name.
    pub compound: ReadoutView,
    /// Laps on the fitted set.
    pub age: ReadoutView,
    /// Display order of `DAMAGE_PARTS`.
    pub damage: Vec<DamageView>,
    pub temperature_unit: &'static str,
    pub pressure_unit: &'static str,
    /// Regular and full draw the car art.
    pub show_car: bool,
    pub livery: Option<LiveryCar>,
    pub retired: bool,
    pub message: Option<&'static str>,
}

impl View {
    pub fn wheel(&self, id: WheelId) -> &WheelView {
        self.wheels
            .iter()
            .find(|w| w.id == id)
            .expect("four wheels")
    }
    pub fn part(&self, key: &str) -> &DamageView {
        self.damage
            .iter()
            .find(|d| d.key == key)
            .expect("known part")
    }
}

fn percent_text(n: f32) -> String {
    format!("{}%", n.round())
}

pub fn view(inputs: &Inputs) -> View {
    let units = inputs.units;
    let telemetry = &inputs.telemetry;
    let actual = inputs.status.value().map(|s| s.actual_tyre_compound);
    let temperature = |kind: TemperatureKind, pick: &dyn Fn(&CarTelemetryData) -> f32| {
        let value = view_of(telemetry, |t| {
            format!(
                "{}",
                units::temperature(f64::from(pick(t)), units).value.round()
            )
        });
        match telemetry.value() {
            None => TemperatureCell {
                value,
                band: None,
                glyph: "",
                fill: None,
            },
            Some(t) => {
                let celsius = pick(t);
                let band = band_of(kind, celsius, actual);
                TemperatureCell {
                    value,
                    band,
                    glyph: band_glyph(band),
                    fill: Some(column_fill(kind, celsius)),
                }
            }
        }
    };
    let wheels: Vec<WheelView> = WheelId::GAME_ORDER
        .into_iter()
        .enumerate()
        .map(|(i, id)| {
            let percent = inputs.condition.value().map(|c| c[i].clamp(0.0, 100.0));
            WheelView {
                id,
                condition: ConditionCell {
                    value: view_of(&inputs.condition, |c| percent_text(c[i].clamp(0.0, 100.0))),
                    percent,
                    level: percent.map(condition_level),
                },
                brake: temperature(TemperatureKind::Brake, &|t| {
                    f32::from(t.brakes_temperature[i])
                }),
                surface: temperature(TemperatureKind::Tyre, &|t| {
                    f32::from(t.tyres_surface_temperature[i])
                }),
                inner: temperature(TemperatureKind::Tyre, &|t| {
                    f32::from(t.tyres_inner_temperature[i])
                }),
                pressure: view_of(telemetry, |t| {
                    let p = units::pressure(f64::from(t.tyres_pressure[i]), units);
                    match units.pressure {
                        PressureUnit::Bar => super::format::fixed(p.value, 2),
                        PressureUnit::Psi => super::format::fixed(p.value, 1),
                    }
                }),
                brake_damage: view_of(&inputs.damage, |d| {
                    percent_text(f32::from(d.brakes_damage[i]))
                }),
            }
        })
        .collect();
    let compound = view_of(&inputs.status, |s| {
        visual_compound_name(s.visual_tyre_compound).into()
    });
    let age = view_of(&inputs.status, |s| s.tyres_age_laps.to_string());
    let damage: Vec<DamageView> = DAMAGE_PARTS
        .iter()
        .map(|part| DamageView {
            key: part.key,
            label: part.label,
            value: view_of(&inputs.damage, |d| percent_text(f32::from((part.read)(d)))),
            percent: inputs
                .damage
                .value()
                .map(|d| f32::from((part.read)(d)).clamp(0.0, 100.0)),
        })
        .collect();

    let states = [compound.state, age.state]
        .into_iter()
        .chain(damage.iter().map(|d| d.value.state))
        .chain(wheels.iter().flat_map(|w| {
            [
                w.condition.value.state,
                w.brake.value.state,
                w.surface.value.state,
                w.inner.value.state,
                w.pressure.state,
                w.brake_damage.state,
            ]
        }))
        .collect::<Vec<_>>();
    let density = DENSITIES.pick(inputs.width, inputs.height);
    View {
        density,
        wheels,
        compound,
        age,
        damage,
        temperature_unit: units::temperature(0.0, units).unit,
        pressure_unit: units::pressure(0.0, units).unit,
        show_car: density != Density::Compact,
        livery: inputs.livery,
        retired: inputs.retired,
        message: whole_tile_message(states, inputs.lifecycle, true),
    }
}

/// The livery pack folder (`pitwall/liveries` in the data folder). The host sets it at
/// startup from its resolved data folder; unset, the OS data folder's is used.
pub static LIVERY_FOLDER: OnceLock<PathBuf> = OnceLock::new();

/// What the tile keeps between renders: the livery pack listing (read when the tile is
/// first drawn, as the web build lists it on mount) and the last car picture uploaded.
#[derive(Default)]
pub struct Memory {
    images: Option<Arc<LiveryImages>>,
    last: Option<(Arc<RgbaImage>, Arc<RenderImage>)>,
}

impl Memory {
    /// The car picture for `car`: its pack image, else the fallback car in its colours.
    pub fn picture(&mut self, car: Option<&LiveryCar>) -> Arc<RenderImage> {
        let images = self
            .images
            .get_or_insert_with(|| {
                let folder = LIVERY_FOLDER.get().cloned().unwrap_or_else(|| {
                    pitwall_core::paths::AppPaths::resolve()
                        .map(|p| p.liveries)
                        .unwrap_or_default()
                });
                Arc::new(LiveryImages::new(folder))
            })
            .clone();
        let rgba = images.image_for(car);
        if let Some((source, image)) = &self.last
            && Arc::ptr_eq(source, &rgba)
        {
            return image.clone();
        }
        let image = Arc::new(to_render_image(&rgba));
        self.last = Some((rgba, image.clone()));
        image
    }
}

/// gpui draws BGRA frames.
fn to_render_image(rgba: &RgbaImage) -> RenderImage {
    let mut pixels = rgba.pixels.clone();
    for p in pixels.chunks_exact_mut(4) {
        p.swap(0, 2);
    }
    let buffer = image::RgbaImage::from_raw(rgba.width, rgba.height, pixels)
        .expect("width × height × 4 bytes");
    RenderImage::new([image::Frame::new(buffer)])
}

fn band_colour(band: Option<BandName>) -> Hsla {
    match band {
        Some(BandName::Cold) => theme::cold(),
        Some(BandName::Hot) => theme::hot(),
        Some(BandName::Window) => theme::window(),
        None => theme::text(),
    }
}

fn level_colour(level: Option<ConditionLevel>) -> Hsla {
    match level {
        Some(ConditionLevel::Low) => theme::hot(),
        Some(ConditionLevel::Mid) => theme::muted(),
        _ => theme::window(),
    }
}

/// A value with its band glyph, coloured by band while live.
fn value(view: &ReadoutView, size: f32, colour: Option<Hsla>, glyph: &str) -> Div {
    let shown = ReadoutView {
        state: view.state,
        text: if view.state.has_data() {
            format!("{}{glyph}", view.text)
        } else {
            view.text.clone()
        },
    };
    widgets::readout_coloured(&shown, size, colour)
}

fn column(label: &'static str, cell: &TemperatureCell, unit: &'static str) -> Div {
    let fill = cell.fill.unwrap_or(0.0);
    let ink = if cell.value.state == ReadoutState::Stale {
        theme::muted()
    } else {
        band_colour(cell.band)
    };
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap_1()
        .flex_1()
        .child(widgets::label(label))
        .child(
            div()
                .flex()
                .items_baseline()
                .gap(px(2.0))
                .child(value(
                    &cell.value,
                    15.0,
                    cell.band.map(|_| band_colour(cell.band)),
                    cell.glyph,
                ))
                .child(widgets::label(unit)),
        )
        .child(
            div()
                .relative()
                .w(px(8.0))
                .flex_1()
                .min_h(px(18.0))
                .rounded(px(2.0))
                .bg(theme::meter_empty())
                .child(
                    div()
                        .absolute()
                        .bottom_0()
                        .left_0()
                        .right_0()
                        .h(relative(fill))
                        .rounded(px(2.0))
                        .bg(ink),
                ),
        )
}

fn line(label: &'static str, v: &ReadoutView) -> Div {
    div()
        .flex()
        .justify_between()
        .items_baseline()
        .gap_2()
        .child(widgets::label(label))
        .child(widgets::readout(v, 14.0))
}

fn bar(percent: Option<f32>, colour: Hsla) -> Div {
    div()
        .h(px(6.0))
        .w_full()
        .rounded(px(3.0))
        .bg(theme::meter_empty())
        .child(
            div()
                .h_full()
                .w(relative(percent.unwrap_or(0.0) / 100.0))
                .rounded(px(3.0))
                .bg(colour),
        )
}

fn wheel(w: &WheelView, view: &View) -> Div {
    let condition_ink = if w.condition.value.state == ReadoutState::Stale {
        theme::muted()
    } else {
        level_colour(w.condition.level)
    };
    let mut block = div()
        .flex()
        .flex_col()
        .gap_1()
        .min_w(px(148.0))
        .flex_1()
        .child(
            div()
                .flex()
                .justify_between()
                .items_baseline()
                .child(widgets::label(format!("{} \u{b7} Tyre", w.id.label())))
                .child(widgets::readout_coloured(
                    &w.condition.value,
                    18.0,
                    w.condition.level.map(|l| level_colour(Some(l))),
                )),
        )
        .child(bar(w.condition.percent, condition_ink))
        .child(widgets::label("Condition remaining"))
        .child(
            div()
                .flex()
                .gap_2()
                .flex_1()
                .child(column("BRK", &w.brake, view.temperature_unit))
                .child(column("SFC", &w.surface, view.temperature_unit))
                .child(column("INR", &w.inner, view.temperature_unit)),
        )
        .child(line(view.pressure_unit, &w.pressure));
    if view.density == Density::Full {
        block = block.child(line("Brake damage", &w.brake_damage));
    }
    block
}

fn wing(part: &DamageView) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(line(part.label, &part.value))
        .child(bar(part.percent, theme::hot()))
}

pub fn render(view: View, memory: &mut Memory) -> impl IntoElement {
    let mut head = div()
        .flex()
        .items_baseline()
        .gap_3()
        .child(widgets::title("Car status"));
    if view.retired {
        head = head.child(widgets::label("Retired").text_color(theme::hot()));
    }
    head = head.child(
        div()
            .ml_auto()
            .flex()
            .items_baseline()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .gap_1()
                    .child(widgets::readout(&view.compound, 14.0))
                    .child(widgets::label("tyres")),
            )
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .gap_1()
                    .child(widgets::readout(&view.age, 24.0))
                    .child(widgets::label("laps")),
            ),
    );
    let side = |a: WheelId, b: WheelId, wing_key: &str| {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .flex_1()
            .child(wheel(view.wheel(a), &view))
            .child(wheel(view.wheel(b), &view))
            .child(wing(view.part(wing_key)))
    };
    // Driver's left on the image's right: the right-hand wheels on the left.
    let mut body = div().flex().gap_3().flex_1().min_h_0().child(side(
        WheelId::RR,
        WheelId::FR,
        "front-right-wing",
    ));
    if view.show_car {
        let picture = memory.picture(view.livery.as_ref());
        body = body.child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    img(ImageSource::Render(picture))
                        .size_full()
                        .object_fit(ObjectFit::Contain),
                ),
        );
    }
    body = body.child(side(WheelId::RL, WheelId::FL, "front-left-wing"));
    let mut root = div()
        .flex()
        .flex_col()
        .gap_2()
        .size_full()
        .child(head)
        .child(body);
    if view.density == Density::Full {
        let mut rest = div().flex().flex_wrap().gap_x_4().gap_y_1();
        for part in view
            .damage
            .iter()
            .filter(|d| !d.key.ends_with("-wing") || d.key == "rear-wing")
        {
            rest = rest.child(div().w(px(160.0)).child(line(part.label, &part.value)));
        }
        root = root.child(rest);
    }
    // The message floats over the tile; the readouts keep their places.
    let mut shell = div().relative().size_full().p_2().child(root);
    if let Some(message) = view.message {
        shell = shell.child(
            div()
                .absolute()
                .top(px(32.0))
                .left(px(12.0))
                .right(px(12.0))
                .p_1()
                .rounded(px(6.0))
                .border_1()
                .border_color(theme::line())
                .bg(theme::surface())
                .text_center()
                .text_color(theme::muted())
                .child(message),
        );
    }
    shell
}

#[cfg(test)]
mod tests {
    use super::*;
    use pitwall_core::{
        readout::ABSENT_TEXT,
        units::{PressureUnit, TemperatureUnit},
    };

    fn telemetry(surface: [u8; 4], brakes: [u16; 4]) -> CarTelemetryData {
        CarTelemetryData {
            speed: 0,
            throttle: 0.0,
            steer: 0.0,
            brake: 0.0,
            clutch: 0,
            gear: 0,
            engine_rpm: 0,
            drs: 0,
            rev_lights_percent: 0,
            rev_lights_bit_value: 0,
            brakes_temperature: brakes,
            tyres_surface_temperature: surface,
            tyres_inner_temperature: [78, 78, 80, 80],
            engine_temperature: 0,
            tyres_pressure: [21.8, 21.7, 23.5, 23.4],
            surface_type: [0; 4],
        }
    }
    const SURFACE: [u8; 4] = [97, 95, 94, 92];
    const BRAKES: [u16; 4] = [569, 571, 587, 588];
    fn status(actual: u8) -> StatusData {
        StatusData {
            traction_control: 0,
            anti_lock_brakes: 0,
            pit_limiter_status: 0,
            max_rpm: 0,
            idle_rpm: 0,
            max_gears: 0,
            drs_allowed: 0,
            drs_activation_distance: 0,
            actual_tyre_compound: actual,
            visual_tyre_compound: 17,
            tyres_age_laps: 9,
            vehicle_fia_flags: 0,
            network_paused: 0,
        }
    }
    fn damage() -> CarDamageData {
        CarDamageData {
            tyres_wear: [0.0; 4],
            tyres_damage: [0; 4],
            brakes_damage: [0, 1, 2, 3],
            tyre_blisters: [0; 4],
            front_left_wing_damage: 12,
            front_right_wing_damage: 0,
            rear_wing_damage: 4,
            floor_damage: 1,
            diffuser_damage: 0,
            sidepod_damage: 0,
            drs_fault: 0,
            ers_fault: 0,
            gear_box_damage: 0,
            engine_damage: 0,
            engine_mguh_wear: 0,
            engine_es_wear: 0,
            engine_ce_wear: 0,
            engine_ice_wear: 0,
            engine_mguk_wear: 0,
            engine_tc_wear: 0,
            engine_blown: 0,
            engine_seized: 0,
        }
    }
    fn car() -> Inputs {
        Inputs {
            telemetry: Readout::Live(telemetry(SURFACE, BRAKES)),
            status: Readout::Live(status(18)),
            damage: Readout::Live(damage()),
            condition: Readout::Live([73.0, 76.0, 79.0, 82.0]),
            livery: None,
            retired: false,
            units: Units::DEFAULT,
            lifecycle: Lifecycle::Live,
            width: 700.0,
            height: 460.0,
        }
    }
    const IMPERIAL: Units = Units {
        temperature: TemperatureUnit::Fahrenheit,
        pressure: PressureUnit::Bar,
        ..Units::DEFAULT
    };

    #[test]
    fn declares_the_catalogue_entry() {
        assert_eq!(
            (TYPE, pitwall_core::tile_catalog::CAR_STATUS.name),
            ("car-status", "Car status")
        );
        assert_eq!(DENSITIES.pick(559.0, 380.0), Density::Compact);
        assert_eq!(DENSITIES.pick(560.0, 380.0), Density::Regular);
        assert_eq!(DENSITIES.pick(800.0, 600.0), Density::Full);
    }

    #[test]
    fn game_order_maps_to_its_wheel_and_each_wheel_reads_its_own_values() {
        let v = view(&car());
        let ids: Vec<_> = v.wheels.iter().map(|w| w.id).collect();
        assert_eq!(ids, WheelId::GAME_ORDER);
        let fr = v.wheel(WheelId::FR);
        assert_eq!(fr.condition.value.text, "82%");
        assert_eq!(fr.brake.value.text, "588");
        assert_eq!(fr.surface.value.text, "92");
        assert_eq!(fr.inner.value.text, "80");
        assert_eq!(fr.pressure.text, "23.4");
        assert_eq!(v.wheel(WheelId::RL).condition.value.text, "73%");
        assert_eq!(v.compound.text, "Medium");
        assert_eq!(v.age.text, "9");
        assert_eq!(v.part("front-left-wing").value.text, "12%");
    }

    #[test]
    fn condition_is_a_number_with_a_bar_length_and_colour_only_repeats_it() {
        let rl = view(&car()).wheels[0].clone();
        assert_eq!(rl.condition.percent, Some(73.0));
        assert_eq!(rl.condition.level, Some(ConditionLevel::Healthy));
        let worn = view(&Inputs {
            condition: Readout::Live([30.0, 55.0, 100.0, 71.0]),
            ..car()
        });
        let levels: Vec<_> = worn.wheels.iter().map(|w| w.condition.level).collect();
        assert_eq!(
            levels,
            [
                Some(ConditionLevel::Low),
                Some(ConditionLevel::Mid),
                Some(ConditionLevel::Healthy),
                Some(ConditionLevel::Healthy)
            ]
        );
        assert_eq!(worn.wheels[0].condition.percent, Some(30.0));
        assert_eq!(worn.wheels[2].condition.value.text, "100%");
    }

    #[test]
    fn restricted_wear_never_shows_100_or_any_number_and_absent_is_a_dash() {
        let v = view(&Inputs {
            condition: Readout::Restricted,
            ..car()
        });
        for w in &v.wheels {
            assert_eq!(w.condition.value.text, "Restricted");
            assert_eq!((w.condition.percent, w.condition.level), (None, None));
        }
        let absent = view(&Inputs {
            condition: Readout::Absent,
            ..car()
        });
        assert_eq!(absent.wheels[0].condition.value.text, ABSENT_TEXT);
        assert_eq!(absent.wheels[0].condition.percent, None);
    }

    #[test]
    fn temperatures_are_banded_by_actual_compound_with_glyphs_on_cold_and_hot() {
        let v = view(&Inputs {
            telemetry: Readout::Live(telemetry([40, 90, 120, 90], BRAKES)),
            ..car()
        });
        let bands: Vec<_> = v
            .wheels
            .iter()
            .map(|w| (w.surface.band, w.surface.glyph))
            .collect();
        assert_eq!(
            bands,
            [
                (Some(BandName::Cold), "\u{25BC}"),
                (Some(BandName::Window), ""),
                (Some(BandName::Hot), "\u{25B2}"),
                (Some(BandName::Window), "")
            ]
        );
    }

    #[test]
    fn an_unknown_or_missing_compound_leaves_tyres_unbanded_and_brakes_banded() {
        let unknown = view(&Inputs {
            status: Readout::Live(status(99)),
            ..car()
        });
        assert_eq!(unknown.wheels[0].surface.value.text, "97");
        assert_eq!(
            (
                unknown.wheels[0].surface.band,
                unknown.wheels[0].surface.glyph
            ),
            (None, "")
        );
        assert_eq!(unknown.wheels[0].brake.band, Some(BandName::Window));
        let missing = view(&Inputs {
            status: Readout::Absent,
            ..car()
        });
        assert_eq!(missing.wheels[0].surface.band, None);
        assert_eq!(missing.compound.text, ABSENT_TEXT);
        assert_eq!(missing.age.text, ABSENT_TEXT);
    }

    #[test]
    fn converts_to_fahrenheit_and_bar_for_the_device_but_bands_on_celsius() {
        let v = view(&Inputs {
            units: IMPERIAL,
            ..car()
        });
        assert_eq!(v.wheels[0].surface.value.text, "207");
        assert_eq!(v.wheels[0].surface.band, Some(BandName::Window));
        assert_eq!(v.wheels[0].pressure.text, "1.50");
        assert_eq!((v.temperature_unit, v.pressure_unit), ("°F", "bar"));
    }

    #[test]
    fn column_fill_is_a_clamped_share_of_the_scale_whatever_the_unit_and_none_without_value() {
        let v = view(&Inputs {
            telemetry: Readout::Live(telemetry(SURFACE, [500, 1200, 0, 250])),
            ..car()
        });
        let fills: Vec<_> = v.wheels.iter().map(|w| w.brake.fill).collect();
        assert_eq!(fills, [Some(0.5), Some(1.0), Some(0.0), Some(0.25)]);
        let absent = view(&Inputs {
            telemetry: Readout::Absent,
            ..car()
        });
        assert_eq!(absent.wheels[0].brake.fill, None);
        let f = view(&Inputs {
            units: IMPERIAL,
            ..car()
        });
        assert_eq!(f.wheels[0].brake.fill, view(&car()).wheels[0].brake.fill);
    }

    #[test]
    fn restricted_damage_and_wear_do_not_touch_the_temperatures() {
        let v = view(&Inputs {
            damage: Readout::Restricted,
            condition: Readout::Restricted,
            ..car()
        });
        assert_eq!(v.wheels[0].surface.value.state, ReadoutState::Live);
        assert_eq!(v.wheels[3].surface.value.text, "92");
        assert!(
            v.damage
                .iter()
                .all(|d| d.value.text == "Restricted" && d.percent.is_none())
        );
        assert_eq!(v.wheels[0].brake_damage.state, ReadoutState::Restricted);
        assert_eq!(v.message, None);
    }

    #[test]
    fn absent_telemetry_shows_em_dashes_never_zero() {
        let v = view(&Inputs {
            telemetry: Readout::Absent,
            ..car()
        });
        for w in &v.wheels {
            for c in [&w.brake, &w.surface, &w.inner] {
                assert_eq!((c.value.text.as_str(), c.band), (ABSENT_TEXT, None));
            }
            assert_eq!(w.pressure.text, ABSENT_TEXT);
        }
    }

    #[test]
    fn a_stale_car_keeps_its_values() {
        let v = view(&Inputs {
            telemetry: Readout::Stale(telemetry(SURFACE, BRAKES)),
            condition: Readout::Stale([73.0, 76.0, 79.0, 82.0]),
            ..car()
        });
        assert_eq!(
            (
                v.wheels[0].surface.value.state,
                v.wheels[0].surface.value.text.as_str()
            ),
            (ReadoutState::Stale, "97")
        );
        assert_eq!(v.wheels[0].condition.value.text, "73%");
    }

    #[test]
    fn a_car_with_no_data_at_all_shows_the_whole_tile_message_and_keeps_its_shape() {
        let empty = view(&Inputs {
            telemetry: Readout::Absent,
            status: Readout::Absent,
            damage: Readout::Absent,
            condition: Readout::Absent,
            ..car()
        });
        assert_eq!(empty.message, Some("No data"));
        assert_eq!(empty.wheel(WheelId::FR).surface.value.text, ABSENT_TEXT);
        assert_eq!(view(&car()).message, None);
    }

    #[test]
    fn damage_is_numeric_percent_per_part() {
        let v = view(&car());
        assert_eq!(v.part("front-left-wing").value.text, "12%");
        assert_eq!(v.part("front-right-wing").value.text, "0%");
        assert_eq!(v.part("rear-wing").value.text, "4%");
        assert_eq!(v.part("floor").value.text, "1%");
        let brakes: Vec<_> = v
            .wheels
            .iter()
            .map(|w| w.brake_damage.text.clone())
            .collect();
        assert_eq!(brakes, ["0%", "1%", "2%", "3%"]);
    }

    #[test]
    fn compact_drops_the_car_regular_shows_it_and_retired_keeps_the_data() {
        let compact = view(&Inputs {
            width: 360.0,
            height: 240.0,
            ..car()
        });
        assert_eq!(compact.density, Density::Compact);
        assert!(!compact.show_car);
        assert_eq!(compact.wheel(WheelId::FL).pressure.text, "23.5");
        let regular = view(&car());
        assert!(regular.show_car);
        assert_eq!(regular.density, Density::Regular);
        let retired = view(&Inputs {
            retired: true,
            ..car()
        });
        assert!(retired.retired);
        assert_eq!(retired.wheel(WheelId::FR).condition.value.text, "82%");
    }
}
