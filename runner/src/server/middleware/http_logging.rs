use std::collections::HashMap;

use axum::http::Request;
use axum::middleware::Next;
use axum::response::Response;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tracing::debug;

pub async fn log_http_io(mut request: Request<axum::body::Body>, next: Next) -> Response {
    let method = request.method().to_string();
    let path = request
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str().to_owned())
        .unwrap_or_else(|| request.uri().path().to_owned());
    let request_headers = headers_to_map(request.headers());

    let request_body_bytes = request
        .body_mut()
        .collect()
        .await
        .map(|collected| collected.to_bytes())
        .unwrap_or_default();
    let request_body = bytes_to_log_body(&request_body_bytes);
    *request.body_mut() = axum::body::Body::from(request_body_bytes.clone());

    debug!(
        "{}",
        json!({
            "type": "request",
            "method": method,
            "path": path,
            "headers": request_headers,
            "body": request_body
        })
    );

    let response = next.run(request).await;
    let status_code = response.status().as_u16();
    let response_headers = headers_to_map(response.headers());
    let content_type = response_headers
        .get("content-type")
        .cloned()
        .unwrap_or_default()
        .to_ascii_lowercase();

    if content_type.contains("text/event-stream") {
        debug!(
            "{}",
            json!({
                "type": "response",
                "status_code": status_code,
                "headers": response_headers,
                "body": "[stream omitted]"
            })
        );
        return response;
    }

    let (parts, body) = response.into_parts();
    let body_bytes = body
        .collect()
        .await
        .map(|collected| collected.to_bytes())
        .unwrap_or_default();
    let response_body = bytes_to_log_body(&body_bytes);

    debug!(
        "{}",
        json!({
            "type": "response",
            "status_code": status_code,
            "headers": response_headers,
            "body": response_body
        })
    );

    Response::from_parts(parts, axum::body::Body::from(body_bytes))
}

fn headers_to_map(headers: &axum::http::HeaderMap) -> HashMap<String, String> {
    headers
        .iter()
        .map(|(k, v)| {
            let value = if k.as_str().eq_ignore_ascii_case("authorization") {
                "<redacted>".to_owned()
            } else {
                v.to_str().unwrap_or("<non-utf8>").to_owned()
            };
            (k.as_str().to_owned(), value)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, HeaderValue};

    use super::headers_to_map;

    #[test]
    fn redacts_authorization_header() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", HeaderValue::from_static("secret"));
        let map = headers_to_map(&headers);
        assert_eq!(
            map.get("authorization").map(String::as_str),
            Some("<redacted>")
        );
    }
}

fn bytes_to_log_body(bytes: &axum::body::Bytes) -> Value {
    if bytes.is_empty() {
        return Value::Null;
    }
    Value::String(String::from_utf8_lossy(bytes).to_string())
}
