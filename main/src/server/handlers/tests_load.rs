use std::collections::HashMap;
use std::sync::Arc;

use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::Response;
use serde_json::{Value, json};
use tokio::sync::{Mutex, broadcast, mpsc};
use tokio_util::sync::CancellationToken;
use tracing::error;

use crate::server::db::{save_load_history, upsert_load_history};
use crate::server::errors::{
    bad_request_message_response, bad_request_response, internal_error_response,
    service_unavailable_response,
};
use crate::server::execution::{
    add_load_context_fields, calculate_node_plan, determine_load_history_status,
    flush_load_batches, forward_runner_stream_load_chunked, resolve_runtime_specs_for_execution,
    send_sse_best_effort, snapshot_consolidated_metrics, snapshot_latest_lines,
    spawn_broadcast_bridge, split_even, sse_response_from_rx,
};
use crate::server::middleware::transaction::extract_transaction_id;
use crate::server::models::{
    ErrorResponse, HistoryMetadata, LoadEventContext, LoadHistoryWrite, LoadLatencyAccumulator,
    LoadTestRequest, OrchestratorSseEventData, ProjectLoadTestRequest, RunnerLoadLine,
    RunnerLoadPlanItem,
};
use crate::server::state::{
    AppState, EXECUTION_SSE_BUFFER_SIZE, ExecutionCtx, ExecutionKind, LOAD_BATCH_WINDOW_MS,
};
use crate::server::utils::{new_uuid_v7, now_ms};
use crate::server::validation::pipelines::validate_pipeline_templates;

