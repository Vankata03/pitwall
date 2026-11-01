//! Assumed synthetic loopback UDP checks; no game or device qualification.
use pitwall_collector::ingest::{
    config::ForwardConfig,
    udp::{LiveUdp, PreparedUpdate},
};
use std::{
    net::{SocketAddr, UdpSocket},
    time::{Duration, Instant},
};

fn loopback() -> SocketAddr {
    "127.0.0.1:0".parse().unwrap()
}

#[test]
fn assumed_nonblocking_receive_forwards_original_bytes_even_when_not_parseable() {
    let mut live = LiveUdp::bind(loopback(), &ForwardConfig::default()).unwrap();
    let mut tap = |_: u64, _: &[u8]| {};
    assert!(live.poll(10, || 0, &mut tap).unwrap().is_empty());
    let target = UdpSocket::bind(loopback()).unwrap();
    target
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let update = PreparedUpdate::prepare(
        live.local_addr().unwrap(),
        live.local_addr().unwrap(),
        &ForwardConfig {
            enabled: true,
            targets: vec![target.local_addr().unwrap().to_string()],
        },
    )
    .unwrap();
    live.apply(update);
    let sender = UdpSocket::bind(loopback()).unwrap();
    sender
        .send_to(&[0xe8, 0x07, 0xff, 0x00], live.local_addr().unwrap())
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    let packets = loop {
        let packets = live.poll(10, || 123, &mut tap).unwrap();
        if !packets.is_empty() {
            break packets;
        }
        assert!(Instant::now() < deadline, "loopback input not received");
        std::thread::yield_now();
    };
    assert_eq!(packets[0].arrival_time_us, 123);
    assert!(packets[0].result.is_err());
    let mut bytes = [0; 100];
    let (len, _) = target.recv_from(&mut bytes).unwrap();
    assert_eq!(&bytes[..len], &[0xe8, 0x07, 0xff, 0x00]);
    assert_eq!(live.status(123).forwarded, 1);
}

#[test]
fn assumed_live_port_and_forwarding_updates_are_atomic_and_keep_gate_state() {
    use pitwall_collector::ingest::Disposition;
    let packet = std::process::Command::new(if cfg!(windows) { "python" } else { "python3" })
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tools/telemetry/assumed_datagram.py"
        ))
        .args([
            "2026",
            "6",
            "--set",
            "header.session_uid=42",
            "--set",
            "header.overall_frame_identifier=3",
        ])
        .output()
        .unwrap();
    assert!(packet.status.success());
    let first = UdpSocket::bind(loopback()).unwrap();
    let second = UdpSocket::bind(loopback()).unwrap();
    first
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    second
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let config = |socket: &UdpSocket| ForwardConfig {
        enabled: true,
        targets: vec![socket.local_addr().unwrap().to_string()],
    };
    let mut live = LiveUdp::bind(loopback(), &config(&first)).unwrap();
    let sender = UdpSocket::bind(loopback()).unwrap();
    let mut tap = Vec::new();
    let mut receive = |live: &mut LiveUdp| {
        sender
            .send_to(&packet.stdout, live.local_addr().unwrap())
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let packets = live
                .poll(1, || 55, &mut |at, bytes: &[u8]| {
                    tap.push((at, bytes.to_vec()))
                })
                .unwrap();
            if let Some(packet) = packets.into_iter().next() {
                break packet.result.unwrap().1;
            }
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
    };
    assert_eq!(receive(&mut live), Disposition::Apply);
    let mut bytes = [0; 2000];
    assert_eq!(first.recv_from(&mut bytes).unwrap().0, packet.stdout.len());
    let old_address = live.local_addr().unwrap();
    // A failed listener bind must not change the active forwarding destination.
    let occupied = UdpSocket::bind(loopback()).unwrap();
    assert!(
        PreparedUpdate::prepare(
            old_address,
            occupied.local_addr().unwrap(),
            &config(&second)
        )
        .is_err()
    );
    assert_eq!(receive(&mut live), Disposition::DropFrame);
    assert_eq!(first.recv_from(&mut bytes).unwrap().0, packet.stdout.len());
    assert_eq!(live.local_addr().unwrap(), old_address);
    let update = PreparedUpdate::prepare(old_address, loopback(), &config(&second)).unwrap();
    live.apply(update);
    assert_ne!(live.local_addr().unwrap(), old_address);
    assert_eq!(
        receive(&mut live),
        Disposition::DropFrame,
        "port change does not reset session frames"
    );
    assert_eq!(second.recv_from(&mut bytes).unwrap().0, packet.stdout.len());
    first.set_nonblocking(true).unwrap();
    assert_eq!(
        first.recv_from(&mut bytes).unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    let update = PreparedUpdate::prepare(
        live.local_addr().unwrap(),
        live.local_addr().unwrap(),
        &ForwardConfig::default(),
    )
    .unwrap();
    live.apply(update);
    assert_eq!(receive(&mut live), Disposition::DropFrame);
    second.set_nonblocking(true).unwrap();
    assert_eq!(
        second.recv_from(&mut bytes).unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    assert_eq!(
        tap.len(),
        4,
        "recorder receives all duplicates before frame drops"
    );
    assert_eq!(live.status(55).forwarded, 3);
}

#[test]
fn assumed_forward_only_update_preserves_listener_queue_and_poll_budget() {
    let mut live = LiveUdp::bind(loopback(), &ForwardConfig::default()).unwrap();
    let sender = UdpSocket::bind(loopback()).unwrap();
    for _ in 0..3 {
        sender.send_to(&[1, 2], live.local_addr().unwrap()).unwrap();
    }
    let address = live.local_addr().unwrap();
    let update = PreparedUpdate::prepare(address, address, &ForwardConfig::default()).unwrap();
    live.apply(update);
    let mut count = 0;
    let mut tap = |_: u64, _: &[u8]| count += 1;
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut received = 0;
    while received < 3 {
        let packets = live.poll(1, || 0, &mut tap).unwrap();
        assert!(packets.len() <= 1);
        received += packets.len();
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert_eq!(count, 3);
    assert_eq!(live.local_addr().unwrap(), address);
}

#[test]
fn assumed_wait_wakes_on_arrival_instead_of_the_sleep_granularity() {
    let mut live = LiveUdp::bind(loopback(), &ForwardConfig::default()).unwrap();
    let mut tap = |_: u64, _: &[u8]| {};
    let started = Instant::now();
    assert!(!live.wait(Duration::from_millis(60)).unwrap());
    assert!(started.elapsed() >= Duration::from_millis(40));

    let target = live.local_addr().unwrap();
    let sender = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(30));
        UdpSocket::bind(loopback())
            .unwrap()
            .send_to(&[1, 2, 3], target)
            .unwrap();
    });
    let started = Instant::now();
    assert!(live.wait(Duration::from_secs(5)).unwrap());
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "wait must end at arrival, not at the timeout"
    );
    sender.join().unwrap();
    // Waiting only peeks: the datagram is still there, and polling is still nonblocking.
    assert_eq!(live.poll(10, || 7, &mut tap).unwrap().len(), 1);
    let started = Instant::now();
    assert!(live.poll(10, || 7, &mut tap).unwrap().is_empty());
    assert!(started.elapsed() < Duration::from_millis(20));
}
