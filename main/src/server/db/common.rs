use crate::server::db::DbPool;
use sqlx::Transaction;

pub async fn project_exists(db: &DbPool, project_id: &str) -> Result<bool, sqlx::Error> {
    let exists =
        sqlx::query_scalar::<sqlx::Any, i64>(db.sql("SELECT 1 FROM projects WHERE id = ? LIMIT 1"))
            .bind(project_id)
            .fetch_optional(db)
            .await?
            .is_some();
    Ok(exists)
}

pub async fn touch_project_updated_at(
    db: &DbPool,
    tx: &mut Transaction<'_, sqlx::Any>,
    project_id: &str,
    now_iso: &str,
    now_ms_i64: i64,
) -> Result<(), sqlx::Error> {
    db.query("UPDATE projects SET updated_at = ?, updated_at_ms = ? WHERE id = ?")
        .bind(now_iso)
        .bind(now_ms_i64)
        .bind(project_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}
