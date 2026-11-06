//! ERS (parity ERS-01/02): a frame-rate tile following one car.
//! Port of `dashboard/src/tiles/types/ers.tsx`.

use gpui::{Hsla, IntoElement, ParentElement, Styled, div, px};
use pitwall_core::{
    model::{ErsData, Lifecycle, Readout, Regulations, SessionModel},
    readout::{ReadoutState, ReadoutView, readout_view, view_of, whole_tile_message},
    telemetry::CarTelemetry2Data,
};

use super::{Densities, Density, format::grouped, widgets};
use crate::theme;

pub const TYPE: &str = "ers";
pub const NAME: &str = "ERS";
pub const DENSITIES: Densities = Densities {
    regular: (240.0, 100.0),
    full: Some((340.0, 160.0)),
};
/// Segments in the battery.
pub const SEGMENTS: u8 = 20;
/// The game's ERS deploy mode for overtake. A 2026 car's boost reads as this while
/// overtake is not active.
const DEPLOY_OVERTAKE: u8 = 3;
const MODES: [&str; 4] = ["None", "Medium", "Hotlap", "Overtake"];

pub struct Inputs {
    /// Store charge as a percentage of the assumed 4 MJ capacity.
    pub percent: Readout<f32>,
    pub ers: Readout<ErsData>,
    pub aero: Readout<CarTelemetry2Data>,
    pub regulations: Option<Regulations>,
    pub lifecycle: Lifecycle,
    pub width: f32,
    pub height: f32,
}

