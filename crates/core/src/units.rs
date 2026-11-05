//! Display units chosen per device (spec section 10, Units; parity UNIT-01/02).
//!
//! The model keeps the game's units: km/h, metres, °C and psi. A UI converts at display
//! time through these functions. Defaults never follow the game's own preference. Fuel
//! (kg, laps), energy (kJ), time and G-force have no unit choice.

/// Speed unit; distance follows it (metres with km/h, feet with mph).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpeedUnit {
    #[default]
    Kmh,
    Mph,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TemperatureUnit {
    #[default]
    Celsius,
    Fahrenheit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PressureUnit {
    #[default]
    Psi,
    Bar,
}

/// One device's unit choices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Units {
    pub speed: SpeedUnit,
    pub temperature: TemperatureUnit,
    pub pressure: PressureUnit,
}

impl Units {
    /// Where a new device starts: km/h, °C, psi.
    pub const DEFAULT: Units = Units {
        speed: SpeedUnit::Kmh,
        temperature: TemperatureUnit::Celsius,
        pressure: PressureUnit::Psi,
    };
}

/// A converted value and the label to show beside it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quantity {
    pub value: f64,
    pub unit: &'static str,
}

const KMH_TO_MPH: f64 = 0.621371192;
const M_TO_FT: f64 = 3.280839895;
const PSI_TO_BAR: f64 = 0.0689475729;

/// From km/h, as the game reports speed.
pub fn speed(kmh: f64, units: Units) -> Quantity {
    match units.speed {
        SpeedUnit::Kmh => Quantity {
            value: kmh,
            unit: "km/h",
        },
        SpeedUnit::Mph => Quantity {
            value: kmh * KMH_TO_MPH,
            unit: "mph",
        },
    }
}

/// From metres. Distance follows the speed choice.
pub fn distance(metres: f64, units: Units) -> Quantity {
    match units.speed {
        SpeedUnit::Kmh => Quantity {
            value: metres,
            unit: "m",
        },
        SpeedUnit::Mph => Quantity {
            value: metres * M_TO_FT,
            unit: "ft",
        },
    }
}

/// From °C.
pub fn temperature(celsius: f64, units: Units) -> Quantity {
    match units.temperature {
        TemperatureUnit::Celsius => Quantity {
            value: celsius,
            unit: "°C",
        },
        TemperatureUnit::Fahrenheit => Quantity {
            value: celsius * 1.8 + 32.0,
            unit: "°F",
        },
    }
}

/// From psi, as the game reports tyre pressure.
pub fn pressure(psi: f64, units: Units) -> Quantity {
    match units.pressure {
        PressureUnit::Psi => Quantity {
            value: psi,
            unit: "psi",
        },
        PressureUnit::Bar => Quantity {
            value: psi * PSI_TO_BAR,
            unit: "bar",
        },
    }
}
