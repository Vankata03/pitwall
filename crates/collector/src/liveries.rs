//! The livery pack routes (spec sections 8 and 13): `GET /api/liveries` and
//! `GET /liveries/{file}`.
//!
//! The pack is a folder of images on the host, [`AppPaths::liveries`]. Nothing from it is
//! ever bundled; it is read from disk on each request, so copying a file in needs no
//! restart. Register the routes with [`routes`].
//!
//! A pack image is named `my-team.png` or `<teamId>.png`. Only those names are listed or
//! served; any other file in the folder is ignored, and no other name can reach a file,
//! which also rules out path traversal.

use std::path::PathBuf;

use axum::{
    Json,
    extract::{Path, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use serde_json::{Value, json};

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

/// Whether `name` is a pack image name: `my-team.png` or `<teamId>.png`, where the team
/// ID is plain decimal digits.
fn is_pack_name(name: &str) -> bool {
    match name.strip_suffix(".png") {
        Some("my-team") => true,
        Some(id) => !id.is_empty() && id.len() <= 5 && id.bytes().all(|byte| byte.is_ascii_digit()),
        None => false,
    }
}

/// `{"files": [...]}`: the pack image names in the folder, sorted. A missing or unreadable
/// folder is an empty pack.
async fn list(State(folder): State<PathBuf>) -> Json<Value> {
    let mut files: Vec<String> = std::fs::read_dir(&folder)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.path().is_file())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| is_pack_name(name))
        .collect();
    files.sort();
    Json(json!({ "files": files }))
}

async fn image(State(folder): State<PathBuf>, Path(file): Path<String>) -> Response {
    if !is_pack_name(&file) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let path = folder.join(&file);
    if !path.is_file() {
        return StatusCode::NOT_FOUND.into_response();
    }
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
