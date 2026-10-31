//! Offline recording tools. Both commands use the collector's public codec.
use pitwall_collector::pitrec::{Header, MAX_HEADER_BYTES, Reader, Writer};
use std::{
    env,
    error::Error,
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{self, BufReader, BufWriter, Read},
    path::Path,
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const USAGE: &str = "usage: pitrec convert INPUT.pwcap OUTPUT.pitrec [--uid UID --format 2025|2026]\n       pitrec clip INPUT.pitrec OUTPUT.pitrec START_US END_US";

fn main() {
    if let Err(error) = run(env::args_os().skip(1).collect()) {
        eprintln!("pitrec: {error}");
        std::process::exit(1);
    }
}

fn run(args: Vec<OsString>) -> Result<()> {
    match args.first().and_then(|s| s.to_str()) {
        Some("convert") if args.len() >= 3 => {
            let selection = parse_selection(&args[3..])?;
            let input = File::open(&args[1])?;
            write_output(Path::new(&args[2]), |output| {
                convert(BufReader::new(input), output, selection)
            })
        }
        Some("clip") if args.len() == 5 => {
            let start: u64 = args[3].to_str().ok_or("invalid START_US")?.parse()?;
            let end: u64 = args[4].to_str().ok_or("invalid END_US")?.parse()?;
            if start >= end {
                return Err("clip requires START_US < END_US".into());
            }
            let input = File::open(&args[1])?;
            write_output(Path::new(&args[2]), |output| {
                clip(BufReader::new(input), output, start, end)
            })
        }
        _ => Err(USAGE.into()),
    }
}

fn parse_selection(args: &[OsString]) -> Result<Option<(u64, u16)>> {
    if args.is_empty() {
        return Ok(None);
    }
    if args.len() != 4 {
        return Err(USAGE.into());
    }
    let mut uid = None;
    let mut format = None;
    for pair in args.as_chunks::<2>().0 {
        let value = pair[1].to_str().ok_or("invalid selection value")?;
        match pair[0].to_str() {
            Some("--uid") if uid.is_none() => {
                uid = Some(if let Some(hex) = value.strip_prefix("0x") {
                    u64::from_str_radix(hex, 16)?
                } else {
                    value.parse()?
                });
            }
            Some("--format") if format.is_none() => {
                format = Some(value.parse()?);
            }
            _ => return Err(USAGE.into()),
        }
    }
    let uid = uid.filter(|id| *id != 0).ok_or("--uid must be non-zero")?;
    let format = format
        .filter(|f| matches!(f, 2025 | 2026))
        .ok_or("--format must be 2025 or 2026")?;
    Ok(Some((uid, format)))
}

fn clip(input: impl Read, output: BufWriter<File>, start: u64, end: u64) -> Result<()> {
    let mut reader = Reader::new(input)?;
    let mut header = reader.header().clone();
    let mut writer = None;
    let mut output = Some(output);
    for record in &mut reader {
        let record = record?;
        if record.arrival_us >= end {
            break;
        }
        if record.arrival_us < start {
            continue;
        }
        if writer.is_none() {
            header.started_utc = shifted_utc(
                &header.started_utc,
                record.arrival_us / 1_000_000,
                ((record.arrival_us % 1_000_000) * 1000) as u32,
            )?;
            writer = Some(Writer::new(output.take().unwrap(), &header)?);
        }
        writer
            .as_mut()
            .unwrap()
            .write(record.arrival_us, &record.datagram)?;
    }
    writer
        .ok_or("clip range contains no complete records")?
        .finish()?;
    Ok(())
}

// Never overwrite an existing path. On failure remove only the file this call created.
fn write_output(path: &Path, operation: impl FnOnce(BufWriter<File>) -> Result<()>) -> Result<()> {
    let file = OpenOptions::new().write(true).create_new(true).open(path)?;
    match operation(BufWriter::new(file)) {
        Ok(()) => Ok(()),
        Err(error) => {
            if let Err(cleanup) = fs::remove_file(path) {
                return Err(format!(
                    "{error}; could not remove partial output {}: {cleanup}",
                    path.display()
                )
                .into());
            }
            Err(error)
        }
    }
}

fn shifted_utc(start: &str, seconds: u64, nanos: u32) -> Result<String> {
    let date = chrono::DateTime::parse_from_rfc3339(start)?;
    let delta =
        chrono::TimeDelta::new(i64::try_from(seconds)?, nanos).ok_or("UTC offset out of range")?;
    let shifted = date
        .checked_add_signed(delta)
        .ok_or("UTC timestamp out of range")?;
    Ok(shifted
        .with_timezone(&chrono::Utc)
        .to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true))
}

