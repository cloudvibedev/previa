use std::sync::Arc;

use rand::seq::SliceRandom;
use serde_json::json;
use tokio::sync::{Mutex, broadcast, mpsc, oneshot};
use tokio_util::sync::CancellationToken;
use tracing::error;

use crate::server::db::{save_e2e_history, upsert_e2e_history};
use crate::server::execution::{
    add_context_fields, collect_active_nodes, determine_e2e_history_status, forward_runner_stream,
    resolve_runtime_specs_for_execution, send_sse_best_effort, spawn_broadcast_bridge,
};
use crate::server::models::{
    E2eHistoryAccumulator, E2eHistoryWrite, E2eTestRequest, HistoryMetadata, NodePlan, SseMessage,
};
use crate::server::state::{AppState, EXECUTION_SSE_BUFFER_SIZE, ExecutionCtx, ExecutionKind};
use crate::server::utils::{new_uuid_v7, now_ms};
use crate::server::validation::pipelines::validate_pipeline_templates;

#[derive(Debug)]
pub enum StartE2eExecutionError {
    BadRequest(String),
    ServiceUnavailable(String),
    Internal(String),
}

#[derive(Debug)]
pub struct E2eExecutionOutcome {
    pub execution_id: String,
    pub status: String,
}

pub struct StartedE2eExecution {
    pub execution_id: String,
    pub subscriber: broadcast::Receiver<SseMessage>,
    pub completion: oneshot::Receiver<E2eExecutionOutcome>,
}

