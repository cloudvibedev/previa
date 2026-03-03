use sqlx::{Row, SqlitePool};

use crate::server::db::common::touch_project_updated_at;
use crate::server::models::{ProjectSpecRecord, ProjectSpecUpsertRequest, SpecUrlEntry};
use crate::server::utils::{new_uuid_v7, now_iso, now_ms};
use crate::server::validation::specs::build_servers_from_urls;

fn calculate_spec_md5(spec_json: &str) -> String {
    format!("{:x}", md5::compute(spec_json.as_bytes()))
}

pub fn project_spec_from_row(row: &sqlx::sqlite::SqliteRow) -> ProjectSpecRecord {
    let spec_json = row
        .try_get::<String, _>("spec_json")
        .unwrap_or_else(|_| "{}".to_owned());
    let spec_md5 = {
        let value = row.try_get::<String, _>("spec_md5").unwrap_or_default();
        if value.trim().is_empty() {
            calculate_spec_md5(&spec_json)
        } else {
            value
        }
    };
    let urls_json = row
        .try_get::<String, _>("urls_json")
        .unwrap_or_else(|_| "[]".to_owned());
    let sync_value = row.try_get::<i64, _>("sync").unwrap_or(0);
    let live_value = row.try_get::<i64, _>("live").unwrap_or(0);
    let url = row.try_get::<Option<String>, _>("url").ok().flatten();
    let urls = serde_json::from_str::<Vec<SpecUrlEntry>>(&urls_json).unwrap_or_default();
    let servers = build_servers_from_urls(&urls, url.as_deref());
    ProjectSpecRecord {
        id: row.try_get("id").unwrap_or_default(),
        project_id: row.try_get("project_id").unwrap_or_default(),
        spec: serde_json::from_str::<serde_json::Value>(&spec_json)
            .unwrap_or(serde_json::Value::Null),
        spec_md5,
        url,
        slug: row.try_get::<Option<String>, _>("slug").ok().flatten(),
        urls,
        servers,
        sync: sync_value != 0,
        live: live_value != 0,
        created_at: row.try_get("created_at").unwrap_or_default(),
        updated_at: row.try_get("updated_at").unwrap_or_default(),
    }
}

pub async fn list_project_spec_records(
    db: &SqlitePool,
    project_id: &str,
) -> Result<Vec<ProjectSpecRecord>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, project_id, spec_json, spec_md5, url, slug, urls_json, sync, live, created_at, updated_at
        FROM project_openapi_specs
        WHERE project_id = ?
        ORDER BY updated_at_ms DESC, id ASC",
    )
    .bind(project_id)
    .fetch_all(db)
    .await?;

    Ok(rows.iter().map(project_spec_from_row).collect())
}

pub async fn load_project_spec_record_by_id(
    db: &SqlitePool,
    project_id: &str,
    spec_id: &str,
) -> Result<Option<ProjectSpecRecord>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT id, project_id, spec_json, spec_md5, url, slug, urls_json, sync, live, created_at, updated_at
        FROM project_openapi_specs
        WHERE project_id = ? AND id = ?
        LIMIT 1",
    )
    .bind(project_id)
    .bind(spec_id)
    .fetch_optional(db)
    .await?;

    Ok(row.as_ref().map(project_spec_from_row))
}

