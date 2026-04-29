use sqlx::Row;

use crate::server::db::DbPool;
use crate::server::models::{
    RunnerRecord, RunnerRuntimeInfo, RunnerUpdateRequest, RunnerUpsertRequest,
};
use crate::server::utils::{new_uuid_v7, now_iso};

pub fn normalize_runner_endpoint(raw: &str) -> Option<String> {
    let value = raw.trim().trim_end_matches('/');
    if value.is_empty() {
        return None;
    }
    if value.starts_with("http://") || value.starts_with("https://") {
        return Some(value.to_owned());
    }
    Some(format!("http://{value}"))
}

pub async fn seed_env_runner_records(db: &DbPool, endpoints: &[String]) -> Result<(), sqlx::Error> {
    for endpoint in endpoints {
        let Some(endpoint) = normalize_runner_endpoint(endpoint) else {
            continue;
        };
        upsert_runner_record(
            db,
            RunnerUpsertRequest {
                endpoint,
                name: None,
                enabled: Some(true),
            },
            "env",
        )
        .await?;
    }
    Ok(())
}

pub async fn upsert_runner_record(
    db: &DbPool,
    request: RunnerUpsertRequest,
    source: &str,
) -> Result<RunnerRecord, sqlx::Error> {
    let endpoint = normalize_runner_endpoint(&request.endpoint).ok_or_else(|| {
        sqlx::Error::Configuration("runner endpoint cannot be empty".to_owned().into())
    })?;
    let now = now_iso();
    let enabled = request.enabled.unwrap_or(true);
    let enabled_i64 = if enabled { 1i64 } else { 0i64 };
    let id = new_uuid_v7();

    db.query(
        "INSERT INTO runners (
            id, endpoint, name, source, enabled, health_status, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, 'unknown', ?, ?)
        ON CONFLICT(endpoint) DO UPDATE SET
            name = COALESCE(excluded.name, runners.name),
            source = excluded.source,
            enabled = excluded.enabled,
            updated_at = excluded.updated_at",
    )
    .bind(&id)
    .bind(&endpoint)
    .bind(&request.name)
    .bind(source)
    .bind(enabled_i64)
    .bind(&now)
    .bind(&now)
    .execute(db)
    .await?;

    load_runner_record_by_endpoint(db, &endpoint)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

pub async fn update_runner_record(
    db: &DbPool,
    selector: &str,
    request: RunnerUpdateRequest,
) -> Result<Option<RunnerRecord>, sqlx::Error> {
    let Some(existing) = load_runner_record(db, selector).await? else {
        return Ok(None);
    };
    let now = now_iso();
    let enabled = request.enabled.unwrap_or(existing.enabled);
    let enabled_i64 = if enabled { 1i64 } else { 0i64 };
    let name = request.name.or(existing.name);

    db.query(
        "UPDATE runners
        SET name = ?, enabled = ?, updated_at = ?
        WHERE id = ?",
    )
    .bind(&name)
    .bind(enabled_i64)
    .bind(&now)
    .bind(&existing.id)
    .execute(db)
    .await?;

    load_runner_record_by_endpoint(db, &existing.endpoint).await
}

