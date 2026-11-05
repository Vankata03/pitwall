//! Port of dashboard/src/liveries/choose.test.ts and fallbackCar.test.ts.
use pitwall_core::livery::{
    FALLBACK_CAR_HEIGHT, FALLBACK_CAR_WIDTH, LiveryCar, LiveryChoice, NEUTRAL_COLOURS, Rgb,
    choose_livery, fallback_car_mask_svg, fallback_car_svg,
};

const RED: Rgb = Rgb::new(200, 20, 30);
const BLUE: Rgb = Rgb::new(10, 40, 220);
const WHITE: Rgb = Rgb::new(250, 250, 250);
const BLACK: Rgb = Rgb::new(5, 5, 5);

fn car(team_id: u16, my_team: bool, num_colours: u8) -> LiveryCar {
    LiveryCar {
        team_id,
        my_team,
        num_colours,
        colours: [RED, BLUE, WHITE, BLACK],
    }
}

fn files(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| name.to_string()).collect()
}

fn pack_file(choice: &LiveryChoice) -> Option<&str> {
    match choice {
        LiveryChoice::Pack { file, .. } => Some(file),
        LiveryChoice::Fallback { .. } => None,
    }
}

#[test]
fn my_team_uses_my_team_png_whatever_its_team_id() {
    let choice = choose_livery(Some(&car(255, true, 2)), &files(&["3.png", "my-team.png"]));
    assert_eq!(pack_file(&choice), Some("my-team.png"));
}

#[test]
fn a_team_car_uses_its_own_team_id_png() {
    let choice = choose_livery(Some(&car(481, false, 2)), &files(&["3.png", "481.png", "my-team.png"]));
    assert_eq!(pack_file(&choice), Some("481.png"));
}

#[test]
fn a_car_with_no_matching_file_gets_the_fallback() {
    assert_eq!(pack_file(&choose_livery(Some(&car(7, false, 2)), &files(&["3.png", "readme.txt"]))), None);
    assert_eq!(pack_file(&choose_livery(Some(&car(7, false, 2)), &[])), None);
}

#[test]
fn my_team_without_my_team_png_falls_through_to_its_team_id_png_then_the_fallback() {
    assert_eq!(pack_file(&choose_livery(Some(&car(3, true, 2)), &files(&["3.png"]))), Some("3.png"));
    assert_eq!(pack_file(&choose_livery(Some(&car(3, true, 2)), &files(&["4.png"]))), None);
}

#[test]
fn files_that_match_nothing_are_ignored_including_near_misses() {
    let near = files(&["03.png", "3.PNG", "3.png.bak", "team-3.png", "/liveries/3.png", "../3.png"]);
    assert_eq!(pack_file(&choose_livery(Some(&car(3, false, 2)), &near)), None);
}

#[test]
fn game_colours_come_first_in_game_order() {
    let choice = choose_livery(Some(&car(3, false, 3)), &[]);
    assert_eq!(choice.colours()[..3], [RED, BLUE, WHITE]);
}

#[test]
fn colour_slots_the_game_did_not_send_are_neutral() {
    let choice = choose_livery(Some(&car(3, false, 2)), &[]);
    assert_eq!(
        *choice.colours(),
        [RED, BLUE, NEUTRAL_COLOURS[2], NEUTRAL_COLOURS[3]]
    );
}

#[test]
fn no_game_colours_means_all_neutral() {
    assert_eq!(*choose_livery(Some(&car(3, false, 0)), &[]).colours(), NEUTRAL_COLOURS);
}

#[test]
fn a_num_colours_above_the_four_slots_is_capped() {
    assert_eq!(
        *choose_livery(Some(&car(3, false, 9)), &[]).colours(),
        [RED, BLUE, WHITE, BLACK]
    );
}

#[test]
fn a_missing_car_gets_the_neutral_fallback() {
    assert_eq!(
        choose_livery(None, &files(&["3.png"])),
        LiveryChoice::Fallback {
            colours: NEUTRAL_COLOURS
        }
    );
    assert_eq!(
        NEUTRAL_COLOURS,
        [
            Rgb::new(154, 167, 173),
            Rgb::new(232, 237, 239),
            Rgb::new(90, 100, 105),
            Rgb::new(27, 31, 33)
        ]
    );
}

