use std::{
    path::PathBuf,
    sync::atomic::{AtomicU32, Ordering},
};

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode},
};
use pitwall_collector::{
    http::{HttpShell, TrustedHosts},
    layouts::{self, LayoutLibrary},
};
use serde_json::{Value, json};
use tower::ServiceExt;

fn temp_file() -> PathBuf {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "pitwall-layouts-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("layouts.json")
}

fn app(library: &LayoutLibrary) -> Router {
    layouts::routes(HttpShell::new(TrustedHosts::new([])), library.clone()).into_router()
}

async fn call(app: &Router, method: Method, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method.clone())
        .uri(uri)
        .header("host", "localhost:8777")
        .header("origin", "http://localhost:8777");
    if method != Method::GET {
        request = request.header("content-type", "application/json");
    }
    let body = body.map_or_else(Body::empty, |body| Body::from(body.to_string()));
    let response = app
        .clone()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 10_000_000).await.unwrap();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

fn names(list: &Value) -> Vec<&str> {
    list["layouts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|layout| layout["name"].as_str().unwrap())
        .collect()
}

fn tiles<'a>(layout: &'a Value, class: &str) -> &'a Vec<Value> {
    layout["arrangements"][class].as_array().unwrap()
}

fn tile<'a>(layout: &'a Value, class: &str, id: &str) -> &'a Value {
    tiles(layout, class)
        .iter()
        .find(|tile| tile["i"] == id)
        .unwrap_or_else(|| panic!("no tile {id} in {class}"))
}

fn rect(tile: &Value) -> [u64; 4] {
    ["x", "y", "w", "h"].map(|key| tile[key].as_u64().unwrap())
}