pub async fn run_load_test_internal(
    State(state): State<AppState>,
    headers: HeaderMap,
    payload: Result<Json<LoadTestRequest>, JsonRejection>,
) -> Response {
    let Json(payload) = match payload {
        Ok(payload) => payload,
        Err(rejection) => return bad_request_response(rejection),
    };
    if payload.pipeline.steps.is_empty() {
        return bad_request_message_response("pipeline must contain at least one step");
    }

    if state.runner_endpoints.is_empty() {
        return service_unavailable_response("RUNNER_ENDPOINTS not configured");
    }

    let runner_statuses =
        crate::server::execution::collect_runner_statuses(&state.client, &state.runner_endpoints)
            .await;
    let registered_nodes: Vec<String> = runner_statuses
        .iter()
        .map(|runner| runner.endpoint.clone())
        .collect();
    let active_nodes: Vec<String> = runner_statuses
        .into_iter()
        .filter(|runner| runner.active)
        .map(|runner| runner.endpoint)
        .collect();
    if active_nodes.is_empty() {
        return service_unavailable_response("No active runners found via /health");
    }

    let target_rps = (payload.config.concurrency as u64).max(1);

    let plan = calculate_node_plan(
        target_rps,
        state.rps_per_node,
        active_nodes.len(),
        payload.config.total_requests.max(1),
        payload.config.concurrency.max(1),
    );

    let selected_nodes: Vec<String> = active_nodes.iter().take(plan.nodes_used).cloned().collect();
    if selected_nodes.is_empty() {
        return service_unavailable_response("No runner selected for execution");
    }
    let transaction_id = extract_transaction_id(&headers);
    let transaction_id_for_children = transaction_id.clone();
    let started_at_ms = now_ms() as i64;
    let history_metadata = HistoryMetadata {
        project_id: payload.project_id.clone(),
        pipeline_index: payload.pipeline_index,
    };
    let runtime_specs = match resolve_runtime_specs_for_execution(
        &state.db,
        payload.project_id.as_deref(),
        &payload.specs,
    )
    .await
    {
        Ok(specs) => specs,
        Err(err) => {
            return internal_error_response(format!(
                "failed to load project specs for execution: {err}"
            ));
        }
    };
    let template_errors = validate_pipeline_templates(&payload.pipeline, runtime_specs.as_deref());
    if !template_errors.is_empty() {
        return bad_request_message_response(&template_errors.join("; "));
    }
    let runner_pipeline = payload.pipeline.clone();
    let runner_selected_base_url_key = payload.selected_base_url_key.clone();
    let runner_config = payload.config.clone();
    let runner_ramp_up_seconds = runner_config.ramp_up_seconds;
    let history_pipeline_id = payload.pipeline.id.clone();
    let history_pipeline_name = payload.pipeline.name.clone();
    let history_selected_base_url_key = payload.selected_base_url_key.clone();
    let history_request = json!({
        "pipeline": runner_pipeline.clone(),
        "selectedBaseUrlKey": runner_selected_base_url_key.clone(),
        "specs": runtime_specs.clone(),
        "config": runner_config.clone(),
        "projectId": history_metadata.project_id.clone(),
        "pipelineIndex": history_metadata.pipeline_index
    });

    let split_requests = split_even(runner_config.total_requests.max(1), plan.nodes_used);
    let split_concurrency = split_even(runner_config.concurrency.max(1), plan.nodes_used);
    let desired_total_requests = runner_config
        .total_requests
        .max(1)
        .div_ceil(plan.requested_nodes.max(1));
    let runner_load_plan = selected_nodes
        .iter()
        .enumerate()
        .map(|(index, node)| {
            let total_requests = split_requests[index];
            let concurrency = split_concurrency[index];
            RunnerLoadPlanItem {
                node: node.clone(),
                total_requests,
                concurrency,
                desired_total_requests,
                above_desired: total_requests > desired_total_requests,
            }
        })
        .collect::<Vec<_>>();
    let overloaded_nodes = runner_load_plan
        .iter()
        .filter(|item| item.above_desired)
        .map(|item| item.node.clone())
        .collect::<Vec<_>>();
    let overloaded_warning = (!overloaded_nodes.is_empty()).then(|| {
        format!(
            "Configured load above desired per-runner totalRequests (desired <= {}): {}.",
            desired_total_requests,
            overloaded_nodes.join(", ")
        )
    });
    let warning = match (plan.warning.clone(), overloaded_warning) {
        (Some(plan_warning), Some(overloaded)) => Some(format!("{plan_warning} {overloaded}")),
        (Some(plan_warning), None) => Some(plan_warning),
        (None, Some(overloaded)) => Some(overloaded),
        (None, None) => None,
    };
    let Some(project_id_for_execution) = payload.project_id.clone() else {
        return bad_request_message_response("projectId is required");
    };
    let load_context = Arc::new(LoadEventContext {
        plan: plan.clone(),
        warning,
        registered_nodes,
        active_nodes: active_nodes.clone(),
        used_nodes: selected_nodes.clone(),
        runner_load_plan,
        batch_window_ms: LOAD_BATCH_WINDOW_MS,
    });

    let orchestrator_execution_id = new_uuid_v7();
    let init_payload = add_load_context_fields(
        json!({ "executionId": orchestrator_execution_id }),
        load_context.as_ref(),
    );
    let (sse_tx, _) = broadcast::channel(EXECUTION_SSE_BUFFER_SIZE);
    let response_subscriber = sse_tx.subscribe();
    let exec_ctx = Arc::new(ExecutionCtx {
        cancel: CancellationToken::new(),
        project_id: project_id_for_execution,
        kind: ExecutionKind::Load,
        sse_tx: sse_tx.clone(),
        init_payload: init_payload.clone(),
    });

    {
        let mut executions = state.executions.write().await;
        executions.insert(orchestrator_execution_id.clone(), Arc::clone(&exec_ctx));
    }

    let (tx, rx) = mpsc::unbounded_channel();
    spawn_broadcast_bridge(response_subscriber, tx, false);
    let state_clone = state.clone();
    let execution_id_for_cleanup = orchestrator_execution_id.clone();
    let history_execution_id = orchestrator_execution_id.clone();
    let history_record_id = new_uuid_v7();
    let runtime_specs_for_runner = runtime_specs.clone().unwrap_or_default();
    let running_context_payload = add_load_context_fields(json!({}), load_context.as_ref());
    let running_requested_config = serde_json::to_value(&runner_config).unwrap_or(Value::Null);
    if let Err(err) = save_load_history(
        &state.db,
        LoadHistoryWrite {
            id: history_record_id.clone(),
            execution_id: history_execution_id.clone(),
            transaction_id: transaction_id.clone(),
            metadata: history_metadata.clone(),
            pipeline_id: history_pipeline_id.clone(),
            pipeline_name: history_pipeline_name.clone(),
            selected_base_url_key: history_selected_base_url_key.clone(),
            status: "running".to_owned(),
            started_at_ms,
            finished_at_ms: started_at_ms,
            duration_ms: 0,
            requested_config: running_requested_config,
            final_consolidated: None,
            final_lines: Vec::new(),
            errors: Vec::new(),
            request: history_request.clone(),
            context: running_context_payload,
        },
    )
    .await
    {
        error!("failed to save load running history: {}", err);
    }
    let load_chunk: Arc<Mutex<HashMap<String, RunnerLoadLine>>> =
        Arc::new(Mutex::new(HashMap::new()));
    let load_latest: Arc<Mutex<HashMap<String, RunnerLoadLine>>> =
        Arc::new(Mutex::new(HashMap::new()));
    let load_latency: Arc<Mutex<LoadLatencyAccumulator>> =
        Arc::new(Mutex::new(LoadLatencyAccumulator::default()));
    let load_errors: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    tokio::spawn(async move {
        let _ = send_sse_best_effort(&sse_tx, "execution:init", init_payload);

        let flush_stop = CancellationToken::new();
        let flush_handle = tokio::spawn(flush_load_batches(
            sse_tx.clone(),
            exec_ctx.cancel.clone(),
            flush_stop.clone(),
            Arc::clone(&load_chunk),
            Arc::clone(&load_latest),
            Arc::clone(&load_latency),
            Arc::clone(&load_context),
        ));

        let mut handles = Vec::with_capacity(selected_nodes.len());
        for (index, node) in selected_nodes.iter().enumerate() {
            let node = node.clone();
            let client = state_clone.client.clone();
            let cancel = exec_ctx.cancel.clone();
            let tx = sse_tx.clone();
            let load_chunk = Arc::clone(&load_chunk);
            let load_latest = Arc::clone(&load_latest);
            let load_latency = Arc::clone(&load_latency);
            let load_errors = Arc::clone(&load_errors);
            let load_context = Arc::clone(&load_context);
            let selected_base_url_key = runner_selected_base_url_key.clone();
            let pipeline = runner_pipeline.clone();
            let transaction_id = transaction_id_for_children.clone();
            let specs = runtime_specs_for_runner.clone();

            let child_request = json!({
                "pipeline": pipeline,
                "selectedBaseUrlKey": selected_base_url_key,
                "specs": specs,
                "config": {
                    "totalRequests": split_requests[index],
                    "concurrency": split_concurrency[index],
                    "rampUpSeconds": runner_ramp_up_seconds
                }
            });

            handles.push(tokio::spawn(async move {
                forward_runner_stream_load_chunked(
                    &client,
                    node,
                    child_request,
                    tx,
                    cancel,
                    load_chunk,
                    load_latest,
                    load_latency,
                    load_errors,
                    load_context,
                    "/api/v1/tests/load",
                    transaction_id,
                )
                .await;
            }));
        }

        for handle in handles {
            if let Err(err) = handle.await {
                error!("runner stream task failed: {}", err);
            }
        }

        flush_stop.cancel();
        let _ = flush_handle.await;

        if !exec_ctx.cancel.is_cancelled() {
            let lines = crate::server::execution::drain_load_chunk(&load_chunk).await;
            let consolidated = snapshot_consolidated_metrics(&load_latest, &load_latency).await;
            let payload = add_load_context_fields(
                json!({ "lines": lines, "consolidated": consolidated }),
                load_context.as_ref(),
            );
            let _ = send_sse_best_effort(&sse_tx, "complete", payload);
        }

        let finished_at_ms = now_ms() as i64;
        let duration_ms = finished_at_ms.saturating_sub(started_at_ms);
        let final_lines = snapshot_latest_lines(&load_latest).await;
        let final_consolidated = snapshot_consolidated_metrics(&load_latest, &load_latency).await;
        let errors = load_errors.lock().await.clone();
        let status = determine_load_history_status(
            exec_ctx.cancel.is_cancelled(),
            final_consolidated.as_ref(),
            errors.is_empty(),
        );
        let context_payload = add_load_context_fields(json!({}), load_context.as_ref());

        if let Err(err) = upsert_load_history(
            &state_clone.db,
            LoadHistoryWrite {
                id: history_record_id,
                execution_id: history_execution_id,
                transaction_id,
                metadata: history_metadata,
                pipeline_id: history_pipeline_id,
                pipeline_name: history_pipeline_name,
                selected_base_url_key: history_selected_base_url_key,
                status,
                started_at_ms,
                finished_at_ms,
                duration_ms,
                requested_config: serde_json::to_value(runner_config).unwrap_or(Value::Null),
                final_consolidated: final_consolidated
                    .and_then(|value| serde_json::to_value(value).ok()),
                final_lines: final_lines
                    .into_iter()
                    .map(|line| serde_json::to_value(line).unwrap_or(Value::Null))
                    .collect(),
                errors,
                request: history_request,
                context: context_payload,
            },
        )
        .await
        {
            error!("failed to save load history: {}", err);
        }

        let mut executions = state_clone.executions.write().await;
        executions.remove(&execution_id_for_cleanup);
    });

    sse_response_from_rx(rx)
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{projectId}/tests/load",
    params(
        ("projectId" = String, Path, description = "ID do projeto"),
        ("x-transaction-id" = Option<String>, Header, description = "ID de transação para rastreamento; será propagado para os runners e ecoado no response")
    ),
    request_body = ProjectLoadTestRequest,
    responses(
        (
            status = 200,
            description = "Stream SSE unificado de load test (project-scoped).",
            content_type = "text/event-stream",
            body = OrchestratorSseEventData,
            headers(
                ("x-transaction-id" = Option<String>, description = "Eco do x-transaction-id recebido")
            )
        ),
        (
            status = 400,
            description = "Request inválido",
            body = ErrorResponse,
            headers(
                ("x-transaction-id" = Option<String>, description = "Eco do x-transaction-id recebido")
            )
        ),
        (
            status = 503,
            description = "Sem runners disponíveis",
            body = ErrorResponse,
            headers(
                ("x-transaction-id" = Option<String>, description = "Eco do x-transaction-id recebido")
            )
        )
    )
)]
pub async fn run_load_test_for_project(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    headers: HeaderMap,
    payload: Result<Json<ProjectLoadTestRequest>, JsonRejection>,
) -> Response {
    let Json(payload) = match payload {
        Ok(payload) => payload,
        Err(rejection) => return bad_request_response(rejection),
    };

    let forwarded = LoadTestRequest {
        pipeline: payload.pipeline,
        config: payload.config,
        selected_base_url_key: payload.selected_base_url_key,
        project_id: Some(project_id),
        pipeline_index: payload.pipeline_index,
        specs: payload.specs,
    };
    run_load_test_internal(State(state), headers, Ok(Json(forwarded))).await
}
