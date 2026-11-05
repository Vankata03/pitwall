//! Port of dashboard/src/layout/screen.test.ts and the density parts of
//! dashboard/src/runtime/tile.test.ts.
use pitwall_core::layouts::ScreenClass;
use pitwall_core::screen::{
    Density, DensityCells, DensityRule, GRID_MARGIN, NARROWEST_WIDE_CONTAINER, ROW_HEIGHT,
    cells_to_pixels, check_densities, density_rules_from_cells, fit_row_height, pick_density,
    screen_class_of, stream_grid_metrics,
};

#[test]
fn screen_classes_follow_the_window_width() {
    for (width, expected) in [
        (1920.0, ScreenClass::Wide),
        (1280.0, ScreenClass::Wide),
        (1279.0, ScreenClass::Tablet),
        (768.0, ScreenClass::Tablet),
        (767.0, ScreenClass::Phone),
        (320.0, ScreenClass::Phone),
    ] {
        assert_eq!(screen_class_of(width), expected, "{width}px");
    }
}

#[test]
fn columns_per_class() {
    assert_eq!(ScreenClass::Wide.columns(), 24);
    assert_eq!(ScreenClass::Tablet.columns(), 12);
    assert_eq!(ScreenClass::Phone.columns(), 4);
}

#[test]
fn row_height_is_32_px_with_an_8_px_margin() {
    assert_eq!(ROW_HEIGHT, 32.0);
    assert_eq!(GRID_MARGIN, 8.0);
    assert_eq!(NARROWEST_WIDE_CONTAINER, 1249.0);
}

#[test]
fn height_is_rows_plus_the_gaps_between_them() {
    let size = |w, h| cells_to_pixels(ScreenClass::Wide, 1280.0, w, h, ROW_HEIGHT, GRID_MARGIN);
    assert_eq!(size(1, 1).1, 32.0);
    assert_eq!(size(1, 16).1, 16.0 * 32.0 + 15.0 * 8.0);
}

#[test]
fn width_is_columns_plus_gaps_from_the_container_width() {
    // 1264 px container, 24 columns, 23 gaps of 8: column = (1264 - 184) / 24 = 45 px.
    let size = |w, h| cells_to_pixels(ScreenClass::Wide, 1264.0, w, h, ROW_HEIGHT, GRID_MARGIN);
    assert_eq!(size(1, 1).0, 45.0);
    assert_eq!(size(9, 1).0, 9.0 * 45.0 + 8.0 * 8.0);
}

fn cell_rules() -> Vec<DensityRule> {
    density_rules_from_cells(&[
        DensityCells {
            density: Density::Compact,
            w: 0,
            h: 0,
        },
        DensityCells {
            density: Density::Regular,
            w: 5,
            h: 8,
        },
        DensityCells {
            density: Density::Full,
            w: 9,
            h: 16,
        },
    ])
}

fn px(width: f32, w: u32, h: u32) -> (f32, f32) {
    cells_to_pixels(ScreenClass::Wide, width, w, h, ROW_HEIGHT, GRID_MARGIN)
}

#[test]
fn race_preset_timing_tower_9x16_at_the_narrowest_wide_screen_reaches_full() {
    let rules = cell_rules();
    let (w, h) = px(NARROWEST_WIDE_CONTAINER, 9, 16);
    assert_eq!(pick_density(&rules, w, h), Density::Full);
}

#[test]
fn one_cell_narrower_or_shorter_than_the_threshold_stays_regular() {
    let rules = cell_rules();
    let (w, h) = px(NARROWEST_WIDE_CONTAINER, 8, 16);
    assert_eq!(pick_density(&rules, w, h), Density::Regular);
    let (w, h) = px(NARROWEST_WIDE_CONTAINER, 9, 15);
    assert_eq!(pick_density(&rules, w, h), Density::Regular);
}

#[test]
fn wider_screens_only_help() {
    let rules = cell_rules();
    let (w, h) = px(1904.0, 9, 16);
    assert_eq!(pick_density(&rules, w, h), Density::Full);
}

