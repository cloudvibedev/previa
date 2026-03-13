use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::Response;

use crate::server::db::load_project_pipeline_for_execution;
use crate::server::errors::{
    bad_request_message_response, bad_request_response, internal_error_response,
};
use crate::server::execution::{
    StartE2eExecutionError, sse_response_for_started_execution, start_e2e_execution,
};
use crate::server::middleware::transaction::extract_transaction_id;
use crate::server::models::{
    E2eTestRequest, ErrorResponse, OrchestratorSseEventData, ProjectE2eTestRequest,
};
use crate::server::state::AppState;

pub async fn run_e2e_test_internal(
    State(state): State<AppState>,
    headers: HeaderMap,
    payload: Result<Json<E2eTestRequest>, JsonRejection>,
) -> Response {
    let Json(payload) = match payload {
        Ok(payload) => payload,
        Err(rejection) => return bad_request_response(rejection),
    };
    let transaction_id = extract_transaction_id(&headers);
    match start_e2e_execution(state, payload, transaction_id).await {
        Ok(started) => sse_response_for_started_execution(started),
        Err(StartE2eExecutionError::BadRequest(message)) => bad_request_message_response(&message),
        Err(StartE2eExecutionError::ServiceUnavailable(message)) => {
            crate::server::errors::service_unavailable_response(&message)
        }
        Err(StartE2eExecutionError::Internal(message)) => internal_error_response(message),
    }
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

    let (pipeline, pipeline_index) = match (payload.pipeline_id.clone(), payload.pipeline) {
        (Some(pipeline_id), _) if !pipeline_id.trim().is_empty() => {
            match load_project_pipeline_for_execution(&state.db, &project_id, &pipeline_id).await {
                Ok(Some((pipeline, position))) => (pipeline, Some(position)),
                Ok(None) => {
                    return bad_request_message_response("pipelineId not found for project");
                }
                Err(err) => {
                    return internal_error_response(format!(
                        "failed to load pipeline for execution: {err}"
                    ));
                }
            }
        }
        (_, Some(pipeline)) => (pipeline, payload.pipeline_index),
        _ => return bad_request_message_response("pipelineId is required"),
    };

    let forwarded = E2eTestRequest {
        pipeline,
        selected_base_url_key: payload.selected_base_url_key,
        project_id: Some(project_id),
        pipeline_index,
        specs: payload.specs,
    };
    run_e2e_test_internal(State(state), headers, Ok(Json(forwarded))).await
}
