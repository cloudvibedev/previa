use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Response};
use axum::{Json, http::StatusCode};
use serde_json::Value;
use sqlx::{QueryBuilder, Row, Sqlite};

use crate::server::db::{
    clamp_history_limit, clamp_history_offset, history_order_to_sql, project_exists,
};
use crate::server::errors::{internal_error_response, not_found_response};
use crate::server::models::{E2eHistoryRecord, ErrorResponse, HistoryQuery};
use crate::server::state::AppState;

#[utoipa::path(
    get,
    path = "/api/v1/projects/{projectId}/tests/e2e",
    params(
        ("projectId" = String, Path, description = "ID do projeto"),
        ("pipelineIndex" = Option<i64>, Query, description = "Filtra por índice da pipeline"),
        ("limit" = Option<u32>, Query, description = "Limite de registros retornados (default 100, max 500)"),
        ("offset" = Option<u32>, Query, description = "Deslocamento da paginação (default 0)"),
        ("order" = Option<crate::server::models::HistoryOrder>, Query, description = "Ordem por finishedAtMs: asc | desc (default desc)")
    ),
    responses(
        (
            status = 200,
            description = "Histórico de execuções de integração",
            body = Vec<E2eHistoryRecord>
        ),
        (
            status = 500,
            description = "Erro ao consultar histórico",
            body = ErrorResponse
        )
    )
)]
pub async fn list_e2e_history(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    Query(query): Query<HistoryQuery>,
) -> Response {
    let limit = clamp_history_limit(query.limit);
    let offset = clamp_history_offset(query.offset);
    let order_sql = history_order_to_sql(query.order);
    let mut qb = QueryBuilder::<Sqlite>::new(
        "SELECT id, execution_id, transaction_id, project_id, pipeline_index, pipeline_id, pipeline_name, selected_base_url_key, status, started_at_ms, finished_at_ms, duration_ms, summary_json, steps_json, errors_json, request_json
        FROM integration_history
        WHERE project_id = ",
    );
    qb.push_bind(project_id);
    if let Some(pipeline_index) = query.pipeline_index {
        qb.push(" AND pipeline_index = ").push_bind(pipeline_index);
    }

    qb.push(" ORDER BY finished_at_ms ")
        .push(order_sql)
        .push(" LIMIT ")
        .push_bind(limit as i64)
        .push(" OFFSET ")
        .push_bind(offset as i64);

    let rows = match qb.build().fetch_all(&state.db).await {
        Ok(rows) => rows,
        Err(err) => return internal_error_response(format!("failed to query history: {err}")),
    };

    let mut records = Vec::with_capacity(rows.len());
    for row in rows {
        let summary_json = row
            .try_get::<Option<String>, _>("summary_json")
            .ok()
            .flatten();
        let steps_json = row
            .try_get::<String, _>("steps_json")
            .unwrap_or_else(|_| "[]".to_owned());
        let errors_json = row
            .try_get::<String, _>("errors_json")
            .unwrap_or_else(|_| "[]".to_owned());
        let request_json = row
            .try_get::<String, _>("request_json")
            .unwrap_or_else(|_| "{}".to_owned());

        records.push(E2eHistoryRecord {
            id: row.try_get("id").unwrap_or_else(|_| "".to_owned()),
            execution_id: row
                .try_get("execution_id")
                .unwrap_or_else(|_| "".to_owned()),
            transaction_id: row.try_get("transaction_id").ok(),
            project_id: row.try_get("project_id").ok(),
            pipeline_index: row.try_get("pipeline_index").ok(),
            pipeline_id: row.try_get("pipeline_id").ok(),
            pipeline_name: row.try_get("pipeline_name").unwrap_or_default(),
            selected_base_url_key: row.try_get("selected_base_url_key").ok(),
            status: row.try_get("status").unwrap_or_default(),
            started_at_ms: row.try_get("started_at_ms").unwrap_or_default(),
            finished_at_ms: row.try_get("finished_at_ms").unwrap_or_default(),
            duration_ms: row.try_get("duration_ms").unwrap_or_default(),
            summary: summary_json.and_then(|raw| serde_json::from_str::<Value>(&raw).ok()),
            steps: serde_json::from_str::<Vec<Value>>(&steps_json).unwrap_or_default(),
            errors: serde_json::from_str::<Vec<String>>(&errors_json).unwrap_or_default(),
            request: serde_json::from_str::<Value>(&request_json).unwrap_or(Value::Null),
        });
    }

    Json(records).into_response()
}

