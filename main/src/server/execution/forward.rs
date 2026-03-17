use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;
use serde_json::{Map, Value, json};
use tokio::sync::{Mutex, broadcast};
use tokio_stream::StreamExt;
use tokio_util::sync::CancellationToken;

use crate::server::execution::history_capture::capture_e2e_history_event;
use crate::server::execution::runner_auth::apply_runner_auth;
use crate::server::execution::scheduler::SharedValue;
use crate::server::execution::snapshot::build_e2e_snapshot_payload;
use crate::server::models::{E2eHistoryAccumulator, NodePlan, SseMessage};
use crate::server::state::TRANSACTION_ID_HEADER;

pub async fn forward_runner_stream(
    client: &Client,
    node: String,
    body: Value,
    tx: broadcast::Sender<SseMessage>,
    cancel: CancellationToken,
    plan: NodePlan,
    endpoint_path: &str,
    transaction_id: Option<String>,
    runner_auth_key: Option<&str>,
    history_accumulator: Option<(
        String,
        Arc<Mutex<E2eHistoryAccumulator>>,
        SharedValue<Value>,
    )>,
) {
    if cancel.is_cancelled() {
        return;
    }

    let runner_list = vec![node.clone()];
    let url = format!("{}{}", node.trim_end_matches('/'), endpoint_path);

    let mut request = apply_runner_auth(
        client
            .post(url)
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream"),
        runner_auth_key,
    );

    if let Some(transaction_id) = transaction_id.as_deref() {
        request = request.header(TRANSACTION_ID_HEADER, transaction_id);
    }

    let response =
        match tokio::time::timeout(Duration::from_secs(10), request.json(&body).send()).await {
            Ok(Ok(response)) => response,
            Ok(Err(err)) => {
                let payload = add_context_fields(
                    json!({ "message": format!("runner request failed: {}", err) }),
                    &runner_list,
                    &plan,
                );
                let _ = send_sse_best_effort(&tx, "error", payload);
                return;
            }
            Err(_) => {
                let payload = add_context_fields(
                    json!({ "message": "runner request timeout" }),
                    &runner_list,
                    &plan,
                );
                let _ = send_sse_best_effort(&tx, "error", payload);
                return;
            }
        };

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let body_text = response.text().await.unwrap_or_default();
        let payload = add_context_fields(
            json!({ "message": format!("runner returned HTTP {}: {}", status, body_text) }),
            &runner_list,
            &plan,
        );
        let _ = send_sse_best_effort(&tx, "error", payload);
        return;
    }

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();

    loop {
        let next_chunk = tokio::select! {
            _ = cancel.cancelled() => {
                return;
            }
            chunk = stream.next() => chunk,
        };

        let Some(chunk_result) = next_chunk else {
            break;
        };

        let chunk = match chunk_result {
            Ok(chunk) => chunk,
            Err(err) => {
                let payload = add_context_fields(
                    json!({ "message": format!("runner stream read error: {}", err) }),
                    &runner_list,
                    &plan,
                );
                let _ = send_sse_best_effort(&tx, "error", payload);
                return;
            }
        };

        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(idx) = buffer.find("\n\n") {
            let block = buffer[..idx].to_owned();
            buffer = buffer[idx + 2..].to_owned();

            if let Some((event, data_text)) = parse_sse_block(&block) {
                let mut data = serde_json::from_str::<Value>(&data_text)
                    .unwrap_or_else(|_| Value::String(data_text.clone()));

                if let Some((execution_id, acc, snapshot_payload)) = history_accumulator.as_ref() {
                    capture_e2e_history_event(acc, &event, &data).await;
                    let snapshot = acc.lock().await.clone();
                    snapshot_payload
                        .set(build_e2e_snapshot_payload(
                            execution_id,
                            "running",
                            &snapshot,
                        ))
                        .await;
                }
                data = add_context_fields(data, &runner_list, &plan);
                let _ = send_sse_best_effort(&tx, event, data);
            }
        }
    }
}

