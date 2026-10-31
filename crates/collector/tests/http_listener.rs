use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use axum::{http::StatusCode, routing::get};
use pitwall_collector::http::{Access, HttpListener, HttpShell, TrustedHosts};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

const LOOPBACK: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0);

async fn request(addr: SocketAddr, path: &str) -> String {
    tokio::time::timeout(Duration::from_secs(3), async {
        let mut stream = TcpStream::connect(addr).await.unwrap();
        stream
            .write_all(
                format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n")
                    .as_bytes(),
            )
            .await
            .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).await.unwrap();
        response
    })
    .await
    .expect("HTTP request completes")
}

#[tokio::test]
async fn listener_serves_dashboard_and_supplies_real_peer_for_host_routes() {
    let router = HttpShell::new(TrustedHosts::discover().unwrap())
        .route(
            "/api/settings",
            get(|| async { StatusCode::NO_CONTENT }),
            Access::Host,
        )
        .into_router();
    let listener = HttpListener::bind(LOOPBACK, router).await.unwrap();
    let addr = listener.local_addr().unwrap();
    let response = request(addr, "/").await;
    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.contains("<title>pitwall</title>"));
    assert!(
        request(addr, "/api/settings")
            .await
            .starts_with("HTTP/1.1 204 No Content")
    );
    listener.shutdown().await.unwrap();
    assert!(TcpStream::connect(addr).await.is_err());
}

#[tokio::test]
async fn changing_the_http_port_releases_the_old_listener_and_preserves_routes() {
    let router = HttpShell::new(TrustedHosts::new([]))
        .route(
            "/api/settings",
            get(|| async { StatusCode::NO_CONTENT }),
            Access::Host,
        )
        .into_router();
    let mut listener = HttpListener::bind(LOOPBACK, router).await.unwrap();
    let old = listener.local_addr().unwrap();
    let new = listener.restart(LOOPBACK).await.unwrap();
    assert_ne!(old.port(), new.port());
    assert!(TcpStream::connect(old).await.is_err());
    assert!(request(new, "/status").await.starts_with("HTTP/1.1 200 OK"));
    assert!(
        request(new, "/api/settings")
            .await
            .starts_with("HTTP/1.1 204 No Content")
    );
    assert_eq!(listener.restart(new).await.unwrap(), new);
    listener.shutdown().await.unwrap();
}

#[tokio::test]
async fn changing_bind_address_on_the_same_port_restarts_the_listener() {
    let router = HttpShell::new(TrustedHosts::new([])).into_router();
    let mut listener = HttpListener::bind("0.0.0.0:0".parse().unwrap(), router)
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    let loopback = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    assert_eq!(listener.restart(loopback).await.unwrap(), loopback);
    assert!(request(loopback, "/").await.starts_with("HTTP/1.1 200 OK"));
    let all = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port);
    assert_eq!(listener.restart(all).await.unwrap(), all);
    assert!(request(loopback, "/").await.starts_with("HTTP/1.1 200 OK"));
    listener.shutdown().await.unwrap();
}

#[tokio::test]
async fn an_occupied_replacement_port_reports_error_and_keeps_the_current_listener() {
    let occupied = tokio::net::TcpListener::bind(LOOPBACK).await.unwrap();
    let router = HttpShell::new(TrustedHosts::new([])).into_router();
    let mut listener = HttpListener::bind(LOOPBACK, router).await.unwrap();
    let old = listener.local_addr().unwrap();
    assert_eq!(
        listener
            .restart(occupied.local_addr().unwrap())
            .await
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::AddrInUse
    );
    assert_eq!(listener.local_addr().unwrap(), old);
    assert!(request(old, "/").await.starts_with("HTTP/1.1 200 OK"));
    listener.shutdown().await.unwrap();
}

#[tokio::test]
async fn listener_restart_disconnects_an_in_flight_request_without_waiting_for_its_handler() {
    let entered = std::sync::Arc::new(tokio::sync::Notify::new());
    let handler_entered = entered.clone();
    let router = HttpShell::new(TrustedHosts::new([]))
        .route(
            "/pending",
            get(move || {
                let entered = handler_entered.clone();
                async move {
                    entered.notify_one();
                    std::future::pending::<StatusCode>().await
                }
            }),
            Access::Any,
        )
        .into_router();
    let mut listener = HttpListener::bind(LOOPBACK, router).await.unwrap();
    let old = listener.local_addr().unwrap();
    let mut stream = TcpStream::connect(old).await.unwrap();
    stream
        .write_all(b"GET /pending HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(3), entered.notified())
        .await
        .unwrap();
    let new = tokio::time::timeout(Duration::from_secs(3), listener.restart(LOOPBACK))
        .await
        .expect("restart must drop in-flight screens")
        .unwrap();
    let mut bytes = [0; 1];
    let closed = tokio::time::timeout(Duration::from_secs(3), stream.read(&mut bytes))
        .await
        .unwrap();
    assert!(matches!(closed, Ok(0) | Err(_)));
    assert!(request(new, "/").await.starts_with("HTTP/1.1 200 OK"));
    listener.shutdown().await.unwrap();
}

#[tokio::test]
async fn restart_also_disconnects_upgraded_screen_connections() {
    async fn upgrade(mut request: axum::extract::Request) -> axum::response::Response {
        let upgrade = hyper::upgrade::on(&mut request);
        tokio::spawn(async move {
            let mut stream = hyper_util::rt::TokioIo::new(upgrade.await.unwrap());
            stream.write_all(b"ready").await.unwrap();
            let _ = stream.read(&mut [0; 1]).await;
        });
        axum::response::Response::builder()
            .status(StatusCode::SWITCHING_PROTOCOLS)
            .header("connection", "upgrade")
            .header("upgrade", "pitwall-test")
            .body(axum::body::Body::empty())
            .unwrap()
    }
    let router = HttpShell::new(TrustedHosts::new([]))
        .route("/upgrade", get(upgrade), Access::Any)
        .into_router();
    let mut listener = HttpListener::bind(LOOPBACK, router).await.unwrap();
    let mut stream = TcpStream::connect(listener.local_addr().unwrap())
        .await
        .unwrap();
    stream.write_all(b"GET /upgrade HTTP/1.1\r\nHost: localhost\r\nConnection: Upgrade\r\nUpgrade: pitwall-test\r\n\r\n").await.unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        let mut headers = Vec::new();
        while !headers.ends_with(b"\r\n\r\n") {
            headers.push(stream.read_u8().await.unwrap());
        }
        assert!(headers.starts_with(b"HTTP/1.1 101"));
        let mut ready = [0; 5];
        stream.read_exact(&mut ready).await.unwrap();
        assert_eq!(&ready, b"ready");
    })
    .await
    .unwrap();
    listener.restart(LOOPBACK).await.unwrap();
    let closed = tokio::time::timeout(Duration::from_secs(3), stream.read(&mut [0; 1]))
        .await
        .expect("upgraded screens must disconnect too");
    assert!(matches!(closed, Ok(0) | Err(_)));
    listener.shutdown().await.unwrap();
}