impl Inputs {
    pub fn from_model(model: &SessionModel, index: Option<usize>, size: (f32, f32)) -> Self {
        let car = index.and_then(|i| model.car(i));
        Self {
            percent: car.map_or(Readout::Absent, |c| c.ers_percentage()),
            ers: car.map_or(Readout::Absent, |c| c.ers().map(Clone::clone)),
            aero: car.map_or(Readout::Absent, |c| c.aero().map(Clone::clone)),
            regulations: car.and_then(|c| c.regulations()),
            lifecycle: model.lifecycle(),
            width: size.0,
            height: size.1,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Battery {
    pub lit: u8,
    pub state: ReadoutState,
}

/// Regular and full rows, from the State readouts.
#[derive(Debug, Clone, PartialEq)]
pub struct Rows {
    pub mode: ReadoutView,
    pub mgu_k: ReadoutView,
    pub mgu_h: ReadoutView,
    pub total: ReadoutView,
    pub deployed: ReadoutView,
}

/// Full density on a 2026 car only.
#[derive(Debug, Clone, PartialEq)]
pub struct Rows2026 {
    pub overtake: ReadoutView,
    pub boost: ReadoutView,
    pub harvest_limit: ReadoutView,
}

#[derive(Debug, Clone, PartialEq)]
pub struct View {
    pub density: Density,
    pub percent: ReadoutView,
    pub battery: Battery,
    pub rows: Option<Rows>,
    pub rows_2026: Option<Rows2026>,
    pub message: Option<&'static str>,
}

fn kj(joules: f32) -> String {
    format!("{} kJ", grouped(f64::from(joules) / 1000.0))
}

pub fn mode_name(mode: u8) -> String {
    MODES
        .get(usize::from(mode))
        .map_or_else(|| format!("Mode {mode}"), |m| (*m).to_string())
}

/// The more missing of two states: live, stale, restricted, absent.
fn worse(a: ReadoutState, b: ReadoutState) -> ReadoutState {
    let rank = |s| match s {
        ReadoutState::Live => 0,
        ReadoutState::Stale => 1,
        ReadoutState::Restricted => 2,
        ReadoutState::Absent => 3,
    };
    if rank(a) >= rank(b) { a } else { b }
}

pub fn view(inputs: &Inputs) -> View {
    let density = DENSITIES.pick(inputs.width, inputs.height);
    let state = inputs.percent.state();
    let percent = inputs.percent.value().map(|p| p.clamp(0.0, 100.0).round());
    let show_rows = density != Density::Compact;
    let show_2026 =
        density == Density::Full && inputs.regulations == Some(Regulations::Regulations2026);
    let ers = &inputs.ers;
    let aero = &inputs.aero;
    let overtake_active = aero.value().is_some_and(|a| a.overtake_active == 1);

    let rows = show_rows.then(|| Rows {
        mode: view_of(ers, |e| mode_name(e.deploy_mode)),
        mgu_k: view_of(ers, |e| kj(e.harvested_this_lap_mguk)),
        mgu_h: view_of(ers, |e| kj(e.harvested_this_lap_mguh)),
        total: view_of(ers, |e| {
            kj(e.harvested_this_lap_mguk + e.harvested_this_lap_mguh)
        }),
        deployed: view_of(ers, |e| kj(e.deployed_this_lap)),
    });
    let rows_2026 = show_2026.then(|| Rows2026 {
        overtake: view_of(aero, |a| {
            if a.overtake_active == 1 {
                "Active"
            } else if a.overtake_available == 1 {
                "Available"
            } else {
                "Off"
            }
            .into()
        }),
        boost: readout_view(
            worse(ers.state(), aero.state()),
            ers.value().map_or("", |e| {
                if e.deploy_mode == DEPLOY_OVERTAKE && !overtake_active {
                    "On"
                } else {
                    "Off"
                }
            }),
        ),
        // Assumed to be in joules like the other ERS fields, until a capture confirms it.
        harvest_limit: match ers.value().and_then(|e| e.harvest_limit_per_lap) {
            Some(limit) => view_of(ers, |_| kj(limit)),
            None => readout_view(
                if ers.state() == ReadoutState::Restricted {
                    ReadoutState::Restricted
                } else {
                    ReadoutState::Absent
                },
                "",
            ),
        },
    });

    let mut states = vec![state];
    if show_rows {
        states.push(ers.state());
    }
    if show_2026 {
        states.push(aero.state());
    }
    View {
        density,
        percent: readout_view(state, &format!("{}%", percent.unwrap_or(0.0))),
        battery: Battery {
            lit: percent.map_or(0, |p| (p / 100.0 * f32::from(SEGMENTS)).round() as u8),
            state,
        },
        rows,
        rows_2026,
        message: whole_tile_message(states, inputs.lifecycle, true),
    }
}

fn stat(label: &'static str, value: &ReadoutView, tone: Hsla) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .child(widgets::label(label))
        .child(widgets::readout_coloured(value, 16.0, Some(tone)))
}

pub fn render(view: View) -> impl IntoElement {
    let mut battery = div().flex().gap(px(2.0)).h(px(18.0)).flex_1().min_w_0();
    for i in 0..SEGMENTS {
        let on = i < view.battery.lit;
        let colour = match (on, view.battery.state) {
            (false, _) => theme::meter_empty(),
            (true, ReadoutState::Stale) => theme::muted(),
            (true, _) => theme::energy(),
        };
        battery = battery.child(div().flex_1().h_full().rounded(px(1.0)).bg(colour));
    }
    let mut head = div()
        .flex()
        .items_center()
        .gap_3()
        .child(widgets::readout(&view.percent, 34.0))
        .child(battery);
    if let Some(rows) = &view.rows {
        head = head.child(widgets::readout(&rows.mode, 16.0));
    }
    let mut body = div().flex().flex_col().gap_2().size_full();
    if view.rows.is_some() {
        body = body.child(
            div()
                .flex()
                .justify_between()
                .child(widgets::title("Energy recovery"))
                .child(widgets::label("Deployment")),
        );
    }
    body = body.child(head);
    if let Some(rows) = &view.rows {
        let mut grid = div()
            .flex()
            .flex_wrap()
            .gap_x_4()
            .gap_y_2()
            .child(stat("MGU-K", &rows.mgu_k, theme::energy()))
            .child(stat("MGU-H", &rows.mgu_h, theme::cold()))
            .child(stat("Total", &rows.total, theme::hot()))
            .child(stat("Deployed", &rows.deployed, theme::brake()));
        if let Some(r) = &view.rows_2026 {
            grid = grid
                .child(stat("Overtake", &r.overtake, theme::text()))
                .child(stat("Boost", &r.boost, theme::text()))
                .child(stat("Harvest limit", &r.harvest_limit, theme::text()));
        }
        body = body.child(grid);
    }
    widgets::tile_shell(body, view.message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pitwall_core::readout::ABSENT_TEXT;

    fn ers(deploy_mode: u8, harvest_limit_per_lap: Option<f32>) -> ErsData {
        ErsData {
            store_energy: 2_560_000.0,
            deploy_mode,
            harvested_this_lap_mguk: 143_000.0,
            harvested_this_lap_mguh: 144_000.0,
            deployed_this_lap: 106_000.0,
            harvest_limit_per_lap,
            engine_power_ice: 0.0,
            engine_power_mguk: 0.0,
        }
    }
    fn aero(overtake_available: u8, overtake_active: u8) -> CarTelemetry2Data {
        CarTelemetry2Data {
            active_aero_mode: 0,
            active_aero_available: 0,
            active_aero_activation_distance: 0,
            overtake_available,
            overtake_active,
            overtake_activation_distance: 0,
            regulations_2026: 1,
            driving_wrong_way: 0,
        }
    }
    fn base(percent: Readout<f32>) -> Inputs {
        Inputs {
            percent,
            ers: Readout::Absent,
            aero: Readout::Absent,
            regulations: Some(Regulations::Pre2026),
            lifecycle: Lifecycle::Live,
            width: 220.0,
            height: 72.0,
        }
    }
    fn regular(percent: Readout<f32>, e: Readout<ErsData>) -> Inputs {
        Inputs {
            ers: e,
            width: 260.0,
            height: 120.0,
            ..base(percent)
        }
    }
    fn full_2026(e: Readout<ErsData>, a: Readout<CarTelemetry2Data>) -> Inputs {
        Inputs {
            ers: e,
            aero: a,
            regulations: Some(Regulations::Regulations2026),
            width: 380.0,
            height: 200.0,
            ..base(Readout::Live(64.0))
        }
    }

    #[test]
    fn declares_the_catalogue_entry() {
        assert_eq!((TYPE, NAME), ("ers", "ERS"));
        assert_eq!(DENSITIES.pick(239.0, 100.0), Density::Compact);
        assert_eq!(DENSITIES.pick(240.0, 100.0), Density::Regular);
        assert_eq!(DENSITIES.pick(340.0, 160.0), Density::Full);
    }

    #[test]
    fn draws_the_percentage_and_a_battery_of_segments() {
        let v = view(&base(Readout::Live(64.0)));
        assert_eq!(v.percent.text, "64%");
        assert_eq!(v.battery.lit, 13);
        let empty = view(&base(Readout::Live(0.0)));
        assert_eq!(empty.percent.text, "0%");
        assert_eq!(empty.battery.lit, 0);
        assert_eq!(view(&base(Readout::Live(130.0))).battery.lit, SEGMENTS);
    }

    #[test]
    fn the_four_readout_states_of_the_percentage() {
        let stale = view(&base(Readout::Stale(64.0)));
        assert_eq!(
            (stale.percent.state, stale.percent.text.as_str()),
            (ReadoutState::Stale, "64%")
        );
        let restricted = view(&base(Readout::Restricted));
        assert_eq!(
            (restricted.percent.text.as_str(), restricted.battery.lit),
            ("Restricted", 0)
        );
        let absent = view(&base(Readout::Absent));
        assert_eq!(
            (absent.percent.text.as_str(), absent.battery.lit),
            (ABSENT_TEXT, 0)
        );
    }

    #[test]
    fn regular_adds_deploy_mode_harvest_per_mgu_and_energy_deployed_in_kj() {
        let v = view(&regular(Readout::Live(64.0), Readout::Live(ers(1, None))));
        assert_eq!(v.density, Density::Regular);
        let rows = v.rows.unwrap();
        assert_eq!(rows.mode.text, "Medium");
        assert_eq!(rows.mgu_k.text, "143 kJ");
        assert_eq!(rows.mgu_h.text, "144 kJ");
        assert_eq!(rows.total.text, "287 kJ");
        assert_eq!(rows.deployed.text, "106 kJ");
    }

    #[test]
    fn deploy_modes_are_named_and_unknown_modes_shown_by_number() {
        for (mode, name) in [(0, "None"), (2, "Hotlap"), (3, "Overtake"), (9, "Mode 9")] {
            assert_eq!(mode_name(mode), name);
        }
    }

    #[test]
    fn compact_keeps_the_percentage_and_battery_and_drops_every_other_row() {
        let v = view(&Inputs {
            ers: Readout::Live(ers(1, None)),
            ..base(Readout::Live(64.0))
        });
        assert!(v.rows.is_none() && v.rows_2026.is_none());
    }

    #[test]
    fn a_2026_car_adds_overtake_boost_and_harvest_limit_and_pre_2026_hides_them() {
        let v = view(&full_2026(
            Readout::Live(ers(1, Some(8_500_000.0))),
            Readout::Live(aero(0, 0)),
        ));
        assert_eq!(v.rows_2026.unwrap().harvest_limit.text, "8,500 kJ");
        let pre = view(&Inputs {
            regulations: Some(Regulations::Pre2026),
            ..full_2026(Readout::Live(ers(1, None)), Readout::Live(aero(0, 0)))
        });
        assert!(pre.rows_2026.is_none());
        assert!(pre.rows.is_some());
    }

    #[test]
    fn overtake_is_active_available_or_off_and_boost_is_overtake_mode_without_active_overtake() {
        let show = |a: CarTelemetry2Data, mode| {
            let r = view(&full_2026(Readout::Live(ers(mode, None)), Readout::Live(a)))
                .rows_2026
                .unwrap();
            (r.overtake.text, r.boost.text)
        };
        assert_eq!(show(aero(0, 1), 3), ("Active".into(), "Off".into()));
        assert_eq!(show(aero(1, 0), 3), ("Available".into(), "On".into()));
        assert_eq!(show(aero(0, 0), 3), ("Off".into(), "On".into()));
        assert_eq!(show(aero(0, 0), 1), ("Off".into(), "Off".into()));
    }

    #[test]
    fn a_harvest_limit_the_game_did_not_send_is_absent_never_zero() {
        let r = view(&full_2026(
            Readout::Live(ers(1, None)),
            Readout::Live(aero(0, 0)),
        ))
        .rows_2026
        .unwrap();
        assert_eq!(
            (r.harvest_limit.state, r.harvest_limit.text.as_str()),
            (ReadoutState::Absent, ABSENT_TEXT)
        );
    }

    #[test]
    fn the_slow_rows_show_the_four_states_of_the_state_readout() {
        let states = |e| {
            let r = view(&regular(Readout::Live(64.0), e)).rows.unwrap();
            [r.mode, r.mgu_k, r.mgu_h, r.deployed].map(|v| (v.state, v.text))
        };
        assert!(
            states(Readout::Stale(ers(1, None)))
                .iter()
                .all(|(s, _)| *s == ReadoutState::Stale)
        );
        assert!(
            states(Readout::Restricted)
                .iter()
                .all(|(s, t)| *s == ReadoutState::Restricted && t == "Restricted")
        );
        assert!(
            states(Readout::Absent)
                .iter()
                .all(|(s, t)| *s == ReadoutState::Absent && t == ABSENT_TEXT)
        );
    }

    #[test]
    fn the_whole_tile_message_appears_only_when_no_visible_readout_has_data() {
        let rows_hold_data = view(&regular(Readout::Absent, Readout::Live(ers(1, None))));
        assert_eq!(rows_hold_data.message, None);
        let nothing = view(&regular(Readout::Absent, Readout::Absent));
        assert_eq!(nothing.message, Some("No data"));
        // Compact considers only its visible percentage, not hidden rows.
        let compact = view(&Inputs {
            ers: Readout::Live(ers(1, None)),
            ..base(Readout::Absent)
        });
        assert_eq!(compact.message, Some("No data"));
    }

    #[test]
    fn boost_waits_for_aero_data_and_keeps_its_stale_or_restricted_state() {
        let boost = |a| {
            let b = view(&full_2026(Readout::Live(ers(3, None)), a))
                .rows_2026
                .unwrap()
                .boost;
            (b.state, b.text)
        };
        assert_eq!(
            boost(Readout::Absent),
            (ReadoutState::Absent, ABSENT_TEXT.into())
        );
        assert_eq!(
            boost(Readout::Restricted),
            (ReadoutState::Restricted, "Restricted".into())
        );
        assert_eq!(
            boost(Readout::Stale(aero(0, 1))),
            (ReadoutState::Stale, "Off".into())
        );
    }

    #[test]
    fn a_restricted_harvest_limit_says_restricted_rather_than_absent() {
        let r = view(&full_2026(Readout::Restricted, Readout::Absent))
            .rows_2026
            .unwrap();
        assert_eq!(
            (r.harvest_limit.state, r.harvest_limit.text.as_str()),
            (ReadoutState::Restricted, "Restricted")
        );
    }
}