#[test]
fn stream_rows_fit_the_viewport_including_gaps_and_retain_empty_rows() {
    let metrics = stream_grid_metrics(&[(1, 2)], 1080.0);
    assert_eq!(metrics.row_height, 1064.0 / 3.0);
    assert_eq!(metrics.margin, 8.0);
    let metrics = stream_grid_metrics(&[(0, 1)], 1080.0);
    assert_eq!((metrics.row_height, metrics.margin), (1080.0, 8.0));
    let tiny = stream_grid_metrics(&[(0, 100)], 100.0);
    assert!((tiny.row_height * 100.0 + tiny.margin * 99.0 - 100.0).abs() < 1e-3);
}

#[test]
fn rows_share_the_height_left_under_the_chrome_in_whole_pixels() {
    assert_eq!(fit_row_height(&[(0, 10)], 500.0), 42.0); // (500 - 9 * 8) / 10
}

#[test]
fn thirty_two_px_is_the_floor_so_a_layout_taller_than_the_screen_still_scrolls() {
    assert_eq!(fit_row_height(&[(0, 10)], 200.0), ROW_HEIGHT);
}

#[test]
fn a_short_layout_stops_growing_at_64_px() {
    assert_eq!(fit_row_height(&[(0, 10)], 5000.0), 64.0);
    assert_eq!(fit_row_height(&[], 5000.0), 64.0);
}

// runtime/tile.test.ts: pickDensity and the density part of defineTile.
fn speed_rules() -> Vec<DensityRule> {
    vec![
        DensityRule {
            density: Density::Compact,
            min_width: 0.0,
            min_height: 0.0,
        },
        DensityRule {
            density: Density::Regular,
            min_width: 200.0,
            min_height: 120.0,
        },
        DensityRule {
            density: Density::Full,
            min_width: 360.0,
            min_height: 220.0,
        },
    ]
}

#[test]
fn the_largest_density_whose_thresholds_both_fit() {
    let rules = speed_rules();
    assert_eq!(pick_density(&rules, 100.0, 100.0), Density::Compact);
    assert_eq!(pick_density(&rules, 200.0, 120.0), Density::Regular);
    assert_eq!(pick_density(&rules, 500.0, 130.0), Density::Regular); // wide but short
    assert_eq!(pick_density(&rules, 360.0, 220.0), Density::Full);
}

#[test]
fn falls_back_to_compact_below_every_threshold() {
    let rules = [
        DensityRule {
            density: Density::Compact,
            min_width: 80.0,
            min_height: 80.0,
        },
        DensityRule {
            density: Density::Regular,
            min_width: 200.0,
            min_height: 120.0,
        },
    ];
    assert_eq!(pick_density(&rules, 10.0, 10.0), Density::Compact);
}

#[test]
fn densities_must_be_two_or_three_in_order_from_compact_and_never_shrink() {
    let rules = speed_rules();
    assert!(check_densities(&rules).is_ok());
    assert!(
        check_densities(&rules[..1])
            .unwrap_err()
            .contains("densities")
    );
    let reversed: Vec<_> = rules.iter().rev().copied().collect();
    assert!(check_densities(&reversed).is_err());
    let no_compact = [
        DensityRule {
            density: Density::Regular,
            min_width: 0.0,
            min_height: 0.0,
        },
        DensityRule {
            density: Density::Full,
            min_width: 100.0,
            min_height: 100.0,
        },
    ];
    assert!(
        check_densities(&no_compact)
            .unwrap_err()
            .contains("compact")
    );
    let shrinking = [
        DensityRule {
            density: Density::Compact,
            min_width: 0.0,
            min_height: 0.0,
        },
        DensityRule {
            density: Density::Regular,
            min_width: 200.0,
            min_height: 120.0,
        },
        DensityRule {
            density: Density::Full,
            min_width: 300.0,
            min_height: 100.0,
        },
    ];
    assert!(check_densities(&shrinking).unwrap_err().contains("shrink"));
}
