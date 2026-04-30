use axum::Json;
use axum::body::Body;
use axum::http::{Method, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use include_dir::{Dir, File, include_dir};

use crate::server::AppConfig;
use crate::server::models::ErrorResponse;

static APP_DIST: Dir<'static> = include_dir!("$OUT_DIR/app-dist");

pub async fn app_fallback(method: Method, uri: Uri, config: AppConfig) -> Response {
    let path = uri.path();

    if is_api_path(path) {
        return api_not_found_response();
    }

    if !config.enabled || !matches!(method, Method::GET | Method::HEAD) {
        return StatusCode::NOT_FOUND.into_response();
    }

    if is_reserved_path(path, config.mcp_path.as_deref()) {
        return StatusCode::NOT_FOUND.into_response();
    }

    if let Some(file) = asset_for_path(path) {
        return file_response(file, method == Method::HEAD);
    }

    if looks_like_file_path(path) {
        return StatusCode::NOT_FOUND.into_response();
    }

    index_response(method == Method::HEAD)
}

fn asset_for_path(path: &str) -> Option<&'static File<'static>> {
    let asset_path = path.trim_start_matches('/');
    if asset_path.is_empty() || asset_path == "index" {
        return APP_DIST.get_file("index.html");
    }
    if asset_path.contains("..") {
        return None;
    }
    APP_DIST.get_file(asset_path)
}

fn index_response(head: bool) -> Response {
    match APP_DIST.get_file("index.html") {
        Some(file) => file_response(file, head),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

fn file_response(file: &File<'static>, head: bool) -> Response {
    let content_type = content_type_for(file.path().to_string_lossy().as_ref());
    let body = if head {
        Body::empty()
    } else {
        Body::from(file.contents().to_vec())
    };

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .body(body)
        .expect("static app response")
}

fn content_type_for(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or_default() {
        "css" => "text/css; charset=utf-8",
        "html" => "text/html; charset=utf-8",
        "ico" => "image/x-icon",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "map" => "application/json; charset=utf-8",
        "png" => "image/png",
        "svg" => "image/svg+xml",
        "txt" => "text/plain; charset=utf-8",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}

fn api_not_found_response() -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse {
            error: "not_found".to_owned(),
            message: "api route not found".to_owned(),
        }),
    )
        .into_response()
}

fn is_api_path(path: &str) -> bool {
    path == "/api" || path.starts_with("/api/")
}

fn is_reserved_path(path: &str, mcp_path: Option<&str>) -> bool {
    matches!(
        path,
        "/health" | "/info" | "/openapi.json" | "/proxy" | "/mcp"
    ) || path.starts_with("/health/")
        || path.starts_with("/info/")
        || path.starts_with("/proxy/")
        || path.starts_with("/mcp/")
        || mcp_path.is_some_and(|mcp_path| {
            let mcp_path = normalize_reserved_path(mcp_path);
            path == mcp_path || path.starts_with(&format!("{mcp_path}/"))
        })
}

fn normalize_reserved_path(path: &str) -> &str {
    if path.is_empty() { "/mcp" } else { path }
}

fn looks_like_file_path(path: &str) -> bool {
    path.rsplit('/')
        .next()
        .is_some_and(|segment| segment.contains('.'))
}
