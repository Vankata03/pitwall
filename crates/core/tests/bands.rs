//! Port of dashboard/src/tiles/car-status/bands.test.ts, plus the band-related parts of
//! car-status/view.test.ts that do not depend on units or readout text.
use pitwall_core::bands::{
    BAND_GLYPH_COLD, BAND_GLYPH_HOT, BANDS_ARE_PROVISIONAL, BRAKE_BAND, Band, BandName,
    ConditionLevel, DAMAGE_PARTS, TemperatureKind, band_glyph, band_of, column_fill,
    condition_level, tyre_band, visual_compound_name,
};

#[test]
fn the_shipped_bands_are_flagged_provisional_until_the_calibration_ticket_replaces_them() {
    const { assert!(BANDS_ARE_PROVISIONAL) };
}

#[test]
fn a_tyre_temperature_is_cold_below_its_compounds_window_hot_above_in_window_between() {
    let Band { cold, hot } = tyre_band(18).unwrap(); // C3
    assert_eq!((cold, hot), (84.0, 104.0));
    let t = TemperatureKind::Tyre;
    assert_eq!(band_of(t, cold - 1.0, Some(18)), Some(BandName::Cold));
    assert_eq!(band_of(t, cold, Some(18)), Some(BandName::Window));
    assert_eq!(band_of(t, hot, Some(18)), Some(BandName::Window));
    assert_eq!(band_of(t, hot + 1.0, Some(18)), Some(BandName::Hot));
}

#[test]
fn compounds_are_keyed_by_actual_compound_so_a_different_compound_gets_a_different_window() {
    let expected = [
        (22, 78.0, 98.0),
        (16, 80.0, 100.0),
        (17, 82.0, 102.0),
        (18, 84.0, 104.0),
        (19, 86.0, 106.0),
        (20, 88.0, 108.0),
        (21, 90.0, 110.0),
        (7, 60.0, 85.0),
        (8, 50.0, 75.0),
    ];
    for (compound, cold, hot) in expected {
        assert_eq!(tyre_band(compound), Some(Band { cold, hot }), "compound {compound}");
    }
    assert_ne!(tyre_band(16), tyre_band(21));
    assert_eq!(band_of(TemperatureKind::Tyre, 90.0, Some(16)), Some(BandName::Window));
    assert_eq!(band_of(TemperatureKind::Tyre, 40.0, Some(7)), Some(BandName::Cold));
}

#[test]
fn an_unknown_or_missing_compound_has_no_tyre_band_so_the_number_shows_uncoloured() {
    assert_eq!(band_of(TemperatureKind::Tyre, 95.0, Some(99)), None);
    assert_eq!(band_of(TemperatureKind::Tyre, 95.0, None), None);
}

#[test]
fn brakes_use_their_own_table_and_need_no_compound() {
    assert_eq!(BRAKE_BAND, Band { cold: 300.0, hot: 850.0 });
    let b = TemperatureKind::Brake;
    assert_eq!(band_of(b, 299.0, None), Some(BandName::Cold));
    assert_eq!(band_of(b, 851.0, Some(16)), Some(BandName::Hot));
    assert_eq!(band_of(b, 575.0, None), Some(BandName::Window));
}

#[test]
fn cold_and_hot_carry_an_arrow_glyph_in_window_carries_none() {
    assert_eq!(BAND_GLYPH_COLD, "\u{25BC}");
    assert_eq!(BAND_GLYPH_HOT, "\u{25B2}");
    assert_eq!(band_glyph(Some(BandName::Cold)), "\u{25BC}");
    assert_eq!(band_glyph(Some(BandName::Hot)), "\u{25B2}");
    assert_eq!(band_glyph(Some(BandName::Window)), "");
    assert_eq!(band_glyph(None), "");
}

// view.test.ts: "is the temperature as a share of the column's scale, clamped".
#[test]
fn column_fill_is_the_celsius_share_of_the_columns_scale_clamped() {
    assert_eq!(column_fill(TemperatureKind::Tyre, 70.0), 0.5);
    assert_eq!(column_fill(TemperatureKind::Tyre, 280.0), 1.0);
    assert_eq!(column_fill(TemperatureKind::Brake, 500.0), 0.5);
    assert_eq!(column_fill(TemperatureKind::Brake, -20.0), 0.0);
}

// view.test.ts: condition "is a number with a bar length, and colour only repeats it".
#[test]
fn condition_levels_are_healthy_from_70_mid_from_40_and_low_below() {
    assert_eq!(condition_level(100.0), ConditionLevel::Healthy);
    assert_eq!(condition_level(70.0), ConditionLevel::Healthy);
    assert_eq!(condition_level(69.9), ConditionLevel::Mid);
    assert_eq!(condition_level(40.0), ConditionLevel::Mid);
    assert_eq!(condition_level(39.9), ConditionLevel::Low);
}

// view.test.ts: "shows the visual compound name" and "damage is numeric percent per part".
#[test]
fn visual_compound_names_and_damage_parts() {
    assert_eq!(visual_compound_name(16), "Soft");
    assert_eq!(visual_compound_name(17), "Medium");
    assert_eq!(visual_compound_name(18), "Hard");
    assert_eq!(visual_compound_name(7), "Inter");
    assert_eq!(visual_compound_name(8), "Wet");
    assert_eq!(visual_compound_name(3), "Unknown");
    let labels: Vec<&str> = DAMAGE_PARTS.iter().map(|part| part.label).collect();
    assert_eq!(
        labels,
        [
            "Front left wing",
            "Front right wing",
            "Rear wing",
            "Floor",
            "Diffuser",
            "Sidepod",
            "Gearbox",
            "Engine"
        ]
    );
}
