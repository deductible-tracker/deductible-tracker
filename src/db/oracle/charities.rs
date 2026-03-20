use crate::db::models::Charity;
use crate::db::oracle::OracleConnectionManager;
use anyhow::anyhow;
use r2d2::Pool;
use tokio::task;

pub(crate) async fn list_charities(
    pool: &Pool<OracleConnectionManager>,
    user_id: &str,
) -> anyhow::Result<Vec<Charity>> {
    let p = pool.clone();
    let user_id = user_id.to_string();
    let rows = task::spawn_blocking(move || -> anyhow::Result<Vec<Charity>> {
        let conn = p.get()?;
        let parse_utc = |value: Option<String>| {
            value
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(chrono::Utc::now)
        };
        let sql = "SELECT id, user_id, name, ein, created_at, updated_at, nonprofit_type, deductibility, street, city, state, zip, category, status, classification FROM charities WHERE user_id = :1 ORDER BY name ASC";
        let rows_iter = conn.query(sql, &[&user_id])?;
        let mut out = Vec::new();
        for row in rows_iter.flatten() {
            out.push(Charity {
                id: row.get(0).unwrap_or_default(),
                user_id: row.get(1).unwrap_or_default(),
                name: row.get(2).unwrap_or_default(),
                ein: row.get(3).ok(),
                created_at: parse_utc(row.get(4).ok()),
                updated_at: parse_utc(row.get(5).ok()),
                nonprofit_type: row.get(6).ok(),
                deductibility: row.get(7).ok(),
                street: row.get(8).ok(),
                city: row.get(9).ok(),
                state: row.get(10).ok(),
                zip: row.get(11).ok(),
                category: row.get(12).ok(),
                status: row.get(13).ok(),
                classification: row.get(14).ok(),
            });
        }
        Ok(out)
    }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
    Ok(rows)
}