pub async fn delete_runner_record(db: &DbPool, selector: &str) -> Result<bool, sqlx::Error> {
    let Some(existing) = load_runner_record(db, selector).await? else {
        return Ok(false);
    };
    let result = db
        .query("DELETE FROM runners WHERE id = ?")
        .bind(&existing.id)
        .execute(db)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn list_runner_records(db: &DbPool) -> Result<Vec<RunnerRecord>, sqlx::Error> {
    let rows = db
        .query(
            "SELECT id, endpoint, name, source, enabled, health_status, last_seen_at, last_error,
                runtime_json, created_at, updated_at
            FROM runners
            ORDER BY endpoint ASC",
        )
        .fetch_all(db)
        .await?;
    Ok(rows.iter().map(runner_from_row).collect())
}

pub async fn list_enabled_runner_endpoints(db: &DbPool) -> Result<Vec<String>, sqlx::Error> {
    let rows = db
        .query("SELECT endpoint FROM runners WHERE enabled = ? ORDER BY endpoint ASC")
        .bind(1i64)
        .fetch_all(db)
        .await?;
    Ok(rows
        .into_iter()
        .filter_map(|row| row.try_get::<String, _>("endpoint").ok())
        .collect())
}

pub async fn mark_runner_observed(
    db: &DbPool,
    endpoint: &str,
    active: bool,
    error: Option<&str>,
    runtime: Option<&RunnerRuntimeInfo>,
) -> Result<(), sqlx::Error> {
    let Some(endpoint) = normalize_runner_endpoint(endpoint) else {
        return Ok(());
    };
    let now = now_iso();
    let health_status = if active { "healthy" } else { "unhealthy" };
    let runtime_json = runtime.map(|runtime| serde_json::to_string(runtime).unwrap_or_default());
    db.query(
        "UPDATE runners
        SET health_status = ?, last_seen_at = ?, last_error = ?, runtime_json = ?, updated_at = ?
        WHERE endpoint = ?",
    )
    .bind(health_status)
    .bind(if active { Some(now.as_str()) } else { None })
    .bind(error)
    .bind(&runtime_json)
    .bind(&now)
    .bind(&endpoint)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn load_runner_record(
    db: &DbPool,
    selector: &str,
) -> Result<Option<RunnerRecord>, sqlx::Error> {
    if let Some(endpoint) = normalize_runner_endpoint(selector) {
        if let Some(runner) = load_runner_record_by_endpoint(db, &endpoint).await? {
            return Ok(Some(runner));
        }
    }

    let row = db
        .query(
            "SELECT id, endpoint, name, source, enabled, health_status, last_seen_at, last_error,
                runtime_json, created_at, updated_at
            FROM runners
            WHERE id = ?
            LIMIT 1",
        )
        .bind(selector)
        .fetch_optional(db)
        .await?;
    Ok(row.as_ref().map(runner_from_row))
}

async fn load_runner_record_by_endpoint(
    db: &DbPool,
    endpoint: &str,
) -> Result<Option<RunnerRecord>, sqlx::Error> {
    let row = db
        .query(
            "SELECT id, endpoint, name, source, enabled, health_status, last_seen_at, last_error,
                runtime_json, created_at, updated_at
            FROM runners
            WHERE endpoint = ?
            LIMIT 1",
        )
        .bind(endpoint)
        .fetch_optional(db)
        .await?;
    Ok(row.as_ref().map(runner_from_row))
}

fn runner_from_row(row: &sqlx::any::AnyRow) -> RunnerRecord {
    let runtime_json = row
        .try_get::<Option<String>, _>("runtime_json")
        .ok()
        .flatten();
    let runtime = runtime_json
        .as_deref()
        .and_then(|value| serde_json::from_str::<RunnerRuntimeInfo>(value).ok());
    RunnerRecord {
        id: row.try_get("id").unwrap_or_default(),
        endpoint: row.try_get("endpoint").unwrap_or_default(),
        name: row.try_get::<Option<String>, _>("name").ok().flatten(),
        source: row.try_get("source").unwrap_or_default(),
        enabled: row.try_get::<i64, _>("enabled").unwrap_or(0) != 0,
        health_status: row
            .try_get("health_status")
            .unwrap_or_else(|_| "unknown".to_owned()),
        last_seen_at: row
            .try_get::<Option<String>, _>("last_seen_at")
            .ok()
            .flatten(),
        last_error: row
            .try_get::<Option<String>, _>("last_error")
            .ok()
            .flatten(),
        runtime,
        created_at: row.try_get("created_at").unwrap_or_default(),
        updated_at: row.try_get("updated_at").unwrap_or_default(),
    }
}
