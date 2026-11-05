//! Port of dashboard/src/tiles/types/timing-tower.test.tsx: ordering, row selection, gap
//! text and cell formats. Layout, colours and the DOM are the tile's (#72).
use pitwall_core::field::{FieldCar, LapFacts};
use pitwall_core::model::{Gaps, PenaltyState, Readout};
use pitwall_core::screen::{
    Density, GRID_MARGIN, ROW_HEIGHT, ScreenClass, cells_to_pixels, pick_density,
};
use pitwall_core::tower::{
    GapMode, RowsMode, TOWER_DENSITIES, TowerContent, TowerSettings, compound_letter,
    compound_name, gained_text, lap_time_text, penalties_text, status_text, timing_tower,
    wear_text,
};
use serde_json::json;

/// Twelve cars in position order: car `i` is P`i+1`, 2.5 s apart, the last one lapped.
fn cars() -> Vec<FieldCar> {
    (0..12)
        .map(|i| FieldCar {
            index: i,
            label: Some(format!("Driver {}", i + 1)),
            retired: false,
            lap: Readout::Live(LapFacts {
                position: i as u8 + 1,
                result_status: 2,
                pit_status: 0,
            }),
            gaps: Readout::Live(Gaps {
                to_leader_ms: i as u32 * 2500,
                to_car_in_front_ms: (i > 0).then_some(2500),
                laps_behind: Some(u32::from(i == 11)),
            }),
            best_lap_ms: Some(90_000 + i as u32 * 100),
        })
        .collect()
}

fn table(content: TowerContent) -> pitwall_core::tower::TowerTable {
    match content {
        TowerContent::Table(table) => table,
        TowerContent::Message(message) => panic!("message {message}"),
    }
}

const RACE: Option<u8> = Some(15);

fn settings(value: serde_json::Value) -> TowerSettings {
    TowerSettings::from_json(&value)
}

fn row_texts(table: &pitwall_core::tower::TowerTable) -> Vec<(usize, String)> {
    table
        .rows
        .iter()
        .map(|row| {
            let text = match &row.gap {
                Readout::Live(t) | Readout::Stale(t) => t.clone(),
                other => format!("{other:?}"),
            };
            (row.index, text)
        })
        .collect()
}

fn gap_of(table: &pitwall_core::tower::TowerTable, index: usize) -> Readout<String> {
    table
        .rows
        .iter()
        .find(|r| r.index == index)
        .unwrap()
        .gap
        .clone()
}

#[test]
fn full_race_tower_shows_everyone_with_interval_gaps_lapped_cars_and_focus() {
    let t = table(timing_tower(
        &cars(),
        RACE,
        Some(5),
        &settings(json!({})),
        Density::Full,
        1000.0,
    ));
    assert_eq!(t.gap_label, "Interval");
    assert_eq!(t.rows.len(), 12);
    let texts = row_texts(&t);
    assert_eq!(texts[0], (0, "Leader".to_string()));
    assert_eq!(texts[1], (1, "+2.500".to_string()));
    assert_eq!(texts[11], (11, "+1 LAP".to_string()));
    assert!(t.rows.iter().all(|r| r.focus == (r.index == 5)));
}

#[test]
fn the_tower_declaration_reaches_full_only_at_500_px_wide() {
    let (w, h) = cells_to_pixels(ScreenClass::Wide, 1249.0, 9, 16, ROW_HEIGHT, GRID_MARGIN);
    assert_eq!(pick_density(&TOWER_DENSITIES, w, h), Density::Compact);
    assert_eq!(pick_density(&TOWER_DENSITIES, 500.0, h), Density::Full);
}

#[test]
fn compact_pins_the_leader_around_focus_and_regular_falls_back_without_scrolling() {
    let compact = table(timing_tower(
        &cars(),
        RACE,
        Some(5),
        &settings(json!({})),
        Density::Compact,
        280.0,
    ));
    let indices: Vec<usize> = compact.rows.iter().map(|r| r.index).collect();
    assert_eq!(indices, [0, 2, 3, 4, 5, 6, 7]);
    let regular = table(timing_tower(
        &cars(),
        RACE,
        Some(5),
        &settings(json!({})),
        Density::Regular,
        260.0,
    ));
    let indices: Vec<usize> = regular.rows.iter().map(|r| r.index).collect();
    assert_eq!(indices, [3, 4, 5, 6, 7]);
}

#[test]
fn focus_relative_compares_leader_gaps_and_names_the_focus_car() {
    let t = table(timing_tower(
        &cars(),
        RACE,
        Some(5),
        &settings(json!({ "gap": "focus" })),
        Density::Full,
        1000.0,
    ));
    assert_eq!(t.gap_label, "Gap to Driver 6");
    assert_eq!(gap_of(&t, 4), Readout::Live("\u{2212}2.500".to_string()));
    assert_eq!(
        gap_of(&t, 0),
        Readout::Live("\u{2212}12.500".to_string()),
        "no Leader text in focus mode"
    );
    assert_eq!(gap_of(&t, 5), Readout::Live("+0.000".to_string()));
}

