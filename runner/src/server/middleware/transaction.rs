use axum::http::header::{HeaderName, HeaderValue};
use axum::http::{HeaderMap, Request};
use axum::middleware::Next;
use axum::response::Response;
use uuid::Uuid;

use previa_runner::Pipeline;

pub const TRANSACTION_ID_HEADER: &str = "x-transaction-id";

pub async fn propagate_transaction_header(
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let transaction_id =
        extract_transaction_id(request.headers()).unwrap_or_else(|| Uuid::new_v4().to_string());

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

pub fn with_transaction_header(mut pipeline: Pipeline, transaction_id: Option<&str>) -> Pipeline {
    let Some(transaction_id) = transaction_id else {
        return pipeline;
    };

    for step in &mut pipeline.steps {
        step.headers
            .insert(TRANSACTION_ID_HEADER.to_owned(), transaction_id.to_owned());
    }

    pipeline
}
