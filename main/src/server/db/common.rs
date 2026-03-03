use sqlx::{Sqlite, SqlitePool, Transaction};

pub async fn project_exists(db: &SqlitePool, project_id: &str) -> Result<bool, sqlx::Error> {
    let exists = sqlx::query_scalar::<_, i64>("SELECT 1 FROM projects WHERE id = ? LIMIT 1")
        .bind(project_id)
        .fetch_optional(db)
        .await?
        .is_some();
    Ok(exists)
}

pub async fn touch_project_updated_at(
    tx: &mut Transaction<'_, Sqlite>,
    project_id: &str,
    now_iso: &str,
    now_ms_i64: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE projects SET updated_at = ?, updated_at_ms = ? WHERE id = ?")
        .bind(now_iso)
        .bind(now_ms_i64)
        .bind(project_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}
