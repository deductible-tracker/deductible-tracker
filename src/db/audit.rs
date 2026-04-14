use crate::db::DbPool;

pub async fn log_audit(
    pool: &DbPool,
    id: &str,
    user_id: &str,
    action: &str,
    table_name: &str,
    record_id: &Option<String>,
    details: &Option<String>,
) -> anyhow::Result<()> {
    super::log_audit(pool, id, user_id, action, table_name, record_id, details).await
}
