use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::server::models::ErrorResponse;

pub fn bad_request_response(rejection: JsonRejection) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: "bad_request".to_owned(),
            message: rejection.to_string(),
        }),
    )
        .into_response()
}

pub fn bad_request_message_response(message: &str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: "bad_request".to_owned(),
            message: message.to_owned(),
        }),
    )
        .into_response()
}
