//! `/api/tracks` routes. Synthetic and `assumed`: the learned lap is a generated circle.
use axum::{
    Router,
    body::{Body, to_bytes},
    extract::ConnectInfo,
    http::{Method, Request, StatusCode},
};
use pitwall_collector::{
    http::{HttpShell, TrustedHosts},
    tracks::{self, Sample, TrackKey, TrackStore},
};
use serde_json::Value;
use std::net::SocketAddr;
use tower::ServiceExt;

fn store() -> TrackStore {
    let dir = std::env::temp_dir().join(format!("pitwall-tracks-routes-{}", std::process::id()));
    let dir = dir.join(format!("{:?}", std::thread::current().id()).replace(['(', ')'], ""));
    let _ = std::fs::remove_dir_all(&dir);
    TrackStore::open(dir)
}

fn app(store: &TrackStore) -> Router {
    tracks::routes(HttpShell::new(TrustedHosts::new([])), store.clone()).into_router()
}

async fn call(app: &Router, method: Method, uri: &str, peer: &str) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(uri)
        .header("host", "localhost:8777")
        .header("origin", "http://localhost:8777")
        .header("content-type", "application/json")
        .body(Body::empty())
        .unwrap();
    request
        .extensions_mut()
        .insert(ConnectInfo(peer.parse::<SocketAddr>().unwrap()));
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 10_000_000).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

const LAN: &str = "192.168.1.30:5555";
const HOST: &str = "127.0.0.1:5555";

#[tokio::test]
async fn unknown_layout_is_404_and_a_shipped_one_is_served_to_any_screen() {
    let store = store();
    let app = app(&store);
    let (status, _) = call(&app, Method::GET, "/api/tracks/31/6200", LAN).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, map) = call(&app, Method::GET, "/api/tracks/5/3323", LAN).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(map["trackId"], 5);
    assert_eq!(map["trackLength"], 3323);
    assert_eq!(map["source"], "shipped");
    assert_eq!(map["complete"], true);
    assert_eq!(map["pit"], Value::Null);
    assert_eq!(map["revision"], store.revision());
    let first = &map["outline"][0];
    assert_eq!(first.as_array().unwrap().len(), 3, "[bucket, x, z]");
    assert_eq!(first[0], 0);
}

#[tokio::test]
async fn a_map_being_learned_is_served_with_its_revision() {
    let store = store();
    let app = app(&store);
    let key = TrackKey::new(31, 6200);
    for d in (0..800).step_by(8) {
        store.feed(
            key,
            Sample {
                car: 0,
                x: d as f32,
                z: 1.0,
                lap_distance: d as f32,
                in_pit: false,
                lap_invalid: false,
            },
        );
    }
    let (status, map) = call(&app, Method::GET, "/api/tracks/31/6200", LAN).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(map["source"], "learned");
    assert_eq!(map["complete"], false);
    assert_eq!(map["outline"].as_array().unwrap().len(), 100);
    assert_eq!(map["revision"], store.revision());
}

#[tokio::test]
async fn relearn_is_host_only_and_404_for_an_unknown_layout() {
    let store = store();
    let app = app(&store);
    let (status, _) = call(&app, Method::POST, "/api/tracks/5/3323/relearn", LAN).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = call(&app, Method::POST, "/api/tracks/31/6200/relearn", HOST).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let key = TrackKey::new(31, 6200);
    store.feed(
        key,
        Sample {
            car: 0,
            x: 1.0,
            z: 1.0,
            lap_distance: 8.0,
            in_pit: false,
            lap_invalid: false,
        },
    );
    let (status, _) = call(&app, Method::POST, "/api/tracks/31/6200/relearn", HOST).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = call(&app, Method::GET, "/api/tracks/31/6200", LAN).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
