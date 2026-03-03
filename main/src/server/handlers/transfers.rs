use axum::Json;
use axum::extract::{Path, Query, State, rejection::JsonRejection};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::server::db::{
    import_project_bundle, load_e2e_history_for_export, load_load_history_for_export,
    load_project_export, project_exists,
};
use crate::server::errors::{
    bad_request_message_response, bad_request_response, conflict_response, internal_error_response,
    not_found_response,
};
use crate::server::models::{
    ErrorResponse, ProjectExportEnvelope, ProjectImportResponse, ProjectTransferQuery,
};
use crate::server::state::AppState;
use crate::server::utils::now_iso;

const PROJECT_EXPORT_FORMAT: &str = "previa.project.export.v1";

#[utoipa::path(
    get,
    path = "/api/v1/projects/{projectId}/export",
    params(
        ("projectId" = String, Path, description = "Project ID"),
        ("includeHistory" = Option<bool>, Query, description = "Include e2e/load history in export. Default true.")
    ),
    responses(
        (
            status = 200,
            description = "Project export bundle",
            body = ProjectExportEnvelope
        ),
        (
            status = 404,
            description = "Project not found",
            body = ErrorResponse
        ),
        (
            status = 500,
            description = "Failed to export project",
            body = ErrorResponse
        )
    )
)]
pub async fn export_project(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    Query(query): Query<ProjectTransferQuery>,
) -> Response {
    let project_id = project_id.trim().to_owned();
    if project_id.is_empty() {
        return bad_request_message_response("projectId cannot be empty");
    }

    let include_history = query.include_history.unwrap_or(true);
    let mut project = match load_project_export(&state.db, &project_id).await {
        Ok(Some(project)) => project,
        Ok(None) => return not_found_response("project not found"),
        Err(err) => {
            return internal_error_response(format!("failed to load project export: {err}"));
        }
    };

    if include_history {
        let e2e = match load_e2e_history_for_export(&state.db, &project_id).await {
            Ok(items) => items,
            Err(err) => {
                return internal_error_response(format!(
                    "failed to load e2e history export: {err}"
                ));
            }
        };
        let load = match load_load_history_for_export(&state.db, &project_id).await {
            Ok(items) => items,
            Err(err) => {
                return internal_error_response(format!(
                    "failed to load load history export: {err}"
                ));
            }
        };
        project.history.e2e = e2e;
        project.history.load = load;
    }

    Json(ProjectExportEnvelope {
        format: PROJECT_EXPORT_FORMAT.to_owned(),
        exported_at: now_iso(),
        history_included: include_history,
        project,
    })
    .into_response()
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/import",
    params(
        ("includeHistory" = Option<bool>, Query, description = "Persist e2e/load history from payload. Default true.")
    ),
    request_body = ProjectExportEnvelope,
    responses(
        (
            status = 201,
            description = "Project imported",
            body = ProjectImportResponse
        ),
        (
            status = 400,
            description = "Invalid payload or format",
            body = ErrorResponse
        ),
        (
            status = 409,
            description = "Import conflict",
            body = ErrorResponse
        ),
        (
            status = 500,
            description = "Failed to import project",
            body = ErrorResponse
        )
    )
)]
pub async fn import_project(
    State(state): State<AppState>,
    Query(query): Query<ProjectTransferQuery>,
    payload: Result<Json<ProjectExportEnvelope>, JsonRejection>,
) -> Response {
    let Json(mut payload) = match payload {
        Ok(payload) => payload,
        Err(rejection) => return bad_request_response(rejection),
    };

    if payload.format != PROJECT_EXPORT_FORMAT {
        return bad_request_message_response("invalid import format");
    }

    payload.project.id = payload.project.id.trim().to_owned();
    payload.project.name = payload.project.name.trim().to_owned();
    if payload.project.id.is_empty() {
        return bad_request_message_response("project.id is required");
    }
    if payload.project.name.is_empty() {
        return bad_request_message_response("project.name is required");
    }

    match project_exists(&state.db, &payload.project.id).await {
        Ok(true) => return conflict_response("project already exists"),
        Ok(false) => {}
        Err(err) => return internal_error_response(format!("failed to load project: {err}")),
    }

    let include_history = query.include_history.unwrap_or(true);
    match import_project_bundle(&state.db, &payload.project, include_history).await {
        Ok(response) => (StatusCode::CREATED, Json(response)).into_response(),
        Err(sqlx::Error::Database(db_err)) if db_err.is_unique_violation() => {
            conflict_response("import data conflicts with existing records")
        }
        Err(err) => internal_error_response(format!("failed to import project: {err}")),
    }
}
