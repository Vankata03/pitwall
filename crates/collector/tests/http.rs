use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use pitwall_collector::http::{Access, HttpShell, TrustedHosts};
use tower::ServiceExt;

#[tokio::test]
async fn dashboard_and_status_page_serve_the_embedded_entry_point() {
    let router = HttpShell::new(TrustedHosts::new([])).into_router();
    for path in ["/", "/status", "/?layout=Pitwall%20Classic&stream=1"] {
        let response = router
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["content-type"], "text/html");
        let body = to_bytes(response.into_body(), 1_000_000).await.unwrap();
        assert!(
            std::str::from_utf8(&body)
                .unwrap()
                .contains("<title>pitwall</title>")
        );
    }
}

#[tokio::test]
async fn embedded_scripts_have_javascript_type_and_unknown_routes_do_not_become_html() {
    let router = HttpShell::new(TrustedHosts::new([])).into_router();
    let html =
        std::str::from_utf8(pitwall_collector::assets::dashboard_asset("index.html").unwrap())
            .unwrap();
    let script = html
        .split("src=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    let response = router
        .clone()
        .oneshot(Request::builder().uri(script).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "text/javascript");
    for path in [
        "/missing.js",
        "/api/missing",
        "/assets/../index.html",
        "/%2e%2e/Cargo.toml",
    ] {
        assert_eq!(
            router
                .clone()
                .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
                .await
                .unwrap()
                .status(),
            StatusCode::NOT_FOUND
        );
    }
    assert_eq!(
        router
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/status")
                    .body(Body::empty())
                    .unwrap()
            )
            .await
            .unwrap()
            .status(),
        StatusCode::METHOD_NOT_ALLOWED
    );
}

fn mutation(host: Option<&str>, origin: Option<&str>, content_type: Option<&str>) -> Request<Body> {
    let mut request = Request::builder().method("PUT").uri("/api/layouts/test");
    for (name, value) in [
        ("host", host),
        ("origin", origin),
        ("content-type", content_type),
    ] {
        if let Some(value) = value {
            request = request.header(name, value);
        }
    }
    request.body(Body::from("{}")).unwrap()
}