#[test]
fn timed_sessions_name_the_fallback_reference_when_focus_has_no_best_lap() {
    let mut field = cars();
    field[5].best_lap_ms = None;
    let t = table(timing_tower(
        &field,
        Some(5),
        Some(5),
        &settings(json!({ "gap": "focus" })),
        Density::Full,
        1000.0,
    ));
    assert_eq!(t.gap_label, "Gap to session best");
    assert_eq!(gap_of(&t, 0), Readout::Live("1:30.000".to_string()));
    assert_eq!(gap_of(&t, 1), Readout::Live("+0.100".to_string()));
    assert_eq!(gap_of(&t, 5), Readout::Live("\u{2014}".to_string()));
    let leader_mode = table(timing_tower(
        &field,
        Some(5),
        Some(5),
        &settings(json!({})),
        Density::Full,
        1000.0,
    ));
    assert_eq!(leader_mode.gap_label, "Session best");
}

#[test]
fn focus_mode_in_a_timed_session_uses_the_focus_best_when_it_has_one() {
    let t = table(timing_tower(
        &cars(),
        Some(5),
        Some(5),
        &settings(json!({ "gap": "focus" })),
        Density::Full,
        1000.0,
    ));
    assert_eq!(t.gap_label, "Gap to Driver 6");
    assert_eq!(gap_of(&t, 1), Readout::Live("\u{2212}0.400".to_string()));
}

#[test]
fn race_header_labels_follow_the_gap_setting() {
    let leader = table(timing_tower(
        &cars(),
        RACE,
        Some(5),
        &settings(json!({ "gap": "leader" })),
        Density::Full,
        1000.0,
    ));
    assert_eq!(leader.gap_label, "Leader");
    assert_eq!(gap_of(&leader, 3), Readout::Live("+7.500".to_string()));
    assert_eq!(gap_of(&leader, 0), Readout::Live("Leader".to_string()));
    let no_focus = table(timing_tower(
        &cars(),
        RACE,
        None,
        &settings(json!({ "gap": "focus" })),
        Density::Full,
        1000.0,
    ));
    assert_eq!(no_focus.gap_label, "Gap to focus");
}

#[test]
fn pit_rows_retain_gaps_dsq_and_retired_cars_sink_time_trial_replaces_the_tower() {
    let mut field = cars();
    field[0].retired = true;
    if let Readout::Live(lap) = &mut field[1].lap {
        lap.result_status = 5;
    }
    if let Readout::Live(lap) = &mut field[3].lap {
        lap.pit_status = 1;
    }
    let t = table(timing_tower(
        &field,
        RACE,
        Some(5),
        &settings(json!({})),
        Density::Full,
        1000.0,
    ));
    let order: Vec<usize> = t.rows.iter().map(|r| r.index).collect();
    assert_eq!(&order[10..], [0, 1]);
    assert!(t.rows[10].retired && t.rows[11].retired);
    assert_eq!(gap_of(&t, 3), Readout::Live("+2.500".to_string()));
    assert_eq!(status_text(5, false, 0), "DSQ");
    assert_eq!(status_text(2, false, 1), "PIT");
    assert_eq!(status_text(2, false, 0), "Running");
    assert!(matches!(
        timing_tower(
            &field,
            Some(18),
            Some(5),
            &settings(json!({})),
            Density::Full,
            1000.0
        ),
        TowerContent::Message("Time Trial \u{2014} only the player car runs")
    ));
}

#[test]
fn result_statuses_follow_the_game_codes_dnf_and_not_classified_also_sink() {
    let mut field = cars();
    for (i, status) in [(0, 4), (2, 6)] {
        if let Readout::Live(lap) = &mut field[i].lap {
            lap.result_status = status;
        }
    }
    let t = table(timing_tower(
        &field,
        RACE,
        Some(5),
        &settings(json!({})),
        Density::Full,
        1000.0,
    ));
    let order: Vec<usize> = t.rows.iter().map(|r| r.index).collect();
    assert_eq!(&order[10..], [0, 2]);
    assert_eq!(status_text(4, false, 0), "Retired");
    assert_eq!(status_text(6, false, 0), "NC");
    assert_eq!(status_text(2, true, 0), "Retired");
}

