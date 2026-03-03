use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};
use axum::{Json, http::StatusCode};
use previa_runner::Pipeline;

use crate::server::db::{
    delete_pipeline_record, insert_project_pipeline, load_pipelines_for_project,
    load_project_pipeline_record, project_exists, update_project_pipeline,
};
use crate::server::errors::{internal_error_response, not_found_response};
use crate::server::models::{ErrorResponse, PipelineInput};
use crate::server::state::AppState;
use crate::server::utils::new_uuid_v7;

#[utoipa::path(
    get,
    path = "/api/v1/projects/{projectId}/pipelines",
    params(
        ("projectId" = String, Path, description = "ID do projeto")
    ),
    responses(
        (
            status = 200,
            description = "Lista de pipelines do projeto",
            body = Vec<Pipeline>
        ),
        (
            status = 404,
            description = "Projeto não encontrado",
            body = ErrorResponse
        )
    )
)]
pub async fn list_project_pipelines(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> Response {
    match project_exists(&state.db, &project_id).await {
        Ok(false) => return not_found_response("project not found"),
        Ok(true) => {}
        Err(err) => return internal_error_response(format!("failed to load project: {err}")),
    }

    match load_pipelines_for_project(&state.db, &project_id).await {
        Ok(pipelines) => Json(pipelines).into_response(),
        Err(err) => internal_error_response(format!("failed to load project pipelines: {err}")),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{projectId}/pipelines/{pipelineId}",
    params(
        ("projectId" = String, Path, description = "ID do projeto"),
        ("pipelineId" = String, Path, description = "ID da pipeline")
    ),
    responses(
        (
            status = 200,
            description = "Pipeline do projeto",
            body = Pipeline
        ),
        (
            status = 404,
            description = "Projeto ou pipeline não encontrado",
            body = ErrorResponse
        )
    )
)]
pub async fn get_project_pipeline(
    State(state): State<AppState>,
    Path((project_id, pipeline_id)): Path<(String, String)>,
) -> Response {
    match project_exists(&state.db, &project_id).await {
        Ok(false) => return not_found_response("project not found"),
        Ok(true) => {}
        Err(err) => return internal_error_response(format!("failed to load project: {err}")),
    }

    match load_project_pipeline_record(&state.db, &project_id, &pipeline_id).await {
        Ok(Some(pipeline)) => Json(pipeline).into_response(),
        Ok(None) => not_found_response("pipeline not found"),
        Err(err) => internal_error_response(format!("failed to load pipeline: {err}")),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{projectId}/pipelines",
    params(
        ("projectId" = String, Path, description = "ID do projeto")
    ),
    request_body = PipelineInput,
    responses(
        (
            status = 201,
            description = "Pipeline criada",
            body = Pipeline
        ),
        (
            status = 400,
            description = "Payload inválido",
            body = ErrorResponse
        ),
        (
            status = 404,
            description = "Projeto não encontrado",
            body = ErrorResponse
        )
    )
)]
pub async fn create_project_pipeline(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    Json(pipeline): Json<PipelineInput>,
) -> Response {
    if pipeline.name.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "bad_request".to_owned(),
                message: "pipeline name is required".to_owned(),
            }),
        )
            .into_response();
    }

    match project_exists(&state.db, &project_id).await {
        Ok(false) => return not_found_response("project not found"),
        Ok(true) => {}
        Err(err) => return internal_error_response(format!("failed to load project: {err}")),
    }

    let pipeline = Pipeline {
        id: Some(new_uuid_v7()),
        name: pipeline.name,
        description: pipeline.description,
        base_url: pipeline.base_url,
        steps: pipeline.steps,
    };

    match insert_project_pipeline(&state.db, &project_id, pipeline).await {
        Ok(item) => (StatusCode::CREATED, Json(item)).into_response(),
        Err(err) => internal_error_response(format!("failed to create pipeline: {err}")),
    }
}

#[utoipa::path(
    put,
    path = "/api/v1/projects/{projectId}/pipelines/{pipelineId}",
    params(
        ("projectId" = String, Path, description = "ID do projeto"),
        ("pipelineId" = String, Path, description = "ID da pipeline")
    ),
    request_body = PipelineInput,
    responses(
        (
            status = 200,
            description = "Pipeline atualizada",
            body = Pipeline
        ),
        (
            status = 400,
            description = "Payload inválido",
            body = ErrorResponse
        ),
        (
            status = 404,
            description = "Projeto ou pipeline não encontrado",
            body = ErrorResponse
        )
    )
)]
pub async fn upsert_project_pipeline(
    State(state): State<AppState>,
    Path((project_id, pipeline_id)): Path<(String, String)>,
    Json(pipeline): Json<PipelineInput>,
) -> Response {
    if pipeline.name.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "bad_request".to_owned(),
                message: "pipeline name is required".to_owned(),
            }),
        )
            .into_response();
    }

    match project_exists(&state.db, &project_id).await {
        Ok(false) => return not_found_response("project not found"),
        Ok(true) => {}
        Err(err) => return internal_error_response(format!("failed to load project: {err}")),
    }

    let pipeline = Pipeline {
        id: Some(pipeline_id.clone()),
        name: pipeline.name,
        description: pipeline.description,
        base_url: pipeline.base_url,
        steps: pipeline.steps,
    };
    match update_project_pipeline(&state.db, &project_id, &pipeline_id, pipeline).await {
        Ok(Some(item)) => Json(item).into_response(),
        Ok(None) => not_found_response("pipeline not found"),
        Err(err) => internal_error_response(format!("failed to update pipeline: {err}")),
    }
}

#[utoipa::path(
    delete,
    path = "/api/v1/projects/{projectId}/pipelines/{pipelineId}",
    params(
        ("projectId" = String, Path, description = "ID do projeto"),
        ("pipelineId" = String, Path, description = "ID da pipeline")
    ),
    responses(
        (status = 204, description = "Pipeline removida"),
        (status = 404, description = "Projeto ou pipeline não encontrado", body = ErrorResponse)
    )
)]
pub async fn delete_project_pipeline(
    State(state): State<AppState>,
    Path((project_id, pipeline_id)): Path<(String, String)>,
) -> Response {
    match project_exists(&state.db, &project_id).await {
        Ok(false) => return not_found_response("project not found"),
        Ok(true) => {}
        Err(err) => return internal_error_response(format!("failed to load project: {err}")),
    }

    match delete_pipeline_record(&state.db, &project_id, &pipeline_id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => not_found_response("pipeline not found"),
        Err(err) => internal_error_response(format!("failed to delete pipeline: {err}")),
    }
}
