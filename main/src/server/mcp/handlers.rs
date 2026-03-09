use axum::Json;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};

use crate::server::mcp::models::McpRequest;
use crate::server::mcp::service::{McpHttpOutcome, delete_session, process_request};
use crate::server::state::AppState;

const MCP_SESSION_HEADER: &str = "mcp-session-id";
const MCP_PROTOCOL_HEADER: &str = "mcp-protocol-version";

pub async fn preflight() -> StatusCode {
    StatusCode::NO_CONTENT
}

pub async fn handle_http(
    State(state): State<AppState>,
    headers: HeaderMap,
    payload: Result<Json<McpRequest>, JsonRejection>,
) -> Response {
    let session_id = header_value(&headers, MCP_SESSION_HEADER);
    let protocol_version = header_value(&headers, MCP_PROTOCOL_HEADER);

    let Json(request) = match payload {
        Ok(payload) => payload,
        Err(err) => {
            return with_headers(
                StatusCode::BAD_REQUEST,
                Json(crate::server::mcp::models::McpResponse::error(
                    None,
                    -32700,
                    format!("invalid MCP payload: {err}"),
                ))
                .into_response(),
                session_id,
                protocol_version,
            );
        }
    };

    match process_request(&state, session_id, protocol_version, request).await {
        McpHttpOutcome::Accepted => StatusCode::ACCEPTED.into_response(),
        McpHttpOutcome::Response {
            response,
            session_id,
            protocol_version,
        } => with_headers(
            StatusCode::OK,
            Json(response).into_response(),
            session_id.as_deref(),
            protocol_version.as_deref(),
        ),
    }
}

pub async fn delete_http_session(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let removed = delete_session(&state, header_value(&headers, MCP_SESSION_HEADER)).await;
    if removed {
        StatusCode::NO_CONTENT.into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

fn header_value<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|value| value.to_str().ok())
}

fn with_headers(
    status: StatusCode,
    mut response: Response,
    session_id: Option<&str>,
    protocol_version: Option<&str>,
) -> Response {
    *response.status_mut() = status;
    if let Some(session_id) = session_id.and_then(|value| HeaderValue::from_str(value).ok()) {
        response
            .headers_mut()
            .insert(MCP_SESSION_HEADER, session_id);
    }
    if let Some(protocol_version) =
        protocol_version.and_then(|value| HeaderValue::from_str(value).ok())
    {
        response
            .headers_mut()
            .insert(MCP_PROTOCOL_HEADER, protocol_version);
    }
    response
}
