//! Per-device unit choices (spec section 10, Units; parity UNIT-01). Ported from
//! `dashboard/src/device/units.test.ts`.
use pitwall_core::units::{self, PressureUnit, Quantity, SpeedUnit, TemperatureUnit, Units};

fn close(actual: f64, expected: f64, digits: i32) {
    let tolerance = 10f64.powi(-digits) / 2.0;
    assert!(
        (actual - expected).abs() < tolerance,
        "{actual} is not {expected} to {digits} places"
    );
}

const MPH: Units = Units {
    speed: SpeedUnit::Mph,
    ..Units::DEFAULT
};

#[test]
fn defaults_do_not_follow_the_game_kmh_celsius_psi() {
    assert_eq!(
        Units::default(),
        Units {
            speed: SpeedUnit::Kmh,
            temperature: TemperatureUnit::Celsius,
            pressure: PressureUnit::Psi,
        }
    );
    assert_eq!(Units::default(), Units::DEFAULT);
}

#[test]
fn speed_arrives_in_kmh() {
    assert_eq!(
        units::speed(100.0, Units::DEFAULT),
        Quantity {
            value: 100.0,
            unit: "km/h"
        }
    );
    let converted = units::speed(100.0, MPH);
    assert_eq!(converted.unit, "mph");
    close(converted.value, 62.137, 3);
}

#[test]
fn distance_follows_speed_metres_with_kmh_feet_with_mph() {
    assert_eq!(
        units::distance(100.0, Units::DEFAULT),
        Quantity {
            value: 100.0,
            unit: "m"
        }
    );
    let feet = units::distance(100.0, MPH);
    assert_eq!(feet.unit, "ft");
    close(feet.value, 328.084, 3);
}

#[test]
fn temperature_arrives_in_celsius() {
    let fahrenheit = Units {
        temperature: TemperatureUnit::Fahrenheit,
        ..Units::DEFAULT
    };
    assert_eq!(
        units::temperature(100.0, Units::DEFAULT),
        Quantity {
            value: 100.0,
            unit: "°C"
        }
    );
    assert_eq!(
        units::temperature(100.0, fahrenheit),
        Quantity {
            value: 212.0,
            unit: "°F"
        }
    );
    assert_eq!(units::temperature(-40.0, fahrenheit).value, -40.0);
}

#[test]
fn pressure_arrives_in_psi() {
    assert_eq!(
        units::pressure(22.0, Units::DEFAULT),
        Quantity {
            value: 22.0,
            unit: "psi"
        }
    );
    let bar = units::pressure(
        22.0,
        Units {
            pressure: PressureUnit::Bar,
            ..Units::DEFAULT
        },
    );
    assert_eq!(bar.unit, "bar");
    close(bar.value, 1.5168, 4);
}
