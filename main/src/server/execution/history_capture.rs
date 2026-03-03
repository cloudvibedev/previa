use std::sync::Arc;

use serde_json::Value;
use tokio::sync::Mutex;

use crate::server::models::{ConsolidatedLoadMetrics, E2eHistoryAccumulator};

pub fn determine_e2e_history_status(cancelled: bool, snapshot: &E2eHistoryAccumulator) -> String {
    if cancelled {
        return "cancelled".to_owned();
    }
    if !snapshot.errors.is_empty() {
        return "error".to_owned();
    }
    if snapshot
        .steps
        .iter()
        .any(|step| step.get("status").and_then(Value::as_str) == Some("error"))
    {
        return "error".to_owned();
    }
    if snapshot
        .summary
        .as_ref()
        .and_then(|summary| summary.get("failed"))
        .and_then(Value::as_u64)
        .unwrap_or(0)
        > 0
    {
        return "error".to_owned();
    }
    "success".to_owned()
}

pub fn determine_load_history_status(
    cancelled: bool,
    consolidated: Option<&ConsolidatedLoadMetrics>,
    no_errors_reported: bool,
) -> String {
    if cancelled {
        return "cancelled".to_owned();
    }
    if !no_errors_reported {
        return "error".to_owned();
    }
    if consolidated.is_some_and(|item| item.total_error > 0) {
        return "error".to_owned();
    }
    "success".to_owned()
}

pub async fn capture_e2e_history_event(
    accumulator: &Arc<Mutex<E2eHistoryAccumulator>>,
    event: &str,
    data: &Value,
) {
    let mut lock = accumulator.lock().await;
    match event {
        "step:result" => {
            let failed_assertions = extract_failed_assertions(data);
            let mut step_snapshot = data.clone();
            if !failed_assertions.is_empty() {
                if let Value::Object(map) = &mut step_snapshot {
                    map.insert(
                        "assertFailures".to_owned(),
                        Value::Array(failed_assertions.clone()),
                    );
                }
            }

            lock.steps.push(step_snapshot);
            if !failed_assertions.is_empty() {
                lock.errors
                    .push(format_assert_failure_message(data, &failed_assertions));
            } else if data.get("status").and_then(Value::as_str) == Some("error") {
                lock.errors.push(extract_error_message(data));
            }
        }
        "pipeline:complete" => {
            lock.summary = Some(data.clone());
        }
        "error" => {
            lock.errors.push(extract_error_message(data));
        }
        _ => {}
    }
}

pub async fn push_load_error(load_errors: &Arc<Mutex<Vec<String>>>, message: String) {
    let mut lock = load_errors.lock().await;
    lock.push(message);
}

pub fn extract_error_message(data: &Value) -> String {
    data.get("message")
        .and_then(Value::as_str)
        .or_else(|| data.get("error").and_then(Value::as_str))
        .map(str::to_owned)
        .unwrap_or_else(|| data.to_string())
}

pub fn extract_failed_assertions(data: &Value) -> Vec<Value> {
    data.get("assertResults")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter(|item| item.get("passed").and_then(Value::as_bool) == Some(false))
                .cloned()
                .collect::<Vec<Value>>()
        })
        .unwrap_or_default()
}

pub fn format_assert_failure_message(step_data: &Value, failed_assertions: &[Value]) -> String {
    let step_id = step_data
        .get("stepId")
        .and_then(Value::as_str)
        .unwrap_or("unknown_step");
    let details = failed_assertions
        .iter()
        .filter_map(|item| {
            let assertion = item.get("assertion")?;
            let field = assertion
                .get("field")
                .and_then(Value::as_str)
                .unwrap_or("field");
            let operator = assertion
                .get("operator")
                .and_then(Value::as_str)
                .unwrap_or("operator");
            let expected = assertion
                .get("expected")
                .map(|value| value.to_string())
                .unwrap_or_else(|| "null".to_owned());
            let actual = item
                .get("actual")
                .map(|value| value.to_string())
                .unwrap_or_else(|| "null".to_owned());
            Some(format!(
                "{} {} expected={} actual={}",
                field, operator, expected, actual
            ))
        })
        .collect::<Vec<String>>()
        .join("; ");

    if details.is_empty() {
        format!(
            "step {} failed {} assertion(s)",
            step_id,
            failed_assertions.len()
        )
    } else {
        format!(
            "step {} failed {} assertion(s): {}",
            step_id,
            failed_assertions.len(),
            details
        )
    }
}
