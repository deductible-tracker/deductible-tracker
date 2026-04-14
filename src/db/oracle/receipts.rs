use chrono::Utc;
use deadpool_oracle::Pool;

use crate::db::models::NewReceipt;
use crate::db::models::Receipt;

fn parse_utc_or_now(value: Option<String>) -> chrono::DateTime<Utc> {
    crate::db::oracle::parse_utc_from_opt_string(value)
}

pub(crate) async fn add_receipt(
    pool: &Pool,
    input: &NewReceipt,
    created_at: &str,
) -> anyhow::Result<()> {
    let conn = pool.get().await?;
    let sql = "INSERT INTO receipts (id, donation_id, receipt_key, file_name, content_type, receipt_size, ocr_text, ocr_date, ocr_amount, ocr_status, created_at) VALUES (:1,:2,:3,:4,:5,:6,:7,:8,:9,:10, TO_TIMESTAMP_TZ(:11, 'YYYY-MM-DD\"T\"HH24:MI:SS.FF TZH:TZM'))";
    conn.execute(
        sql,
        &crate::oracle_params![
            input.id.clone(),
            input.donation_id.clone(),
            input.key.clone(),
            input.file_name.clone(),
            input.content_type.clone(),
            input.size,
            Option::<String>::None,
            Option::<String>::None,
            Option::<i64>::None,
            Option::<String>::None,
            created_at.to_string(),
        ],
    )
    .await?;
    conn.commit().await?;
    Ok(())
}

pub(crate) async fn list_receipts(
    pool: &Pool,
    user_id: &str,
    donation_id: Option<String>,
) -> anyhow::Result<Vec<Receipt>> {
    let conn = pool.get().await?;
    let sql = if donation_id.is_some() {
        "SELECT r.id, r.donation_id, r.receipt_key, r.file_name, r.content_type, r.receipt_size, r.ocr_text, r.ocr_date, r.ocr_amount, r.ocr_status, r.created_at FROM receipts r JOIN donations d ON d.id = r.donation_id WHERE d.user_id = :1 AND r.donation_id = :2 AND d.deleted = 0"
    } else {
        "SELECT r.id, r.donation_id, r.receipt_key, r.file_name, r.content_type, r.receipt_size, r.ocr_text, r.ocr_date, r.ocr_amount, r.ocr_status, r.created_at FROM receipts r JOIN donations d ON d.id = r.donation_id WHERE d.user_id = :1 AND d.deleted = 0"
    };
    let rows = if let Some(donation_id) = donation_id {
        conn.query(
            sql,
            &crate::oracle_params![user_id.to_string(), donation_id],
        )
        .await?
    } else {
        conn.query(sql, &crate::oracle_params![user_id.to_string()])
            .await?
    };

    let mut out = Vec::new();
    for row in &rows.rows {
        out.push(Receipt {
            id: crate::db::oracle::row_string(row, 0),
            donation_id: crate::db::oracle::row_string(row, 1),
            key: crate::db::oracle::row_string(row, 2),
            file_name: crate::db::oracle::row_opt_string(row, 3),
            content_type: crate::db::oracle::row_opt_string(row, 4),
            size: crate::db::oracle::row_i64(row, 5),
            ocr_text: crate::db::oracle::row_opt_string(row, 6),
            ocr_date: crate::db::oracle::row_naive_date(row, 7),
            ocr_amount: crate::db::oracle::row_i64(row, 8),
            ocr_status: crate::db::oracle::row_opt_string(row, 9),
            created_at: crate::db::oracle::row_datetime_utc(row, 10).unwrap_or_else(Utc::now),
        });
    }
    Ok(out)
}