#[tokio::test]
async fn lan_layout_mutations_require_json_same_origin_and_a_known_host_address() {
    let router = HttpShell::new(TrustedHosts::new(["192.168.1.20".parse().unwrap()]))
        .route(
            "/api/layouts/test",
            axum::routing::put(|| async { StatusCode::NO_CONTENT }),
            Access::Any,
        )
        .into_router();
    let host = Some("192.168.1.20:8777");
    let origin = Some("http://192.168.1.20:8777");
    let json = Some("application/json");
    assert_eq!(
        router
            .clone()
            .oneshot(mutation(host, origin, json))
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    for (host, origin, content_type, expected) in [
        (host, origin, None, StatusCode::UNSUPPORTED_MEDIA_TYPE),
        (
            host,
            origin,
            Some("text/plain"),
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
        ),
        (host, None, json, StatusCode::FORBIDDEN),
        (None, origin, json, StatusCode::FORBIDDEN),
        (host, Some("null"), json, StatusCode::FORBIDDEN),
        (
            host,
            Some("http://evil.test:8777"),
            json,
            StatusCode::FORBIDDEN,
        ),
        (
            host,
            Some("https://192.168.1.20:8777"),
            json,
            StatusCode::FORBIDDEN,
        ),
        (
            host,
            Some("http://192.168.1.20:8778"),
            json,
            StatusCode::FORBIDDEN,
        ),
        (
            Some("evil.test:8777"),
            Some("http://evil.test:8777"),
            json,
            StatusCode::FORBIDDEN,
        ),
        (
            Some("192.168.1.21:8777"),
            Some("http://192.168.1.21:8777"),
            json,
            StatusCode::FORBIDDEN,
        ),
    ] {
        assert_eq!(
            router
                .clone()
                .oneshot(mutation(host, origin, content_type))
                .await
                .unwrap()
                .status(),
            expected,
            "host={host:?}, origin={origin:?}, content-type={content_type:?}"
        );
    }
}

#[tokio::test]
async fn host_routes_require_a_real_loopback_peer_even_for_reads() {
    let router = HttpShell::new(TrustedHosts::new([]))
        .route(
            "/api/settings",
            axum::routing::get(|| async { StatusCode::NO_CONTENT })
                .put(|| async { StatusCode::NO_CONTENT }),
            Access::Host,
        )
        .route(
            "/api/recordings",
            axum::routing::get(|| async { StatusCode::NO_CONTENT }),
            Access::Any,
        )
        .into_router();
    for (peer, expected) in [
        (Some("127.0.0.1:1234"), StatusCode::NO_CONTENT),
        (Some("[::1]:1234"), StatusCode::NO_CONTENT),
        (Some("[::ffff:127.0.0.1]:1234"), StatusCode::NO_CONTENT),
        (Some("192.168.1.30:1234"), StatusCode::FORBIDDEN),
        (None, StatusCode::FORBIDDEN),
    ] {
        for method in ["GET", "PUT"] {
            let mut request = Request::builder()
                .method(method)
                .uri("/api/settings")
                .header("host", "localhost:8777")
                .header("origin", "http://localhost:8777")
                .header("content-type", "application/json")
                .header("x-forwarded-for", "127.0.0.1")
                .header("forwarded", "for=127.0.0.1")
                .body(Body::from("{}"))
                .unwrap();
            if let Some(peer) = peer {
                request.extensions_mut().insert(axum::extract::ConnectInfo(
                    peer.parse::<std::net::SocketAddr>().unwrap(),
                ));
            }
            assert_eq!(
                router.clone().oneshot(request).await.unwrap().status(),
                expected,
                "peer={peer:?}, method={method}"
            );
        }
    }
    assert_eq!(
        router
            .oneshot(
                Request::builder()
                    .uri("/api/recordings")
                    .body(Body::empty())
                    .unwrap()
            )
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
}

#[tokio::test]
async fn trust_checks_cover_all_mutation_methods_and_reject_ambiguous_authorities() {
    let router = HttpShell::new(TrustedHosts::new([
        "2001:db8::20".parse().unwrap(),
        "0.0.0.0".parse().unwrap(),
    ]))
    .route(
        "/api/layouts/test",
        axum::routing::any(|| async { StatusCode::NO_CONTENT }),
        Access::Any,
    )
    .into_router();
    for method in ["POST", "PUT", "PATCH", "DELETE"] {
        let mut request = mutation(Some("localhost:8777"), None, Some("application/json"));
        *request.method_mut() = method.parse().unwrap();
        assert_eq!(
            router.clone().oneshot(request).await.unwrap().status(),
            StatusCode::FORBIDDEN
        );
    }
    for (host, origin) in [
        ("LOCALHOST", "http://localhost:80"),
        ("pitwall.local:8777", "http://pitwall.local:8777"),
        ("127.0.0.1:8777", "http://127.0.0.1:8777"),
        ("[::1]:8777", "http://[::1]:8777"),
        ("[2001:db8::20]:8777", "http://[2001:db8::20]:8777"),
    ] {
        assert_eq!(
            router
                .clone()
                .oneshot(mutation(
                    Some(host),
                    Some(origin),
                    Some("application/json; charset=utf-8")
                ))
                .await
                .unwrap()
                .status(),
            StatusCode::NO_CONTENT,
            "host={host}, origin={origin}"
        );
    }
    for (host, origin) in [
        ("localhost:8777", "http://user@localhost:8777"),
        ("user@localhost:8777", "http://localhost:8777"),
        ("localhost:8777", "http://localhost:8777/evil"),
        ("localhost:8777", "http://localhost:8777,http://evil.test"),
        ("localhost:bad", "http://localhost:bad"),
        ("localhost:99999", "http://localhost:99999"),
        ("0.0.0.0:8777", "http://0.0.0.0:8777"),
    ] {
        assert_eq!(
            router
                .clone()
                .oneshot(mutation(Some(host), Some(origin), Some("application/json")))
                .await
                .unwrap()
                .status(),
            StatusCode::FORBIDDEN,
            "host={host}, origin={origin}"
        );
    }
    for name in ["host", "origin", "content-type"] {
        let mut request = mutation(
            Some("localhost:8777"),
            Some("http://localhost:8777"),
            Some("application/json"),
        );
        let value = request.headers()[name].clone();
        request.headers_mut().append(
            axum::http::HeaderName::from_bytes(name.as_bytes()).unwrap(),
            value,
        );
        assert!(
            !router
                .clone()
                .oneshot(request)
                .await
                .unwrap()
                .status()
                .is_success(),
            "duplicate {name}"
        );
    }
}
