//! The fallback car: one original, neutral, front-facing car drawn as code. It is not
//! traced from or derived from any real car or pack image, and carries no logo, number
//! or text. One body serves both regulations.
//!
//! The picture is 960 by 720 (4:3) on a transparent canvas, like a pack image, viewed
//! from in front and slightly above, so the front axle sits lower than the rear axle. It
//! is symmetric about the centre line; the driver's left is the image's right.
//!
//! The four colour zones are recoloured: [`fallback_car_svg`] paints them with the livery
//! colours; [`fallback_car_mask_svg`] draws the same shapes in four flat mask colours for
//! a renderer that recolours by mask. Everything else (tyres, suspension, halo, helmet,
//! shading) is fixed. The markup holds no external reference; rasterise it once per
//! colour set. Byte-for-byte the same markup as the web build's `fallbackCar.ts`.

use super::Rgb;

pub const FALLBACK_CAR_WIDTH: u32 = 960;
pub const FALLBACK_CAR_HEIGHT: u32 = 720;

const CENTRE: f64 = FALLBACK_CAR_WIDTH as f64 / 2.0;

/// Flat mask colours for the four zones, in slot order.
pub const MASK_FILLS: [&str; 4] = ["#ff0000", "#00ff00", "#0000ff", "#ffff00"];

/// A number as JavaScript's `String(number)` prints it, for the coordinates used here.
fn n(value: f64) -> String {
    format!("{value}")
}

/// Reflect an absolute path across the centre line. Only M, L, C, Q and Z, with pairs of
/// numbers separated by spaces.
fn mirror_path(d: &str) -> String {
    let mut out = String::with_capacity(d.len());
    let mut is_x = true;
    let mut number = String::new();
    let flush = |number: &mut String, out: &mut String, is_x: &mut bool| {
        if number.is_empty() {
            return;
        }
        let value: f64 = number.parse().expect("path number");
        out.push_str(&n(if *is_x { 2.0 * CENTRE - value } else { value }));
        *is_x = !*is_x;
        number.clear();
    };
    for c in d.chars() {
        if c.is_ascii_digit() || c == '.' || (c == '-' && number.is_empty()) {
            number.push(c);
            continue;
        }
        flush(&mut number, &mut out, &mut is_x);
        if c.is_ascii_uppercase() {
            is_x = true;
        }
        out.push(c);
    }
    flush(&mut number, &mut out, &mut is_x);
    out
}

/// A left outline plus its mirror image.
fn pair(left: &str) -> [String; 2] {
    [left.to_string(), mirror_path(left)]
}

/// The four recolour zones, as lists of closed outlines, in the game's slot order.
fn zones() -> [Vec<String>; 4] {
    let mut body =
        vec!["M452 112C438 150 426 212 412 302L548 302C534 212 522 150 508 112Z".to_string()];
    body.extend(pair("M426 190C398 220 350 270 322 338C304 382 300 420 312 450C342 462 384 476 420 492L436 492L428 300Z"));
    body.push("M412 288C398 338 388 402 396 468C404 524 430 566 438 604C442 628 454 646 468 650L492 650C506 646 518 628 522 604C530 566 556 524 564 468C572 402 562 338 548 288Z".to_string());

    let mut secondary = vec![
        "M464 424L496 424L490 646L470 646Z".to_string(),
        "M470 116L490 116L496 292L464 292Z".to_string(),
    ];
    secondary.extend(pair(
        "M332 344C352 298 388 252 424 218L426 256C398 286 372 330 358 376Z",
    ));

    let mut wings = vec![
        "M300 42C380 28 580 28 660 42L668 94C580 82 380 82 292 94Z".to_string(),
        "M308 96C390 86 570 86 652 96L648 118C570 108 390 108 312 118Z".to_string(),
        "M72 640C200 598 340 618 440 648L520 648C620 618 760 598 888 640L898 684C760 650 620 676 520 692L440 692C340 676 200 650 62 684Z".to_string(),
    ];
    wings.extend(pair(
        "M96 610C204 578 322 590 430 624L430 642C322 612 204 602 90 634Z",
    ));

    let mut trim = Vec::new();
    trim.extend(pair("M278 24L306 28L300 124L270 120Z"));
    trim.extend(pair("M50 594L96 588L90 696L42 702Z"));
    trim.extend(pair(
        "M296 446C306 492 352 526 412 534L412 548C344 540 290 502 282 450Z",
    ));

    [body, secondary, wings, trim]
}

