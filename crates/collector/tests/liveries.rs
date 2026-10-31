//! `/api/liveries` and `/liveries/{file}` (spec sections 8 and 13). The pack folder is
//! built from synthetic bytes; no real livery image is involved.
use axum::{
    Router,
    body::{Body, to_bytes},
    extract::ConnectInfo,
    http::{Request, StatusCode, header},
};
use pitwall_collector::{
    http::{HttpShell, TrustedHosts},
    liveries,
    paths::AppPaths,
};
use serde_json::{Value, json};
use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU32, Ordering},
};
use tower::ServiceExt;

const LAN: &str = "192.168.1.30:5555";
const HOST: &str = "127.0.0.1:5555";

/// A fresh data directory; the pack folder is `AppPaths::liveries` and is not created.
fn data_dir() -> PathBuf {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "pitwall-liveries-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn app(paths: &AppPaths) -> Router {
    liveries::routes(HttpShell::new(TrustedHosts::new([])), paths).into_router()
}

fn put(path: &Path, bytes: &[u8]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

async fn get(app: &Router, uri: &str, peer: &str) -> (StatusCode, Option<String>, Vec<u8>) {
    let mut request = Request::builder()
        .uri(uri)
        .header("host", "localhost:8777")
        .body(Body::empty())
        .unwrap();
    request
        .extensions_mut()
        .insert(ConnectInfo(peer.parse::<SocketAddr>().unwrap()));
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .map(|value| value.to_str().unwrap().to_string());
    let bytes = to_bytes(response.into_body(), 10_000_000).await.unwrap();
    (status, content_type, bytes.to_vec())
}

async fn files(app: &Router) -> Value {
    let (status, content_type, body) = get(app, "/api/liveries", LAN).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(content_type.as_deref(), Some("application/json"));
    serde_json::from_slice::<Value>(&body).unwrap()["files"].clone()
}

#[tokio::test]
async fn listing_is_empty_while_the_folder_does_not_exist() {
    let paths = AppPaths::from_data_dir(&data_dir());
    assert!(!paths.liveries.exists());
    assert_eq!(files(&app(&paths)).await, json!([]));
}

#[tokio::test]
async fn listing_reads_the_folder_on_every_request_and_names_only_pack_images() {
    let paths = AppPaths::from_data_dir(&data_dir());
    let router = app(&paths);
    put(&paths.liveries.join("my-team.png"), b"a");
    put(&paths.liveries.join("481.png"), b"b");
    put(&paths.liveries.join("3.png"), b"c");
    // Matches nothing: ignored.
    put(&paths.liveries.join("readme.txt"), b"d");
    put(&paths.liveries.join("unknown.png"), b"e");
    put(&paths.liveries.join("3.png.bak"), b"f");
    put(&paths.liveries.join("nested/4.png"), b"g");
    std::fs::create_dir_all(paths.liveries.join("5.png")).unwrap();

    assert_eq!(
        files(&router).await,
        json!(["3.png", "481.png", "my-team.png"])
    );

    put(&paths.liveries.join("0.png"), b"h");
    std::fs::remove_file(paths.liveries.join("481.png")).unwrap();
    assert_eq!(
        files(&router).await,
        json!(["0.png", "3.png", "my-team.png"])
    );
}

#[tokio::test]
async fn a_pack_image_is_served_as_png_to_any_screen() {
    let paths = AppPaths::from_data_dir(&data_dir());
    let router = app(&paths);
    put(&paths.liveries.join("my-team.png"), b"my team bytes");
    put(&paths.liveries.join("482.png"), b"482 bytes");
    for peer in [LAN, HOST] {
        let (status, content_type, body) = get(&router, "/liveries/my-team.png", peer).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(content_type.as_deref(), Some("image/png"));
        assert_eq!(body, b"my team bytes");
    }
    assert_eq!(get(&router, "/liveries/482.png", LAN).await.2, b"482 bytes");
}

#[tokio::test]
async fn a_replaced_pack_image_is_served_without_a_restart() {
    let paths = AppPaths::from_data_dir(&data_dir());
    let router = app(&paths);
    assert_eq!(
        get(&router, "/liveries/7.png", LAN).await.0,
        StatusCode::NOT_FOUND
    );
    put(&paths.liveries.join("7.png"), b"first");
    assert_eq!(get(&router, "/liveries/7.png", LAN).await.2, b"first");
    put(&paths.liveries.join("7.png"), b"second");
    assert_eq!(get(&router, "/liveries/7.png", LAN).await.2, b"second");
}

#[tokio::test]
async fn names_outside_the_pack_never_leave_the_folder() {
    let dir = data_dir();
    let paths = AppPaths::from_data_dir(&dir);
    let router = app(&paths);
    put(&paths.liveries.join("3.png"), b"inside");
    put(&paths.root.join("7.png"), b"one level up");
    put(&paths.liveries.join("readme.txt"), b"text");
    put(&paths.liveries.join("unknown.png"), b"unmatched");
    put(&paths.liveries.join("nested/4.png"), b"nested");
    put(&paths.config, b"secret = 1");

    for uri in [
        "/liveries/..%2F7.png",
        "/liveries/%2e%2e%2F7.png",
        "/liveries/%2E%2E/7.png",
        "/liveries/../7.png",
        "/liveries/..%5C7.png",
        "/liveries/nested%2F4.png",
        "/liveries/nested/4.png",
        "/liveries/..%2Fconfig.toml",
        "/liveries/readme.txt",
        "/liveries/unknown.png",
        "/liveries/3.PNG",
        "/liveries/3.png%00",
        "/liveries/",
        "/liveries/%2F",
    ] {
        let (status, _, body) = get(&router, uri, LAN).await;
        assert_ne!(status, StatusCode::OK, "{uri}");
        assert!(
            !String::from_utf8_lossy(&body).contains("one level up")
                && !String::from_utf8_lossy(&body).contains("secret")
                && !String::from_utf8_lossy(&body).contains("nested"),
            "{uri}"
        );
    }
}

#[tokio::test]
async fn a_folder_named_like_an_image_is_not_served() {
    let paths = AppPaths::from_data_dir(&data_dir());
    let router = app(&paths);
    std::fs::create_dir_all(paths.liveries.join("5.png")).unwrap();
    assert_eq!(
        get(&router, "/liveries/5.png", LAN).await.0,
        StatusCode::NOT_FOUND
    );
}
