use anyhow::anyhow;
use chrono::Utc;
use r2d2::Pool;
use tokio::task;

use crate::db::oracle::OracleConnectionManager;
use crate::db::models::Receipt;
use crate::db::models::NewReceipt;

fn parse_utc_or_now(value: Option<String>) -> chrono::DateTime<Utc> {
    value
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(Utc::now)
}

pub(crate) async fn add_receipt(
    pool: &Pool<OracleConnectionManager>,
    input: &NewReceipt,
    created_at: &str,
) -> anyhow::Result<()> {
    let p = pool.clone();
    let input = input.clone();
    let created_at = created_at.to_string();
    task::spawn_blocking(move || -> anyhow::Result<()> {
        let conn = p.get()?;
        let sql = "INSERT INTO receipts (id, donation_id, receipt_key, file_name, content_type, receipt_size, ocr_text, ocr_date, ocr_amount, ocr_status, created_at) VALUES (:1,:2,:3,:4,:5,:6,:7,:8,:9,:10, TO_TIMESTAMP_TZ(:11, 'YYYY-MM-DD\"T\"HH24:MI:SS.FF TZH:TZM'))";
        conn.execute(sql, &[&input.id, &input.donation_id, &input.key, &input.file_name, &input.content_type, &input.size, &Option::<String>::None, &Option::<String>::None, &Option::<i64>::None, &Option::<String>::None, &created_at])?;
        conn.commit()?;
        Ok(())
    })
    .await
    .map_err(|e| anyhow!("DB task join error: {}", e))??;
    Ok(())
}

pub(crate) async fn list_receipts(
    pool: &Pool<OracleConnectionManager>,
    user_id: &str,
    donation_id: Option<String>,
) -> anyhow::Result<Vec<Receipt>> {
    let p = pool.clone();
    let user_id = user_id.to_string();
    let rows = task::spawn_blocking(move || -> anyhow::Result<Vec<Receipt>> {
        let conn = p.get()?;
        let parse_utc = |value: Option<String>| {
            value
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(Utc::now)
        };
        let sql = if donation_id.is_some() {
            "SELECT r.id, r.donation_id, r.receipt_key, r.file_name, r.content_type, r.receipt_size, r.ocr_text, r.ocr_date, r.ocr_amount, r.ocr_status, r.created_at FROM receipts r JOIN donations d ON d.id = r.donation_id WHERE d.user_id = :1 AND r.donation_id = :2 AND d.deleted = 0"
        } else {
            "SELECT r.id, r.donation_id, r.receipt_key, r.file_name, r.content_type, r.receipt_size, r.ocr_text, r.ocr_date, r.ocr_amount, r.ocr_status, r.created_at FROM receipts r JOIN donations d ON d.id = r.donation_id WHERE d.user_id = :1 AND d.deleted = 0"
        };
        let rows_iter = if let Some(did) = donation_id {
            conn.query(sql, &[&user_id, &did])?
        } else {
            conn.query(sql, &[&user_id])?
        };

        let mut out = Vec::new();
        for row in rows_iter.flatten() {
            out.push(Receipt {
                id: row.get(0).unwrap_or_default(),
                donation_id: row.get(1).unwrap_or_default(),
                key: row.get(2).unwrap_or_default(),
                file_name: row.get(3).ok(),
                content_type: row.get(4).ok(),
                size: row.get(5).ok(),
                ocr_text: row.get(6).ok(),
                ocr_date: row.get(7).ok(),
                ocr_amount: row.get(8).ok(),
                ocr_status: row.get(9).ok(),
                created_at: parse_utc(row.get(10).ok()),
            });
        }
        Ok(out)
    })
    .await
    .map_err(|e| anyhow!("DB task join error: {}", e))??;
    Ok(rows)
}

pub(crate) async fn get_receipt(
    pool: &Pool<OracleConnectionManager>,
    user_id: &str,
    receipt_id: &str,
) -> anyhow::Result<Option<Receipt>> {
    let p = pool.clone();
    let user_id = user_id.to_string();
    let receipt_id = receipt_id.to_string();
    let row = task::spawn_blocking(move || -> anyhow::Result<Option<Receipt>> {
        let conn = p.get()?;
        let sql = "SELECT r.id, r.donation_id, r.receipt_key, r.file_name, r.content_type, r.receipt_size, r.ocr_text, r.ocr_date, r.ocr_amount, r.ocr_status, r.created_at FROM receipts r JOIN donations d ON d.id = r.donation_id WHERE d.user_id = :1 AND r.id = :2";
        let mut rows = conn.query(sql, &[&user_id, &receipt_id])?;
        if let Some(r) = rows.next().transpose()? {
            return Ok(Some(Receipt {
                id: r.get(0).unwrap_or_default(),
                donation_id: r.get(1).unwrap_or_default(),
                key: r.get(2).unwrap_or_default(),
                file_name: r.get(3).ok(),
                content_type: r.get(4).ok(),
                size: r.get(5).ok(),
                ocr_text: r.get(6).ok(),
                ocr_date: r.get(7).ok(),
                ocr_amount: r.get(8).ok(),
                ocr_status: r.get(9).ok(),
                created_at: parse_utc_or_now(r.get(10).ok()),
            }));
        }
        Ok(None)
    })
    .await
    .map_err(|e| anyhow!("DB task join error: {}", e))??;
    Ok(row)
}

pub(crate) async fn set_receipt_ocr(
    pool: &Pool<OracleConnectionManager>,
    id: &str,
    text: &Option<String>,
    date: &Option<chrono::NaiveDate>,
    amount: &Option<f64>,
    status: &Option<String>,
) -> anyhow::Result<()> {
    let p = pool.clone();
    let id = id.to_string();
    let text = text.clone();
    let date = date.map(|d| d.format("%Y-%m-%d").to_string());
    let amount = *amount;
    let status = status.clone();
    let updated_at = chrono::Utc::now().to_rfc3339();

    task::spawn_blocking(move || -> anyhow::Result<()> {
        let conn = p.get()?;
        let sql = "UPDATE receipts SET ocr_text = :1, ocr_date = TO_DATE(:2, 'YYYY-MM-DD'), ocr_amount = :3, ocr_status = :4, updated_at = TO_TIMESTAMP_TZ(:5, 'YYYY-MM-DD\"T\"HH24:MI:SS.FF TZH:TZM') WHERE id = :6";
        conn.execute(sql, &[&text, &date, &amount, &status, &updated_at, &id])?;
        conn.commit()?;
        Ok(())
    })
    .await
    .map_err(|e| anyhow!("DB task join error: {}", e))??;
    Ok(())
}
