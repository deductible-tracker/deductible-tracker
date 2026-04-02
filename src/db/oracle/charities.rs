use crate::db::models::Charity;
use chrono::Utc;
use deadpool_oracle::Pool;

pub(crate) async fn list_charities(pool: &Pool, user_id: &str) -> anyhow::Result<Vec<Charity>> {
    let conn = pool.get().await?;
    let sql = "SELECT id, user_id, name, ein, created_at, updated_at, nonprofit_type, deductibility, street, city, state, zip, category, status, classification FROM charities WHERE user_id = :1 ORDER BY name ASC";
    let rows = conn
        .query(sql, &crate::oracle_params![user_id.to_string()])
        .await?;
    let mut out = Vec::new();
    for row in &rows.rows {
        out.push(Charity {
            id: crate::db::oracle::row_string(row, 0),
            user_id: crate::db::oracle::row_string(row, 1),
            name: crate::db::oracle::row_string(row, 2),
            ein: crate::db::oracle::row_opt_string(row, 3),
            created_at: crate::db::oracle::row_datetime_utc(row, 4).unwrap_or_else(Utc::now),
            updated_at: crate::db::oracle::row_datetime_utc(row, 5).unwrap_or_else(Utc::now),
            nonprofit_type: crate::db::oracle::row_opt_string(row, 6),
            deductibility: crate::db::oracle::row_opt_string(row, 7),
            street: crate::db::oracle::row_opt_string(row, 8),
            city: crate::db::oracle::row_opt_string(row, 9),
            state: crate::db::oracle::row_opt_string(row, 10),
            zip: crate::db::oracle::row_opt_string(row, 11),
            category: crate::db::oracle::row_opt_string(row, 12),
            status: crate::db::oracle::row_opt_string(row, 13),
            classification: crate::db::oracle::row_opt_string(row, 14),
        });
    }
    Ok(out)
}
