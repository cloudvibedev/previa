use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::{Map, Value, json};
use sqlx::{Row, SqlitePool};
use tokio::sync::mpsc;

use crate::server::errors::{
    bad_request_message_response, internal_error_response, not_found_response,
};
use crate::server::execution::{spawn_broadcast_bridge, sse_response_from_rx};
use crate::server::models::{
    CancelExecutionResponse, ErrorResponse, OrchestratorSseEventData, SseMessage,
};
use crate::server::state::{AppState, ExecutionKind};

#[derive(Debug)]
struct FinishedExecutionSnapshot {
    finished_at_ms: i64,
    init_payload: Value,
    terminal_event: &'static str,
    terminal_payload: Value,
}

fn value_to_object(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        other => {
            let mut map = Map::new();
            map.insert("payload".to_owned(), other);
            map
        }
    }
}

async fn load_finished_e2e_snapshot(
    db: &SqlitePool,
    project_id: &str,
    execution_id: &str,
) -> Result<Option<FinishedExecutionSnapshot>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT status, finished_at_ms, summary_json, errors_json
        FROM integration_history
        WHERE project_id = ? AND execution_id = ?
        LIMIT 1",
    )
    .bind(project_id)
    .bind(execution_id)
    .fetch_optional(db)
    .await?;

    let Some(row) = row else {
        return Ok(None);
    };

    let status = row.try_get::<String, _>("status").unwrap_or_default();
    let finished_at_ms = row.try_get::<i64, _>("finished_at_ms").unwrap_or_default();
    let summary_json = row
        .try_get::<Option<String>, _>("summary_json")
        .ok()
        .flatten();
    let errors_json = row
        .try_get::<String, _>("errors_json")
        .unwrap_or_else(|_| "[]".to_owned());
    let errors = serde_json::from_str::<Vec<String>>(&errors_json).unwrap_or_default();

    let mut init_payload = Map::new();
    init_payload.insert("executionId".to_owned(), json!(execution_id));
    init_payload.insert("status".to_owned(), json!(status));

    let mut terminal_payload = summary_json
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        .map(value_to_object)
        .unwrap_or_default();
    terminal_payload.insert("executionId".to_owned(), json!(execution_id));
    terminal_payload.insert("status".to_owned(), json!(status));
    terminal_payload.insert("errors".to_owned(), json!(errors));

    Ok(Some(FinishedExecutionSnapshot {
        finished_at_ms,
        init_payload: Value::Object(init_payload),
        terminal_event: "pipeline:complete",
        terminal_payload: Value::Object(terminal_payload),
    }))
}

async fn load_finished_load_snapshot(
    db: &SqlitePool,
    project_id: &str,
    execution_id: &str,
) -> Result<Option<FinishedExecutionSnapshot>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT status, finished_at_ms, context_json, final_lines_json, final_consolidated_json, errors_json
        FROM load_history
        WHERE project_id = ? AND execution_id = ?
        LIMIT 1",
    )
    .bind(project_id)
    .bind(execution_id)
    .fetch_optional(db)
    .await?;

    let Some(row) = row else {
        return Ok(None);
    };

    let status = row.try_get::<String, _>("status").unwrap_or_default();
    let finished_at_ms = row.try_get::<i64, _>("finished_at_ms").unwrap_or_default();
    let context_json = row
        .try_get::<String, _>("context_json")
        .unwrap_or_else(|_| "{}".to_owned());
    let final_lines_json = row
        .try_get::<String, _>("final_lines_json")
        .unwrap_or_else(|_| "[]".to_owned());
    let final_consolidated_json = row
        .try_get::<Option<String>, _>("final_consolidated_json")
        .ok()
        .flatten();
    let errors_json = row
        .try_get::<String, _>("errors_json")
        .unwrap_or_else(|_| "[]".to_owned());

    let context_value = serde_json::from_str::<Value>(&context_json).unwrap_or(Value::Null);
    let context_object = value_to_object(context_value);
    let lines = serde_json::from_str::<Vec<Value>>(&final_lines_json).unwrap_or_default();
    let consolidated = final_consolidated_json
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        .unwrap_or(Value::Null);
    let errors = serde_json::from_str::<Vec<String>>(&errors_json).unwrap_or_default();

    let mut init_payload = context_object.clone();
    init_payload.insert("executionId".to_owned(), json!(execution_id));
    init_payload.insert("status".to_owned(), json!(status));

    let mut terminal_payload = context_object;
    terminal_payload.insert("executionId".to_owned(), json!(execution_id));
    terminal_payload.insert("status".to_owned(), json!(status));
    terminal_payload.insert("lines".to_owned(), Value::Array(lines));
    terminal_payload.insert("consolidated".to_owned(), consolidated);
    terminal_payload.insert("errors".to_owned(), json!(errors));

    Ok(Some(FinishedExecutionSnapshot {
        finished_at_ms,
        init_payload: Value::Object(init_payload),
        terminal_event: "complete",
        terminal_payload: Value::Object(terminal_payload),
    }))
}

