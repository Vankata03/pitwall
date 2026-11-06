//! Timing tower (parity TOWER-01..05): a session-wide slow tile listing the field. Port of
//! `dashboard/src/tiles/types/timing-tower.tsx`. Which cars, in which order, and the gap
//! column come from `pitwall_core::tower`; this module adds the other cells, the columns
//! of each density and the drawing.
//!
//! The tower has no car setting. It highlights the window's focus car, which the host
//! passes as the tile's resolved car (a session-wide tile resolves the default Focus
//! target: the window's focus, or the player car while following the player).

use gpui::{
    AnyElement, Div, FontWeight, Hsla, IntoElement, ParentElement, SharedString, Styled, div,
    prelude::FluentBuilder, px, rgb,
};
use pitwall_core::{
    field::{FieldCar, LapFacts, field_of},
    livery::{Rgb, team_colour},
    model::{Gaps, Lifecycle, PenaltyState, Readout, SessionModel},
    race_status::race_status,
    readout::{ReadoutState, ReadoutView, readout_view, view_of},
    screen,
    tower::{
        TowerContent, TowerSettings, compound_letter, compound_name, gained_text, lap_time_text,
        penalties_text, status_text, timing_tower, wear_text,
    },
};

use super::{Densities, Density, widgets};
use crate::theme;

pub const TYPE: &str = "timing-tower";
pub const NAME: &str = "Timing tower";
pub const DENSITIES: Densities = Densities {
    regular: (500.0, 240.0),
    full: Some((500.0, 480.0)),
};

/// The Lap Data fields the tower shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TowerLap {
    /// `m_carPosition`; 0 when the game has none.
    pub position: u8,
    pub result_status: u8,
    pub pit_status: u8,
    pub num_pit_stops: u8,
    pub grid_position: u8,
    pub last_lap_ms: u32,
}

/// The fitted set as Car Status reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tyres {
    pub visual_compound: u8,
    pub age_laps: u8,
}

/// One car slot, in plain readouts.
#[derive(Debug, Clone, PartialEq)]
pub struct TowerCar {
    pub index: usize,
    /// Privacy-safe shown name; `None` when the car is not a listed participant.
    pub label: Option<String>,
    /// The participant readout's state (the driver cell's state).
    pub participant: ReadoutState,
    /// First livery colour; `None` draws the marker in the muted colour.
    pub team: Option<Rgb>,
    pub retired: bool,
    pub lap: Readout<TowerLap>,
    pub gaps: Readout<Gaps>,
    pub best_lap_ms: Option<u32>,
    pub tyres: Readout<Tyres>,
    pub penalties: Readout<PenaltyState>,
    /// Percent remaining, RL RR FL FR.
    pub tyre_condition: Readout<[f32; 4]>,
}

impl TowerCar {
    fn field(&self) -> FieldCar {
        FieldCar {
            index: self.index,
            label: self.label.clone(),
            retired: self.retired,
            lap: self.lap.map(|l| LapFacts {
                position: l.position,
                result_status: l.result_status,
                pit_status: l.pit_status,
            }),
            gaps: self.gaps,
            best_lap_ms: self.best_lap_ms,
        }
    }
}

pub struct Inputs {
    pub cars: Vec<TowerCar>,
    /// `m_sessionType` while the session readout has a value.
    pub session_type: Option<u8>,
    /// The session readout's state (the title note's state).
    pub session: ReadoutState,
    /// Safety car, virtual safety car, formation lap or red flag; empty while racing.
    pub band: &'static str,
    /// The window's focus car; `None` when it is not in the session.
    pub focus: Option<usize>,
    pub settings: TowerSettings,
    pub lifecycle: Lifecycle,
    pub width: f32,
    pub height: f32,
}