fn read_capture_record(input: &mut impl Read) -> io::Result<Option<(u64, Vec<u8>)>> {
    let mut prefix = [0; 10];
    if let Err(error) = input.read_exact(&mut prefix) {
        return if error.kind() == io::ErrorKind::UnexpectedEof {
            Ok(None)
        } else {
            Err(error)
        };
    }
    let time = u64::from_le_bytes(prefix[..8].try_into().unwrap());
    let length = u16::from_le_bytes(prefix[8..].try_into().unwrap()) as usize;
    let mut data = vec![0; length];
    if let Err(error) = input.read_exact(&mut data) {
        return if error.kind() == io::ErrorKind::UnexpectedEof {
            Ok(None)
        } else {
            Err(error)
        };
    }
    Ok(Some((time, data)))
}

fn convert(
    mut input: impl Read,
    output: BufWriter<File>,
    selection: Option<(u64, u16)>,
) -> Result<()> {
    let mut magic = [0; 7];
    input.read_exact(&mut magic)?;
    if &magic != b"PWCAP1\n" {
        return Err("unsupported pwcap magic".into());
    }
    let mut length = [0; 4];
    input.read_exact(&mut length)?;
    let length = u32::from_le_bytes(length) as usize;
    if length > MAX_HEADER_BYTES {
        return Err("pwcap metadata exceeds size limit".into());
    }
    let mut json = vec![0; length];
    input.read_exact(&mut json)?;
    let metadata: serde_json::Value = serde_json::from_slice(&json)?;
    let started = metadata["started_utc"]
        .as_str()
        .ok_or("pwcap metadata has no started_utc")?;
    let mut identity = selection;
    let mut origin = None;
    let mut last_ns = 0;
    let mut writer = None;
    let mut output = Some(output);
    while let Some((ns, data)) = read_capture_record(&mut input)? {
        if ns < last_ns {
            return Err("pwcap timestamps must be monotonic".into());
        }
        last_ns = ns;
        if data.len() < 29 {
            return Err("pwcap datagram is shorter than a telemetry header".into());
        }
        let format = u16::from_le_bytes(data[..2].try_into().unwrap());
        let uid = u64::from_le_bytes(data[7..15].try_into().unwrap());
        if uid == 0 {
            continue;
        }
        let selected = *identity.get_or_insert((uid, format));
        if (uid, format) != selected {
            if selection.is_some() {
                continue;
            }
            return Err(
                "pwcap contains multiple session identities; select --uid and --format".into(),
            );
        }
        if !matches!(format, 2025 | 2026) {
            return Err("unsupported telemetry format".into());
        }
        if writer.is_none() {
            let header = Header {
                packet_format: format,
                game_version: format!("{}.{}.{}", data[2], data[3], data[4]),
                pitwall_version: env!("CARGO_PKG_VERSION").into(),
                started_utc: shifted_utc(started, ns / 1_000_000_000, (ns % 1_000_000_000) as u32)?,
            };
            writer = Some(Writer::new(output.take().unwrap(), &header)?);
            origin = Some(ns);
        }
        writer
            .as_mut()
            .unwrap()
            .write((ns - origin.unwrap()) / 1000, &data)?;
    }
    writer
        .ok_or("pwcap contains no selected non-zero session datagrams")?
        .finish()?;
    Ok(())
}