pub fn parse_sse_block(block: &str) -> Option<(String, String)> {
    let mut event = "message".to_owned();
    let mut data = String::new();

    for line in block.lines() {
        if let Some(rest) = line.strip_prefix("event: ") {
            event = rest.trim().to_owned();
            continue;
        }

        if let Some(rest) = line.strip_prefix("data: ") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(rest);
            continue;
        }

        if let Some(rest) = line.strip_prefix("data:") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(rest);
        }
    }

    if data.is_empty() {
        None
    } else {
        Some((event, data))
    }
}

pub fn send_sse_best_effort(
    tx: &broadcast::Sender<SseMessage>,
    event: impl Into<String>,
    data: Value,
) -> bool {
    let _ = tx.send(SseMessage {
        event: event.into(),
        data,
    });
    true
}

pub fn add_context_fields(data: Value, runners: &[String], plan: &NodePlan) -> Value {
    let mut object = match data {
        Value::Object(obj) => obj,
        other => {
            let mut obj = Map::new();
            obj.insert("payload".to_owned(), other);
            obj
        }
    };

    object.insert("requestedNodes".to_owned(), json!(plan.requested_nodes));
    object.insert("nodesFound".to_owned(), json!(plan.nodes_found));
    object.insert("nodesUsed".to_owned(), json!(plan.nodes_used));
    object.insert("runners".to_owned(), json!(runners));

    if let Some(warning) = &plan.warning {
        object.insert("warning".to_owned(), json!(warning));
    }

    Value::Object(object)
}

#[cfg(test)]
mod tests {
    use axum::extract::State;
    use axum::http::{HeaderMap, StatusCode};
    use axum::routing::post;
    use axum::{Router, response::IntoResponse};
    use serde_json::json;
    use tokio::net::TcpListener;
    use tokio::sync::broadcast;
    use tokio_util::sync::CancellationToken;

    use crate::server::execution::forward::{add_context_fields, forward_runner_stream};
    use crate::server::models::{NodePlan, SseMessage};

    #[test]
    fn context_fields_include_warning_and_runners() {
        let plan = NodePlan {
            requested_nodes: 10,
            nodes_found: 2,
            nodes_used: 2,
            warning: Some("warn".to_owned()),
        };

        let data = add_context_fields(
            json!({"x": 1}),
            &[String::from("http://runner:3000")],
            &plan,
        );
        assert_eq!(data["nodesFound"], json!(2));
        assert_eq!(data["nodesUsed"], json!(2));
        assert_eq!(data["runners"], json!(["http://runner:3000"]));
        assert_eq!(data["warning"], json!("warn"));
    }

    #[tokio::test]
    async fn forwards_execution_requests_with_authorization_header() {
        let endpoint = spawn_sse_runner(Some("secret")).await;
        let (tx, mut rx) = broadcast::channel::<SseMessage>(8);

        forward_runner_stream(
            &reqwest::Client::new(),
            endpoint,
            json!({"pipeline": {"name": "ignored", "steps": []}}),
            tx,
            CancellationToken::new(),
            NodePlan {
                requested_nodes: 1,
                nodes_found: 1,
                nodes_used: 1,
                warning: None,
            },
            "/api/v1/tests/e2e",
            None,
            Some("secret"),
            None,
        )
        .await;

        let message = rx.recv().await.expect("sse message");
        assert_eq!(message.event, "step:result");
        assert_eq!(message.data["ok"], json!(true));
    }

    async fn spawn_sse_runner(expected_auth: Option<&str>) -> String {
        let app = Router::new()
            .route("/api/v1/tests/e2e", post(sse_handler))
            .with_state(expected_auth.map(str::to_owned));

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let address = listener.local_addr().expect("local addr");
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve sse runner");
        });

        format!("http://{}", address)
    }

    async fn sse_handler(
        State(expected_auth): State<Option<String>>,
        headers: HeaderMap,
    ) -> impl IntoResponse {
        if let Some(expected) = expected_auth.as_deref() {
            let provided = headers
                .get("authorization")
                .and_then(|value| value.to_str().ok())
                .map(str::trim);
            if provided != Some(expected) {
                return (
                    StatusCode::UNAUTHORIZED,
                    "missing or invalid authorization".to_owned(),
                )
                    .into_response();
            }
        }

        (
            StatusCode::OK,
            [("content-type", "text/event-stream")],
            "event: step:result\ndata: {\"ok\":true}\n\n",
        )
            .into_response()
    }
}