impl Inputs {
    pub fn from_model(
        model: &SessionModel,
        focus: Option<usize>,
        settings: TowerSettings,
        size: (f32, f32),
    ) -> Self {
        let cars = field_of(model)
            .into_iter()
            .filter_map(|field| {
                let car = model.car(field.index)?;
                Some(TowerCar {
                    index: field.index,
                    label: field.label,
                    participant: car.participant().state(),
                    team: team_colour(car.participant().value().copied()),
                    retired: field.retired,
                    lap: car.lap().map(|l| TowerLap {
                        position: l.car_position,
                        result_status: l.result_status,
                        pit_status: l.pit_status,
                        num_pit_stops: l.num_pit_stops,
                        grid_position: l.grid_position,
                        last_lap_ms: l.last_lap_time_in_ms,
                    }),
                    gaps: field.gaps,
                    best_lap_ms: field.best_lap_ms,
                    tyres: car.status().map(|s| Tyres {
                        visual_compound: s.visual_tyre_compound,
                        age_laps: s.tyres_age_laps,
                    }),
                    penalties: car.penalty_state(),
                    tyre_condition: car.tyre_condition(),
                })
            })
            .collect();
        let session = model.session();
        Self {
            cars,
            session_type: session.value().map(|s| s.session_type),
            session: session.state(),
            band: race_status(
                session.value().map(|s| s.safety_car_status),
                model.race_control(),
            ),
            focus,
            settings,
            lifecycle: model.lifecycle(),
            width: size.0,
            height: size.1,
        }
    }
}

/// One cell. `badge` is the tyre letter drawn as a coloured ring; its text says the same.
#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    pub value: ReadoutView,
    pub badge: Option<String>,
    /// Accessible name, where it differs from the text ("Soft tyres").
    pub label: Option<String>,
}

