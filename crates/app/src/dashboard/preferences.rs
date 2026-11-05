//! Native per-window choices. No browser storage is imported.

use pitwall_core::{
    binding::CarIdentity,
    units::{PressureUnit, SpeedUnit, TemperatureUnit, Units},
};
use serde_json::{Value, json};
use std::{fs, io, path::Path};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WindowPreferences {
    pub layout: Option<String>,
    pub focus: Option<CarIdentity>,
    pub units: Units,
}

impl WindowPreferences {
    pub fn load(path: &Path) -> io::Result<Self> {
        let text = match fs::read(path) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(e),
        };
        let v: Value = serde_json::from_slice(&text)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        if !v.is_object() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "window preferences must be an object",
            ));
        }
        Ok(Self {
            layout: v["layout"].as_str().map(str::to_owned),
            focus: serde_json::from_value(v["focus"].clone()).ok(),
            units: Units {
                speed: if v["units"]["speed"] == "mph" {
                    SpeedUnit::Mph
                } else {
                    SpeedUnit::Kmh
                },
                temperature: if v["units"]["temperature"] == "f" {
                    TemperatureUnit::Fahrenheit
                } else {
                    TemperatureUnit::Celsius
                },
                pressure: if v["units"]["pressure"] == "bar" {
                    PressureUnit::Bar
                } else {
                    PressureUnit::Psi
                },
            },
        })
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let v = json!({"schemaVersion":1,"layout":self.layout,"focus":self.focus,"units":{
            "speed":match self.units.speed { SpeedUnit::Kmh=>"kmh", SpeedUnit::Mph=>"mph" },
            "temperature":match self.units.temperature { TemperatureUnit::Celsius=>"c", TemperatureUnit::Fahrenheit=>"f" },
            "pressure":match self.units.pressure { PressureUnit::Psi=>"psi", PressureUnit::Bar=>"bar" }
        }});
        let temporary = path.with_extension("json.tmp");
        fs::write(&temporary, serde_json::to_vec_pretty(&v)?)?;
        fs::rename(temporary, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pitwall_core::{binding::CarIdentity, units::SpeedUnit};

    #[test]
    fn windows_remember_independent_choices_after_reopen() {
        let root = std::env::temp_dir().join(format!("pitwall-window-test-{}", std::process::id()));
        let first = root.join("first.json");
        let second = root.join("second.json");
        let mut prefs = WindowPreferences {
            layout: Some("Driver copy".into()),
            focus: Some(CarIdentity::Driver { driver_id: 9 }),
            ..Default::default()
        };
        prefs.units.speed = SpeedUnit::Mph;
        prefs.save(&first).unwrap();
        assert_eq!(WindowPreferences::load(&first).unwrap(), prefs);
        assert_eq!(
            WindowPreferences::load(&second).unwrap(),
            WindowPreferences::default()
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
