use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde_json::{Value, json};

use super::{Source, TrackError, TrackKey, TrackStore};
use crate::http::{Access, HttpShell};

/// Register the `/api/tracks` routes (spec section 8) on the shell.
pub fn routes(shell: HttpShell, store: TrackStore) -> HttpShell {
    shell
        .route(
            "/api/tracks/{track_id}/{track_length}",
            get(read).with_state(store.clone()),
            Access::Any,
        )
        .route(
            "/api/tracks/{track_id}/{track_length}/relearn",
            post(relearn).with_state(store),
            Access::Host,
        )
}

impl IntoResponse for TrackError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            TrackError::NotFound => (StatusCode::NOT_FOUND, "no such track map".to_string()),
            TrackError::Io(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
        };
        (status, Json(json!({"error": message}))).into_response()
    }
}

/// The map as served: outline points are `[bucket, x, z]`, bucket `n` at lap distance
/// `n * 8` m; `pit` is a list of `[x, z]` or null. `revision` is the store's counter, so
/// a client refetches when it moves.
async fn read(
    State(store): State<TrackStore>,
    Path((track_id, track_length)): Path<(i8, u16)>,
) -> Result<Json<Value>, TrackError> {
    let key = TrackKey::new(track_id, track_length);
    let revision = store.revision();
    let map = store.get(key).ok_or(TrackError::NotFound)?;
    Ok(Json(json!({
        "trackId": track_id,
        "trackLength": track_length,
        "source": match map.source {
            Source::Shipped => "shipped",
            Source::Cached => "cached",
            Source::Learned => "learned",
        },
        "complete": map.complete,
        "revision": revision,
        "outline": map.outline,
        "pit": map.pit,
    })))
}

async fn relearn(
    State(store): State<TrackStore>,
    Path((track_id, track_length)): Path<(i8, u16)>,
) -> Result<StatusCode, TrackError> {
    store.relearn(TrackKey::new(track_id, track_length))?;
    Ok(StatusCode::OK)
}
