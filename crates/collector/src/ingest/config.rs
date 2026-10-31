//! TOML settings and CLI overrides. File I/O belongs on the control path,
//! never on the UDP receive path.
use serde::Deserialize;
use std::{
    ffi::OsString,
    fs, io,
    net::IpAddr,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub udp: UdpConfig,
    pub http: HttpConfig,
    pub forward: ForwardConfig,
    pub recording: RecordingConfig,
    pub app: AppConfig,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UdpConfig {
    pub port: u16,
}
impl Default for UdpConfig {
    fn default() -> Self {
        Self { port: 20777 }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HttpConfig {
    pub port: u16,
    pub bind: IpAddr,
}
impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            port: 8777,
            bind: IpAddr::from([0, 0, 0, 0]),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct ForwardConfig {
    pub enabled: bool,
    pub targets: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RecordingConfig {
    pub enabled: bool,
    pub size_cap_gb: u64,
}
impl Default for RecordingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            size_cap_gb: 20,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct AppConfig {
    pub start_with_windows: bool,
}

impl Config {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        let config: Self = toml::from_str(text).map_err(|error| error.to_string())?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.udp.port == 0 || self.http.port == 0 {
            return Err("UDP and HTTP ports must be between 1 and 65535".into());
        }
        if self.recording.size_cap_gb == 0 {
            return Err("recording.size_cap_gb must be positive".into());
        }
        Ok(())
    }
}

/// Arguments exclude the executable name. CLI values override settings on every
/// load, including live reload. `--bind` is the HTTP address, not the UDP bind.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Cli {
    pub console: bool,
    pub help: bool,
    pub config_path: Option<PathBuf>,
    pub udp_port: Option<u16>,
    pub http_port: Option<u16>,
    pub bind: Option<IpAddr>,
}

pub const HELP: &str =
    "pitwall [--console] [--udp-port PORT] [--http-port PORT] [--bind ADDRESS] [--config PATH]";

impl Cli {
    pub fn parse(args: impl IntoIterator<Item = impl Into<OsString>>) -> Result<Self, String> {
        let mut cli = Self::default();
        let mut args = args.into_iter().map(Into::into);
        while let Some(arg) = args.next() {
            let text = arg.to_str().ok_or("flag must be UTF-8")?;
            let (flag, inline) = text
                .split_once('=')
                .map_or((text, None), |(k, v)| (k, Some(v)));
            match flag {
                "--console" if inline.is_none() => cli.console = true,
                "--help" | "-h" if inline.is_none() => cli.help = true,
                "--config" | "--udp-port" | "--http-port" | "--bind" => {
                    let value = inline
                        .map(OsString::from)
                        .or_else(|| args.next())
                        .ok_or_else(|| format!("missing value for {flag}"))?;
                    if flag == "--config" {
                        if value.is_empty() {
                            return Err("--config needs a path".into());
                        }
                        cli.config_path = Some(value.into());
                        continue;
                    }
                    let value = value.to_str().ok_or("option value must be UTF-8")?;
                    match flag {
                        "--bind" => {
                            cli.bind = Some(value.parse().map_err(|_| "invalid HTTP bind address")?)
                        }
                        _ => {
                            let port = value
                                .parse::<u16>()
                                .ok()
                                .filter(|port| *port != 0)
                                .ok_or_else(|| format!("invalid port for {flag}: {value}"))?;
                            if flag == "--udp-port" {
                                cli.udp_port = Some(port);
                            } else {
                                cli.http_port = Some(port);
                            }
                        }
                    }
                }
                _ => return Err(format!("unknown argument: {text}")),
            }
        }
        Ok(cli)
    }

    pub fn apply(&self, config: &mut Config) {
        if let Some(port) = self.udp_port {
            config.udp.port = port;
        }
        if let Some(port) = self.http_port {
            config.http.port = port;
        }
        if let Some(bind) = self.bind {
            config.http.bind = bind;
        }
    }

    /// A missing default file means defaults. An explicitly named missing file
    /// is an error; malformed reloads leave the caller's active config intact.
    pub fn load(&self, default_path: &Path) -> io::Result<Config> {
        let path = self.config_path.as_deref().unwrap_or(default_path);
        let mut config = match fs::read_to_string(path) {
            Ok(text) => Config::from_toml(&text).map_err(io::Error::other)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound && self.config_path.is_none() => {
                Config::default()
            }
            Err(error) => return Err(error),
        };
        self.apply(&mut config);
        config.validate().map_err(io::Error::other)?;
        Ok(config)
    }
}
