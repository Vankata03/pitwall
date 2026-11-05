//! `LiveryImages`: the picture each car is drawn with, from the pack folder or the
//! fallback car, cached.
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use pitwall_core::livery::{LiveryCar, NEUTRAL_COLOURS, Rgb};
use pitwall_livery_art::{LiveryImages, RgbaImage, decode_png, fallback_car_image};

const COLOURS: [Rgb; 4] = [
    Rgb::new(201, 18, 31),
    Rgb::new(11, 42, 222),
    Rgb::new(240, 241, 242),
    Rgb::new(6, 7, 8),
];

fn car(team_id: u16, my_team: bool) -> LiveryCar {
    LiveryCar {
        team_id,
        my_team,
        num_colours: 4,
        colours: COLOURS,
    }
}

fn folder() -> PathBuf {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "pitwall-livery-images-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A `width`×`height` PNG of one colour, in the given PNG colour type.
fn png_bytes(width: u32, height: u32, colour: png::ColorType, pixel: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, width, height);
    encoder.set_color(colour);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    let data: Vec<u8> = (0..width * height).flat_map(|_| pixel.to_vec()).collect();
    writer.write_image_data(&data).unwrap();
    writer.finish().unwrap();
    out
}

fn write_png(dir: &Path, name: &str, rgba: [u8; 4]) {
    std::fs::write(dir.join(name), png_bytes(4, 3, png::ColorType::Rgba, &rgba)).unwrap();
}

fn solid(width: u32, height: u32, rgba: [u8; 4]) -> RgbaImage {
    RgbaImage {
        width,
        height,
        pixels: (0..width * height).flat_map(|_| rgba).collect(),
    }
}

#[test]
fn a_team_with_a_pack_image_is_drawn_with_it() {
    let dir = folder();
    write_png(&dir, "3.png", [10, 20, 30, 128]);
    let images = LiveryImages::new(dir);
    assert_eq!(
        *images.image_for(Some(&car(3, false))),
        solid(4, 3, [10, 20, 30, 128])
    );
}

#[test]
fn my_team_is_drawn_with_my_team_png() {
    let dir = folder();
    write_png(&dir, "3.png", [10, 20, 30, 255]);
    write_png(&dir, "my-team.png", [1, 2, 3, 255]);
    let images = LiveryImages::new(dir);
    assert_eq!(
        *images.image_for(Some(&car(3, true))),
        solid(4, 3, [1, 2, 3, 255])
    );
}

#[test]
fn a_team_without_a_pack_image_is_the_fallback_car_in_its_colours() {
    let images = LiveryImages::new(folder());
    assert_eq!(
        *images.image_for(Some(&car(5, false))),
        fallback_car_image(&COLOURS)
    );
}

#[test]
fn a_missing_pack_folder_draws_every_car_as_the_fallback() {
    let images = LiveryImages::new(std::env::temp_dir().join("pitwall-no-such-livery-folder"));
    assert_eq!(
        *images.image_for(Some(&car(0, false))),
        fallback_car_image(&COLOURS)
    );
}

#[test]
fn a_car_without_participant_data_is_the_neutral_fallback() {
    let images = LiveryImages::new(folder());
    assert_eq!(
        *images.image_for(None),
        fallback_car_image(&NEUTRAL_COLOURS)
    );
}

#[test]
fn a_pack_image_that_fails_to_decode_falls_back_with_the_same_colours() {
    let dir = folder();
    std::fs::write(dir.join("3.png"), b"not a png").unwrap();
    let images = LiveryImages::new(dir);
    assert_eq!(
        *images.image_for(Some(&car(3, false))),
        fallback_car_image(&COLOURS)
    );
}

#[test]
fn pictures_are_cached_and_shared() {
    let dir = folder();
    write_png(&dir, "3.png", [10, 20, 30, 255]);
    let images = LiveryImages::new(dir);
    let first = images.image_for(Some(&car(3, false)));
    assert!(Arc::ptr_eq(&first, &images.image_for(Some(&car(3, false)))));
    // Two teams in the same colours share one fallback car.
    let a = images.image_for(Some(&car(6, false)));
    let b = images.image_for(Some(&car(7, false)));
    assert!(Arc::ptr_eq(&a, &b));
}

#[test]
fn refresh_picks_up_added_and_replaced_pack_images() {
    let dir = folder();
    write_png(&dir, "3.png", [10, 20, 30, 255]);
    let images = LiveryImages::new(dir.clone());
    assert_eq!(
        *images.image_for(Some(&car(3, false))),
        solid(4, 3, [10, 20, 30, 255])
    );
    write_png(&dir, "3.png", [40, 50, 60, 255]);
    write_png(&dir, "4.png", [70, 80, 90, 255]);
    images.refresh();
    assert_eq!(
        *images.image_for(Some(&car(3, false))),
        solid(4, 3, [40, 50, 60, 255])
    );
    assert_eq!(
        *images.image_for(Some(&car(4, false))),
        solid(4, 3, [70, 80, 90, 255])
    );
    assert_eq!(
        images.files(),
        vec!["3.png".to_string(), "4.png".to_string()]
    );
}

#[test]
fn for_team_is_the_pack_image_or_the_neutral_fallback() {
    let dir = folder();
    write_png(&dir, "1.png", [10, 20, 30, 255]);
    let images = LiveryImages::new(dir);
    assert_eq!(*images.for_team(1), solid(4, 3, [10, 20, 30, 255]));
    assert_eq!(*images.for_team(2), fallback_car_image(&NEUTRAL_COLOURS));
}

#[test]
fn png_colour_types_all_decode_to_rgba() {
    let cases: [(png::ColorType, &[u8], [u8; 4]); 4] = [
        (png::ColorType::Rgb, &[10, 20, 30], [10, 20, 30, 255]),
        (png::ColorType::Grayscale, &[77], [77, 77, 77, 255]),
        (png::ColorType::GrayscaleAlpha, &[77, 9], [77, 77, 77, 9]),
        (png::ColorType::Rgba, &[1, 2, 3, 4], [1, 2, 3, 4]),
    ];
    for (colour, pixel, rgba) in cases {
        let image = decode_png(&png_bytes(2, 2, colour, pixel)).unwrap();
        assert_eq!(image, solid(2, 2, rgba), "{colour:?}");
    }
    assert!(decode_png(b"nope").is_err());
}