pub async fn insert_project_spec_record(
    db: &SqlitePool,
    project_id: &str,
    payload: ProjectSpecUpsertRequest,
) -> Result<ProjectSpecRecord, sqlx::Error> {
    let now_iso = now_iso();
    let now_ms_i64 = now_ms() as i64;
    let mut tx = db.begin().await?;
    let spec_id = new_uuid_v7();

    let spec_json = payload.spec.to_string();
    let spec_md5 = calculate_spec_md5(&spec_json);
    let urls_json = serde_json::to_string(&payload.urls).unwrap_or_else(|_| "[]".to_owned());
    sqlx::query(
        "INSERT INTO project_openapi_specs (
            id, project_id, spec_json, spec_md5, url, slug, urls_json, sync, live, created_at, updated_at, created_at_ms, updated_at_ms
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&spec_id)
    .bind(project_id)
    .bind(&spec_json)
    .bind(&spec_md5)
    .bind(&payload.url)
    .bind(&payload.slug)
    .bind(&urls_json)
    .bind(if payload.sync { 1i64 } else { 0i64 })
    .bind(if payload.live { 1i64 } else { 0i64 })
    .bind(&now_iso)
    .bind(&now_iso)
    .bind(now_ms_i64)
    .bind(now_ms_i64)
    .execute(&mut *tx)
    .await?;

    touch_project_updated_at(&mut tx, project_id, &now_iso, now_ms_i64).await?;

    tx.commit().await?;

    load_project_spec_record_by_id(db, project_id, &spec_id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

pub async fn update_project_spec_record(
    db: &SqlitePool,
    project_id: &str,
    spec_id: &str,
    payload: ProjectSpecUpsertRequest,
) -> Result<Option<ProjectSpecRecord>, sqlx::Error> {
    let now_iso = now_iso();
    let now_ms_i64 = now_ms() as i64;
    let mut tx = db.begin().await?;

    let spec_json = payload.spec.to_string();
    let spec_md5 = calculate_spec_md5(&spec_json);
    let urls_json = serde_json::to_string(&payload.urls).unwrap_or_else(|_| "[]".to_owned());
    let result = sqlx::query(
        "UPDATE project_openapi_specs SET
            spec_json = ?,
            spec_md5 = ?,
            url = ?,
            slug = ?,
            urls_json = ?,
            sync = ?,
            live = ?,
            updated_at = ?,
            updated_at_ms = ?
        WHERE project_id = ? AND id = ?",
    )
    .bind(&spec_json)
    .bind(&spec_md5)
    .bind(&payload.url)
    .bind(&payload.slug)
    .bind(&urls_json)
    .bind(if payload.sync { 1i64 } else { 0i64 })
    .bind(if payload.live { 1i64 } else { 0i64 })
    .bind(&now_iso)
    .bind(now_ms_i64)
    .bind(project_id)
    .bind(spec_id)
    .execute(&mut *tx)
    .await?;

    if result.rows_affected() == 0 {
        tx.rollback().await?;
        return Ok(None);
    }

    touch_project_updated_at(&mut tx, project_id, &now_iso, now_ms_i64).await?;
    tx.commit().await?;

    load_project_spec_record_by_id(db, project_id, spec_id).await
}

pub async fn delete_project_spec_record(
    db: &SqlitePool,
    project_id: &str,
    spec_id: &str,
) -> Result<bool, sqlx::Error> {
    let now_iso = now_iso();
    let now_ms_i64 = now_ms() as i64;
    let mut tx = db.begin().await?;
    let result = sqlx::query("DELETE FROM project_openapi_specs WHERE project_id = ? AND id = ?")
        .bind(project_id)
        .bind(spec_id)
        .execute(&mut *tx)
        .await?;
    if result.rows_affected() == 0 {
        tx.rollback().await?;
        return Ok(false);
    }

    touch_project_updated_at(&mut tx, project_id, &now_iso, now_ms_i64).await?;

    tx.commit().await?;
    Ok(true)
}

pub async fn backfill_project_spec_md5_hashes(db: &SqlitePool) -> Result<u64, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, spec_json
        FROM project_openapi_specs
        WHERE COALESCE(spec_md5, '') = ''",
    )
    .fetch_all(db)
    .await?;

    if rows.is_empty() {
        return Ok(0);
    }

    let mut tx = db.begin().await?;
    let mut updated_rows = 0u64;

    for row in rows {
        let spec_id: String = row.try_get("id")?;
        let spec_json = row
            .try_get::<String, _>("spec_json")
            .unwrap_or_else(|_| "{}".to_owned());
        let spec_md5 = calculate_spec_md5(&spec_json);
        let result = sqlx::query(
            "UPDATE project_openapi_specs
            SET spec_md5 = ?
            WHERE id = ?
              AND COALESCE(spec_md5, '') = ''",
        )
        .bind(spec_md5)
        .bind(spec_id)
        .execute(&mut *tx)
        .await?;
        updated_rows += result.rows_affected();
    }

    tx.commit().await?;
    Ok(updated_rows)
}