pub async fn start_e2e_execution(
    state: AppState,
    payload: E2eTestRequest,
    transaction_id: Option<String>,
) -> Result<StartedE2eExecution, StartE2eExecutionError> {
    if payload.pipeline.steps.is_empty() {
        return Err(StartE2eExecutionError::BadRequest(
            "pipeline must contain at least one step".to_owned(),
        ));
    }

    if state.runner_endpoints.is_empty() {
        return Err(StartE2eExecutionError::ServiceUnavailable(
            "RUNNER_ENDPOINTS not configured".to_owned(),
        ));
    }

    let mut randomized = state.runner_endpoints.clone();
    randomized.shuffle(&mut rand::rng());
    let active_nodes = collect_active_nodes(&state.client, &randomized).await;
    if active_nodes.is_empty() {
        return Err(StartE2eExecutionError::ServiceUnavailable(
            "No active runners found via /health".to_owned(),
        ));
    }

    let selected_node = active_nodes[0].clone();
    let selected_runners = vec![selected_node.clone()];
    let started_at_ms = now_ms() as i64;
    let history_metadata = HistoryMetadata {
        project_id: payload.project_id.clone(),
        pipeline_index: payload.pipeline_index,
    };
    let runtime_specs = resolve_runtime_specs_for_execution(
        &state.db,
        payload.project_id.as_deref(),
        &payload.specs,
    )
    .await
    .map_err(|err| {
        StartE2eExecutionError::Internal(format!(
            "failed to load project specs for execution: {err}"
        ))
    })?;

    let template_errors = validate_pipeline_templates(&payload.pipeline, runtime_specs.as_deref());
    if !template_errors.is_empty() {
        return Err(StartE2eExecutionError::BadRequest(
            template_errors.join("; "),
        ));
    }

    let pipeline_for_runner = payload.pipeline.clone();
    let pipeline_id = payload.pipeline.id.clone();
    let pipeline_name = payload.pipeline.name.clone();
    let selected_base_url_key = payload.selected_base_url_key.clone();
    let selected_base_url_key_for_runner = payload.selected_base_url_key.clone();
    let history_request = json!({
        "pipeline": payload.pipeline,
        "selectedBaseUrlKey": payload.selected_base_url_key,
        "specs": runtime_specs.clone(),
        "projectId": payload.project_id,
        "pipelineIndex": payload.pipeline_index
    });
    let plan = NodePlan {
        requested_nodes: 1,
        nodes_found: active_nodes.len(),
        nodes_used: 1,
        warning: None,
    };
    let Some(project_id_for_execution) = payload.project_id.clone() else {
        return Err(StartE2eExecutionError::BadRequest(
            "projectId is required".to_owned(),
        ));
    };

    let orchestrator_execution_id = new_uuid_v7();
    let init_payload = add_context_fields(
        json!({ "executionId": orchestrator_execution_id }),
        &selected_runners,
        &plan,
    );
    let (sse_tx, _) = broadcast::channel(EXECUTION_SSE_BUFFER_SIZE);
    let response_subscriber = sse_tx.subscribe();
    let exec_ctx = Arc::new(ExecutionCtx {
        cancel: CancellationToken::new(),
        project_id: project_id_for_execution,
        kind: ExecutionKind::E2e,
        sse_tx: sse_tx.clone(),
        init_payload: init_payload.clone(),
    });

    {
        let mut executions = state.executions.write().await;
        executions.insert(orchestrator_execution_id.clone(), Arc::clone(&exec_ctx));
    }

    let history_record_id = new_uuid_v7();
    let runtime_specs_for_runner = runtime_specs.clone().unwrap_or_default();
    let transaction_id_for_runner = transaction_id.clone();
    save_e2e_history(
        &state.db,
        E2eHistoryWrite {
            id: history_record_id.clone(),
            execution_id: orchestrator_execution_id.clone(),
            transaction_id: transaction_id.clone(),
            metadata: history_metadata.clone(),
            pipeline_id: pipeline_id.clone(),
            pipeline_name: pipeline_name.clone(),
            selected_base_url_key: selected_base_url_key.clone(),
            status: "running".to_owned(),
            started_at_ms,
            finished_at_ms: started_at_ms,
            duration_ms: 0,
            summary: None,
            steps: Vec::new(),
            errors: Vec::new(),
            request: history_request.clone(),
        },
    )
    .await
    .map_err(|err| {
        StartE2eExecutionError::Internal(format!("failed to save e2e running history: {err}"))
    })?;

    let state_clone = state.clone();
    let execution_id_for_cleanup = orchestrator_execution_id.clone();
    let history_execution_id = orchestrator_execution_id.clone();
    let (completion_tx, completion_rx) = oneshot::channel();

    tokio::spawn(async move {
        let history_accumulator = Arc::new(Mutex::new(E2eHistoryAccumulator::default()));
        let _ = send_sse_best_effort(&sse_tx, "execution:init", init_payload);

        let request_body = json!({
            "pipeline": pipeline_for_runner,
            "selectedBaseUrlKey": selected_base_url_key_for_runner,
            "specs": runtime_specs_for_runner
        });

        forward_runner_stream(
            &state_clone.client,
            selected_node,
            request_body,
            sse_tx,
            exec_ctx.cancel.clone(),
            plan,
            "/api/v1/tests/e2e",
            transaction_id_for_runner,
            Some(Arc::clone(&history_accumulator)),
        )
        .await;

        let finished_at_ms = now_ms() as i64;
        let duration_ms = finished_at_ms.saturating_sub(started_at_ms);
        let snapshot = history_accumulator.lock().await.clone();
        let status = determine_e2e_history_status(exec_ctx.cancel.is_cancelled(), &snapshot);

        if let Err(err) = upsert_e2e_history(
            &state_clone.db,
            E2eHistoryWrite {
                id: history_record_id,
                execution_id: history_execution_id.clone(),
                transaction_id: transaction_id.clone(),
                metadata: history_metadata,
                pipeline_id,
                pipeline_name,
                selected_base_url_key,
                status: status.clone(),
                started_at_ms,
                finished_at_ms,
                duration_ms,
                summary: snapshot.summary,
                steps: snapshot.steps,
                errors: snapshot.errors,
                request: history_request,
            },
        )
        .await
        {
            error!("failed to save e2e history: {}", err);
        }

        let mut executions = state_clone.executions.write().await;
        executions.remove(&execution_id_for_cleanup);
        let _ = completion_tx.send(E2eExecutionOutcome {
            execution_id: history_execution_id,
            status,
        });
    });

    Ok(StartedE2eExecution {
        execution_id: orchestrator_execution_id,
        subscriber: response_subscriber,
        completion: completion_rx,
    })
}

pub fn sse_response_for_started_execution(
    started: StartedE2eExecution,
) -> axum::response::Response {
    let (tx, rx) = mpsc::unbounded_channel();
    spawn_broadcast_bridge(started.subscriber, tx, false);
    crate::server::execution::sse_response_from_rx(rx)
}
