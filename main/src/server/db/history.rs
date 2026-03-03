use sqlx::SqlitePool;

use crate::server::models::{E2eHistoryWrite, LoadHistoryWrite};

pub async fn save_e2e_history(db: &SqlitePool, write: E2eHistoryWrite) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO integration_history (
            id,
            execution_id, transaction_id, project_id, pipeline_index, pipeline_id, pipeline_name,
            selected_base_url_key, status, started_at_ms, finished_at_ms, duration_ms,
            summary_json, steps_json, errors_json, request_json
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(write.id)
    .bind(write.execution_id)
    .bind(write.transaction_id)
    .bind(write.metadata.project_id)
    .bind(write.metadata.pipeline_index)
    .bind(write.pipeline_id)
    .bind(write.pipeline_name)
    .bind(write.selected_base_url_key)
    .bind(write.status)
    .bind(write.started_at_ms)
    .bind(write.finished_at_ms)
    .bind(write.duration_ms)
    .bind(write.summary.map(|value| value.to_string()))
    .bind(serde_json::to_string(&write.steps).unwrap_or_else(|_| "[]".to_owned()))
    .bind(serde_json::to_string(&write.errors).unwrap_or_else(|_| "[]".to_owned()))
    .bind(write.request.to_string())
    .execute(db)
    .await?;

    Ok(())
}

pub async fn save_load_history(
    db: &SqlitePool,
    write: LoadHistoryWrite,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO load_history (
            id,
            execution_id, transaction_id, project_id, pipeline_index, pipeline_id, pipeline_name,
            selected_base_url_key, status, started_at_ms, finished_at_ms, duration_ms,
            requested_config_json, final_consolidated_json, final_lines_json, errors_json,
            request_json, context_json
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(write.id)
    .bind(write.execution_id)
    .bind(write.transaction_id)
    .bind(write.metadata.project_id)
    .bind(write.metadata.pipeline_index)
    .bind(write.pipeline_id)
    .bind(write.pipeline_name)
    .bind(write.selected_base_url_key)
    .bind(write.status)
    .bind(write.started_at_ms)
    .bind(write.finished_at_ms)
    .bind(write.duration_ms)
    .bind(write.requested_config.to_string())
    .bind(write.final_consolidated.map(|value| value.to_string()))
    .bind(serde_json::to_string(&write.final_lines).unwrap_or_else(|_| "[]".to_owned()))
    .bind(serde_json::to_string(&write.errors).unwrap_or_else(|_| "[]".to_owned()))
    .bind(write.request.to_string())
    .bind(write.context.to_string())
    .execute(db)
    .await?;

    Ok(())
}

pub async fn upsert_e2e_history(
    db: &SqlitePool,
    write: E2eHistoryWrite,
) -> Result<(), sqlx::Error> {
    let rows_affected = sqlx::query(
        "UPDATE integration_history SET
            transaction_id = ?,
            project_id = ?,
            pipeline_index = ?,
            pipeline_id = ?,
            pipeline_name = ?,
            selected_base_url_key = ?,
            status = ?,
            started_at_ms = ?,
            finished_at_ms = ?,
            duration_ms = ?,
            summary_json = ?,
            steps_json = ?,
            errors_json = ?,
            request_json = ?
        WHERE execution_id = ?",
    )
    .bind(write.transaction_id.clone())
    .bind(write.metadata.project_id.clone())
    .bind(write.metadata.pipeline_index)
    .bind(write.pipeline_id.clone())
    .bind(write.pipeline_name.clone())
    .bind(write.selected_base_url_key.clone())
    .bind(write.status.clone())
    .bind(write.started_at_ms)
    .bind(write.finished_at_ms)
    .bind(write.duration_ms)
    .bind(write.summary.clone().map(|value| value.to_string()))
    .bind(serde_json::to_string(&write.steps).unwrap_or_else(|_| "[]".to_owned()))
    .bind(serde_json::to_string(&write.errors).unwrap_or_else(|_| "[]".to_owned()))
    .bind(write.request.to_string())
    .bind(write.execution_id.clone())
    .execute(db)
    .await?
    .rows_affected();

    if rows_affected == 0 {
        save_e2e_history(db, write).await?;
    }

    Ok(())
}

pub async fn upsert_load_history(
    db: &SqlitePool,
    write: LoadHistoryWrite,
) -> Result<(), sqlx::Error> {
    let rows_affected = sqlx::query(
        "UPDATE load_history SET
            transaction_id = ?,
            project_id = ?,
            pipeline_index = ?,
            pipeline_id = ?,
            pipeline_name = ?,
            selected_base_url_key = ?,
            status = ?,
            started_at_ms = ?,
            finished_at_ms = ?,
            duration_ms = ?,
            requested_config_json = ?,
            final_consolidated_json = ?,
            final_lines_json = ?,
            errors_json = ?,
            request_json = ?,
            context_json = ?
        WHERE execution_id = ?",
    )
    .bind(write.transaction_id.clone())
    .bind(write.metadata.project_id.clone())
    .bind(write.metadata.pipeline_index)
    .bind(write.pipeline_id.clone())
    .bind(write.pipeline_name.clone())
    .bind(write.selected_base_url_key.clone())
    .bind(write.status.clone())
    .bind(write.started_at_ms)
    .bind(write.finished_at_ms)
    .bind(write.duration_ms)
    .bind(write.requested_config.to_string())
    .bind(
        write
            .final_consolidated
            .clone()
            .map(|value| value.to_string()),
    )
    .bind(serde_json::to_string(&write.final_lines).unwrap_or_else(|_| "[]".to_owned()))
    .bind(serde_json::to_string(&write.errors).unwrap_or_else(|_| "[]".to_owned()))
    .bind(write.request.to_string())
    .bind(write.context.to_string())
    .bind(write.execution_id.clone())
    .execute(db)
    .await?
    .rows_affected();

    if rows_affected == 0 {
        save_load_history(db, write).await?;
    }

    Ok(())
}