#[tokio::test]
async fn fresh_library_lists_the_three_read_only_presets() {
    let library = LayoutLibrary::open(temp_file()).unwrap();
    let app = app(&library);

    let (status, list) = call(&app, Method::GET, "/api/layouts", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(names(&list), ["Pitwall Classic", "Driver", "Race"]);
    for entry in list["layouts"].as_array().unwrap() {
        assert_eq!(entry["preset"], true);
    }
}

#[tokio::test]
async fn pitwall_classic_wide_arrangement_matches_the_spec_table() {
    let library = LayoutLibrary::open(temp_file()).unwrap();
    let app = app(&library);

    let (status, layout) = call(&app, Method::GET, "/api/layouts/Pitwall%20Classic", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(layout["name"], "Pitwall Classic");
    assert_eq!(layout["preset"], true);

    let expected = [
        ("speed-rpm-gear", [0, 0, 6, 6]),
        ("engine-temp", [0, 6, 3, 2]),
        ("g-force", [3, 6, 3, 2]),
        ("ers", [0, 8, 6, 4]),
        ("fuel", [0, 12, 6, 2]),
        ("gaps", [6, 0, 8, 2]),
        ("sm", [14, 0, 4, 2]),
        ("car-status", [6, 2, 12, 12]),
        ("weather", [18, 0, 6, 2]),
        ("track-map", [18, 2, 6, 6]),
        ("lap-state", [18, 8, 6, 2]),
        ("lap-timing", [18, 10, 6, 4]),
        ("throttle-brake-traces", [0, 14, 10, 7]),
        ("stint", [10, 14, 8, 7]),
        ("timing-tower", [18, 14, 6, 7]),
        ("notice", [0, 21, 24, 1]),
    ];
    assert_eq!(tiles(&layout, "wide").len(), expected.len());
    for (id, at) in expected {
        let tile = tile(&layout, "wide", id);
        assert_eq!(tile["type"], id);
        assert_eq!(rect(tile), at, "{id}");
    }
}

#[tokio::test]
async fn unknown_layout_is_404() {
    let library = LayoutLibrary::open(temp_file()).unwrap();
    let (status, _) = call(&app(&library), Method::GET, "/api/layouts/Nope", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

fn overlaps(a: [u64; 4], b: [u64; 4]) -> bool {
    a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3]
}

fn assert_valid_grid(layout: &Value, class: &str, columns: u64) {
    let all = tiles(layout, class);
    for (n, a) in all.iter().enumerate() {
        let [x, _, w, h] = rect(a);
        assert!(w >= 1 && h >= 1 && x + w <= columns, "{class} {a}");
        for b in &all[n + 1..] {
            assert!(!overlaps(rect(a), rect(b)), "{class}: {a} overlaps {b}");
        }
    }
}

fn type_set(layout: &Value, class: &str) -> Vec<String> {
    let mut set: Vec<String> = tiles(layout, class)
        .iter()
        .map(|tile| tile["type"].as_str().unwrap().to_string())
        .collect();
    set.sort();
    set
}

#[tokio::test]
async fn every_preset_has_valid_wide_tablet_and_phone_arrangements_with_a_notice() {
    // Minimum sizes from the section 11 catalogue, in wide-grid columns and rows.
    let minimum = |kind: &str| match kind {
        "speed-rpm-gear" | "lap-timing" => (4, 3),
        "gaps" | "ers" | "track-discipline" | "lap-state" | "weather" => (4, 2),
        "notice" => (4, 1),
        "engine-temp" | "g-force" | "sm" => (2, 2),
        "fuel" => (3, 2),
        "throttle-brake-traces" => (6, 4),
        "track-map" => (4, 4),
        "race-control-feed" => (4, 3),
        "car-status" => (8, 6),
        "timing-tower" => (4, 6),
        "stint" => (6, 4),
        other => panic!("unexpected tile type {other}"),
    };

    let library = LayoutLibrary::open(temp_file()).unwrap();
    let app = app(&library);
    for name in ["Pitwall%20Classic", "Driver", "Race"] {
        let (_, layout) = call(&app, Method::GET, &format!("/api/layouts/{name}"), None).await;
        for (class, columns) in [("wide", 24), ("tablet", 12), ("phone", 4)] {
            assert_valid_grid(&layout, class, columns);
            assert_eq!(
                type_set(&layout, class),
                type_set(&layout, "wide"),
                "{name} {class}"
            );
            assert!(type_set(&layout, class).contains(&"notice".to_string()));
        }
        for class in ["wide", "tablet"] {
            for tile in tiles(&layout, class) {
                let (w, h) = minimum(tile["type"].as_str().unwrap());
                assert!(tile["w"].as_u64().unwrap() >= w, "{name} {class} {tile}");
                assert!(tile["h"].as_u64().unwrap() >= h, "{name} {class} {tile}");
            }
        }
        assert_eq!(layout["generated"], json!([]));
    }
}

#[tokio::test]
async fn phone_arrangements_are_one_column_with_lap_timing_and_gaps_first() {
    let library = LayoutLibrary::open(temp_file()).unwrap();
    let app = app(&library);
    for name in ["Pitwall%20Classic", "Driver"] {
        let (_, layout) = call(&app, Method::GET, &format!("/api/layouts/{name}"), None).await;
        let mut phone: Vec<&Value> = tiles(&layout, "phone").iter().collect();
        phone.sort_by_key(|tile| tile["y"].as_u64().unwrap());
        assert_eq!(phone[0]["type"], "lap-timing");
        assert_eq!(phone[1]["type"], "gaps");
    }
}

fn custom(name: &str) -> Value {
    json!({
        "schemaVersion": 1,
        "name": name,
        "arrangements": {"wide": [
            {"i": "a", "type": "fuel", "x": 0, "y": 0, "w": 6, "h": 2, "settings": {"unit": "kg"}},
            {"i": "b", "type": "gaps", "x": 6, "y": 0, "w": 8, "h": 2, "settings": {}},
            {"i": "c", "type": "notice", "x": 0, "y": 2, "w": 24, "h": 1, "settings": {}},
        ]},
    })
}

#[tokio::test]
async fn saving_creates_then_replaces_and_survives_a_restart() {
    let file = temp_file();
    let library = LayoutLibrary::open(file.clone()).unwrap();
    let app = app(&library);

    let (status, _) = call(&app, Method::PUT, "/api/layouts/Mine", Some(custom("Mine"))).await;
    assert_eq!(status, StatusCode::CREATED);
    let mut replaced = custom("Mine");
    replaced["arrangements"]["wide"]
        .as_array_mut()
        .unwrap()
        .pop();
    let (status, _) = call(&app, Method::PUT, "/api/layouts/Mine", Some(replaced)).await;
    assert_eq!(status, StatusCode::OK);

    let reopened = LayoutLibrary::open(file).unwrap();
    let (_, list) = call(&self::app(&reopened), Method::GET, "/api/layouts", None).await;
    assert_eq!(names(&list), ["Pitwall Classic", "Driver", "Race", "Mine"]);
    let (_, mine) = call(
        &self::app(&reopened),
        Method::GET,
        "/api/layouts/Mine",
        None,
    )
    .await;
    assert_eq!(tiles(&mine, "wide").len(), 2);
    assert_eq!(mine["preset"], false);
}

#[tokio::test]
async fn revision_advances_on_every_change_and_only_then() {
    let library = LayoutLibrary::open(temp_file()).unwrap();
    let app = app(&library);
    let revision = |list: &Value| list["revision"].as_u64().unwrap();
    let (_, list) = call(&app, Method::GET, "/api/layouts", None).await;
    let start = revision(&list);

    call(&app, Method::PUT, "/api/layouts/Mine", Some(custom("Mine"))).await;
    call(
        &app,
        Method::POST,
        "/api/layouts/Mine/duplicate",
        Some(json!({"name": "Two"})),
    )
    .await;
    call(
        &app,
        Method::POST,
        "/api/layouts/Two/rename",
        Some(json!({"name": "Three"})),
    )
    .await;
    call(&app, Method::DELETE, "/api/layouts/Three", None).await;
    let (_, list) = call(&app, Method::GET, "/api/layouts", None).await;
    assert_eq!(revision(&list), start + 4);
    assert_eq!(library.revision(), start + 4);

    call(&app, Method::DELETE, "/api/layouts/Driver", None).await;
    call(&app, Method::GET, "/api/layouts/Mine", None).await;
    assert_eq!(library.revision(), start + 4);
}

#[tokio::test]
async fn presets_refuse_save_rename_and_delete_but_can_be_duplicated() {
    let library = LayoutLibrary::open(temp_file()).unwrap();
    let app = app(&library);

    let (status, _) = call(&app, Method::PUT, "/api/layouts/Race", Some(custom("Race"))).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = call(
        &app,
        Method::POST,
        "/api/layouts/Race/rename",
        Some(json!({"name": "X"})),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = call(&app, Method::DELETE, "/api/layouts/Race", None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, copy) = call(
        &app,
        Method::POST,
        "/api/layouts/Race/duplicate",
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(copy["name"], "Race copy");
    assert_eq!(copy["preset"], false);
    let (_, original) = call(&app, Method::GET, "/api/layouts/Race", None).await;
    assert_eq!(copy["arrangements"], original["arrangements"]);
}

#[tokio::test]
async fn names_stay_unique_and_valid() {
    let library = LayoutLibrary::open(temp_file()).unwrap();
    let app = app(&library);
    call(&app, Method::PUT, "/api/layouts/Mine", Some(custom("Mine"))).await;

    let (status, _) = call(
        &app,
        Method::POST,
        "/api/layouts/Mine/duplicate",
        Some(json!({"name": "mine"})),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, _) = call(
        &app,
        Method::POST,
        "/api/layouts/Mine/rename",
        Some(json!({"name": "Race"})),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, _) = call(
        &app,
        Method::PUT,
        "/api/layouts/Pitwall%20classic",
        Some(custom("Pitwall classic")),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = call(
        &app,
        Method::POST,
        "/api/layouts/Mine/rename",
        Some(json!({"name": "  "})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = call(
        &app,
        Method::PUT,
        "/api/layouts/Other",
        Some(custom("Mine")),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = call(
        &app,
        Method::POST,
        "/api/layouts/Nope/rename",
        Some(json!({"name": "Z"})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(&app, Method::DELETE, "/api/layouts/Nope", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, renamed) = call(
        &app,
        Method::POST,
        "/api/layouts/Mine/rename",
        Some(json!({"name": "Mine2"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(renamed["name"], "Mine2");
    let (status, _) = call(&app, Method::GET, "/api/layouts/Mine", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(&app, Method::DELETE, "/api/layouts/Mine2", None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn unknown_tile_types_settings_and_fields_are_kept_through_save_and_restart() {
    let file = temp_file();
    let library = LayoutLibrary::open(file.clone()).unwrap();
    let app = app(&library);
    let mut body = custom("Future");
    body["schemaVersion"] = json!(7);
    body["futureField"] = json!({"keep": [1, 2]});
    body["arrangements"]["wide"][0] = json!({
        "i": "z", "type": "hologram", "x": 0, "y": 0, "w": 4, "h": 4,
        "settings": {"mode": "spooky", "nested": {"n": 1}}, "tileExtra": true,
    });
    call(&app, Method::PUT, "/api/layouts/Future", Some(body.clone())).await;

    let reopened = LayoutLibrary::open(file).unwrap();
    let (_, saved) = call(
        &self::app(&reopened),
        Method::GET,
        "/api/layouts/Future",
        None,
    )
    .await;
    assert_eq!(saved["schemaVersion"], 7);
    assert_eq!(saved["futureField"], body["futureField"]);
    assert_eq!(
        saved["arrangements"]["wide"][0],
        body["arrangements"]["wide"][0]
    );
}

#[tokio::test]
async fn unarranged_classes_are_generated_from_wide_until_edited() {
    let library = LayoutLibrary::open(temp_file()).unwrap();
    let app = app(&library);
    call(&app, Method::PUT, "/api/layouts/Mine", Some(custom("Mine"))).await;

    let (_, mine) = call(&app, Method::GET, "/api/layouts/Mine", None).await;
    assert_eq!(mine["generated"], json!(["tablet", "phone"]));
    // Tablet: wide reading order in 12 columns, sizes kept; gaps (8) cannot sit beside fuel
    // (6), so it wraps to a new row.
    let tablet: Vec<[u64; 4]> = tiles(&mine, "tablet").iter().map(rect).collect();
    assert_eq!(tablet, [[0, 0, 6, 2], [0, 2, 8, 2], [0, 4, 12, 1]]);
    // Phone: one column, widths clamped to 4, stacked.
    let phone: Vec<[u64; 4]> = tiles(&mine, "phone").iter().map(rect).collect();
    assert_eq!(phone, [[0, 0, 4, 2], [0, 2, 4, 2], [0, 4, 4, 1]]);
    assert_eq!(tiles(&mine, "tablet")[0]["settings"], json!({"unit": "kg"}));

    // Saving what was read keeps generated classes unarranged, so they follow wide edits.
    let mut edited = mine.clone();
    edited["arrangements"]["wide"][0]["w"] = json!(10);
    call(&app, Method::PUT, "/api/layouts/Mine", Some(edited)).await;
    let (_, mine) = call(&app, Method::GET, "/api/layouts/Mine", None).await;
    assert_eq!(mine["generated"], json!(["tablet", "phone"]));
    assert_eq!(rect(&tiles(&mine, "tablet")[0]), [0, 0, 10, 2]);

    // Arranging tablet makes it the layout's own.
    let mut arranged = mine.clone();
    arranged["generated"] = json!(["phone"]);
    arranged["arrangements"]["tablet"][0]["w"] = json!(12);
    call(&app, Method::PUT, "/api/layouts/Mine", Some(arranged)).await;
    let (_, mine) = call(&app, Method::GET, "/api/layouts/Mine", None).await;
    assert_eq!(mine["generated"], json!(["phone"]));
    assert_eq!(rect(&tiles(&mine, "tablet")[0]), [0, 0, 12, 2]);
}

#[tokio::test]
async fn malformed_layouts_are_rejected() {
    let library = LayoutLibrary::open(temp_file()).unwrap();
    let app = app(&library);
    let mut dup = custom("Mine");
    dup["arrangements"]["wide"][1]["i"] = json!("a");
    let (status, _) = call(&app, Method::PUT, "/api/layouts/Mine", Some(dup)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let mut empty = custom("Mine");
    empty["arrangements"]["wide"][0]["w"] = json!(0);
    let (status, _) = call(&app, Method::PUT, "/api/layouts/Mine", Some(empty)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(library.revision(), 0);
}

#[tokio::test]
async fn mutations_without_the_lan_trust_headers_are_refused() {
    let library = LayoutLibrary::open(temp_file()).unwrap();
    let response = app(&library)
        .oneshot(
            Request::builder()
                .method(Method::DELETE)
                .uri("/api/layouts/Mine")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
}
