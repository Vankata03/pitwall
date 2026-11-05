//! The registered tile declarations: parity "Registered declarations and size variants"
//! table and the add-tile picker order (`dashboard/src/tiles/registry.ts`).
use pitwall_core::tile_catalog::{Binding, declaration, picker};

#[test]
fn the_picker_lists_all_nineteen_types_sorted_by_display_name() {
    let names: Vec<&str> = picker().iter().map(|d| d.name).collect();
    assert_eq!(
        names,
        [
            "Car status",
            "Engine temp",
            "ERS",
            "Fuel",
            "G-force",
            "Gaps",
            "Lap state",
            "Lap timing",
            "Live delta",
            "Notice",
            "Race-control feed",
            "SM",
            "Speed/RPM/gear",
            "Stint",
            "Throttle/brake traces",
            "Timing tower",
            "Track discipline",
            "Track map",
            "Weather",
        ]
    );
}

#[test]
fn declarations_match_the_parity_table() {
    // (type, binding, minimum, default) from docs/native/parity.md.
    let car = Binding::FollowsCar;
    let session = Binding::SessionWide;
    let table = [
        ("car-status", car, (8, 6), (12, 12)),
        ("engine-temp", car, (2, 2), (3, 2)),
        ("ers", car, (4, 2), (6, 4)),
        ("fuel", car, (3, 2), (6, 2)),
        ("g-force", car, (2, 2), (3, 2)),
        ("gaps", car, (4, 2), (8, 2)),
        ("lap-state", car, (4, 2), (6, 2)),
        ("lap-timing", car, (4, 3), (6, 4)),
        ("live-delta", car, (6, 3), (12, 6)),
        ("notice", session, (4, 1), (6, 1)),
        ("race-control-feed", session, (4, 2), (6, 6)),
        ("sm", car, (2, 2), (4, 2)),
        ("speed-rpm-gear", car, (4, 3), (6, 6)),
        ("stint", car, (6, 4), (8, 7)),
        ("throttle-brake-traces", car, (6, 4), (10, 7)),
        ("timing-tower", session, (4, 6), (6, 12)),
        ("track-discipline", car, (4, 2), (6, 3)),
        ("track-map", session, (4, 4), (6, 6)),
        ("weather", session, (4, 2), (6, 2)),
    ];
    assert_eq!(picker().len(), table.len());
    for (kind, binding, minimum, default) in table {
        let d = declaration(kind).unwrap_or_else(|| panic!("{kind} is registered"));
        assert_eq!(
            (d.kind, d.binding, d.minimum, d.default),
            (kind, binding, minimum, default)
        );
    }
}

#[test]
fn an_unknown_type_has_no_declaration() {
    assert!(declaration("future-tile").is_none());
}
