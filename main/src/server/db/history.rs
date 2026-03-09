use serde_json::Value;
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool};

use crate::server::db::{clamp_history_limit, clamp_history_offset, history_order_to_sql};
use crate::server::models::{
    E2eHistoryRecord, E2eHistoryWrite, HistoryQuery, LoadHistoryRecord, LoadHistoryWrite,
};

pub async fn list_e2e_history_records(
    db: &SqlitePool,
    project_id: &str,
    query: HistoryQuery,
) -> Result<Vec<E2eHistoryRecord>, sqlx::Error> {
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

    let rows = qb.build().fetch_all(db).await?;
    Ok(rows.iter().map(e2e_history_record_from_row).collect())
}

pub async fn load_e2e_history_record_by_id(
    db: &SqlitePool,
    project_id: &str,
    test_id: &str,
) -> Result<Option<E2eHistoryRecord>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT id, execution_id, transaction_id, project_id, pipeline_index, pipeline_id, pipeline_name, selected_base_url_key, status, started_at_ms, finished_at_ms, duration_ms, summary_json, steps_json, errors_json, request_json
        FROM integration_history
        WHERE project_id = ? AND (id = ? OR execution_id = ?)
        ORDER BY finished_at_ms DESC
        LIMIT 1",
    )
    .bind(project_id)
    .bind(test_id)
    .bind(test_id)
    .fetch_optional(db)
    .await?;

    Ok(row.as_ref().map(e2e_history_record_from_row))
}

pub async fn list_load_history_records(
    db: &SqlitePool,
    project_id: &str,
    query: HistoryQuery,
) -> Result<Vec<LoadHistoryRecord>, sqlx::Error> {
    let limit = clamp_history_limit(query.limit);
    let offset = clamp_history_offset(query.offset);
    let order_sql = history_order_to_sql(query.order);
    let mut qb = QueryBuilder::<Sqlite>::new(
        "SELECT id, execution_id, transaction_id, project_id, pipeline_index, pipeline_id, pipeline_name, selected_base_url_key, status, started_at_ms, finished_at_ms, duration_ms, requested_config_json, final_consolidated_json, final_lines_json, errors_json, request_json, context_json
        FROM load_history
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

    let rows = qb.build().fetch_all(db).await?;
    Ok(rows.iter().map(load_history_record_from_row).collect())
}

pub async fn load_load_history_record_by_id(
    db: &SqlitePool,
    project_id: &str,
    test_id: &str,
) -> Result<Option<LoadHistoryRecord>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT id, execution_id, transaction_id, project_id, pipeline_index, pipeline_id, pipeline_name, selected_base_url_key, status, started_at_ms, finished_at_ms, duration_ms, requested_config_json, final_consolidated_json, final_lines_json, errors_json, request_json, context_json
        FROM load_history
        WHERE project_id = ? AND (id = ? OR execution_id = ?)
        ORDER BY finished_at_ms DESC
        LIMIT 1",
    )
    .bind(project_id)
    .bind(test_id)
    .bind(test_id)
    .fetch_optional(db)
    .await?;

    Ok(row.as_ref().map(load_history_record_from_row))
}

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

fn e2e_history_record_from_row(row: &sqlx::sqlite::SqliteRow) -> E2eHistoryRecord {
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

    E2eHistoryRecord {
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
    }
}

fn load_history_record_from_row(row: &sqlx::sqlite::SqliteRow) -> LoadHistoryRecord {
    let requested_config_json = row
        .try_get::<String, _>("requested_config_json")
        .unwrap_or_else(|_| "{}".to_owned());
    let final_consolidated_json = row
        .try_get::<Option<String>, _>("final_consolidated_json")
        .ok()
        .flatten();
    let final_lines_json = row
        .try_get::<String, _>("final_lines_json")
        .unwrap_or_else(|_| "[]".to_owned());
    let errors_json = row
        .try_get::<String, _>("errors_json")
        .unwrap_or_else(|_| "[]".to_owned());
    let request_json = row
        .try_get::<String, _>("request_json")
        .unwrap_or_else(|_| "{}".to_owned());
    let context_json = row
        .try_get::<String, _>("context_json")
        .unwrap_or_else(|_| "{}".to_owned());

    LoadHistoryRecord {
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
        requested_config: serde_json::from_str(&requested_config_json).unwrap_or(Value::Null),
        final_consolidated: final_consolidated_json
            .and_then(|raw| serde_json::from_str::<Value>(&raw).ok()),
        final_lines: serde_json::from_str::<Vec<Value>>(&final_lines_json).unwrap_or_default(),
        errors: serde_json::from_str::<Vec<String>>(&errors_json).unwrap_or_default(),
        request: serde_json::from_str(&request_json).unwrap_or(Value::Null),
        context: serde_json::from_str(&context_json).unwrap_or(Value::Null),
    }
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
