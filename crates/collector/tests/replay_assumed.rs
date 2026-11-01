//! Synthetic assumed recordings; no live-game or performance qualification.
use pitwall_collector::{
    pitrec::{Header, Writer},
    replay::Replay,
};
use std::{fs::File, process::Command, time::Duration};
fn recording() -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "replay-assumed-{}-{:?}.pitrec",
        std::process::id(),
        std::thread::current().id()
    ));
    let mut writer = Writer::new(
        File::create(&path).unwrap(),
        &Header {
            packet_format: 2026,
            game_version: "assumed".into(),
            pitwall_version: "test".into(),
            started_utc: "2026-01-01T00:00:00Z".into(),
        },
    )
    .unwrap();
    for (time, frame, lap, session) in [
        (0, 1, 1, 0),
        (20, 2, 2, 20),
        (40, 3, 3, 40),
        (45, 4, 2, 21),
        (65, 5, 3, 41),
    ] {
        let out = Command::new("python3")
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tools/telemetry/assumed_datagram.py"
            ))
            .args([
                "2026",
                "2",
                "--set",
                "header.session_uid=101",
                "--set",
                &format!("header.overall_frame_identifier={frame}"),
                "--set",
                &format!("header.session_time={session}"),
                "--set",
                "lap_data.0.result_status=2",
                "--set",
                &format!("lap_data.0.current_lap_num={lap}"),
            ])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        writer.write(time * 1_000_000, &out.stdout).unwrap();
    }
    writer.finish().unwrap();
    path
}
#[test]
fn assumed_seek_equals_straight_play_across_checkpoints_silence_and_flashback() {
    let path = recording();
    let mut progress = vec![];
    let mut seek = Replay::load(File::open(&path).unwrap(), |p| progress.push(p)).unwrap();
    assert_eq!(progress.last(), Some(&1.0));
    for target in [0, 20, 31, 44, 45, 61, 65, 5] {
        let mut straight = Replay::load(File::open(&path).unwrap(), |_| {}).unwrap();
        for t in 0..=target {
            straight
                .tick(Duration::from_secs(if t == 0 { 0 } else { 1 }))
                .unwrap();
        }
        seek.seek(Duration::from_secs(target)).unwrap();
        assert_eq!(seek.model(), straight.model(), "target {target}");
    }
    seek.jump_lap(0, 2).unwrap();
    assert_eq!(seek.position(), Duration::from_secs(45));
    std::fs::remove_file(path).unwrap();
}
#[test]
fn assumed_transport_speeds_pause_eof_and_validation() {
    let path = recording();
    let mut replay = Replay::load(File::open(&path).unwrap(), |_| {}).unwrap();
    replay.pause(true);
    replay.tick(Duration::from_secs(10)).unwrap();
    assert_eq!(replay.position(), Duration::ZERO);
    replay.set_speed(4.0).unwrap();
    replay.pause(false);
    replay.tick(Duration::from_secs(10)).unwrap();
    assert_eq!(replay.position(), Duration::from_secs(40));
    assert!(replay.set_speed(3.0).is_err());
    assert!(replay.seek(Duration::from_secs(66)).is_err());
    replay.tick(Duration::from_secs(10)).unwrap();
    let state = replay.state(None);
    assert!(state.paused);
    assert!(state.banner.contains("restart or exit"));
    std::fs::remove_file(path).unwrap();
}
#[tokio::test]
async fn assumed_routes_enforce_host_start_stop_and_lan_transport() {
    use axum::{
        body::Body,
        extract::ConnectInfo,
        http::{Request, StatusCode},
    };
    use pitwall_collector::{
        http::{HttpShell, TrustedHosts},
        recorder::{Recorder, RecorderConfig},
        replay::{self, Controller},
    };
    use std::sync::{Arc, Mutex};
    use tower::ServiceExt;
    let dir = std::env::temp_dir().join(format!("replay-routes-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let recorder = Arc::new(Mutex::new(
        Recorder::start(RecorderConfig::new(dir.clone(), u64::MAX), || 0).unwrap(),
    ));
    let controller = Controller::new(dir.clone(), recorder);
    let app = replay::routes(HttpShell::new(TrustedHosts::new([])), controller).into_router();
    for (route, body, peer, expected) in [
        (
            "start",
            r#"{"name":"test.pitrec"}"#,
            "192.168.1.2:1234",
            StatusCode::FORBIDDEN,
        ),
        ("stop", "{}", "192.168.1.2:1234", StatusCode::FORBIDDEN),
        (
            "pause",
            r#"{"paused":true}"#,
            "192.168.1.2:1234",
            StatusCode::CONFLICT,
        ),
        (
            "start",
            r#"{"name":"../secret.pitrec"}"#,
            "127.0.0.1:1234",
            StatusCode::BAD_REQUEST,
        ),
    ] {
        let mut req = Request::post(format!("/api/replay/{route}"))
            .header("host", "localhost:8777")
            .header("origin", "http://localhost:8777")
            .header("content-type", "application/json")
            .body(Body::from(body))
            .unwrap();
        req.extensions_mut()
            .insert(ConnectInfo(peer.parse::<std::net::SocketAddr>().unwrap()));
        assert_eq!(
            app.clone().oneshot(req).await.unwrap().status(),
            expected,
            "{route}"
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn assumed_loaded_replay_publishes_seek_epochs_protects_retention_and_returns_to_live() {
    use axum::{
        body::Body,
        extract::ConnectInfo,
        http::{Request, StatusCode},
    };
    use pitwall_collector::{
        http::{HttpShell, TrustedHosts},
        model::SessionModel,
        recorder::{Recorder, RecorderConfig},
        replay::{self, Controller},
        wire::{Context, Message, Stream},
    };
    use std::sync::{Arc, Mutex};
    use tower::ServiceExt;
    let input = recording();
    let dir = input.with_extension("dir");
    std::fs::create_dir_all(&dir).unwrap();
    let saved = dir.join("test.pitrec");
    std::fs::copy(&input, &saved).unwrap();
    let recorder = Arc::new(Mutex::new(
        Recorder::start(RecorderConfig::new(dir.clone(), u64::MAX), || 0).unwrap(),
    ));
    recorder.lock().unwrap().sync().unwrap();
    let controller = Controller::new(dir.clone(), recorder.clone());
    let app =
        replay::routes(HttpShell::new(TrustedHosts::new([])), controller.clone()).into_router();
    async fn post(app: &axum::Router, route: &str, body: &str) -> StatusCode {
        let mut request = Request::post(format!("/api/replay/{route}"))
            .header("host", "localhost:8777")
            .header("origin", "http://localhost:8777")
            .header("content-type", "application/json")
            .body(Body::from(body.to_owned()))
            .unwrap();
        request.extensions_mut().insert(ConnectInfo(
            "127.0.0.1:1234".parse::<std::net::SocketAddr>().unwrap(),
        ));
        app.clone().oneshot(request).await.unwrap().status()
    }
    let stream = Stream::new("assumed");
    let mut live = SessionModel::default();
    let first = pitwall_collector::pitrec::Reader::new(File::open(&input).unwrap())
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    live.apply(
        Duration::ZERO,
        &pitwall_collector::telemetry::parse_datagram(&first.datagram).unwrap(),
    );
    fn snapshot(stream: &Stream) -> serde_json::Value {
        let screen = stream.subscribe();
        screen.pop();
        match screen.pop().unwrap() {
            Message::Text(v) => v,
            _ => panic!("expected snapshot"),
        }
    }
    assert_eq!(
        post(&app, "start", r#"{"name":"test.pitrec"}"#).await,
        StatusCode::ACCEPTED
    );
    let loaded = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            controller.publish(Duration::ZERO, &live, &stream, Context::default());
            let v = snapshot(&stream);
            if v["state"]["context"]["replay"]["loadProgress"] == 1.0 {
                break v;
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(loaded["source"], "replay");
    assert!(
        loaded["state"]["context"]["replay"]["backgroundLive"]
            .as_str()
            .unwrap()
            .contains("recording in the background")
    );
    recorder.lock().unwrap().set_size_cap(0);
    recorder.lock().unwrap().sync().unwrap();
    assert!(saved.exists(), "loaded recording survives retention");
    assert_eq!(
        post(&app, "seek", r#"{"position":31}"#).await,
        StatusCode::NO_CONTENT
    );
    controller.publish(Duration::from_secs(1), &live, &stream, Context::default());
    let sought = snapshot(&stream);
    assert!(sought["epoch"].as_u64() > loaded["epoch"].as_u64());
    assert_eq!(post(&app, "stop", "{}").await, StatusCode::NO_CONTENT);
    controller.publish(Duration::from_secs(2), &live, &stream, Context::default());
    let stopped = snapshot(&stream);
    assert_eq!(stopped["source"], "live");
    assert!(stopped["state"]["context"]["replay"].is_null());
    recorder.lock().unwrap().rescan();
    recorder.lock().unwrap().sync().unwrap();
    assert!(!saved.exists(), "stop releases retention guard");
    assert_eq!(
        std::fs::read_dir(&dir).unwrap().count(),
        0,
        "replay creates no recordings"
    );
    drop(controller);
    drop(app);
    drop(recorder);
    std::fs::remove_dir_all(dir).unwrap();
    std::fs::remove_file(input).unwrap();
}
