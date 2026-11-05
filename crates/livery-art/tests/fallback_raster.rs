//! The fallback car rasterised to RGBA pixels.
use pitwall_core::livery::{NEUTRAL_COLOURS, Rgb, fallback_car_mask_svg};
use pitwall_livery_art::{RgbaImage, fallback_car_image, rasterise_svg};

const COLOURS: [Rgb; 4] = [
    Rgb::new(201, 18, 31),
    Rgb::new(11, 42, 222),
    Rgb::new(240, 241, 242),
    Rgb::new(6, 7, 8),
];

fn pixel(image: &RgbaImage, x: u32, y: u32) -> [u8; 4] {
    let at = ((y * image.width + x) * 4) as usize;
    image.pixels[at..at + 4].try_into().unwrap()
}

#[test]
fn the_fallback_car_is_a_960_by_720_rgba_picture_with_a_transparent_background() {
    let car = fallback_car_image(&COLOURS);
    assert_eq!((car.width, car.height), (960, 720));
    assert_eq!(car.pixels.len(), 960 * 720 * 4);
    for (x, y) in [(0, 0), (959, 0), (0, 719), (959, 719)] {
        assert_eq!(pixel(&car, x, y)[3], 0, "corner {x},{y}");
    }
    // The body centre is painted and opaque.
    assert_eq!(pixel(&car, 480, 400)[3], 255);
}

#[test]
fn each_colour_slot_repaints_only_its_own_zone_of_the_mask() {
    let mask = rasterise_svg(&fallback_car_mask_svg()).unwrap();
    let zone_fill: [[u8; 3]; 4] = [[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 0]];
    let base = fallback_car_image(&COLOURS);
    for slot in 0..4 {
        let mut changed = COLOURS;
        changed[slot] = Rgb::new(30, 200, 90);
        let other = fallback_car_image(&changed);
        let (mut differing, mut in_zone) = (0, 0);
        for y in 0..720 {
            for x in 0..960 {
                if pixel(&base, x, y) == pixel(&other, x, y) {
                    continue;
                }
                differing += 1;
                let m = pixel(&mask, x, y);
                assert!(m[3] > 0, "slot {slot} changed {x},{y} outside every zone");
                // Anti-aliased edges blend zones, so a neighbouring mask pixel counts.
                let near_zone = (x.saturating_sub(1)..=(x + 1).min(959)).any(|nx| {
                    (y.saturating_sub(1)..=(y + 1).min(719))
                        .any(|ny| pixel(&mask, nx, ny)[..3] == zone_fill[slot])
                });
                if near_zone {
                    in_zone += 1;
                }
            }
        }
        assert!(
            differing > 1000,
            "slot {slot} changed only {differing} pixels"
        );
        assert!(
            in_zone * 1000 >= differing * 999,
            "slot {slot}: {in_zone} of {differing} changed pixels in its zone"
        );
    }
}

#[test]
fn the_body_takes_the_first_colour() {
    // Under the shading the centre of the body leans towards slot 0's colour.
    let red = fallback_car_image(&[
        Rgb::new(255, 0, 0),
        NEUTRAL_COLOURS[1],
        NEUTRAL_COLOURS[2],
        NEUTRAL_COLOURS[3],
    ]);
    let [r, g, b, _] = pixel(&red, 400, 360);
    assert!(r > 150 && g < 80 && b < 80, "body pixel {r},{g},{b}");
}
