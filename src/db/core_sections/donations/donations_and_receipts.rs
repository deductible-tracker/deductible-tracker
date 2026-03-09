struct DonationRevisionSnapshot {
    donation_id: String,
    user_id: String,
    donation_date: String,
    donation_year: i32,
    donation_category: Option<String>,
    donation_amount: Option<f64>,
    charity_id: String,
    notes: Option<String>,
    deleted: bool,
    updated_at: Option<String>,
}

fn build_donation_revision_json(snapshot: &DonationRevisionSnapshot) -> String {
    json!({
        "id": snapshot.donation_id,
        "user_id": snapshot.user_id,
        "donation_date": snapshot.donation_date,
        "donation_year": snapshot.donation_year,
        "donation_category": snapshot.donation_category,
        "donation_amount": snapshot.donation_amount,
        "charity_id": snapshot.charity_id,
        "notes": snapshot.notes,
        "deleted": snapshot.deleted,
        "updated_at": snapshot.updated_at,
    })
    .to_string()
}

async fn donation_owner_user_id(pool: &DbPool, donation_id: &str) -> anyhow::Result<Option<String>> {
    match &**pool {
        DbPoolEnum::Oracle(p) => {
            let p = p.clone();
            let donation_id = donation_id.to_string();
            let user_id = task::spawn_blocking(move || -> anyhow::Result<Option<String>> {
                let conn = p.get()?;
                let mut rows = conn.query("SELECT user_id FROM donations WHERE id = :1", &[&donation_id])?;
                if let Some(row) = rows.next().transpose()? {
                    let user_id: String = row.get(0).unwrap_or_default();
                    if user_id.is_empty() {
                        return Ok(None);
                    }
                    return Ok(Some(user_id));
                }
                Ok(None)
            })
            .await
            .map_err(|e| anyhow!("DB task join error: {}", e))??;
            Ok(user_id)
        }
    }
}

async fn receipt_owner_user_id(pool: &DbPool, receipt_id: &str) -> anyhow::Result<Option<String>> {
    match &**pool {
        DbPoolEnum::Oracle(p) => {
            let p = p.clone();
            let receipt_id = receipt_id.to_string();
            let user_id = task::spawn_blocking(move || -> anyhow::Result<Option<String>> {
                let conn = p.get()?;
                let mut rows = conn.query(
                    "SELECT d.user_id FROM receipts r JOIN donations d ON d.id = r.donation_id WHERE r.id = :1",
                    &[&receipt_id],
                )?;
                if let Some(row) = rows.next().transpose()? {
                    let user_id: String = row.get(0).unwrap_or_default();
                    if user_id.is_empty() {
                        return Ok(None);
                    }
                    return Ok(Some(user_id));
                }
                Ok(None)
            })
            .await
            .map_err(|e| anyhow!("DB task join error: {}", e))??;
            Ok(user_id)
        }
    }
}

