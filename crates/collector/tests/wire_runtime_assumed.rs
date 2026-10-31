//! Real binary/loopback smoke with re-sent synthetic datagrams; not game qualification.
use futures_util::StreamExt;
use std::{
    net::{TcpListener, UdpSocket},
    process::{Command, Stdio},
    time::Duration,
};
struct Child(std::process::Child);
impl Drop for Child {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
#[tokio::test]
async fn assumed_binary_resends_datagrams_to_websocket_and_composes_routes() {
    let udp = UdpSocket::bind("127.0.0.1:0").unwrap();
    let udp_port = udp.local_addr().unwrap().port();
    let http = TcpListener::bind("127.0.0.1:0").unwrap();
    let http_port = http.local_addr().unwrap().port();
    let config =
        std::env::temp_dir().join(format!("pitwall-wire-runtime-{}.toml", std::process::id()));
    std::fs::write(&config,format!("[udp]\nport={udp_port}\n[http]\nbind='127.0.0.1'\nport={http_port}\n[recording]\nenabled=false\n")).unwrap();
    drop(udp);
    drop(http);
    let _child = Child(
        Command::new(env!("CARGO_BIN_EXE_pitwall"))
            .arg("--config")
            .arg(&config)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let mut ws = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Ok((ws, _)) =
                tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{http_port}/ws")).await
            {
                break ws;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let hello = ws.next().await.unwrap().unwrap().into_text().unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&hello).unwrap()["type"],
        "hello"
    );
    let snapshot = ws.next().await.unwrap().unwrap().into_text().unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&snapshot).unwrap()["type"],
        "snapshot"
    );
    let p = Command::new(if cfg!(windows) { "python" } else { "python3" })
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tools/telemetry/assumed_datagram.py"
        ))
        .args([
            "2026",
            "2",
            "--set",
            "header.session_uid=123",
            "--set",
            "lap_data.0.result_status=2",
        ])
        .output()
        .unwrap();
    assert!(p.status.success(), "{}", String::from_utf8_lossy(&p.stderr));
    let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
    sender.send_to(&p.stdout, ("127.0.0.1", udp_port)).unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let msg = ws.next().await.unwrap().unwrap();
            if msg.is_binary() {
                let bytes = msg.into_data();
                if bytes[54] & 0x30 == 0 {
                    assert_eq!(bytes.len(), 976);
                    break;
                }
            }
        }
    })
    .await
    .unwrap();
    // HTTP route composition is observable through the same configured listener.
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    for path in ["/api/layouts", "/api/tracks/5/3323"] {
        let mut tcp = tokio::net::TcpStream::connect(("127.0.0.1", http_port))
            .await
            .unwrap();
        tcp.write_all(
            format!(
                "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{http_port}\r\nConnection: close\r\n\r\n"
            )
            .as_bytes(),
        )
        .await
        .unwrap();
        let mut text = String::new();
        tcp.read_to_string(&mut text).await.unwrap();
        assert!(text.starts_with("HTTP/1.1 200"), "{text}");
    }
    std::fs::remove_file(config).unwrap();
}
