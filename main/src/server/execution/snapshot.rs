use serde_json::{Map, Value, json};

use crate::server::models::{
    ConsolidatedLoadMetrics, E2eHistoryAccumulator, LoadEventContext, RunnerLoadLine,
};

pub fn build_e2e_snapshot_payload(
    execution_id: &str,
    status: &str,
    accumulator: &E2eHistoryAccumulator,
) -> Value {
    json!({
        "executionId": execution_id,
        "status": status,
        "kind": "e2e",
        "steps": accumulator.steps,
        "summary": accumulator.summary,
        "errors": accumulator.errors,
    })
}

pub fn build_load_snapshot_payload(
    execution_id: &str,
    status: &str,
    context: Value,
    lines: Vec<Value>,
    consolidated: Option<Value>,
    errors: Vec<String>,
) -> Value {
    json!({
        "executionId": execution_id,
        "status": status,
        "kind": "load",
        "context": context,
        "lines": lines,
        "consolidated": consolidated,
        "errors": errors,
    })
}

pub fn build_live_load_snapshot_payload(
    execution_id: &str,
    status: &str,
    context: &LoadEventContext,
    lines: &[RunnerLoadLine],
    consolidated: Option<&ConsolidatedLoadMetrics>,
    errors: &[String],
) -> Value {
    build_load_snapshot_payload(
        execution_id,
        status,
        load_context_snapshot_value(context),
        lines
            .iter()
            .map(|line| serde_json::to_value(line).unwrap_or(Value::Null))
            .collect(),
        consolidated.and_then(|value| serde_json::to_value(value).ok()),
        errors.to_vec(),
    )
}

pub fn extract_load_context_value(payload: &Value) -> Value {
    let mut context = match payload.clone() {
        Value::Object(map) => map,
        _ => Map::new(),
    };
    for field in [
        "executionId",
        "status",
        "message",
        "queuePosition",
        "lines",
        "consolidated",
        "errors",
        "kind",
    ] {
        context.remove(field);
    }
    Value::Object(context)
}

pub fn load_context_snapshot_value(context: &LoadEventContext) -> Value {
    json!({
        "requestedNodes": context.plan.requested_nodes,
        "nodesFound": context.plan.nodes_found,
        "nodesUsed": context.plan.nodes_used,
        "warning": context.warning,
        "registeredNodesTotal": context.registered_nodes.len(),
        "activeNodesTotal": context.active_nodes.len(),
        "usedNodesTotal": context.used_nodes.len(),
        "registeredNodes": context.registered_nodes,
        "activeNodes": context.active_nodes,
        "usedNodes": context.used_nodes,
        "runnerLoadPlan": context.runner_load_plan,
        "batchWindowMs": context.batch_window_ms,
    })
}
