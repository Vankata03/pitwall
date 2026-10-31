use pitwall_collector::names::team_name;

#[test]
fn both_grids_use_short_team_names_without_title_sponsors() {
    let expected = [
        (0, "Mercedes"),
        (1, "Ferrari"),
        (2, "Red Bull Racing"),
        (3, "Williams"),
        (4, "Aston Martin"),
        (5, "Alpine"),
        (6, "RB"),
        (7, "Haas"),
        (8, "McLaren"),
        (9, "Sauber"),
        (476, "Mercedes"),
        (477, "Ferrari"),
        (478, "Red Bull Racing"),
        (479, "Williams"),
        (480, "Aston Martin"),
        (481, "Alpine"),
        (482, "RB"),
        (483, "Haas"),
        (484, "McLaren"),
        (485, "Audi"),
        (486, "Cadillac"),
    ];
    for (id, name) in expected {
        assert_eq!(team_name(id, false), Some(name), "team {id}");
    }
}

#[test]
fn my_team_is_selected_by_the_flag_even_for_known_or_sentinel_ids() {
    for id in [0, 104, 255, 476, 65535] {
        assert_eq!(team_name(id, true), Some("My Team"), "team {id}");
    }
    assert_eq!(team_name(255, false), None);
    assert_eq!(team_name(65535, false), None);
}

#[test]
fn tracks_use_appendix_place_names_including_reverse_layouts_and_madrid() {
    use pitwall_collector::names::track_name;
    let expected = [
        (0, "Melbourne"),
        (2, "Shanghai"),
        (3, "Sakhir (Bahrain)"),
        (4, "Catalunya"),
        (5, "Monaco"),
        (6, "Montreal"),
        (7, "Silverstone"),
        (9, "Hungaroring"),
        (10, "Spa"),
        (11, "Monza"),
        (12, "Singapore"),
        (13, "Suzuka"),
        (14, "Abu Dhabi"),
        (15, "Texas"),
        (16, "Brazil"),
        (17, "Austria"),
        (19, "Mexico"),
        (20, "Baku (Azerbaijan)"),
        (26, "Zandvoort"),
        (27, "Imola"),
        (29, "Jeddah"),
        (30, "Miami"),
        (31, "Las Vegas"),
        (32, "Losail"),
        (39, "Silverstone (Reverse)"),
        (40, "Austria (Reverse)"),
        (41, "Zandvoort (Reverse)"),
        (42, "Madrid"),
    ];
    for (id, name) in expected {
        assert_eq!(track_name(id), Some(name), "track {id}");
    }
}

#[test]
fn sessions_use_plain_appendix_names() {
    use pitwall_collector::names::session_name;
    let expected = [
        (1, "Practice 1"),
        (2, "Practice 2"),
        (3, "Practice 3"),
        (4, "Short Practice"),
        (5, "Qualifying 1"),
        (6, "Qualifying 2"),
        (7, "Qualifying 3"),
        (8, "Short Qualifying"),
        (9, "One-Shot Qualifying"),
        (10, "Sprint Shootout 1"),
        (11, "Sprint Shootout 2"),
        (12, "Sprint Shootout 3"),
        (13, "Short Sprint Shootout"),
        (14, "One-Shot Sprint Shootout"),
        (15, "Race"),
        (16, "Race 2"),
        (17, "Race 3"),
        (18, "Time Trial"),
    ];
    for (id, name) in expected {
        assert_eq!(session_name(id), Some(name), "session {id}");
    }
}

#[test]
fn unknown_ids_and_unused_slots_do_not_acquire_invented_names() {
    use pitwall_collector::names::{session_name, track_name};
    for id in [10, 255, 475, 487, u16::MAX] {
        assert_eq!(team_name(id, false), None, "team {id}");
    }
    for id in [i8::MIN, -1, 1, 8, 18, 28, 33, 38, 43, i8::MAX] {
        assert_eq!(track_name(id), None, "track {id}");
    }
    for id in [0, 19, 255] {
        assert_eq!(session_name(id), None, "session {id}");
    }
}