pub async fn user_owns_donation(pool: &DbPool, user_id: &str, donation_id: &str) -> anyhow::Result<bool> {
    match &**pool {
        DbPoolEnum::Oracle(p) => {
            let p = p.clone();
            let user_id = user_id.to_string();
            let donation_id = donation_id.to_string();
            let exists = task::spawn_blocking(move || -> anyhow::Result<bool> {
                let conn = p.get()?;
                let mut rows = conn.query("SELECT COUNT(1) FROM donations WHERE id = :1 AND user_id = :2", &[&donation_id, &user_id])?;
                if let Some(row) = rows.next().transpose()? {
                    let count: i64 = row.get(0).unwrap_or(0);
                    return Ok(count > 0);
                }
                Ok(false)
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
            Ok(exists)
        }
    }
}

pub async fn list_receipts(pool: &DbPool, user_id: &str, donation_id: Option<String>) -> anyhow::Result<Vec<crate::db::models::Receipt>> {
    match &**pool {
        DbPoolEnum::Oracle(p) => crate::db::oracle::receipts::list_receipts(p, user_id, donation_id).await,
    }
}

pub async fn get_receipt(pool: &DbPool, user_id: &str, receipt_id: &str) -> anyhow::Result<Option<crate::db::models::Receipt>> {
    match &**pool {
        DbPoolEnum::Oracle(p) => crate::db::oracle::receipts::get_receipt(p, user_id, receipt_id).await,
    }
}

pub async fn list_donations(pool: &DbPool, user_id: &str, year: Option<i32>) -> anyhow::Result<Vec<DonationModel>> {
    match &**pool {
        DbPoolEnum::Oracle(p) => crate::db::oracle::donations::list_donations(p, user_id, year).await,
    }
}

pub async fn soft_delete_donation(pool: &DbPool, user_id: &str, donation_id: &str) -> anyhow::Result<bool> {
    let user_for_revision = Some(user_id.to_string());
    match &**pool {
        DbPoolEnum::Oracle(p) => {
            let p = p.clone();
            let user_id = user_id.to_string();
            let donation_id = donation_id.to_string();
            let donation_id_for_revision = donation_id.clone();
            let updated_at = chrono::Utc::now().to_rfc3339();
            let revision_payload = task::spawn_blocking(move || -> anyhow::Result<Option<(String, String)>> {
                let conn = p.get()?;
                let mut existing_rows = conn.query(
                    "SELECT donation_date, donation_year, donation_category, donation_amount, charity_id, notes, deleted FROM donations WHERE id = :1 AND user_id = :2",
                    &[&donation_id, &user_id],
                )?;
                let Some(existing) = existing_rows.next().transpose()? else {
                    return Ok(None);
                };

                let existing_date = existing
                    .get::<usize, chrono::NaiveDate>(0)
                    .ok()
                    .map(|d| d.format("%Y-%m-%d").to_string())
                    .unwrap_or_default();
                let existing_year: i32 = existing.get(1).unwrap_or(0);
                let existing_category: Option<String> = existing.get(2).ok();
                let existing_amount: Option<f64> = existing.get(3).ok();
                let existing_charity_id: String = existing.get(4).unwrap_or_default();
                let existing_notes: Option<String> = existing.get(5).ok();
                let existing_deleted: i64 = existing.get(6).unwrap_or(0);

                let sql = "UPDATE donations SET deleted = 1, updated_at = TO_TIMESTAMP_TZ(:1, 'YYYY-MM-DD\"T\"HH24:MI:SS.FF TZH:TZM') WHERE id = :2 AND user_id = :3";
                if let Err(e) = conn.execute(sql, &[&updated_at, &donation_id, &user_id]) {
                    tracing::error!("Failed to soft delete donation: {}. SQL: {}", e, sql);
                    return Err(anyhow::anyhow!("Donation soft delete failed: {}", e));
                }

                // Cascade-delete associated receipts
                if let Err(e) = conn.execute("DELETE FROM receipts WHERE donation_id = :1", &[&donation_id]) {
                    tracing::error!("Failed to cascade delete receipts for donation {}: {}", donation_id, e);
                    return Err(anyhow::anyhow!("Cascade delete receipts failed: {}", e));
                }

                let _ = conn.commit();
                let mut cnt_rows = conn.query("SELECT COUNT(1) FROM donations WHERE id = :1 AND user_id = :2 AND deleted = 1", &[&donation_id, &user_id])?;
                if let Some(r) = cnt_rows.next().transpose()? {
                    let cnt: i64 = r.get(0).unwrap_or(0);
                    if cnt > 0 {
                        let old_values = build_donation_revision_json(&DonationRevisionSnapshot {
                            donation_id: donation_id.clone(),
                            user_id: user_id.clone(),
                            donation_date: existing_date.clone(),
                            donation_year: existing_year,
                            donation_category: existing_category.clone(),
                            donation_amount: existing_amount,
                            charity_id: existing_charity_id.clone(),
                            notes: existing_notes.clone(),
                            deleted: existing_deleted == 1,
                            updated_at: None,
                        });
                        let new_values = build_donation_revision_json(&DonationRevisionSnapshot {
                            donation_id: donation_id.clone(),
                            user_id: user_id.clone(),
                            donation_date: existing_date,
                            donation_year: existing_year,
                            donation_category: existing_category,
                            donation_amount: existing_amount,
                            charity_id: existing_charity_id,
                            notes: existing_notes,
                            deleted: true,
                            updated_at: Some(updated_at.clone()),
                        });
                        return Ok(Some((old_values, new_values)));
                    }
                }
                Ok(None)
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
            if let Some((old_values, new_values)) = revision_payload {
                let revision = RevisionLogEntry {
                    id: Uuid::new_v4().to_string(),
                    user_id: user_for_revision,
                    table_name: "donations".to_string(),
                    record_id: donation_id_for_revision,
                    operation: "delete".to_string(),
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

