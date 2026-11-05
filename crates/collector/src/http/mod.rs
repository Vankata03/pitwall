//! Embedded dashboard and HTTP route composition for the collector.
//!
//! Register every handler through [`HttpShell::route`] before consuming the shell.
//! Handler state can be supplied with Axum's `MethodRouter::with_state`.
//!
//! ```no_run
//! use axum::routing::{get, put};
//! use pitwall_collector::http::{Access, HttpListener, HttpShell, TrustedHosts};
//!
//! # async fn run() -> std::io::Result<()> {
//! let router = HttpShell::new(TrustedHosts::discover()?)
//!     .route("/api/layouts", get(|| async { "[]" }), Access::Any)
//!     .route("/api/settings", put(|| async { "{}" }), Access::Host)
//!     .into_router();
//! let mut http = HttpListener::bind("0.0.0.0:8777".parse().unwrap(), router).await?;
//! // A settings handler's owner applies a new HTTP bind address or port here.
//! http.restart("127.0.0.1:8778".parse().unwrap()).await?;
//! http.shutdown().await
//! # }
//! ```

mod connection;
mod listener;
mod trust;

pub use listener::HttpListener;
pub use trust::TrustedHosts;

use axum::{
    Router,
    body::Body,
    http::{StatusCode, Uri},
    middleware,
    response::{IntoResponse, Response},
    routing::MethodRouter,
};

use crate::assets::dashboard_asset;

/// Access classes from the HTTP routes table in the v1 specification.
pub enum Access {
    /// Open to every screen on the LAN; mutations still require trust checks.
    Any,
    /// Require the actual TCP peer to be loopback, including for reads.
    Host,
}

/// Compose later tickets' routes with embedded dashboard serving.
pub struct HttpShell {
    router: Router,
    hosts: TrustedHosts,
}

impl HttpShell {
    pub fn new(hosts: TrustedHosts) -> Self {
        Self {
            router: Router::new(),
            hosts,
        }
    }

    /// Add a ticket's handlers. Resolve handler state with `with_state` first.
    pub fn route(mut self, path: &str, routes: MethodRouter, access: Access) -> Self {
        let routes = match access {
            Access::Any => routes,
            Access::Host => routes.route_layer(middleware::from_fn(trust::check_loopback)),
        };
        self.router = self.router.route(path, routes);
        self
    }

    pub fn into_router(self) -> Router {
        // Apply trust checks to all handlers registered through the shell.
        self.router
            .layer(middleware::from_fn_with_state(
                self.hosts,
                trust::check_mutation,
            ))
            .fallback_service(axum::routing::get(embedded_asset))
    }
}

async fn embedded_asset(uri: Uri) -> Response {
    let path = match uri.path() {
        "/" | "/status" | "/overlay/throttle-brake" => "index.html",
        path => path.trim_start_matches('/'),
    };
    match dashboard_asset(path) {
        Some(bytes) => Response::builder()
            .header(
                "content-type",
                mime_guess::from_path(path).first_or_octet_stream().as_ref(),
            )
            .body(Body::from(bytes))
            .unwrap(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
