use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;
use serde_json::{Map, Value, json};
use tokio::sync::{Mutex, broadcast};
use tokio_stream::StreamExt;
use tokio_util::sync::CancellationToken;

use crate::server::execution::history_capture::capture_e2e_history_event;
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

    let mut request = client
        .post(url)
        .header("Content-Type", "application/json")
        .header("Accept", "text/event-stream");

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
    use serde_json::json;

    use crate::server::execution::forward::add_context_fields;
    use crate::server::models::NodePlan;

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
}