impl Cell {
    fn text(value: ReadoutView) -> Self {
        Self {
            value,
            badge: None,
            label: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RowView {
    pub index: usize,
    /// The window's focus car: highlighted.
    pub focus: bool,
    /// Retired, did not finish, disqualified or not classified: muted.
    pub retired: bool,
    /// The team marker beside the name, at every density.
    pub team: Option<Rgb>,
    /// One per column.
    pub cells: Vec<Cell>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Content {
    /// A whole-tile message instead of the table.
    Message(&'static str),
    Table {
        columns: Vec<String>,
        rows: Vec<RowView>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct View {
    pub density: Density,
    /// Race status band, or "Racing".
    pub note: ReadoutView,
    pub content: Content,
    pub size: (f32, f32),
}

fn core_density(density: Density) -> screen::Density {
    match density {
        Density::Compact => screen::Density::Compact,
        Density::Regular => screen::Density::Regular,
        Density::Full => screen::Density::Full,
    }
}

/// A live value is only the last one seen once the game is lost.
fn dim(mut view: ReadoutView, stale: bool) -> ReadoutView {
    if stale && view.state == ReadoutState::Live {
        view.state = ReadoutState::Stale;
    }
    view
}

pub fn view(inputs: &Inputs) -> View {
    let density = DENSITIES.pick(inputs.width, inputs.height);
    let stale = inputs.lifecycle == Lifecycle::Lost;
    let note = readout_view(
        if stale {
            ReadoutState::Stale
        } else {
            inputs.session
        },
        if inputs.band.is_empty() {
            "Racing"
        } else {
            inputs.band
        },
    );
    let field: Vec<FieldCar> = inputs.cars.iter().map(TowerCar::field).collect();
    let table = match timing_tower(
        &field,
        inputs.session_type,
        inputs.focus,
        &inputs.settings,
        core_density(density),
        inputs.height,
    ) {
        TowerContent::Message(message) => {
            return View {
                density,
                note,
                content: Content::Message(message),
                size: (inputs.width, inputs.height),
            };
        }
        TowerContent::Table(table) => table,
    };
    let regular = density != Density::Compact;
    let full = density == Density::Full;
    let mut columns: Vec<String> = vec![
        "Pos".into(),
        "Driver".into(),
        table.gap_label.clone(),
        "Tyre".into(),
    ];
    if regular {
        columns.extend(["Age", "Stops", "Status"].map(String::from));
    }
    if full {
        columns.extend(
            [
                "Last lap",
                "Best lap",
                "Penalties / warnings",
                "Wear (max)",
                "Gained",
            ]
            .map(String::from),
        );
    }
    let rows = table
        .rows
        .iter()
        .filter_map(|row| {
            let car = inputs.cars.iter().find(|c| c.index == row.index)?;
            let cell = |view: ReadoutView| Cell::text(dim(view, stale));
            let lap = |format: &dyn Fn(&TowerLap) -> String| cell(view_of(&car.lap, |l| format(l)));
            let mut cells = vec![
                lap(&|l| l.position.to_string()),
                Cell::text(readout_view(
                    if stale || car.participant == ReadoutState::Stale {
                        ReadoutState::Stale
                    } else {
                        car.participant
                    },
                    car.label.as_deref().unwrap_or_default(),
                )),
                cell(if row.gap == Readout::Restricted {
                    readout_view(ReadoutState::Absent, "")
                } else {
                    view_of(&row.gap, String::clone)
                }),
                match car.tyres.value() {
                    Some(t) => {
                        let letter = compound_letter(t.visual_compound);
                        Cell {
                            value: dim(view_of(&car.tyres, |_| letter.clone()), stale),
                            label: Some(format!("{} tyres", compound_name(&letter))),
                            badge: Some(letter),
                        }
                    }
                    None => cell(view_of(&car.tyres, |_| String::new())),
                },
            ];
            if regular {
                cells.push(cell(view_of(&car.tyres, |t| t.age_laps.to_string())));
                cells.push(lap(&|l| l.num_pit_stops.to_string()));
                cells.push(lap(&|l| {
                    status_text(l.result_status, car.retired, l.pit_status).to_string()
                }));
            }
            if full {
                cells.push(lap(&|l| lap_time_text(Some(l.last_lap_ms))));
                let best = car.best_lap_ms.filter(|&ms| ms > 0);
                cells.push(Cell::text(readout_view(
                    if stale || car.lap.state() == ReadoutState::Stale {
                        ReadoutState::Stale
                    } else if best.is_some() {
                        ReadoutState::Live
                    } else {
                        ReadoutState::Absent
                    },
                    &lap_time_text(best),
                )));
                cells.push(cell(view_of(&car.penalties, penalties_text)));
                cells.push(cell(view_of(&car.tyre_condition, |c| wear_text(*c))));
                cells.push(lap(&|l| gained_text(l.grid_position, l.position)));
            }
            Some(RowView {
                index: row.index,
                focus: row.focus,
                retired: row.retired,
                team: car.team,
                cells,
            })
        })
        .collect();
    View {
        density,
        note,
        content: Content::Table { columns, rows },
        size: (inputs.width, inputs.height),
    }
}

/// The tyre ring's colour: the compound letter is always in the text too.
fn compound_colour(letter: &str) -> Hsla {
    match letter {
        "S" | "SS" => theme::brake(),
        "M" => theme::slower(),
        "H" => theme::window(),
        "I" => theme::personal_best(),
        "W" => theme::cold(),
        _ => theme::muted(),
    }
}

const PADDING: f32 = 12.0;
const GAP: f32 = 6.0;
const VALUE: f32 = 14.0;

/// Column widths in px. Regular is one line per car; Compact and Full use a 12-column
/// grid, Full stacking its extra columns under each car (`tower.css`).
fn widths(density: Density, width: f32, columns: usize) -> Vec<f32> {
    let inner = (width - 2.0 * PADDING).max(0.0);
    match density {
        Density::Regular => {
            let fixed = 32.0 + 40.0 + 36.0 + 48.0 + GAP * 6.0;
            let share = (inner - fixed).max(0.0) / (1.6 + 1.1 + 1.0);
            vec![32.0, share * 1.6, share * 1.1, 40.0, 36.0, 48.0, share]
        }
        Density::Compact | Density::Full => {
            const SPANS: [f32; 12] = [1.0, 4.0, 5.0, 2.0, 2.0, 2.0, 8.0, 3.0, 3.0, 3.0, 2.0, 1.0];
            SPANS[..columns]
                .iter()
                .map(|span| (inner * span / 12.0).floor())
                .collect()
        }
    }
}

fn cell_box(width: f32) -> Div {
    div()
        .w(px(width))
        .flex_none()
        .px(px(2.0))
        .overflow_hidden()
        .whitespace_nowrap()
        .text_ellipsis()
}

fn header_cell(text: &str, width: f32) -> Div {
    cell_box(width)
        .text_size(px(11.0))
        .text_color(theme::muted())
        .child(SharedString::from(text.to_uppercase()))
}

fn value_cell(
    cell: &Cell,
    width: f32,
    retired: bool,
    focus: bool,
    team: Option<Option<Rgb>>,
) -> Div {
    let colour = if retired { Some(theme::muted()) } else { None };
    let weight = if focus {
        FontWeight::BOLD
    } else {
        FontWeight::SEMIBOLD
    };
    let mut text = widgets::readout_coloured(&cell.value, VALUE, colour).font_weight(weight);
    if let Some(letter) = &cell.badge {
        let ring = if cell.value.state == ReadoutState::Live {
            compound_colour(letter)
        } else {
            compound_colour(letter).opacity(0.65)
        };
        text = div()
            .flex()
            .items_center()
            .justify_center()
            .size(px(24.0))
            .rounded_full()
            .border_2()
            .border_color(ring)
            .text_color(ring)
            .text_size(px(VALUE))
            .font_weight(FontWeight::BOLD)
            .child(SharedString::from(letter.clone()));
    }
    let mut out = cell_box(width).flex().items_center().gap_2();
    if let Some(team) = team {
        let marker = team.map_or_else(theme::muted, |c| {
            rgb(u32::from(c.r) << 16 | u32::from(c.g) << 8 | u32::from(c.b)).into()
        });
        out = out.child(
            div()
                .flex_none()
                .w(px(3.0))
                .h(px(14.0))
                .rounded_sm()
                .bg(marker),
        );
    }
    out.child(text.min_w_0().overflow_hidden().text_ellipsis())
}

pub fn render(view: View) -> AnyElement {
    let title = div()
        .flex()
        .justify_between()
        .items_baseline()
        .gap_2()
        .child(widgets::title("Timing tower"))
        .child(widgets::readout(&view.note, 12.0).font_weight(FontWeight::NORMAL));
    let (columns, rows) = match view.content {
        Content::Message(message) => {
            return widgets::tile_shell(div().size_full(), Some(message));
        }
        Content::Table { columns, rows } => (columns, rows),
    };
    let widths = widths(view.density, view.size.0, columns.len());
    let line = |min_height: f32| {
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .w_full()
            .min_h(px(min_height))
            .border_b_1()
            .border_color(theme::line())
            .when(view.density == Density::Regular, |line| line.gap_x(px(GAP)))
    };
    let mut header = line(20.0);
    for (column, width) in columns.iter().zip(&widths) {
        header = header.child(header_cell(column, *width));
    }
    let row_height = match view.density {
        Density::Full => 72.0,
        Density::Regular => 32.0,
        Density::Compact => 28.0,
    };
    let mut table = div().flex().flex_col().w_full().child(header);
    for row in rows {
        let mut r = line(row_height);
        if row.focus {
            r = r.bg(theme::raised());
        }
        for (i, (cell, width)) in row.cells.iter().zip(&widths).enumerate() {
            r = r.child(value_cell(
                cell,
                *width,
                row.retired,
                row.focus,
                (i == 1).then_some(row.team),
            ));
        }
        table = table.child(r);
    }
    div()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .size_full()
        .px(px(PADDING))
        .py(px(PADDING))
        .overflow_hidden()
        .child(title)
        .child(table)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pitwall_core::screen::{GRID_MARGIN, ROW_HEIGHT, ScreenClass, cells_to_pixels};
    use serde_json::json;

    /// Port of the web fixture: twelve cars in position order, 2.5 s apart, the last one
    /// lapped; tyre condition restricted. Car 3 hides its online name.
    fn cars() -> Vec<TowerCar> {
        (0..12)
            .map(|i| TowerCar {
                index: i,
                label: Some(if i == 2 {
                    "#3".to_string()
                } else {
                    format!("Driver {}", i + 1)
                }),
                participant: ReadoutState::Live,
                team: Some(Rgb::new(100, 120, 140)),
                retired: false,
                lap: Readout::Live(TowerLap {
                    position: i as u8 + 1,
                    result_status: 2,
                    pit_status: 0,
                    num_pit_stops: 1,
                    grid_position: i as u8 + 2,
                    last_lap_ms: 91_234,
                }),
                gaps: Readout::Live(Gaps {
                    to_leader_ms: i as u32 * 2500,
                    to_car_in_front_ms: (i > 0).then_some(2500),
                    laps_behind: Some(u32::from(i == 11)),
                }),
                best_lap_ms: Some(90_000 + i as u32 * 100),
                tyres: Readout::Live(Tyres {
                    visual_compound: 16,
                    age_laps: 4,
                }),
                penalties: Readout::Live(PenaltyState {
                    seconds: 5,
                    total_warnings: 0,
                    corner_cutting_warnings: 2,
                    drive_throughs: 0,
                    stop_gos: 0,
                }),
                tyre_condition: Readout::Restricted,
            })
            .collect()
    }

    fn inputs(cars: Vec<TowerCar>) -> Inputs {
        Inputs {
            cars,
            session_type: Some(15),
            session: ReadoutState::Live,
            band: "Safety car",
            focus: Some(5),
            settings: TowerSettings::default(),
            lifecycle: Lifecycle::Live,
            width: 700.0,
            height: 1000.0,
        }
    }

    fn table(v: &View) -> (&[String], &[RowView]) {
        match &v.content {
            Content::Table { columns, rows } => (columns, rows),
            Content::Message(m) => panic!("message {m}"),
        }
    }

    fn texts(row: &RowView) -> Vec<&str> {
        row.cells.iter().map(|c| c.value.text.as_str()).collect()
    }

    fn row_of<'a>(rows: &'a [RowView], driver: &str) -> &'a RowView {
        rows.iter()
            .find(|r| r.cells[1].value.text == driver)
            .unwrap_or_else(|| panic!("{driver} listed"))
    }

    #[test]
    fn declares_the_catalogue_entry_and_reaches_full_only_at_500_px_wide() {
        assert_eq!((TYPE, NAME), ("timing-tower", "Timing tower"));
        let (w, h) = cells_to_pixels(ScreenClass::Wide, 1249.0, 9, 16, ROW_HEIGHT, GRID_MARGIN);
        assert_eq!(DENSITIES.pick(w, h), Density::Compact);
        assert_eq!(DENSITIES.pick(500.0, h), Density::Full);
        assert_eq!(DENSITIES.pick(500.0, 240.0), Density::Regular);
    }

    #[test]
    fn full_race_tower_keeps_fixed_columns_hidden_names_lapped_gaps_focus_and_status() {
        let v = view(&inputs(cars()));
        assert_eq!(v.density, Density::Full);
        let (columns, rows) = table(&v);
        assert_eq!(
            columns,
            [
                "Pos",
                "Driver",
                "Interval",
                "Tyre",
                "Age",
                "Stops",
                "Status",
                "Last lap",
                "Best lap",
                "Penalties / warnings",
                "Wear (max)",
                "Gained"
            ]
        );
        assert!(rows.iter().any(|r| r.cells[1].value.text == "#3"));
        assert!(rows.iter().all(|r| r.cells[1].value.text != "Driver 3"));
        assert_eq!(rows[11].cells[2].value.text, "+1 LAP");
        assert_eq!(v.note.text, "Safety car");
        let focus = row_of(rows, "Driver 6");
        assert!(focus.focus);
        assert_eq!(rows.iter().filter(|r| r.focus).count(), 1);
        assert_eq!(focus.cells[10].value.state, ReadoutState::Restricted);
        assert_eq!(
            texts(focus),
            [
                "6",
                "Driver 6",
                "+2.500",
                "S",
                "4",
                "1",
                "Running",
                "1:31.234",
                "1:30.500",
                "5s / 2 TL",
                "Restricted",
                "+1"
            ]
        );
        assert_eq!(rows[0].cells[2].value.text, "Leader");
    }

    #[test]
    fn compact_pins_the_leader_around_focus_and_regular_falls_back_without_scrolling() {
        let compact = view(&Inputs {
            width: 400.0,
            height: 280.0,
            ..inputs(cars())
        });
        assert_eq!(compact.density, Density::Compact);
        let (columns, rows) = table(&compact);
        assert_eq!(rows.len(), 7);
        row_of(rows, "Driver 1");
        row_of(rows, "Driver 6");
        assert_eq!(columns, ["Pos", "Driver", "Interval", "Tyre"]);
        let regular = view(&Inputs {
            height: 260.0,
            ..inputs(cars())
        });
        assert_eq!(regular.density, Density::Regular);
        let (columns, rows) = table(&regular);
        assert!(rows.len() < 7);
        row_of(rows, "Driver 6");
        assert!(columns.iter().any(|c| c == "Stops"));
        assert!(!columns.iter().any(|c| c == "Last lap"));
    }

    #[test]
    fn focus_relative_compares_leader_gaps_and_timed_sessions_name_the_fallback_reference() {
        let v = view(&Inputs {
            settings: TowerSettings::from_json(&json!({ "gap": "focus" })),
            ..inputs(cars())
        });
        let (columns, rows) = table(&v);
        assert_eq!(columns[2], "Gap to Driver 6");
        assert_eq!(
            row_of(rows, "Driver 5").cells[2].value.text,
            "\u{2212}2.500"
        );
        let mut timed = cars();
        timed[5].best_lap_ms = None;
        let v = view(&Inputs {
            settings: TowerSettings::from_json(&json!({ "gap": "focus" })),
            session_type: Some(5),
            band: "",
            ..inputs(timed)
        });
        let (columns, rows) = table(&v);
        assert_eq!(columns[2], "Gap to session best");
        let leader = texts(row_of(rows, "Driver 1"));
        assert_eq!(leader.iter().filter(|t| **t == "1:30.000").count(), 2);
        assert!(texts(row_of(rows, "Driver 6")).contains(&"\u{2014}"));
        assert_eq!(v.note.text, "Racing");
    }

    #[test]
    fn pit_rows_retain_gaps_dsq_and_retired_cars_sink_time_trial_replaces_the_tower() {
        let mut field = cars();
        field[0].retired = true;
        if let Readout::Live(lap) = &mut field[1].lap {
            lap.result_status = 5; // disqualified
        }
        if let Readout::Live(lap) = &mut field[3].lap {
            lap.pit_status = 1;
        }
        let v = view(&inputs(field));
        let (_, rows) = table(&v);
        assert_eq!(rows[rows.len() - 2].cells[1].value.text, "Driver 1");
        assert_eq!(rows[rows.len() - 1].cells[1].value.text, "Driver 2");
        assert!(rows[rows.len() - 1].retired);
        assert_eq!(rows[rows.len() - 1].cells[6].value.text, "DSQ");
        let pit = row_of(rows, "Driver 4");
        assert_eq!(pit.cells[6].value.text, "PIT");
        assert_eq!(pit.cells[2].value.text, "+2.500");
        let tt = view(&Inputs {
            session_type: Some(18),
            ..inputs(cars())
        });
        assert_eq!(
            tt.content,
            Content::Message("Time Trial \u{2014} only the player car runs")
        );
    }

    #[test]
    fn result_statuses_follow_the_game_codes_dnf_and_not_classified_also_sink() {
        let mut field = cars();
        if let Readout::Live(lap) = &mut field[0].lap {
            lap.result_status = 4; // did not finish
        }
        if let Readout::Live(lap) = &mut field[2].lap {
            lap.result_status = 6; // not classified
        }
        let v = view(&inputs(field));
        let (_, rows) = table(&v);
        assert_eq!(rows[rows.len() - 2].cells[1].value.text, "Driver 1");
        assert_eq!(rows[rows.len() - 1].cells[1].value.text, "#3");
        assert_eq!(row_of(rows, "Driver 1").cells[6].value.text, "Retired");
        assert_eq!(row_of(rows, "#3").cells[6].value.text, "NC");
        assert!(rows.iter().all(|r| r.cells[6].value.text != "DSQ"));
    }

    #[test]
    fn restricted_gap_uses_the_web_towers_absent_marker() {
        let mut field = cars();
        field[5].gaps = Readout::Restricted;
        let v = view(&inputs(field));
        let (_, rows) = table(&v);
        let cell = &row_of(rows, "Driver 6").cells[2].value;
        assert_eq!(cell.text, pitwall_core::readout::ABSENT_TEXT);
        assert_eq!(cell.state, ReadoutState::Absent);
    }

    #[test]
    fn a_lost_game_marks_displayed_values_stale_and_leaves_restricted_alone() {
        let v = view(&Inputs {
            lifecycle: Lifecycle::Lost,
            ..inputs(cars())
        });
        let (_, rows) = table(&v);
        let focus = row_of(rows, "Driver 6");
        assert_eq!(focus.cells[1].value.state, ReadoutState::Stale);
        assert_eq!(focus.cells[2].value.state, ReadoutState::Stale);
        assert_eq!(focus.cells[10].value.state, ReadoutState::Restricted);
        assert_eq!(v.note.state, ReadoutState::Stale);
    }

    #[test]
    fn the_red_flag_band_keeps_gaps_visible() {
        let v = view(&Inputs {
            band: "Red flag",
            ..inputs(cars())
        });
        assert_eq!(v.note.text, "Red flag");
        let (_, rows) = table(&v);
        assert_eq!(rows[11].cells[2].value.text, "+1 LAP");
    }

    #[test]
    fn full_density_shows_the_highest_wheel_wear_with_an_explicit_label() {
        let mut field = cars();
        field[0].tyre_condition = Readout::Live([80.0, 70.0, 60.0, 50.0]);
        let v = view(&inputs(field));
        let (columns, rows) = table(&v);
        assert_eq!(columns[10], "Wear (max)");
        assert_eq!(row_of(rows, "Driver 1").cells[10].value.text, "50%");
    }

    #[test]
    fn around_focus_means_plus_minus_n_without_adding_distant_cars_at_field_edges() {
        let v = view(&Inputs {
            width: 400.0,
            height: 500.0,
            focus: Some(0),
            settings: TowerSettings::from_json(&json!({ "rows": "around", "radius": 3 })),
            ..inputs(cars())
        });
        let (_, rows) = table(&v);
        assert_eq!(rows.len(), 4);
        assert!(rows.iter().all(|r| r.cells[1].value.text != "Driver 5"));
    }

    #[test]
    fn team_colour_sits_beside_the_driver_without_a_spare_column_at_every_density() {
        for (width, height, density) in [
            (400.0, 1000.0, Density::Compact),
            (700.0, 300.0, Density::Regular),
            (700.0, 1000.0, Density::Full),
        ] {
            let v = view(&Inputs {
                width,
                height,
                ..inputs(cars())
            });
            assert_eq!(v.density, density);
            let (columns, rows) = table(&v);
            assert!(columns.iter().all(|c| c != "Team"));
            assert_eq!(row_of(rows, "Driver 6").team, Some(Rgb::new(100, 120, 140)));
            assert!(rows.iter().all(|r| r.cells.len() == columns.len()));
            assert_eq!(widths(density, width, columns.len()).len(), columns.len());
        }
    }

    #[test]
    fn tyres_have_a_coloured_letter_badge_and_an_accessible_compound_name() {
        let v = view(&Inputs {
            height: 300.0,
            ..inputs(cars())
        });
        let (_, rows) = table(&v);
        let tyre = &row_of(rows, "Driver 6").cells[3];
        assert_eq!(tyre.badge.as_deref(), Some("S"));
        assert_eq!(tyre.value.text, "S");
        assert_eq!(tyre.label.as_deref(), Some("Soft tyres"));
        assert_eq!(compound_colour("S"), theme::brake());
        assert_eq!(compound_colour("M"), theme::slower());
    }

    #[test]
    fn no_listed_car_is_no_timing_data() {
        let v = view(&inputs(Vec::new()));
        assert_eq!(v.content, Content::Message("No timing data"));
    }
}
