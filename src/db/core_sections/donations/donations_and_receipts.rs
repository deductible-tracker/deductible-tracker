struct DonationRevisionSnapshot {
    donation_id: String,
    user_id: String,
    donation_date: String,
    donation_year: i32,
    donation_category: Option<String>,
    donation_amount: Option<f64>,
    charity_id: String,
    notes: Option<String>,
    is_encrypted: Option<bool>,
    encrypted_payload: Option<String>,
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
        "is_encrypted": snapshot.is_encrypted,
        "encrypted_payload": snapshot.encrypted_payload,
        "deleted": snapshot.deleted,
        "updated_at": snapshot.updated_at,
    })
    .to_string()
}

async fn donation_owner_user_id(pool: &DbPool, donation_id: &str) -> anyhow::Result<Option<String>> {
    match &**pool {
        DbPoolEnum::Oracle(p) => {
            let conn = p.get().await?;
            let rows = conn
                .query(
                    "SELECT user_id FROM donations WHERE id = :1",
                    &crate::oracle_params![donation_id.to_string()],
                )
                .await?;
            let user_id = rows.first().map(|row| crate::db::oracle::row_string(row, 0));
            Ok(user_id)
        }
    }
}

pub async fn user_owns_donation(pool: &DbPool, user_id: &str, donation_id: &str) -> anyhow::Result<bool> {
    match &**pool {
        DbPoolEnum::Oracle(p) => {
            let conn = p.get().await?;
            let rows = conn
                .query(
                    "SELECT 1 FROM donations WHERE id = :1 AND user_id = :2",
                    &crate::oracle_params![donation_id.to_string(), user_id.to_string()],
                )
                .await?;
            Ok(rows.first().is_some())
        }
    }
}

pub async fn list_receipts(pool: &DbPool, user_id: &str, donation_id: Option<String>) -> anyhow::Result<Vec<crate::db::models::Receipt>> {
    match &**pool {
        DbPoolEnum::Oracle(p) => crate::db::oracle::receipts::list_receipts(p, user_id, donation_id).await,
    }
}

pub async fn list_receipt_summaries(pool: &DbPool, user_id: &str, donation_id: Option<String>) -> anyhow::Result<Vec<crate::db::models::Receipt>> {
    match &**pool {
        DbPoolEnum::Oracle(p) => crate::db::oracle::receipts::list_receipt_summaries(p, user_id, donation_id).await,
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

pub async fn list_donation_years(pool: &DbPool, user_id: &str) -> anyhow::Result<Vec<i32>> {
    match &**pool {
        DbPoolEnum::Oracle(p) => crate::db::oracle::donations::list_donation_years(p, user_id).await,
    }
}

pub async fn soft_delete_donation(pool: &DbPool, user_id: &str, donation_id: &str) -> anyhow::Result<bool> {
    let user_for_revision = Some(user_id.to_string());
    match &**pool {
        DbPoolEnum::Oracle(p) => {
            let conn = p.get().await?;
            let user_id = user_id.to_string();
            let donation_id = donation_id.to_string();
            let donation_id_for_revision = donation_id.clone();
            let updated_at = chrono::Utc::now().to_rfc3339();
            let existing_rows = conn
                .query(
                    "SELECT donation_date, donation_year, donation_category, donation_amount, charity_id, notes, deleted, is_encrypted, encrypted_payload FROM donations WHERE id = :1 AND user_id = :2",
                    &crate::oracle_params![donation_id.clone(), user_id.clone()],
                )
                .await?;
            let Some(existing) = existing_rows.first() else {
                return Ok(false);
            };

            let existing_date = crate::db::oracle::row_naive_date(existing, 0)
                .map(|value| value.format("%Y-%m-%d").to_string())
                .unwrap_or_default();
            let existing_year = crate::db::oracle::row_i64(existing, 1).unwrap_or(0) as i32;
            let existing_category = crate::db::oracle::row_opt_string(existing, 2);
            let existing_amount = crate::db::oracle::row_f64(existing, 3);
            let existing_charity_id = crate::db::oracle::row_string(existing, 4);
            let existing_notes = crate::db::oracle::row_opt_string(existing, 5);
            let existing_deleted = crate::db::oracle::row_bool(existing, 6).unwrap_or(false);
            let existing_is_encrypted = crate::db::oracle::row_bool(existing, 7);
            let existing_encrypted_payload = crate::db::oracle::row_opt_string(existing, 8);

            let sql = "UPDATE donations SET deleted = 1, updated_at = TO_TIMESTAMP_TZ(:1, 'YYYY-MM-DD\"T\"HH24:MI:SS.FF TZH:TZM') WHERE id = :2 AND user_id = :3";
            if let Err(e) = conn
                .execute(
                    sql,
                    &crate::oracle_params![updated_at.clone(), donation_id.clone(), user_id.clone()],
                )
                .await
            {
                tracing::error!("Failed to soft delete donation: {}. SQL: {}", e, sql);
                return Err(anyhow::anyhow!("Donation soft delete failed: {}", e));
            }

            if let Err(e) = conn
                .execute(
                    "DELETE FROM receipts WHERE donation_id = :1",
                    &crate::oracle_params![donation_id.clone()],
                )
                .await
            {
                tracing::error!("Failed to cascade delete receipts for donation {}: {}", donation_id, e);
                return Err(anyhow::anyhow!("Cascade delete receipts failed: {}", e));
            }

            conn.commit().await?;
            let revision_payload = {
                let old_values = build_donation_revision_json(&DonationRevisionSnapshot {
                    donation_id: donation_id.clone(),
                    user_id: user_id.clone(),
                    donation_date: existing_date.clone(),
                    donation_year: existing_year,
                    donation_category: existing_category.clone(),
                    donation_amount: existing_amount,
                    charity_id: existing_charity_id.clone(),
                    notes: existing_notes.clone(),
                    is_encrypted: existing_is_encrypted,
                    encrypted_payload: existing_encrypted_payload.clone(),
                    deleted: existing_deleted,
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
                    is_encrypted: existing_is_encrypted,
                    encrypted_payload: existing_encrypted_payload,
                    deleted: true,
                    updated_at: Some(updated_at.clone()),
                });
                Some((old_values, new_values))
            };
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

