use pitwall_collector::notice::THIRD_PARTY_NOTICES;

#[test]
fn the_binary_carries_the_windman007_credit_and_licence_text() {
    for required in [
        "windman007/f1-25-pit-wall",
        "Copyright (c) 2021 racehub-io",
        "Delta Zero, LLC",
        "Permission is hereby granted, free of charge",
        "THE SOFTWARE IS PROVIDED \"AS IS\"",
        "modified from https://github.com/racehub-io/f1-telemetry-client.git",
    ] {
        assert!(
            THIRD_PARTY_NOTICES.contains(required),
            "missing: {required}"
        );
    }
}

#[test]
fn the_notice_carries_oxanium_and_the_dependency_licences() {
    assert!(THIRD_PARTY_NOTICES.contains("Oxanium"));
    assert!(THIRD_PARTY_NOTICES.contains("SIL OPEN FONT LICENSE Version 1.1"));
    assert!(THIRD_PARTY_NOTICES.contains("| axum |"));
}

#[test]
fn corners_data_is_not_credited_because_it_is_not_imported() {
    assert!(THIRD_PARTY_NOTICES.contains("Only each map's track ID, track length and"));
}