// fallbackCar.test.ts

const COLOURS: [Rgb; 4] = [
    Rgb::new(201, 18, 31),
    Rgb::new(11, 42, 222),
    Rgb::new(240, 241, 242),
    Rgb::new(6, 7, 8),
];

/// Every opening tag of the markup containing `marker`.
fn tagged(svg: &str, marker: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = svg;
    while let Some(start) = rest.find('<') {
        let end = start + rest[start..].find('>').unwrap() + 1;
        let tag = &rest[start..end];
        if tag.contains(marker) {
            out.push(tag.to_string());
        }
        rest = &rest[end..];
    }
    out
}

fn attr(tag: &str, name: &str) -> f64 {
    let needle = format!(" {name}=\"");
    let start = tag.find(&needle).unwrap() + needle.len();
    let end = start + tag[start..].find('"').unwrap();
    tag[start..end].parse().unwrap()
}

fn path_d(tag: &str) -> String {
    let start = tag.find(" d=\"").unwrap() + 4;
    let end = start + tag[start..].find('"').unwrap();
    tag[start..end].to_string()
}

/// Goldens written by the web build's `fallbackCar.ts` (`fallbackCarSvg(COLOURS)` and
/// `fallbackCarMaskSvg()`), so the native car is the same picture.
#[test]
fn the_markup_is_the_web_builds_byte_for_byte() {
    let golden = include_str!("fixtures/livery/fallback-car.svg");
    assert_eq!(fallback_car_svg(&COLOURS), golden);
    let golden_mask = include_str!("fixtures/livery/fallback-car-mask.svg");
    assert_eq!(fallback_car_mask_svg(), golden_mask);
}

#[test]
fn the_car_is_a_transparent_4_3_picture_at_960_by_720() {
    assert_eq!((FALLBACK_CAR_WIDTH, FALLBACK_CAR_HEIGHT), (960, 720));
    let svg = fallback_car_svg(&COLOURS);
    assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
    assert!(svg.contains("viewBox=\"0 0 960 720\""));
    assert!(svg.contains("width=\"960\""));
    assert!(svg.contains("height=\"720\""));
    for tag in tagged(&svg, "<rect") {
        assert!(!(tag.contains("width=\"960\"") && tag.contains("height=\"720\"")), "backdrop {tag}");
    }
    let root = &tagged(&svg, "<svg")[0];
    for banned in ["fill=", "style=", "background="] {
        assert!(!root.contains(banned));
    }
}

#[test]
fn the_art_is_self_contained_no_images_links_text_or_script() {
    for svg in [fallback_car_svg(&COLOURS), fallback_car_mask_svg()] {
        let lower = svg.to_lowercase();
        for element in ["<image", "<text", "<script", "<foreignobject", "<use", "<a "] {
            assert!(!lower.contains(element), "{element}");
        }
        assert!(!lower.contains("href="));
        assert!(!lower.contains("data:"));
        assert_eq!(lower.matches("http").count(), 1, "only the SVG namespace");
        assert!(!lower.replace("url(#", "").contains("url("));
    }
}

#[test]
fn each_of_the_four_colour_slots_paints_its_own_zone() {
    let svg = fallback_car_svg(&COLOURS);
    for (slot, Rgb { r, g, b }) in COLOURS.iter().enumerate() {
        let zone = tagged(&svg, &format!("data-zone=\"{slot}\""));
        assert!(!zone.is_empty());
        for tag in zone {
            assert!(tag.contains(&format!("fill=\"rgb({r},{g},{b})\"")), "{tag}");
        }
    }
}

