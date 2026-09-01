use axum::{
    body::Body,
    http,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

pub(crate) fn build_internal_control_error_response(
    status: http::StatusCode,
    message: impl Into<String>,
) -> Response<Body> {
    (status, Json(json!({ "detail": message.into() }))).into_response()
}
