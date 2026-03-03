use std::sync::Arc;

use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::Response;
use rand::seq::SliceRandom;
use serde_json::json;
use tokio::sync::{Mutex, broadcast, mpsc};
use tokio_util::sync::CancellationToken;
use tracing::error;

use crate::server::db::{save_e2e_history, upsert_e2e_history};
use crate::server::errors::{
    bad_request_message_response, bad_request_response, internal_error_response,
    service_unavailable_response,
};
use crate::server::execution::{
    add_context_fields, collect_active_nodes, determine_e2e_history_status, forward_runner_stream,
    resolve_runtime_specs_for_execution, send_sse_best_effort, spawn_broadcast_bridge,
    sse_response_from_rx,
};
use crate::server::middleware::transaction::extract_transaction_id;
use crate::server::models::{
    E2eHistoryAccumulator, E2eHistoryWrite, E2eTestRequest, ErrorResponse, HistoryMetadata,
    OrchestratorSseEventData, ProjectE2eTestRequest,
};
use crate::server::state::{AppState, EXECUTION_SSE_BUFFER_SIZE, ExecutionCtx, ExecutionKind};
use crate::server::utils::{new_uuid_v7, now_ms};

pub async fn run_e2e_test_internal(
    State(state): State<AppState>,
    headers: HeaderMap,
    payload: Result<Json<E2eTestRequest>, JsonRejection>,
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

    let mut randomized = state.runner_endpoints.clone();
    randomized.shuffle(&mut rand::rng());

    // Random order + first healthy node
    let active_nodes = collect_active_nodes(&state.client, &randomized).await;
    if active_nodes.is_empty() {
        return service_unavailable_response("No active runners found via /health");
    }

    let selected_node = active_nodes[0].clone();
    let selected_runners = vec![selected_node.clone()];
    let transaction_id = extract_transaction_id(&headers);
    let transaction_id_for_runner = transaction_id.clone();
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
    let plan = crate::server::models::NodePlan {
        requested_nodes: 1,
        nodes_found: active_nodes.len(),
        nodes_used: 1,
        warning: None,
    };
    let Some(project_id_for_execution) = payload.project_id.clone() else {
        return bad_request_message_response("projectId is required");
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

    let (tx, rx) = mpsc::unbounded_channel();
    spawn_broadcast_bridge(response_subscriber, tx, false);
    let state_clone = state.clone();
    let execution_id_for_cleanup = orchestrator_execution_id.clone();
    let history_execution_id = orchestrator_execution_id.clone();
    let history_record_id = new_uuid_v7();
    let runtime_specs_for_runner = runtime_specs.clone().unwrap_or_default();
    if let Err(err) = save_e2e_history(
        &state.db,
        E2eHistoryWrite {
            id: history_record_id.clone(),
            execution_id: history_execution_id.clone(),
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
    {
        error!("failed to save e2e running history: {}", err);
    }

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
                execution_id: history_execution_id,
                transaction_id,
                metadata: history_metadata,
                pipeline_id,
                pipeline_name,
                selected_base_url_key,
                status,
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
    });

    sse_response_from_rx(rx)
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{projectId}/tests/e2e",
    params(
        ("projectId" = String, Path, description = "ID do projeto"),
        ("x-transaction-id" = Option<String>, Header, description = "ID de transação para rastreamento; será propagado para os runners e ecoado no response")
    ),
    request_body = ProjectE2eTestRequest,
    responses(
        (
            status = 200,
            description = "Stream SSE unificado para teste de integração (project-scoped).",
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
pub async fn run_e2e_test_for_project(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    headers: HeaderMap,
    payload: Result<Json<ProjectE2eTestRequest>, JsonRejection>,
) -> Response {
    let Json(payload) = match payload {
        Ok(payload) => payload,
        Err(rejection) => return bad_request_response(rejection),
    };

    let forwarded = E2eTestRequest {
        pipeline: payload.pipeline,
        selected_base_url_key: payload.selected_base_url_key,
        project_id: Some(project_id),
        pipeline_index: payload.pipeline_index,
        specs: payload.specs,
    };
    run_e2e_test_internal(State(state), headers, Ok(Json(forwarded))).await
}
