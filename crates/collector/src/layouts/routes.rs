use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Layout, LayoutError, LayoutLibrary, ScreenClass};
use crate::http::{Access, HttpShell};

/// Register the `/api/layouts` routes (spec section 8) on the shell.
pub fn routes(shell: HttpShell, library: LayoutLibrary) -> HttpShell {
    shell
        .route(
            "/api/layouts",
            get(list).with_state(library.clone()),
            Access::Any,
        )
        .route(
            "/api/layouts/{name}",
            get(read)
                .put(save)
                .delete(delete)
                .with_state(library.clone()),
            Access::Any,
        )
        .route(
            "/api/layouts/{name}/duplicate",
            post(duplicate).with_state(library.clone()),
            Access::Any,
        )
        .route(
            "/api/layouts/{name}/rename",
            post(rename).with_state(library),
            Access::Any,
        )
}

/// A [`LayoutError`] as an HTTP response. The error type lives in the core crate, so the
/// response conversion needs a local wrapper.
struct ApiError(LayoutError);

impl From<LayoutError> for ApiError {
    fn from(error: LayoutError) -> Self {
        Self(error)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self.0 {
            LayoutError::NotFound => (StatusCode::NOT_FOUND, "no such layout".to_string()),
            LayoutError::NameTaken => (StatusCode::CONFLICT, "name already in use".to_string()),
            LayoutError::InvalidName => {
                (StatusCode::BAD_REQUEST, "invalid layout name".to_string())
            }
            LayoutError::InvalidLayout(message) => (StatusCode::BAD_REQUEST, message),
            LayoutError::ReadOnly => (StatusCode::FORBIDDEN, "presets are read-only".to_string()),
            LayoutError::Io(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
        };
        (status, Json(json!({"error": message}))).into_response()
    }
}

/// The layout as served: all three arrangements filled in, with `generated` naming the
/// classes derived from wide. A client saves it back unchanged and those classes stay
/// unarranged.
fn view(library: &LayoutLibrary, layout: &Layout) -> Value {
    let arrangements = &layout.arrangements;
    let mut generated = Vec::new();
    if arrangements.tablet.is_none() {
        generated.push("tablet");
    }
    if arrangements.phone.is_none() {
        generated.push("phone");
    }
    let mut value = serde_json::to_value(layout).unwrap();
    value["arrangements"]["tablet"] = json!(layout.arrangement(ScreenClass::Tablet));
    value["arrangements"]["phone"] = json!(layout.arrangement(ScreenClass::Phone));
    value["generated"] = json!(generated);
    value["preset"] = json!(library.is_preset(&layout.name));
    value
}

/// Parse a saved body: `preset` is ignored and classes named in `generated` are dropped.
fn parse(mut body: Value) -> Result<Layout, LayoutError> {
    let invalid = |error: serde_json::Error| LayoutError::InvalidLayout(error.to_string());
    let object = body
        .as_object_mut()
        .ok_or_else(|| LayoutError::InvalidLayout("layout must be an object".into()))?;
    object.remove("preset");
    let generated = object.remove("generated");
    let generated: Vec<String> = match generated {
        Some(list) => serde_json::from_value(list).map_err(invalid)?,
        None => Vec::new(),
    };
    if let Some(arrangements) = object
        .get_mut("arrangements")
        .and_then(Value::as_object_mut)
    {
        for class in generated.iter().filter(|class| *class != "wide") {
            arrangements.remove(class);
        }
    }
    serde_json::from_value(body).map_err(invalid)
}

async fn list(State(library): State<LayoutLibrary>) -> Json<Value> {
    let layouts: Vec<Value> = library
        .list()
        .iter()
        .map(|layout| json!({"name": layout.name, "preset": library.is_preset(&layout.name)}))
        .collect();
    Json(json!({"revision": library.revision(), "layouts": layouts}))
}

async fn read(
    State(library): State<LayoutLibrary>,
    Path(name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let layout = library.get(&name).ok_or(LayoutError::NotFound)?;
    Ok(Json(view(&library, &layout)))
}

async fn save(
    State(library): State<LayoutLibrary>,
    Path(name): Path<String>,
    Json(body): Json<Value>,
) -> Result<Response, ApiError> {
    let created = library.save(&name, parse(body)?)?;
    let saved = library.get(&name).ok_or(LayoutError::NotFound)?;
    let status = if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status, Json(view(&library, &saved))).into_response())
}

#[derive(Deserialize)]
struct NameBody {
    name: Option<String>,
}

async fn duplicate(
    State(library): State<LayoutLibrary>,
    Path(name): Path<String>,
    Json(body): Json<NameBody>,
) -> Result<Response, ApiError> {
    let copy = library.duplicate(&name, body.name.as_deref())?;
    Ok((StatusCode::CREATED, Json(view(&library, &copy))).into_response())
}

async fn rename(
    State(library): State<LayoutLibrary>,
    Path(name): Path<String>,
    Json(body): Json<NameBody>,
) -> Result<Json<Value>, ApiError> {
    let new_name = body.name.ok_or(LayoutError::InvalidName)?;
    let renamed = library.rename(&name, &new_name)?;
    Ok(Json(view(&library, &renamed)))
}

async fn delete(
    State(library): State<LayoutLibrary>,
    Path(name): Path<String>,
) -> Result<StatusCode, ApiError> {
    library.delete(&name)?;
    Ok(StatusCode::NO_CONTENT)
}