fn css(Rgb { r, g, b }: Rgb) -> String {
    format!("rgb({r},{g},{b})")
}

fn wheel(axle: &str, x: f64, y: f64, width: f64, height: f64) -> String {
    format!(
        "<rect data-axle=\"{axle}\" x=\"{x}\" y=\"{y}\" width=\"{width}\" height=\"{height}\" rx=\"{rx}\" fill=\"url(#tyre)\"/>\
<rect x=\"{g1}\" y=\"{gy}\" width=\"3\" height=\"{gh}\" fill=\"#000\" opacity=\".35\"/>\
<rect x=\"{g2}\" y=\"{gy}\" width=\"3\" height=\"{gh}\" fill=\"#000\" opacity=\".35\"/>\
<rect x=\"{lx}\" y=\"{ly}\" width=\"{lw}\" height=\"{lh}\" rx=\"10\" fill=\"#fff\" opacity=\".05\"/>",
        x = n(x),
        y = n(y),
        width = n(width),
        height = n(height),
        rx = n((width / 5.0).round()),
        g1 = n(x + width * 0.3),
        g2 = n(x + width * 0.68),
        gy = n(y + 6.0),
        gh = n(height - 12.0),
        lx = n(x + 8.0),
        ly = n(y + 8.0),
        lw = n(width - 16.0),
        lh = n(height * 0.2),
    )
}

fn wheel_pair(axle: &str, offset: f64, y: f64, width: f64, height: f64) -> String {
    wheel(axle, CENTRE - offset - width / 2.0, y, width, height)
        + &wheel(axle, CENTRE + offset - width / 2.0, y, width, height)
}

fn line(x1: f64, y1: f64, x2: f64, y2: f64, width: f64) -> String {
    format!(
        "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#2a3136\" stroke-width=\"{}\" stroke-linecap=\"round\"/>",
        n(x1),
        n(y1),
        n(x2),
        n(y2),
        n(width)
    )
}

/// The fixed parts drawn under the zones.
fn under() -> String {
    let rear = wheel_pair("rear", 262.0, 138.0, 112.0, 194.0);
    let front = wheel_pair("front", 362.0, 366.0, 132.0, 214.0);
    let arms: String = [-1.0, 1.0]
        .iter()
        .map(|side| {
            let x = |distance: f64| CENTRE + side * distance;
            line(x(120.0), 200.0, x(214.0), 210.0, 7.0)
                + &line(x(120.0), 262.0, x(214.0), 252.0, 7.0)
                + &line(x(60.0), 438.0, x(298.0), 448.0, 9.0)
                + &line(x(60.0), 510.0, x(298.0), 492.0, 9.0)
                + &line(x(70.0), 462.0, x(298.0), 470.0, 5.0)
        })
        .collect();
    format!(
        "<ellipse cx=\"{c}\" cy=\"420\" rx=\"430\" ry=\"290\" fill=\"url(#shadow)\"/>\
<path d=\"M330 250C280 340 268 470 340 556L620 556C692 470 680 340 630 250Z\" fill=\"#101315\"/>\
{rear}{front}{arms}\
<path d=\"M462 96L498 96L506 160L454 160Z\" fill=\"#1c2226\"/>\
<rect x=\"444\" y=\"596\" width=\"12\" height=\"60\" fill=\"#1c2226\"/>\
<rect x=\"504\" y=\"596\" width=\"12\" height=\"60\" fill=\"#1c2226\"/>",
        c = n(CENTRE)
    )
}