#[utoipa::path(
    delete,
    path = "/api/v1/projects/{projectId}/tests/e2e",
    params(
        ("projectId" = String, Path, description = "ID do projeto"),
        ("pipelineIndex" = Option<i64>, Query, description = "Se informado, remove histórico apenas do índice da pipeline")
    ),
    responses(
        (status = 204, description = "Histórico de integração removido"),
        (status = 404, description = "Projeto não encontrado", body = ErrorResponse),
        (status = 500, description = "Erro ao remover histórico", body = ErrorResponse)
    )
)]
pub async fn delete_e2e_history(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    Query(query): Query<HistoryQuery>,
) -> Response {
    match project_exists(&state.db, &project_id).await {
        Ok(false) => return not_found_response("project not found"),
        Ok(true) => {}
        Err(err) => return internal_error_response(format!("failed to load project: {err}")),
    }

    let mut qb = QueryBuilder::<Sqlite>::new("DELETE FROM integration_history WHERE project_id = ");
    qb.push_bind(&project_id);
    if let Some(pipeline_index) = query.pipeline_index {
        qb.push(" AND pipeline_index = ").push_bind(pipeline_index);
    }

    match qb.build().execute(&state.db).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => internal_error_response(format!("failed to delete e2e history: {err}")),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{projectId}/tests/e2e/{test_id}",
    params(
        ("projectId" = String, Path, description = "ID do projeto"),
        ("test_id" = String, Path, description = "ID do teste (id do histórico ou execution_id)")
    ),
    responses(
        (
            status = 200,
            description = "Execução individual de integração",
            body = E2eHistoryRecord
        ),
        (
            status = 404,
            description = "Teste não encontrado",
            body = ErrorResponse
        )
    )
)]
pub async fn get_e2e_test_by_id(
    State(state): State<AppState>,
    Path((project_id, test_id)): Path<(String, String)>,
) -> Response {
    let row = match sqlx::query(
        "SELECT id, execution_id, transaction_id, project_id, pipeline_index, pipeline_id, pipeline_name, selected_base_url_key, status, started_at_ms, finished_at_ms, duration_ms, summary_json, steps_json, errors_json, request_json
        FROM integration_history
        WHERE project_id = ? AND (id = ? OR execution_id = ?)
        ORDER BY finished_at_ms DESC
        LIMIT 1",
    )
    .bind(&project_id)
    .bind(&test_id)
    .bind(&test_id)
    .fetch_optional(&state.db)
    .await
    {
        Ok(row) => row,
        Err(err) => {
            return internal_error_response(format!("failed to query e2e history: {err}"));
        }
    };

    let Some(row) = row else {
        return not_found_response("e2e test not found");
    };

    let summary_json = row
        .try_get::<Option<String>, _>("summary_json")
        .ok()
        .flatten();
    let steps_json = row
        .try_get::<String, _>("steps_json")
        .unwrap_or_else(|_| "[]".to_owned());
    let errors_json = row
        .try_get::<String, _>("errors_json")
        .unwrap_or_else(|_| "[]".to_owned());
    let request_json = row
        .try_get::<String, _>("request_json")
        .unwrap_or_else(|_| "{}".to_owned());

    Json(E2eHistoryRecord {
        id: row.try_get("id").unwrap_or_else(|_| "".to_owned()),
        execution_id: row
            .try_get("execution_id")
            .unwrap_or_else(|_| "".to_owned()),
        transaction_id: row.try_get("transaction_id").ok(),
        project_id: row.try_get("project_id").ok(),
        pipeline_index: row.try_get("pipeline_index").ok(),
        pipeline_id: row.try_get("pipeline_id").ok(),
        pipeline_name: row.try_get("pipeline_name").unwrap_or_default(),
        selected_base_url_key: row.try_get("selected_base_url_key").ok(),
        status: row.try_get("status").unwrap_or_default(),
        started_at_ms: row.try_get("started_at_ms").unwrap_or_default(),
        finished_at_ms: row.try_get("finished_at_ms").unwrap_or_default(),
        duration_ms: row.try_get("duration_ms").unwrap_or_default(),
        summary: summary_json.and_then(|raw| serde_json::from_str::<Value>(&raw).ok()),
        steps: serde_json::from_str::<Vec<Value>>(&steps_json).unwrap_or_default(),
        errors: serde_json::from_str::<Vec<String>>(&errors_json).unwrap_or_default(),
        request: serde_json::from_str::<Value>(&request_json).unwrap_or(Value::Null),
    })
    .into_response()
}

#[utoipa::path(
    delete,
    path = "/api/v1/projects/{projectId}/tests/e2e/{test_id}",
    params(
        ("projectId" = String, Path, description = "ID do projeto"),
        ("test_id" = String, Path, description = "ID do teste (id do histórico ou execution_id)")
    ),
    responses(
        (status = 204, description = "Execução de integração removida"),
        (status = 404, description = "Projeto ou teste não encontrado", body = ErrorResponse),
        (status = 500, description = "Erro ao remover execução", body = ErrorResponse)
    )
)]
pub async fn delete_e2e_test_by_id(
    State(state): State<AppState>,
    Path((project_id, test_id)): Path<(String, String)>,
) -> Response {
    match project_exists(&state.db, &project_id).await {
        Ok(false) => return not_found_response("project not found"),
        Ok(true) => {}
        Err(err) => return internal_error_response(format!("failed to load project: {err}")),
    }

    match sqlx::query(
        "DELETE FROM integration_history WHERE project_id = ? AND (id = ? OR execution_id = ?)",
    )
    .bind(&project_id)
    .bind(&test_id)
    .bind(&test_id)
    .execute(&state.db)
    .await
    {
        Ok(result) if result.rows_affected() > 0 => StatusCode::NO_CONTENT.into_response(),
        Ok(_) => not_found_response("e2e test not found"),
        Err(err) => internal_error_response(format!("failed to delete e2e history record: {err}")),
    }
}
