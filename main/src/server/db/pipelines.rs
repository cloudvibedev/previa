use std::collections::HashSet;

use previa_runner::Pipeline;
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool};

use crate::server::db::common::touch_project_updated_at;
use crate::server::utils::{new_uuid_v7, now_iso, now_ms};

pub async fn load_pipelines_for_project(
    db: &SqlitePool,
    project_id: &str,
) -> Result<Vec<Pipeline>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT pipeline_json FROM pipelines WHERE project_id = ? ORDER BY position ASC",
    )
    .bind(project_id)
    .fetch_all(db)
    .await?;

    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let raw = row
            .try_get::<String, _>("pipeline_json")
            .unwrap_or_else(|_| "{}".to_owned());
        if let Ok(pipeline) = serde_json::from_str::<Pipeline>(&raw) {
            items.push(pipeline);
        }
    }
    Ok(items)
}

pub async fn load_project_pipeline_record(
    db: &SqlitePool,
    project_id: &str,
    pipeline_id: &str,
) -> Result<Option<Pipeline>, sqlx::Error> {
    let row = sqlx::query("SELECT pipeline_json FROM pipelines WHERE project_id = ? AND id = ?")
        .bind(project_id)
        .bind(pipeline_id)
        .fetch_optional(db)
        .await?;

    let Some(row) = row else {
        return Ok(None);
    };

    let raw = row
        .try_get::<String, _>("pipeline_json")
        .unwrap_or_else(|_| "{}".to_owned());
    Ok(serde_json::from_str::<Pipeline>(&raw).ok())
}

pub async fn load_project_pipeline_for_execution(
    db: &SqlitePool,
    project_id: &str,
    pipeline_id: &str,
) -> Result<Option<(Pipeline, i64)>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT position, pipeline_json FROM pipelines WHERE project_id = ? AND id = ? LIMIT 1",
    )
    .bind(project_id)
    .bind(pipeline_id)
    .fetch_optional(db)
    .await?;

    let Some(row) = row else {
        return Ok(None);
    };

    let raw = row
        .try_get::<String, _>("pipeline_json")
        .unwrap_or_else(|_| "{}".to_owned());
    let position = row.try_get::<i64, _>("position").unwrap_or_default();
    Ok(serde_json::from_str::<Pipeline>(&raw)
        .ok()
        .map(|pipeline| (pipeline, position)))
}

pub async fn load_existing_project_pipeline_ids(
    db: &SqlitePool,
    project_id: &str,
    pipeline_ids: &[String],
) -> Result<HashSet<String>, sqlx::Error> {
    if pipeline_ids.is_empty() {
        return Ok(HashSet::new());
    }

    let unique_ids = pipeline_ids
        .iter()
        .map(|pipeline_id| pipeline_id.trim())
        .filter(|pipeline_id| !pipeline_id.is_empty())
        .collect::<Vec<_>>();
    if unique_ids.is_empty() {
        return Ok(HashSet::new());
    }

    let mut qb = QueryBuilder::<Sqlite>::new("SELECT id FROM pipelines WHERE project_id = ");
    qb.push_bind(project_id);
    qb.push(" AND id IN (");
    {
        let mut separated = qb.separated(", ");
        for pipeline_id in &unique_ids {
            separated.push_bind(*pipeline_id);
        }
    }
    qb.push(")");

    let rows = qb.build().fetch_all(db).await?;
    Ok(rows
        .into_iter()
        .filter_map(|row| row.try_get::<String, _>("id").ok())
        .collect())
}

pub async fn insert_project_pipeline(
    db: &SqlitePool,
    project_id: &str,
    mut pipeline: Pipeline,
) -> Result<Pipeline, sqlx::Error> {
    let now_iso = now_iso();
    let now_ms_i64 = now_ms() as i64;
    let pipeline_id = pipeline.id.clone().unwrap_or_else(new_uuid_v7);
    pipeline.id = Some(pipeline_id.clone());

    let mut tx = db.begin().await?;
    let next_position = sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(MAX(position) + 1, 0) FROM pipelines WHERE project_id = ?",
    )
    .bind(project_id)
    .fetch_one(&mut *tx)
    .await?
    .max(0);

    sqlx::query(
        "INSERT INTO pipelines (
            id, project_id, position, name, description, created_at, updated_at, pipeline_json
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&pipeline_id)
    .bind(project_id)
    .bind(next_position)
    .bind(&pipeline.name)
    .bind(&pipeline.description)
    .bind(&now_iso)
    .bind(&now_iso)
    .bind(serde_json::to_string(&pipeline).unwrap_or_else(|_| "{}".to_owned()))
    .execute(&mut *tx)
    .await?;

    touch_project_updated_at(&mut tx, project_id, &now_iso, now_ms_i64).await?;
    tx.commit().await?;
    Ok(pipeline)
}

pub async fn update_project_pipeline(
    db: &SqlitePool,
    project_id: &str,
    pipeline_id: &str,
    mut pipeline: Pipeline,
) -> Result<Option<Pipeline>, sqlx::Error> {
    let now_iso = now_iso();
    let now_ms_i64 = now_ms() as i64;
    let mut tx = db.begin().await?;

    let row = sqlx::query(
        "SELECT created_at, position FROM pipelines WHERE project_id = ? AND id = ? LIMIT 1",
    )
    .bind(project_id)
    .bind(pipeline_id)
    .fetch_optional(&mut *tx)
    .await?;

    if row.is_none() {
        tx.rollback().await?;
        return Ok(None);
    }

    pipeline.id = Some(pipeline_id.to_owned());
    sqlx::query(
        "UPDATE pipelines SET
            name = ?,
            description = ?,
            updated_at = ?,
            pipeline_json = ?
        WHERE project_id = ? AND id = ?",
    )
    .bind(&pipeline.name)
    .bind(&pipeline.description)
    .bind(&now_iso)
    .bind(serde_json::to_string(&pipeline).unwrap_or_else(|_| "{}".to_owned()))
    .bind(project_id)
    .bind(pipeline_id)
    .execute(&mut *tx)
    .await?;

    touch_project_updated_at(&mut tx, project_id, &now_iso, now_ms_i64).await?;
    tx.commit().await?;
    Ok(Some(pipeline))
}

pub async fn delete_pipeline_record(
    db: &SqlitePool,
    project_id: &str,
    pipeline_id: &str,
) -> Result<bool, sqlx::Error> {
    let now_iso = now_iso();
    let now_ms_i64 = now_ms() as i64;
    let mut tx = db.begin().await?;

    let position = sqlx::query_scalar::<_, i64>(
        "SELECT position FROM pipelines WHERE project_id = ? AND id = ? LIMIT 1",
    )
    .bind(project_id)
    .bind(pipeline_id)
    .fetch_optional(&mut *tx)
    .await?;

    let Some(position) = position else {
        tx.rollback().await?;
        return Ok(false);
    };

    sqlx::query("DELETE FROM pipelines WHERE project_id = ? AND id = ?")
        .bind(project_id)
        .bind(pipeline_id)
        .execute(&mut *tx)
        .await?;

    sqlx::query(
        "UPDATE pipelines SET position = position - 1 WHERE project_id = ? AND position > ?",
    )
    .bind(project_id)
    .bind(position)
    .execute(&mut *tx)
    .await?;

    touch_project_updated_at(&mut tx, project_id, &now_iso, now_ms_i64).await?;
    tx.commit().await?;
    Ok(true)
}
