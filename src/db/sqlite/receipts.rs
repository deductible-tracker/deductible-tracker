use anyhow::anyhow;
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager as R2SqliteManager;
use rusqlite::params;
use tokio::task;

use crate::db::models::Receipt;
use crate::db::models::NewReceipt;

pub(crate) async fn add_receipt(
    pool: &Pool<R2SqliteManager>,
    input: &NewReceipt,
    created_at: &str,
) -> anyhow::Result<()> {
    let p = pool.clone();
    let input = input.clone();
    let created_at = created_at.to_string();
    task::spawn_blocking(move || -> anyhow::Result<()> {
        let conn = p.get()?;
        let sql = "INSERT INTO receipts (id, donation_id, key, file_name, content_type, size, ocr_text, ocr_date, ocr_amount, ocr_status, created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)";
        conn.execute(
            sql,
            params![input.id, input.donation_id, input.key, input.file_name, input.content_type, input.size, Option::<String>::None, Option::<String>::None, Option::<i64>::None, Option::<String>::None, created_at],
        )?;
        Ok(())
    })
    .await
    .map_err(|e| anyhow!("DB task join error: {}", e))??;
    Ok(())
}

pub(crate) async fn list_receipts(
    pool: &Pool<R2SqliteManager>,
    user_id: &str,
    donation_id: Option<String>,
) -> anyhow::Result<Vec<Receipt>> {
    let p = pool.clone();
    let user_id = user_id.to_string();
    let rows = task::spawn_blocking(move || -> anyhow::Result<Vec<Receipt>> {
        let conn = p.get()?;
        let sql_with_donation = "SELECT r.id, r.donation_id, r.key, r.file_name, r.content_type, r.size, r.ocr_text, r.ocr_date, r.ocr_amount, r.ocr_status, r.created_at FROM receipts r JOIN donations d ON d.id = r.donation_id WHERE d.user_id = ?1 AND r.donation_id = ?2";
        let sql_no_donation = "SELECT r.id, r.donation_id, r.key, r.file_name, r.content_type, r.size, r.ocr_text, r.ocr_date, r.ocr_amount, r.ocr_status, r.created_at FROM receipts r JOIN donations d ON d.id = r.donation_id WHERE d.user_id = ?1";
        let mut out = Vec::new();
        if let Some(did) = donation_id {
            let mut stmt = conn.prepare(sql_with_donation)?;
            let rows_iter = stmt.query_map(params![user_id, did], |row| {
                let created_at_str: Option<String> = row.get(10)?;
                let created_at = created_at_str
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(Utc::now);
                Ok(Receipt {
                    id: row.get(0)?,
                    donation_id: row.get(1)?,
                    key: row.get(2)?,
                    file_name: row.get(3).ok(),
                    content_type: row.get(4).ok(),
                    size: row.get(5).ok(),
                    ocr_text: row.get(6).ok(),
                    ocr_date: None,
                    ocr_amount: row.get(8).ok(),
                    ocr_status: row.get(9).ok(),
                    created_at,
                })
            })?;
            for r in rows_iter {
                out.push(r?);
            }
        } else {
            let mut stmt = conn.prepare(sql_no_donation)?;
            let rows_iter = stmt.query_map(params![user_id], |row| {
                let created_at_str: Option<String> = row.get(10)?;
                let created_at = created_at_str
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(Utc::now);
                Ok(Receipt {
                    id: row.get(0)?,
                    donation_id: row.get(1)?,
                    key: row.get(2)?,
                    file_name: row.get(3).ok(),
                    content_type: row.get(4).ok(),
                    size: row.get(5).ok(),
                    ocr_text: row.get(6).ok(),
                    ocr_date: None,
                    ocr_amount: row.get(8).ok(),
                    ocr_status: row.get(9).ok(),
                    created_at,
                })
            })?;
            for r in rows_iter {
                out.push(r?);
            }
        }
        Ok(out)
    })
    .await
    .map_err(|e| anyhow!("DB task join error: {}", e))??;
    Ok(rows)
}

pub(crate) async fn get_receipt(
    pool: &Pool<R2SqliteManager>,
    user_id: &str,
    receipt_id: &str,
) -> anyhow::Result<Option<Receipt>> {
    let p = pool.clone();
    let user_id = user_id.to_string();
    let receipt_id = receipt_id.to_string();
    let row = task::spawn_blocking(move || -> anyhow::Result<Option<Receipt>> {
        let conn = p.get()?;
        let sql = "SELECT r.id, r.donation_id, r.key, r.file_name, r.content_type, r.size, r.ocr_text, r.ocr_date, r.ocr_amount, r.ocr_status, r.created_at FROM receipts r JOIN donations d ON d.id = r.donation_id WHERE d.user_id = ?1 AND r.id = ?2";
        let mut stmt = conn.prepare(sql)?;
        let mut rows = stmt.query(params![user_id, receipt_id])?;
        if let Some(row) = rows.next()? {
            let created_at_str: Option<String> = row.get(10)?;
            let created_at = created_at_str
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(Utc::now);
            return Ok(Some(Receipt {
                id: row.get(0)?,
                donation_id: row.get(1)?,
                key: row.get(2)?,
                file_name: row.get(3).ok(),
                content_type: row.get(4).ok(),
                size: row.get(5).ok(),
                ocr_text: row.get(6).ok(),
                ocr_date: None,
                ocr_amount: row.get(8).ok(),
                ocr_status: row.get(9).ok(),
                created_at,
            }));
        }
        Ok(None)
    })
    .await
    .map_err(|e| anyhow!("DB task join error: {}", e))??;
    Ok(row)
}
