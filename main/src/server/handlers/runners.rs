use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Json, response::IntoResponse};

use crate::server::db::{
    delete_runner_record, list_runner_records, load_runner_record, update_runner_record,
    upsert_runner_record,
};
use crate::server::models::{ErrorResponse, RunnerUpdateRequest, RunnerUpsertRequest};
use crate::server::state::AppState;

#[utoipa::path(
    get,
    path = "/api/v1/runners",
    responses(
        (
            status = 200,
            description = "Lista de runners cadastrados",
            body = Vec<crate::server::models::RunnerRecord>
        ),
        (
            status = 500,
            description = "Erro ao consultar runners",
            body = ErrorResponse
        )
    )
)]
pub async fn list_runners(State(state): State<AppState>) -> impl IntoResponse {
    match list_runner_records(&state.db).await {
        Ok(runners) => Json(runners).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "runner_error".to_owned(),
                message: format!("failed to list runners: {err}"),
            }),
        )
            .into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/runners",
    request_body = RunnerUpsertRequest,
    responses(
        (
            status = 201,
            description = "Runner cadastrado ou atualizado",
            body = crate::server::models::RunnerRecord
        ),
        (
            status = 400,
            description = "Payload inválido",
            body = ErrorResponse
        )
    )
)]
pub async fn create_runner(
    State(state): State<AppState>,
    Json(payload): Json<RunnerUpsertRequest>,
) -> impl IntoResponse {
    match upsert_runner_record(&state.db, payload, "api").await {
        Ok(runner) => (StatusCode::CREATED, Json(runner)).into_response(),
        Err(err) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "runner_error".to_owned(),
                message: format!("failed to create runner: {err}"),
            }),
        )
            .into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/runners/{runnerId}",
    params(
        ("runnerId" = String, Path, description = "ID do runner")
    ),
    responses(
        (
            status = 200,
            description = "Runner cadastrado",
            body = crate::server::models::RunnerRecord
        ),
        (
            status = 404,
            description = "Runner não encontrado",
            body = ErrorResponse
        ),
        (
            status = 500,
            description = "Erro ao consultar runner",
            body = ErrorResponse
        )
    )
)]
pub async fn get_runner(
    State(state): State<AppState>,
    Path(runner_id): Path<String>,
) -> impl IntoResponse {
    match load_runner_record(&state.db, &runner_id).await {
        Ok(Some(runner)) => Json(runner).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "runner_error".to_owned(),
                message: "runner not found".to_owned(),
            }),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "runner_error".to_owned(),
                message: format!("failed to load runner: {err}"),
            }),
        )
            .into_response(),
    }
}

#[utoipa::path(
    patch,
    path = "/api/v1/runners/{runnerId}",
    params(
        ("runnerId" = String, Path, description = "ID do runner")
    ),
    request_body = RunnerUpdateRequest,
    responses(
        (
            status = 200,
            description = "Runner atualizado",
            body = crate::server::models::RunnerRecord
        ),
        (
            status = 400,
            description = "Payload inválido",
            body = ErrorResponse
        ),
        (
            status = 404,
            description = "Runner não encontrado",
            body = ErrorResponse
        )
    )
)]
pub async fn update_runner(
    State(state): State<AppState>,
    Path(runner_id): Path<String>,
    Json(payload): Json<RunnerUpdateRequest>,
) -> impl IntoResponse {
    match update_runner_record(&state.db, &runner_id, payload).await {
        Ok(Some(runner)) => Json(runner).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "runner_error".to_owned(),
                message: "runner not found".to_owned(),
            }),
        )
            .into_response(),
        Err(err) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "runner_error".to_owned(),
                message: format!("failed to update runner: {err}"),
            }),
        )
            .into_response(),
    }
}

#[utoipa::path(
    delete,
    path = "/api/v1/runners/{runnerId}",
    params(
        ("runnerId" = String, Path, description = "ID do runner")
    ),
    responses(
        (
            status = 204,
            description = "Runner removido"
        ),
        (
            status = 404,
            description = "Runner não encontrado",
            body = ErrorResponse
        ),
        (
            status = 500,
            description = "Erro ao remover runner",
            body = ErrorResponse
        )
    )
)]
pub async fn delete_runner(
    State(state): State<AppState>,
    Path(runner_id): Path<String>,
) -> impl IntoResponse {
    match delete_runner_record(&state.db, &runner_id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "runner_error".to_owned(),
                message: "runner not found".to_owned(),
            }),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "runner_error".to_owned(),
                message: format!("failed to delete runner: {err}"),
            }),
        )
            .into_response(),
    }
}