async fn load_finished_execution_snapshot(
    db: &SqlitePool,
    project_id: &str,
    execution_id: &str,
) -> Result<Option<FinishedExecutionSnapshot>, sqlx::Error> {
    let e2e = load_finished_e2e_snapshot(db, project_id, execution_id).await?;
    let load = load_finished_load_snapshot(db, project_id, execution_id).await?;

    Ok(match (e2e, load) {
        (Some(e2e), Some(load)) => {
            if load.finished_at_ms > e2e.finished_at_ms {
                Some(load)
            } else {
                Some(e2e)
            }
        }
        (Some(e2e), None) => Some(e2e),
        (None, Some(load)) => Some(load),
        (None, None) => None,
    })
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{projectId}/executions/{executionId}",
    params(
        ("projectId" = String, Path, description = "ID do projeto"),
        ("executionId" = String, Path, description = "ID da execução retornado no evento SSE execution:init")
    ),
    responses(
        (
            status = 200,
            description = "Stream SSE da execução ativa ou stream SSE finito de execução já finalizada.",
            content_type = "text/event-stream",
            body = OrchestratorSseEventData
        ),
        (
            status = 400,
            description = "Parâmetro inválido",
            body = ErrorResponse
        ),
        (
            status = 404,
            description = "Execução não encontrada para o projeto",
            body = ErrorResponse
        ),
        (
            status = 500,
            description = "Erro interno ao recuperar execução",
            body = ErrorResponse
        )
    )
)]
pub async fn stream_execution(
    State(state): State<AppState>,
    Path((project_id, execution_id)): Path<(String, String)>,
) -> Response {
    let project_id = project_id.trim().to_owned();
    let execution_id = execution_id.trim().to_owned();
    if project_id.is_empty() {
        return bad_request_message_response("projectId cannot be empty");
    }
    if execution_id.is_empty() {
        return bad_request_message_response("executionId cannot be empty");
    }

    let execution = {
        let executions = state.executions.read().await;
        executions.get(&execution_id).cloned()
    };

    if let Some(execution) = execution {
        if execution.project_id != project_id {
            return not_found_response("execution not found for project");
        }

        let skip_execution_init = match execution.kind {
            ExecutionKind::E2e | ExecutionKind::Load => true,
        };
        let (tx, rx) = mpsc::unbounded_channel::<SseMessage>();
        let init_payload = execution.init_payload.get().await;
        let _ = tx.send(SseMessage {
            event: "execution:init".to_owned(),
            data: init_payload,
        });
        spawn_broadcast_bridge(execution.sse_tx.subscribe(), tx, skip_execution_init);
        return sse_response_from_rx(rx);
    }

    let snapshot =
        match load_finished_execution_snapshot(&state.db, &project_id, &execution_id).await {
            Ok(snapshot) => snapshot,
            Err(err) => {
                return internal_error_response(format!("failed to load execution history: {err}"));
            }
        };

    let Some(snapshot) = snapshot else {
        return not_found_response("execution not found for project");
    };

    let (tx, rx) = mpsc::unbounded_channel::<SseMessage>();
    let _ = tx.send(SseMessage {
        event: "execution:init".to_owned(),
        data: snapshot.init_payload,
    });
    let _ = tx.send(SseMessage {
        event: snapshot.terminal_event.to_owned(),
        data: snapshot.terminal_payload,
    });
    drop(tx);

    sse_response_from_rx(rx)
}

#[utoipa::path(
    post,
    path = "/api/v1/executions/{executionId}/cancel",
    params(
        ("executionId" = String, Path, description = "ID da execução retornado no evento SSE execution:init")
    ),
    responses(
        (
            status = 202,
            description = "Cancelamento solicitado",
            body = CancelExecutionResponse
        ),
        (
            status = 400,
            description = "Parâmetro inválido",
            body = ErrorResponse
        ),
        (
            status = 404,
            description = "Execução não encontrada ou já finalizada",
            body = ErrorResponse
        )
    )
)]
pub async fn cancel_execution(
    State(state): State<AppState>,
    Path(execution_id): Path<String>,
) -> Response {
    let execution_id = execution_id.trim().to_owned();
    if execution_id.is_empty() {
        return bad_request_message_response("executionId cannot be empty");
    }

    let execution = {
        let executions = state.executions.read().await;
        executions.get(&execution_id).cloned()
    };

    let Some(execution) = execution else {
        return not_found_response("execution not found or already finished");
    };

    let already_cancelled = execution.cancel.is_cancelled();
    execution.cancel.cancel();

    (
        StatusCode::ACCEPTED,
        Json(CancelExecutionResponse {
            execution_id,
            cancelled: true,
            already_cancelled,
            message: if already_cancelled {
                "cancellation already requested".to_owned()
            } else {
                "cancellation requested".to_owned()
            },
        }),
    )
        .into_response()
}