pub(crate) async fn list_receipt_summaries(
    pool: &Pool,
    user_id: &str,
    donation_id: Option<String>,
) -> anyhow::Result<Vec<Receipt>> {
    let conn = pool.get().await?;
    let sql = if donation_id.is_some() {
        "SELECT r.id, r.donation_id, r.receipt_key, r.file_name, r.content_type, r.receipt_size, r.created_at FROM receipts r JOIN donations d ON d.id = r.donation_id WHERE d.user_id = :1 AND r.donation_id = :2 AND d.deleted = 0"
    } else {
        "SELECT r.id, r.donation_id, r.receipt_key, r.file_name, r.content_type, r.receipt_size, r.created_at FROM receipts r JOIN donations d ON d.id = r.donation_id WHERE d.user_id = :1 AND d.deleted = 0"
    };
    let rows = if let Some(donation_id) = donation_id {
        conn.query(
            sql,
            &crate::oracle_params![user_id.to_string(), donation_id],
        )
        .await?
    } else {
        conn.query(sql, &crate::oracle_params![user_id.to_string()])
            .await?
    };

    let mut out = Vec::new();
    for row in &rows.rows {
        out.push(Receipt {
            id: crate::db::oracle::row_string(row, 0),
            donation_id: crate::db::oracle::row_string(row, 1),
            key: crate::db::oracle::row_string(row, 2),
            file_name: crate::db::oracle::row_opt_string(row, 3),
            content_type: crate::db::oracle::row_opt_string(row, 4),
            size: crate::db::oracle::row_i64(row, 5),
            ocr_text: None,
            ocr_date: None,
            ocr_amount: None,
            ocr_status: None,
            created_at: crate::db::oracle::row_datetime_utc(row, 6).unwrap_or_else(Utc::now),
        });
    }
    Ok(out)
}

pub(crate) async fn get_receipt(
    pool: &Pool,
    user_id: &str,
    receipt_id: &str,
) -> anyhow::Result<Option<Receipt>> {
    let conn = pool.get().await?;
    let sql = "SELECT r.id, r.donation_id, r.receipt_key, r.file_name, r.content_type, r.receipt_size, r.ocr_text, r.ocr_date, r.ocr_amount, r.ocr_status, r.created_at FROM receipts r JOIN donations d ON d.id = r.donation_id WHERE d.user_id = :1 AND r.id = :2";
    let rows = conn
        .query(
            sql,
            &crate::oracle_params![user_id.to_string(), receipt_id.to_string()],
        )
        .await?;
    let row = rows.first().map(|r| Receipt {
        id: crate::db::oracle::row_string(r, 0),
        donation_id: crate::db::oracle::row_string(r, 1),
        key: crate::db::oracle::row_string(r, 2),
        file_name: crate::db::oracle::row_opt_string(r, 3),
        content_type: crate::db::oracle::row_opt_string(r, 4),
        size: crate::db::oracle::row_i64(r, 5),
        ocr_text: crate::db::oracle::row_opt_string(r, 6),
        ocr_date: crate::db::oracle::row_naive_date(r, 7),
        ocr_amount: crate::db::oracle::row_i64(r, 8),
        ocr_status: crate::db::oracle::row_opt_string(r, 9),
        created_at: crate::db::oracle::row_datetime_utc(r, 10)
            .unwrap_or_else(|| parse_utc_or_now(None)),
    });
    Ok(row)
}

pub(crate) async fn set_receipt_ocr(
    pool: &Pool,
    id: &str,
    text: &Option<String>,
    date: &Option<chrono::NaiveDate>,
    amount: &Option<f64>,
    status: &Option<String>,
) -> anyhow::Result<()> {
    let conn = pool.get().await?;
    let updated_at = chrono::Utc::now().to_rfc3339();
    let sql =
        "UPDATE receipts SET ocr_text = :1, ocr_date = TO_DATE(:2, 'YYYY-MM-DD'), ocr_amount = :3, ocr_status = :4, updated_at = TO_TIMESTAMP_TZ(:5, 'YYYY-MM-DD\"T\"HH24:MI:SS.FF TZH:TZM') WHERE id = :6";
    // Truncate OCR text to 4000 chars (VARCHAR2 limit) to avoid CLOB binding issues with oracle-rs 0.1.7
    let ocr_text_truncated = text.as_deref().map(|t| {
        if t.len() > 4000 {
            t[..4000].to_string()
        } else {
            t.to_string()
        }
    });
    conn.execute(
        sql,
        &crate::oracle_params![
            ocr_text_truncated,
            date.map(|value| value.format("%Y-%m-%d").to_string()),
            *amount,
            status.clone(),
            updated_at,
            id.to_string(),
        ],
    )
    .await?;

    conn.commit().await?;
    Ok(())
}
