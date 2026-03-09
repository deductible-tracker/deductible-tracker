use crate::db::DbPool;
use crate::db::models::AuditLog;

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

pub async fn list_audit_logs(
    pool: &DbPool,
    user_id: &str,
    since: Option<chrono::DateTime<chrono::Utc>>,
) -> anyhow::Result<Vec<AuditLog>> {
    super::list_audit_logs(pool, user_id, since).await
}
