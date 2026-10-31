//! Manual 1x UDP re-send tool, using the collector's public recording reader.
use pitwall_collector::pitrec::Reader;
use std::{
    env,
    error::Error,
    fs::File,
    io::BufReader,
    net::{ToSocketAddrs, UdpSocket},
    thread,
    time::{Duration, Instant},
};

const USAGE: &str = "usage: pitrec-resend INPUT.pitrec HOST:PORT";

fn main() {
    if let Err(error) = run() {
        eprintln!("pitrec-resend: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err(USAGE.into());
    }
    let mut reader = Reader::new(BufReader::new(File::open(&args[0])?))?;
    let destination = args[1]
        .to_str()
        .ok_or("destination must be HOST:PORT")?
        .to_socket_addrs()?
        .next()
        .ok_or("destination resolved to no addresses")?;
    let socket = UdpSocket::bind(if destination.is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    })?;
    socket.connect(destination)?;
    let mut start = None;
    let mut sent = 0u64;
    for record in &mut reader {
        let record = record?;
        // Start after decoding the first block. Absolute deadlines keep per-record
        // decoding and send overhead from accumulating into a slower replay.
        let start = *start.get_or_insert_with(Instant::now);
        let deadline = start
            .checked_add(Duration::from_micros(record.arrival_us))
            .ok_or("recorded arrival time exceeds the monotonic clock range")?;
        while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
            thread::sleep(remaining);
        }
        socket.send(&record.datagram)?;
        sent += 1;
    }
    if reader.has_incomplete_tail() {
        return Err(
            format!("recording has an incomplete tail; sent {sent} complete datagrams").into(),
        );
    }
    eprintln!("sent {sent} datagrams to {destination}");
    Ok(())
}
