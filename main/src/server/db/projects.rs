use previa_runner::Pipeline;
use serde_json::Value;
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool};

use crate::server::db::query_utils::{
    clamp_history_limit, clamp_history_offset, history_order_to_sql,
};
use crate::server::models::{
    ProjectListQuery, ProjectMetadataUpsertRequest, ProjectRecord, ProjectUpsertRequest,
};
use crate::server::utils::{new_uuid_v7, now_iso, now_ms};

pub async fn list_project_records(
    db: &SqlitePool,
    query: ProjectListQuery,
) -> Result<Vec<ProjectRecord>, sqlx::Error> {
    let limit = clamp_history_limit(query.limit);
    let offset = clamp_history_offset(query.offset);
    let order_sql = history_order_to_sql(query.order);

    let mut qb = QueryBuilder::<Sqlite>::new(
        "SELECT id, name, description, created_at, updated_at FROM projects ORDER BY updated_at_ms ",
    );
    qb.push(order_sql)
        .push(" LIMIT ")
        .push_bind(limit as i64)
        .push(" OFFSET ")
        .push_bind(offset as i64);

    let rows = qb.build().fetch_all(db).await?;
    let mut projects = Vec::with_capacity(rows.len());
    for row in rows {
        let description = row
            .try_get::<Option<String>, _>("description")
            .ok()
            .flatten();
        projects.push(ProjectRecord {
            id: row.try_get("id").unwrap_or_default(),
            name: row.try_get("name").unwrap_or_default(),
            description,
            created_at: row.try_get("created_at").unwrap_or_default(),
            updated_at: row.try_get("updated_at").unwrap_or_default(),
        });
    }

    Ok(projects)
}

pub async fn load_project_record(
    db: &SqlitePool,
    project_id: &str,
) -> Result<Option<ProjectRecord>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT id, name, description, created_at, updated_at FROM projects WHERE id = ?",
    )
    .bind(project_id)
    .fetch_optional(db)
    .await?;

    let Some(row) = row else {
        return Ok(None);
    };

    let description = row
        .try_get::<Option<String>, _>("description")
        .ok()
        .flatten();

    Ok(Some(ProjectRecord {
        id: row.try_get("id").unwrap_or_default(),
        name: row.try_get("name").unwrap_or_default(),
        description,
        created_at: row.try_get("created_at").unwrap_or_default(),
        updated_at: row.try_get("updated_at").unwrap_or_default(),
    }))
}

pub async fn upsert_project_metadata(
    db: &SqlitePool,
    project_id: String,
    payload: ProjectMetadataUpsertRequest,
) -> Result<ProjectRecord, sqlx::Error> {
    let now_iso = now_iso();
    let now_ms_i64 = now_ms() as i64;
    let mut tx = db.begin().await?;

    let existing = sqlx::query("SELECT created_at, created_at_ms FROM projects WHERE id = ?")
        .bind(&project_id)
        .fetch_optional(&mut *tx)
        .await?;
    let created_at = existing
        .as_ref()
        .and_then(|row| row.try_get::<String, _>("created_at").ok())
        .unwrap_or_else(|| now_iso.clone());
    let created_at_ms = existing
        .as_ref()
        .and_then(|row| row.try_get::<i64, _>("created_at_ms").ok())
        .unwrap_or(now_ms_i64);

    sqlx::query(
        "INSERT INTO projects (
            id, name, description, created_at, updated_at, created_at_ms, updated_at_ms
        ) VALUES (?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            description = excluded.description,
            updated_at = excluded.updated_at,
            updated_at_ms = excluded.updated_at_ms",
    )
    .bind(&project_id)
    .bind(&payload.name)
    .bind(&payload.description)
    .bind(&created_at)
    .bind(&now_iso)
    .bind(created_at_ms)
    .bind(now_ms_i64)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    load_project_record(db, &project_id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

pub async fn upsert_project_with_pipelines(
    db: &SqlitePool,
    project_id: String,
    payload: ProjectUpsertRequest,
) -> Result<ProjectRecord, sqlx::Error> {
    let now_iso = now_iso();
    let now_ms_i64 = now_ms() as i64;
    let mut tx = db.begin().await?;

    let existing = sqlx::query("SELECT created_at, created_at_ms FROM projects WHERE id = ?")
        .bind(&project_id)
        .fetch_optional(&mut *tx)
        .await?;
    let created_at = payload.created_at.clone().unwrap_or_else(|| {
        existing
            .as_ref()
            .and_then(|row| row.try_get::<String, _>("created_at").ok())
            .unwrap_or_else(|| now_iso.clone())
    });
    let created_at_ms = existing
        .as_ref()
        .and_then(|row| row.try_get::<i64, _>("created_at_ms").ok())
        .unwrap_or(now_ms_i64);
    let updated_at = payload
        .updated_at
        .clone()
        .unwrap_or_else(|| now_iso.clone());

    sqlx::query(
        "INSERT INTO projects (
            id, name, description, created_at, updated_at, created_at_ms, updated_at_ms, spec_json
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            description = excluded.description,
            updated_at = excluded.updated_at,
            updated_at_ms = excluded.updated_at_ms,
            spec_json = excluded.spec_json",
    )
    .bind(&project_id)
    .bind(&payload.name)
    .bind(&payload.description)
    .bind(&created_at)
    .bind(&updated_at)
    .bind(created_at_ms)
    .bind(now_ms_i64)
    .bind(payload.spec.as_ref().map(Value::to_string))
    .execute(&mut *tx)
    .await?;

    sqlx::query("DELETE FROM pipelines WHERE project_id = ?")
        .bind(&project_id)
        .execute(&mut *tx)
        .await?;

    for (index, pipeline_input) in payload.pipelines.into_iter().enumerate() {
        let pipeline_id = new_uuid_v7();
        let pipeline = Pipeline {
            id: Some(pipeline_id.clone()),
            name: pipeline_input.name,
            description: pipeline_input.description,
            steps: pipeline_input.steps,
        };

        sqlx::query(
            "INSERT INTO pipelines (
                id, project_id, position, name, description, created_at, updated_at, pipeline_json
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(pipeline_id)
        .bind(&project_id)
        .bind(index as i64)
        .bind(&pipeline.name)
        .bind(&pipeline.description)
        .bind(&now_iso)
        .bind(&updated_at)
        .bind(serde_json::to_string(&pipeline).unwrap_or_else(|_| "{}".to_owned()))
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    load_project_record(db, &project_id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}
