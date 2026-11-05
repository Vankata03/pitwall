//! Native command line. Obsolete HTTP values are accepted without parsing: they
//! cannot affect startup because this app has no HTTP listener.
use pitwall_core::ingest::config::Cli;
use std::{path::PathBuf, time::Duration};

pub const HELP: &str = "pitwall-native [OPTIONS]

  --console           Attach a console for debugging output
  --udp-port PORT     UDP telemetry port (overrides the saved setting)
  --config PATH       Settings file (default: <data folder>/config.toml)
  --data-dir DIR      Keep all data under DIR/pitwall instead of %APPDATA%/pitwall
  --replay FILE       Play a .pitrec recording (it is not copied into the library)
  --from SECONDS      Start the replay at this position
  --loop              Restart the replay from --from when it ends
  --size WxH          Dashboard window size
  --stats             Print render statistics each second
  -h, --help          Show this help

Obsolete --http-port and --bind are accepted and ignored.";

pub struct Args {
    pub cli: Cli,
    pub replay: Option<PathBuf>,
    pub from: Duration,
    pub looped: bool,
    pub size: (f32, f32),
    pub stats: bool,
    pub obsolete: Vec<String>,
}
impl Args {
    pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut result = Self {
            cli: Cli::default(),
            replay: None,
            from: Duration::ZERO,
            looped: false,
            size: (900.0, 480.0),
            stats: false,
            obsolete: vec![],
        };
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            let (flag, inline) = arg
                .split_once('=')
                .map_or((arg.as_str(), None), |(f, v)| (f, Some(v.to_owned())));
            let mut value = || {
                inline
                    .clone()
                    .or_else(|| args.next())
                    .ok_or_else(|| format!("{flag} needs a value"))
            };
            match flag {
                "--console" => result.cli.console = true,
                "--help" | "-h" => result.cli.help = true,
                "--stats" => result.stats = true,
                "--loop" => result.looped = true,
                "--http-port" | "--bind" => {
                    value()?;
                    result.obsolete.push(flag.to_owned());
                }
                "--config" => result.cli.config_path = Some(value()?.into()),
                "--data-dir" => result.cli.data_dir = Some(value()?.into()),
                "--port" | "--udp-port" => {
                    let port: u16 = value()?.parse().map_err(|_| "invalid UDP port")?;
                    if port == 0 {
                        return Err("invalid UDP port".into());
                    }
                    result.cli.udp_port = Some(port);
                }
                "--replay" => result.replay = Some(value()?.into()),
                "--from" => {
                    result.from = Duration::try_from_secs_f64(
                        value()?.parse().map_err(|_| "invalid --from")?,
                    )
                    .map_err(|_| "invalid --from")?;
                }
                "--size" => {
                    let text = value()?;
                    let (w, h) = text.split_once('x').ok_or("--size takes WIDTHxHEIGHT")?;
                    result.size = (
                        w.parse().map_err(|_| "invalid width")?,
                        h.parse().map_err(|_| "invalid height")?,
                    );
                    if !result.size.0.is_finite()
                        || !result.size.1.is_finite()
                        || result.size.0 <= 0.0
                        || result.size.1 <= 0.0
                    {
                        return Err("invalid window size".into());
                    }
                }
                _ => return Err(format!("unknown argument {arg}")),
            }
        }
        Ok(result)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn obsolete_flags_never_validate_their_unused_values() {
        let args = Args::parse(
            [
                "--http-port=not-a-port",
                "--bind",
                "not-an-address",
                "--data-dir=C:/copy",
            ]
            .map(str::to_owned),
        )
        .unwrap();
        assert_eq!(args.obsolete, ["--http-port", "--bind"]);
        assert_eq!(args.cli.data_dir, Some(PathBuf::from("C:/copy")));
    }
    #[test]
    fn rejects_unsafe_duration_and_unknown_arguments() {
        for input in [
            vec!["--from", "NaN"],
            vec!["--from", "-1"],
            vec!["--udp-port", "0"],
            vec!["--wat"],
            vec!["--config"],
        ] {
            assert!(Args::parse(input.into_iter().map(str::to_owned)).is_err());
        }
    }
}