/// Fixed parts drawn over the zones: intakes, cockpit, helmet, halo, mirrors, highlights.
fn over() -> String {
    let mirrors: String = [-1.0, 1.0]
        .iter()
        .map(|side| {
            let x = |distance: f64| CENTRE + side * distance;
            line(x(62.0), 300.0, x(108.0), 288.0, 5.0)
                + &format!(
                    "<ellipse cx=\"{}\" cy=\"286\" rx=\"16\" ry=\"8\" fill=\"#1a2024\"/>",
                    n(x(116.0))
                )
        })
        .collect();
    let inlets: String = pair("M314 428C344 432 380 444 412 460L412 486C380 470 344 456 316 450Z")
        .iter()
        .map(|d| format!("<path d=\"{d}\" fill=\"#0c0f11\" opacity=\".82\"/>"))
        .collect();
    let shines: String = pair("M322 412C326 372 346 326 378 280L388 284C358 330 338 376 334 416Z")
        .iter()
        .map(|d| format!("<path d=\"{d}\" fill=\"#fff\" opacity=\".14\"/>"))
        .collect();
    format!(
        "{inlets}{shines}\
<path d=\"M454 122C462 108 498 108 506 122C510 142 450 142 454 122Z\" fill=\"#0c0f11\"/>\
<path d=\"M452 470C454 530 462 590 472 632L480 632L470 470Z\" fill=\"#fff\" opacity=\".12\"/>\
<ellipse cx=\"{c}\" cy=\"330\" rx=\"52\" ry=\"40\" fill=\"#0d1012\"/>\
<circle cx=\"{c}\" cy=\"318\" r=\"27\" fill=\"url(#helmet)\"/>\
<rect x=\"456\" y=\"312\" width=\"48\" height=\"14\" rx=\"7\" fill=\"#14191c\"/>\
{mirrors}\
<path d=\"M418 372Q420 292 {c} 272Q540 292 542 372\" fill=\"none\" stroke=\"#14181b\" stroke-width=\"13\" stroke-linecap=\"round\"/>\
<path d=\"M430 362Q432 300 {c} 284\" fill=\"none\" stroke=\"#fff\" stroke-opacity=\".12\" stroke-width=\"3\" stroke-linecap=\"round\"/>\
<line x1=\"{c}\" y1=\"272\" x2=\"{c}\" y2=\"392\" stroke=\"#14181b\" stroke-width=\"12\" stroke-linecap=\"round\"/>",
        c = n(CENTRE)
    )
}

const SHADE: &str = concat!(
    "<defs>",
    // Dark toward the edges, so flat zone colours read as rounded.
    "<linearGradient id=\"shade\" x1=\"0\" x2=\"1\" y1=\"0\" y2=\"0\">",
    "<stop offset=\"0\" stop-color=\"#000\" stop-opacity=\".34\"/>",
    "<stop offset=\".5\" stop-color=\"#fff\" stop-opacity=\".1\"/>",
    "<stop offset=\"1\" stop-color=\"#000\" stop-opacity=\".34\"/>",
    "</linearGradient>",
    "<radialGradient id=\"shadow\"><stop offset=\"0\" stop-color=\"#000\" stop-opacity=\".5\"/>",
    "<stop offset=\"1\" stop-color=\"#000\" stop-opacity=\"0\"/></radialGradient>",
    "<linearGradient id=\"tyre\" x1=\"0\" x2=\"1\" y1=\"0\" y2=\"0\">",
    "<stop offset=\"0\" stop-color=\"#0b0d0f\"/><stop offset=\".5\" stop-color=\"#23292d\"/><stop offset=\"1\" stop-color=\"#0b0d0f\"/>",
    "</linearGradient>",
    "<radialGradient id=\"helmet\" cx=\".4\" cy=\".35\"><stop offset=\"0\" stop-color=\"#eef1f2\"/>",
    "<stop offset=\"1\" stop-color=\"#8d969b\"/></radialGradient>",
    "</defs>",
);

fn open() -> String {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\">",
        w = FALLBACK_CAR_WIDTH,
        h = FALLBACK_CAR_HEIGHT
    )
}

/// The car painted with the four livery colours, one per zone, in slot order.
pub fn fallback_car_svg(colours: &[Rgb; 4]) -> String {
    let mut zones_markup = String::new();
    for (slot, outlines) in zones().iter().enumerate() {
        let fill = css(colours[slot]);
        for d in outlines {
            zones_markup.push_str(&format!(
                "<path data-zone=\"{slot}\" d=\"{d}\" fill=\"{fill}\"/><path d=\"{d}\" fill=\"url(#shade)\"/>"
            ));
        }
    }
    format!("{}{SHADE}{}{zones_markup}{}</svg>", open(), under(), over())
}

/// The recolour mask: the same zones in flat red, green, blue and yellow, no fixed parts.
pub fn fallback_car_mask_svg() -> String {
    let mut zones_markup = String::new();
    for (slot, outlines) in zones().iter().enumerate() {
        for d in outlines {
            zones_markup.push_str(&format!(
                "<path data-zone=\"{slot}\" d=\"{d}\" fill=\"{}\"/>",
                MASK_FILLS[slot]
            ));
        }
    }
    format!("{}{zones_markup}</svg>", open())
}