#[test]
fn inactive_and_invalid_cars_and_non_participants_are_not_listed() {
    let mut field = cars();
    if let Readout::Live(lap) = &mut field[7].lap {
        lap.result_status = 0;
    }
    if let Readout::Live(lap) = &mut field[8].lap {
        lap.result_status = 1;
    }
    field[9].label = None;
    let t = table(timing_tower(
        &field,
        RACE,
        None,
        &settings(json!({})),
        Density::Full,
        1000.0,
    ));
    assert!(t.rows.iter().all(|r| ![7, 8, 9].contains(&r.index)));
    assert!(matches!(
        timing_tower(&[], RACE, None, &settings(json!({})), Density::Full, 1000.0),
        TowerContent::Message("No timing data")
    ));
}

#[test]
fn stale_compare_gaps_taint_the_gap_cell() {
    let mut field = cars();
    if let Readout::Live(gaps) = field[3].gaps {
        field[3].gaps = Readout::Stale(gaps);
    }
    let t = table(timing_tower(
        &field,
        RACE,
        None,
        &settings(json!({})),
        Density::Full,
        1000.0,
    ));
    assert_eq!(
        gap_of(&t, 4),
        Readout::Stale("+2.500".to_string()),
        "interval to a stale car"
    );
    assert_eq!(gap_of(&t, 3), Readout::Stale("+2.500".to_string()));
    let mut restricted = cars();
    restricted[2].gaps = Readout::Restricted;
    let t = table(timing_tower(
        &restricted,
        RACE,
        None,
        &settings(json!({})),
        Density::Full,
        1000.0,
    ));
    assert_eq!(gap_of(&t, 2), Readout::Restricted);
}

#[test]
fn settings_default_and_clamp_and_ignore_unknown_values() {
    let defaults = settings(json!({ "other": "keep" }));
    assert_eq!(
        defaults,
        TowerSettings {
            gap: GapMode::Interval,
            rows: RowsMode::Auto,
            radius: 3
        }
    );
    let parsed = settings(json!({ "gap": "focus", "rows": "around", "radius": 20.7 }));
    assert_eq!(
        parsed,
        TowerSettings {
            gap: GapMode::Focus,
            rows: RowsMode::Around,
            radius: 12
        }
    );
    assert_eq!(
        settings(json!({ "gap": "leader", "rows": "full", "radius": -2 })).radius,
        0
    );
    assert_eq!(settings(json!({ "radius": 4.9 })).radius, 4);
    assert_eq!(
        settings(json!({ "radius": "5", "gap": "nope", "rows": "auto" })),
        defaults
    );
}

#[test]
fn full_density_shows_the_highest_wheel_wear() {
    assert_eq!(wear_text([80.0, 70.0, 60.0, 50.0]), "50%");
    assert_eq!(wear_text([99.6, 100.0, 100.0, 100.0]), "0%");
}

#[test]
fn around_focus_means_plus_minus_n_without_adding_distant_cars_at_field_edges() {
    let t = table(timing_tower(
        &cars(),
        RACE,
        Some(0),
        &settings(json!({ "rows": "around", "radius": 3 })),
        Density::Compact,
        500.0,
    ));
    let indices: Vec<usize> = t.rows.iter().map(|r| r.index).collect();
    assert_eq!(indices, [0, 1, 2, 3]);
}

#[test]
fn tyres_have_a_letter_and_an_accessible_compound_name() {
    let letters: Vec<String> = [16, 17, 18, 7, 8, 19, 20, 21, 22, 9]
        .iter()
        .map(|&c| compound_letter(c))
        .collect();
    assert_eq!(letters, ["S", "M", "H", "I", "W", "SS", "S", "M", "H", "9"]);
    assert_eq!(compound_name("S"), "Soft");
    assert_eq!(compound_name("SS"), "Super soft");
    assert_eq!(compound_name("I"), "Intermediate");
    assert_eq!(compound_name("9"), "Compound 9");
}

#[test]
fn full_density_cell_formats() {
    assert_eq!(lap_time_text(Some(91_234)), "1:31.234");
    assert_eq!(lap_time_text(Some(605_007)), "10:05.007");
    assert_eq!(lap_time_text(Some(0)), "\u{2014}");
    assert_eq!(lap_time_text(None), "\u{2014}");
    let penalties = |seconds, cuts, dt, sg| PenaltyState {
        seconds,
        total_warnings: 0,
        corner_cutting_warnings: cuts,
        drive_throughs: dt,
        stop_gos: sg,
    };
    assert_eq!(penalties_text(&penalties(5, 2, 0, 0)), "5s / 2 TL");
    assert_eq!(
        penalties_text(&penalties(0, 0, 1, 2)),
        "0s / 0 TL / 1 DT / 2 SG"
    );
    assert_eq!(gained_text(3, 1), "+2");
    assert_eq!(gained_text(3, 3), "+0");
    assert_eq!(gained_text(1, 4), "-3"); // the web build prints a hyphen-minus here
    assert_eq!(gained_text(0, 4), "\u{2014}");
}
