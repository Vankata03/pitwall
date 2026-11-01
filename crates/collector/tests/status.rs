use axum::{
    body::{Body, to_bytes},
    extract::ConnectInfo,
    http::{Request, StatusCode},
};
use pitwall_collector::{
    http::{HttpShell, TrustedHosts},
    ingest::config::{Cli, Config},
    paths::AppPaths,
    recorder::{Recorder, RecorderConfig},
    status::{Status, routes},
};
use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tower::ServiceExt;

#[tokio::test]
async fn assumed_status_settings_and_missing_stream_round_trip_obey_host_boundary() {
    let dir = std::env::temp_dir().join(format!("pitwall-status-{}", std::process::id()));
    let paths = AppPaths::from_data_dir(&dir);
    std::fs::create_dir_all(&paths.root).unwrap();
    let recorder = Arc::new(Mutex::new(
        Recorder::start(
            RecorderConfig::new(paths.recordings.clone(), u64::MAX),
            || 0,
        )
        .unwrap(),
    ));
    let status = Status::new(
        paths.clone(),
        paths.config.clone(),
        Cli::default(),
        Config::default(),
        recorder.clone(),
    );
    let router = routes(HttpShell::new(TrustedHosts::new([])), status).into_router();
    let request = |method: &str, path: &str, body: String, host: bool| {
        let mut r = Request::builder()
            .method(method)
            .uri(path)
            .header("host", "localhost:8777")
            .header("origin", "http://localhost:8777")
            .header("content-type", "application/json")
            .body(Body::from(body))
            .unwrap();
        r.extensions_mut().insert(ConnectInfo(
            if host {
                "127.0.0.1:42"
            } else {
                "192.168.1.2:42"
            }
            .parse::<SocketAddr>()
            .unwrap(),
        ));
        r
    };
    assert_eq!(
        router
            .clone()
            .oneshot(request("GET", "/api/settings", String::new(), false))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    let mut config = Config::default();
    config.udp.port = 23456;
    let body = serde_json::to_string(&config).unwrap();
    assert_eq!(
        router
            .clone()
            .oneshot(request("PUT", "/api/settings", body.clone(), false))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        router
            .clone()
            .oneshot(request("PUT", "/api/settings", body, true))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let response = router
        .clone()
        .oneshot(request("GET", "/api/settings", String::new(), true))
        .await
        .unwrap();
    let value: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 100000).await.unwrap()).unwrap();
    assert_eq!(value["udp"]["port"], 23456);
    assert_eq!(
        router
            .clone()
            .oneshot(request(
                "POST",
                "/api/status/missing-stream",
                r#"{"client":"screen","name":"Missing"}"#.into(),
                false
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    let response = router
        .clone()
        .oneshot(request("GET", "/api/status", String::new(), false))
        .await
        .unwrap();
    let value: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 100000).await.unwrap()).unwrap();
    assert_eq!(value["host"], false);
    assert_eq!(value["missingStreamLayouts"][0], "Missing");
    assert_eq!(
        router
            .clone()
            .oneshot(request(
                "DELETE",
                "/api/recordings/..%5Csecret.pitrec",
                "{}".into(),
                true
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    for (method, route) in [
        ("POST", "/api/recordings/rescan"),
        ("POST", "/api/recordings/open-folder"),
        ("POST", "/api/recordings/race.pitrec/pin"),
        ("DELETE", "/api/recordings/race.pitrec"),
    ] {
        assert_eq!(
            router
                .clone()
                .oneshot(request(method, route, "{}".into(), false))
                .await
                .unwrap()
                .status(),
            StatusCode::FORBIDDEN
        );
    }
    std::fs::write(
        paths.recordings.join("race.pitrec"),
        b"assumed closed recording",
    )
    .unwrap();
    std::fs::write(paths.recordings.join("race.json"), r#"{"session_uid":"1","packet_format":2026,"started_utc":"2026-11-01T00:00:00Z","track":"Spa","session_type":"Practice","duration_us":60000000,"laps":1,"pinned":false,"recompressed":true}"#).unwrap();
    assert_eq!(
        router
            .clone()
            .oneshot(request(
                "POST",
                "/api/recordings/race.pitrec/pin",
                r#"{"pinned":true}"#.into(),
                true
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    let response = router
        .clone()
        .oneshot(request("GET", "/api/recordings", String::new(), false))
        .await
        .unwrap();
    let files: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 100000).await.unwrap()).unwrap();
    assert_eq!(files[0]["sidecar"]["pinned"], true);
    let guard = recorder.lock().unwrap().mark_loaded("race.pitrec");
    assert_eq!(
        router
            .clone()
            .oneshot(request(
                "DELETE",
                "/api/recordings/race.pitrec",
                "{}".into(),
                true
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::CONFLICT
    );
    drop(guard);
    assert_eq!(
        router
            .clone()
            .oneshot(request(
                "DELETE",
                "/api/recordings/race.pitrec",
                "{}".into(),
                true
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    let response = router
        .clone()
        .oneshot(request("GET", "/api/recordings", String::new(), false))
        .await
        .unwrap();
    let files: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 100000).await.unwrap()).unwrap();
    assert_eq!(files, serde_json::json!([]));
    assert_eq!(
        router
            .clone()
            .oneshot(request(
                "POST",
                "/api/status/missing-stream",
                r#"{"client":"screen","name":null}"#.into(),
                false
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    let log = std::fs::read_to_string(paths.logs.join("collector.log")).unwrap();
    assert!(log.contains("Missing Stream view layout: \"Missing\""));
    let invalid = router
        .clone()
        .oneshot(request(
            "PUT",
            "/api/settings",
            r#"{"udp":{"port":0}}"#.into(),
            true,
        ))
        .await
        .unwrap();
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    let mut changed = Config::default();
    changed.http.port = 9876;
    let response = router
        .clone()
        .oneshot(request(
            "PUT",
            "/api/settings",
            serde_json::to_string(&changed).unwrap(),
            true,
        ))
        .await
        .unwrap();
    let value: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 100000).await.unwrap()).unwrap();
    assert_eq!(value["redirect"], "http://127.0.0.1:9876/status");
    drop(router);
    drop(recorder);
    std::fs::remove_dir_all(dir).unwrap();
}
