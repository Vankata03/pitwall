//! The livery pack routes (spec sections 8 and 13): `GET /api/liveries` and
//! `GET /liveries/{file}`.
//!
//! The pack lookup and its naming rules live in [`pitwall_core::liveries`]; this module
//! only serves them. Register the routes with [`routes`].

use std::path::PathBuf;

use axum::{
    Json,
    extract::{Path, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use serde_json::{Value, json};

pub use pitwall_core::liveries::*;

use crate::{
    http::{Access, HttpShell},
    paths::AppPaths,
};

/// Register the livery routes on the shell.
pub fn routes(shell: HttpShell, paths: &AppPaths) -> HttpShell {
    let folder = paths.liveries.clone();
    shell
        .route(
            "/api/liveries",
            get(list).with_state(folder.clone()),
            Access::Any,
        )
        .route(
            "/liveries/{file}",
            get(image).with_state(folder),
            Access::Any,
        )
}

/// `{"files": [...]}`: the pack image names in the folder, sorted.
async fn list(State(folder): State<PathBuf>) -> Json<Value> {
    Json(json!({ "files": pack_files(&folder) }))
}

async fn image(State(folder): State<PathBuf>, Path(file): Path<String>) -> Response {
    let Some(path) = pack_image(&folder, &file) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match std::fs::read(&path) {
        Ok(bytes) => (
            [
                (header::CONTENT_TYPE, "image/png"),
                // A replaced file must show without clearing the browser cache.
                (header::CACHE_CONTROL, "no-cache"),
            ],
            bytes,
        )
            .into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}