#[test]
fn changing_a_colour_changes_only_that_zone() {
    let base = fallback_car_svg(&COLOURS);
    let changed = fallback_car_svg(&[COLOURS[0], Rgb::new(1, 2, 3), COLOURS[2], COLOURS[3]]);
    assert_ne!(changed, base);
    let zone = |svg: &str, slot: u8| tagged(svg, &format!("data-zone=\"{slot}\""));
    assert_eq!(zone(&changed, 0), zone(&base, 0));
    assert_ne!(zone(&changed, 1), zone(&base, 1));
    assert_eq!(zone(&changed, 2), zone(&base, 2));
}

#[test]
fn neutral_colours_draw_a_grey_car() {
    let svg = fallback_car_svg(&NEUTRAL_COLOURS);
    for Rgb { r, g, b } in NEUTRAL_COLOURS {
        assert!(svg.contains(&format!("rgb({r},{g},{b})")));
    }
}

#[test]
fn the_front_axle_sits_below_the_rear_axle() {
    let svg = fallback_car_svg(&COLOURS);
    let centre_y = |axle: &str| -> Vec<f64> {
        tagged(&svg, &format!("data-axle=\"{axle}\""))
            .iter()
            .map(|tag| attr(tag, "y") + attr(tag, "height") / 2.0)
            .collect()
    };
    let (front, rear) = (centre_y("front"), centre_y("rear"));
    assert_eq!((front.len(), rear.len()), (2, 2));
    let front_min = front.iter().cloned().fold(f64::INFINITY, f64::min);
    let rear_max = rear.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    assert!(front_min > rear_max);
}

#[test]
fn the_car_is_symmetric_about_the_centre_line_wheels_included() {
    let svg = fallback_car_svg(&COLOURS);
    for axle in ["front", "rear"] {
        let centres: Vec<f64> = tagged(&svg, &format!("data-axle=\"{axle}\""))
            .iter()
            .map(|tag| attr(tag, "x") + attr(tag, "width") / 2.0)
            .collect();
        assert_eq!(centres[0] + centres[1], 960.0);
    }
    // Mirrored outlines: each paired zone outline reflects x about 480.
    let zone0: Vec<String> = tagged(&svg, "data-zone=\"0\"").iter().map(|t| path_d(t)).collect();
    assert!(zone0.contains(&"M426 190C398 220 350 270 322 338C304 382 300 420 312 450C342 462 384 476 420 492L436 492L428 300Z".to_string()));
    assert!(zone0.contains(&"M534 190C562 220 610 270 638 338C656 382 660 420 648 450C618 462 576 476 540 492L524 492L532 300Z".to_string()));
}

#[test]
fn the_whole_car_stays_inside_the_canvas() {
    let svg = fallback_car_svg(&COLOURS);
    for tag in tagged(&svg, " d=\"") {
        for number in path_d(&tag)
            .split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-'))
            .filter(|s| !s.is_empty())
        {
            let value: f64 = number.parse().unwrap();
            assert!((0.0..=960.0).contains(&value), "{value} in {tag}");
        }
    }
    for tag in tagged(&svg, "data-axle") {
        assert!(attr(&tag, "x") >= 0.0);
        assert!(attr(&tag, "x") + attr(&tag, "width") <= 960.0);
        assert!(attr(&tag, "y") + attr(&tag, "height") <= 720.0);
    }
}

#[test]
fn the_mask_marks_the_same_four_zones_in_four_fixed_colours_and_nothing_else() {
    let car = fallback_car_svg(&COLOURS);
    let mask = fallback_car_mask_svg();
    assert!(mask.contains("viewBox=\"0 0 960 720\""));
    for (slot, fill) in ["#ff0000", "#00ff00", "#0000ff", "#ffff00"].iter().enumerate() {
        let marker = format!("data-zone=\"{slot}\"");
        let zone = tagged(&mask, &marker);
        let car_zone = tagged(&car, &marker);
        assert_eq!(zone.len(), car_zone.len());
        for tag in &zone {
            assert!(tag.contains(&format!("fill=\"{fill}\"")));
        }
        let shapes = |tags: &[String]| tags.iter().map(|t| path_d(t)).collect::<Vec<_>>();
        assert_eq!(shapes(&zone), shapes(&car_zone));
    }
    assert!(!mask.contains("data-axle"));
    assert!(!mask.contains("<rect"));
}
