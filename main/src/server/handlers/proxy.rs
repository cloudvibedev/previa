use axum::Json;
use axum::body::Body;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::response::Response;
use previa_runner::render_template_value_simple;
use reqwest::Method;
use reqwest::header::{CONTENT_LENGTH, CONTENT_TYPE, HeaderName, HeaderValue};
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tracing::warn;

use crate::server::errors::{
    bad_request_message_response, bad_request_response, internal_error_response,
};
use crate::server::execution::forward::parse_sse_block;
use crate::server::execution::sse_response_from_rx;
use crate::server::models::{ErrorResponse, ProxyRequest, SseMessage};
use crate::server::state::AppState;

#[utoipa::path(
    post,
    path = "/proxy",
    request_body = ProxyRequest,
    responses(
        (
            status = 200,
            description = "Resposta proxiada. Retorna SSE apenas quando o upstream também retorna SSE.",
            body = String
        ),
        (
            status = 400,
            description = "Payload inválido",
            body = ErrorResponse
        ),
        (
            status = 500,
            description = "Falha no request upstream",
            body = ErrorResponse
        )
    )
)]
pub async fn proxy_request(
    State(state): State<AppState>,
    payload: Result<Json<ProxyRequest>, JsonRejection>,
) -> Response {
    let Json(payload) = match payload {
        Ok(payload) => payload,
        Err(rejection) => return bad_request_response(rejection),
    };
    let payload = match render_proxy_payload(payload) {
        Ok(payload) => payload,
        Err(message) => return bad_request_message_response(&message),
    };

    forward_proxy_request(&state, payload).await
}

async fn forward_proxy_request(state: &AppState, payload: ProxyRequest) -> Response {
    let method = match Method::from_bytes(payload.method.trim().as_bytes()) {
        Ok(method) => method,
        Err(_) => {
            return bad_request_message_response(&format!("invalid method: {}", payload.method));
        }
    };

    let url = payload.url.trim();
    if url.is_empty() {
        return bad_request_message_response("url is required and cannot be empty");
    }

    if let Err(err) = reqwest::Url::parse(url) {
        return bad_request_message_response(&format!("invalid url: {}", err));
    }

    let mut request = state.client.request(method, url);

    for (name, value) in &payload.headers {
        let header_name = match HeaderName::from_bytes(name.as_bytes()) {
            Ok(header_name) => header_name,
            Err(_) => {
                return bad_request_message_response(&format!("invalid header name: {}", name));
            }
        };

        let header_value = match HeaderValue::from_str(value) {
            Ok(header_value) => header_value,
            Err(_) => {
                return bad_request_message_response(&format!(
                    "invalid header value for {}: {}",
                    name, value
                ));
            }
        };

        request = request.header(header_name, header_value);
    }

    if let Some(body) = payload.body {
        request = match body {
            Value::Null => request,
            Value::String(raw) => request.body(raw),
            value => request.json(&value),
        };
    }

    let response = match request.send().await {
        Ok(response) => response,
        Err(err) => return internal_error_response(format!("proxy request failed: {}", err)),
    };

    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();

    if content_type.contains("text/event-stream") {
        return stream_sse_response(response);
    }

    let status = response.status();
    let upstream_headers = response.headers().clone();
    let bytes = match response.bytes().await {
        Ok(bytes) => bytes,
        Err(err) => {
            return internal_error_response(format!(
                "failed to read upstream response body: {}",
                err
            ));
        }
    };

    let mut proxy_response = Response::builder().status(status);
    if let Some(headers) = proxy_response.headers_mut() {
        for (name, value) in &upstream_headers {
            if name == CONTENT_LENGTH {
                continue;
            }
            headers.append(name.clone(), value.clone());
        }
    }

    proxy_response
        .body(Body::from(bytes))
        .unwrap_or_else(|_| internal_error_response("failed to build proxy response".to_owned()))
}

fn stream_sse_response(response: reqwest::Response) -> Response {
    let (tx, rx) = mpsc::unbounded_channel::<SseMessage>();
    tokio::spawn(async move {
        forward_sse_response(&tx, response).await;
    });
    sse_response_from_rx(rx)
}

async fn forward_sse_response(tx: &mpsc::UnboundedSender<SseMessage>, response: reqwest::Response) {
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();

    while let Some(chunk_result) = stream.next().await {
        let chunk = match chunk_result {
            Ok(chunk) => chunk,
            Err(err) => {
                let _ = send_event(
                    tx,
                    "error",
                    Value::String(format!("failed to read upstream SSE stream: {}", err)),
                );
                return;
            }
        };

        let chunk_text = String::from_utf8_lossy(&chunk);
        buffer.push_str(&chunk_text.replace("\r\n", "\n"));

        while let Some(idx) = buffer.find("\n\n") {
            let block = buffer[..idx].to_owned();
            buffer = buffer[idx + 2..].to_owned();

            if let Some((event, data_text)) = parse_sse_block(&block) {
                let parsed = serde_json::from_str::<Value>(&data_text)
                    .unwrap_or_else(|_| Value::String(data_text.clone()));
                if !send_event(tx, &event, parsed) {
                    return;
                }
            }
        }
    }

    if !buffer.trim().is_empty()
        && let Some((event, data_text)) = parse_sse_block(&buffer)
    {
        let parsed =
            serde_json::from_str::<Value>(&data_text).unwrap_or_else(|_| Value::String(data_text));
        let _ = send_event(tx, &event, parsed);
    }
}

fn send_event(tx: &mpsc::UnboundedSender<SseMessage>, event: &str, data: Value) -> bool {
    if tx
        .send(SseMessage {
            event: event.to_owned(),
            data,
        })
        .is_err()
    {
        warn!("failed to send SSE event for proxy");
        false
    } else {
        true
    }
}

fn render_proxy_payload(payload: ProxyRequest) -> Result<ProxyRequest, String> {
    let value = serde_json::to_value(payload).map_err(|err| {
        format!(
            "failed to serialize proxy payload for template render: {}",
            err
        )
    })?;
    let rendered = render_template_value_simple(&value);
    serde_json::from_value(rendered)
        .map_err(|err| format!("failed to parse rendered proxy payload: {}", err))
}
