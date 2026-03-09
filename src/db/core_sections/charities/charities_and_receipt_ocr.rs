pub async fn list_charities(pool: &DbPool, user_id: &str) -> anyhow::Result<Vec<crate::db::models::Charity>> {
    match &**pool {
        DbPoolEnum::Oracle(p) => {
            let p = p.clone();
            let user_id = user_id.to_string();
            let rows = task::spawn_blocking(move || -> anyhow::Result<Vec<crate::db::models::Charity>> {
                let conn = p.get()?;
                let parse_utc = |value: Option<String>| {
                    value
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(chrono::Utc::now)
                };
                let sql = "SELECT id, user_id, name, ein, created_at, updated_at, nonprofit_type, deductibility, street, city, state, zip, category, status, classification FROM charities WHERE user_id = :1";
                let rows = conn.query(sql, &[&user_id])?;
                let mut out = Vec::new();
                for row in rows.flatten() {
                    let c = crate::db::models::Charity {
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
                    };
                    out.push(c);
                }
                Ok(out)
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
            Ok(rows)
        }
    }
}

pub async fn set_receipt_ocr(
    pool: &DbPool,
    receipt_id: &str,
    ocr_text: &Option<String>,
    ocr_date: &Option<chrono::NaiveDate>,
    ocr_amount: &Option<i64>,
    ocr_status: &Option<String>,
) -> anyhow::Result<bool> {
    let user_for_revision = receipt_owner_user_id(pool, receipt_id).await?;
    match &**pool {
        DbPoolEnum::Oracle(p) => {
            let p = p.clone();
            let receipt_id = receipt_id.to_string();
            let receipt_id_for_revision = receipt_id.clone();
            let text = ocr_text.clone();
            let o_date = ocr_date.map(|d| d.to_string());
            let amt = *ocr_amount;
            let status = ocr_status.clone();
            let revision_payload = task::spawn_blocking(move || -> anyhow::Result<Option<(String, String)>> {
                let conn = p.get()?;
                let mut existing_rows = conn.query("SELECT donation_id, receipt_key, file_name, content_type, receipt_size, ocr_text, ocr_date, ocr_amount, ocr_status, created_at FROM receipts WHERE id = :1", &[&receipt_id])?;
                let Some(existing) = existing_rows.next().transpose()? else {
                    return Ok(None);
                };

                let existing_donation_id: String = existing.get(0).unwrap_or_default();
                let existing_key: String = existing.get(1).unwrap_or_default();
                let existing_file_name: Option<String> = existing.get(2).ok();
                let existing_content_type: Option<String> = existing.get(3).ok();
                let existing_size: Option<i64> = existing.get(4).ok();
                let existing_ocr_text: Option<String> = existing.get(5).ok();
                let existing_ocr_date: Option<String> = existing.get(6).ok();
                let existing_ocr_amount: Option<i64> = existing.get(7).ok();
                let existing_ocr_status: Option<String> = existing.get(8).ok();
                let existing_created_at: Option<String> = existing.get(9).ok();

                let sql = "UPDATE receipts SET ocr_text = :1, ocr_date = :2, ocr_amount = :3, ocr_status = :4, updated_at = TO_TIMESTAMP_TZ(:5, 'YYYY-MM-DD\"T\"HH24:MI:SS.FF TZH:TZM') WHERE id = :6";
                let updated_at_str = chrono::Utc::now().to_rfc3339();
                if let Err(e) = conn.execute(sql, &[&text, &o_date, &amt, &status, &updated_at_str, &receipt_id]) {
                    tracing::error!("Failed to update receipt OCR: {}. SQL: {}", e, sql);
                    return Err(anyhow::anyhow!("Receipt OCR update failed: {}", e));
                }
                let _ = conn.commit();
                let old_values = json!({
                    "id": receipt_id,
                    "donation_id": existing_donation_id,
                    "key": existing_key,
                    "file_name": existing_file_name,
                    "content_type": existing_content_type,
                    "size": existing_size,
                    "ocr_text": existing_ocr_text,
                    "ocr_date": existing_ocr_date,
                    "ocr_amount": existing_ocr_amount,
                    "ocr_status": existing_ocr_status,
                    "created_at": existing_created_at
                }).to_string();
                let new_values = json!({
                    "id": receipt_id,
                    "donation_id": existing_donation_id,
                    "key": existing_key,
                    "file_name": existing_file_name,
                    "content_type": existing_content_type,
                    "size": existing_size,
                    "ocr_text": text,
                    "ocr_date": o_date,
                    "ocr_amount": amt,
                    "ocr_status": status,
                    "created_at": existing_created_at
                }).to_string();
                Ok(Some((old_values, new_values)))
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
            if let Some((old_values, new_values)) = revision_payload {
                let revision = RevisionLogEntry {
                    id: Uuid::new_v4().to_string(),
                    user_id: user_for_revision,
                    table_name: "receipts".to_string(),
                    record_id: receipt_id_for_revision,
                    operation: "update".to_string(),
                    old_values: Some(old_values),
                    new_values: Some(new_values),
                };
                log_revision(pool, &revision).await?;
                Ok(true)
            } else {
                Ok(false)
            }
        }
    }
}

pub async fn log_audit(
    pool: &DbPool,
    id: &str,
    user_id: &str,
    action: &str,
    table_name: &str,
    record_id: &Option<String>,
    details: &Option<String>,
) -> anyhow::Result<()> {
    let id = id.to_string();
    let user_id = user_id.to_string();
    let action = action.to_string();
    let table_name = table_name.to_string();
    let record_id_cloned = record_id.clone();
    let details_cloned = details.clone();
    let created_at = chrono::Utc::now().to_rfc3339();
    match &**pool {
        DbPoolEnum::Oracle(p) => {
            let p = p.clone();
            task::spawn_blocking(move || -> anyhow::Result<()> {
                let conn = p.get()?;
                let sql = "INSERT INTO audit_logs (id, user_id, action, table_name, record_id, details, created_at) VALUES (:1,:2,:3,:4,:5,:6, TO_TIMESTAMP_TZ(:7, 'YYYY-MM-DD\"T\"HH24:MI:SS.FF TZH:TZM'))";
                conn.execute(sql, &[&id, &user_id, &action, &table_name, &record_id_cloned, &details_cloned, &created_at])?;
                let _ = conn.commit();
                Ok(())
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
            Ok(())
        }
    }
}

