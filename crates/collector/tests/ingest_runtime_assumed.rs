//! Assumed executable smoke: live config edits redirect synthetic UDP input.
use std::{
    io::{BufRead, BufReader},
    net::UdpSocket,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

struct Collector(Child);
impl Drop for Collector {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn probe(sender: &UdpSocket, port: u16, target: &UdpSocket, marker: &[u8], child: &mut Child) {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut bytes = [0; 100];
    loop {
        assert!(
            child.try_wait().unwrap().is_none(),
            "collector exited before receiving"
        );
        sender.send_to(marker, ("127.0.0.1", port)).unwrap();
        if let Ok((length, _)) = target.recv_from(&mut bytes)
            && &bytes[..length] == marker
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "collector did not forward after startup/reload"
        );
    }
}

#[test]
fn assumed_binary_receives_and_reloads_udp_port_and_forwarding() {
    let old_reservation = UdpSocket::bind("127.0.0.1:0").unwrap();
    let new_reservation = UdpSocket::bind("127.0.0.1:0").unwrap();
    let old_port = old_reservation.local_addr().unwrap().port();
    let new_port = new_reservation.local_addr().unwrap().port();
    let first = UdpSocket::bind("127.0.0.1:0").unwrap();
    let second = UdpSocket::bind("127.0.0.1:0").unwrap();
    first
        .set_read_timeout(Some(Duration::from_millis(30)))
        .unwrap();
    second
        .set_read_timeout(Some(Duration::from_millis(30)))
        .unwrap();
    let root = std::env::temp_dir().join(format!("pitwall-ingest-runtime-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("config.toml");
    let write_config = |port, target: &UdpSocket| {
        std::fs::write(
            &path,
            format!(
                "[udp]\nport={port}\n[recording]\nenabled=false\n[forward]\nenabled=true\ntargets=['{}']\n",
                target.local_addr().unwrap()
            ),
        )
        .unwrap()
    };
    write_config(old_port, &first);
    drop(old_reservation);
    let mut child = Collector(
        Command::new(env!("CARGO_BIN_EXE_pitwall"))
            .arg("--console")
            .arg("--config")
            .arg(&path)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let (logs, logged) = std::sync::mpsc::channel();
    let stderr = child.0.stderr.take().unwrap();
    std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines() {
            if logs.send(line.unwrap()).is_err() {
                break;
            }
        }
    });
    let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
    probe(&sender, old_port, &first, b"first", &mut child.0);
    drop(new_reservation);
    write_config(new_port, &second);
    probe(&sender, new_port, &second, b"second", &mut child.0);
    // A malformed edit leaves the last valid listener and forwarding active.
    std::fs::write(&path, "broken TOML").unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let line = logged
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap();
        if line.contains("Config reload rejected; keeping active settings") {
            break;
        }
    }
    probe(&sender, new_port, &second, b"retained", &mut child.0);
    drop(child);
    std::fs::remove_dir_all(root).unwrap();
}
