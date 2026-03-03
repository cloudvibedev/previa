use axum::http::header::{HeaderName, HeaderValue};
use axum::http::{HeaderMap, Request};
use axum::middleware::Next;
use axum::response::Response;

use crate::server::state::TRANSACTION_ID_HEADER;
use crate::server::utils::new_uuid_v7;

pub async fn propagate_transaction_header(
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let transaction_id = extract_transaction_id(request.headers()).unwrap_or_else(new_uuid_v7);

    let mut request = request;
    if let Ok(header_value) = HeaderValue::from_str(&transaction_id) {
        request
            .headers_mut()
            .insert(HeaderName::from_static(TRANSACTION_ID_HEADER), header_value);
    }

    let mut response = next.run(request).await;
    if let Ok(header_value) = HeaderValue::from_str(&transaction_id) {
        response
            .headers_mut()
            .insert(HeaderName::from_static(TRANSACTION_ID_HEADER), header_value);
    }

    response
}

pub fn extract_transaction_id(headers: &HeaderMap) -> Option<String> {
    headers
        .get(TRANSACTION_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}
