pub async fn update_charity(pool: &DbPool, patch: &crate::db::models::CharityPatch) -> anyhow::Result<bool> {
    let patch = patch.clone();
    let charity_id = patch.charity_id.clone();
    let user_id = patch.user_id.clone();
    let name = patch.name.clone();
    let ein_cloned = patch.ein.clone();
    let category_cloned = patch.category.clone();
    let status_cloned = patch.status.clone();
    let classification_cloned = patch.classification.clone();
    let nonprofit_type_cloned = patch.nonprofit_type.clone();
    let deductibility_cloned = patch.deductibility.clone();
    let street_cloned = patch.street.clone();
    let city_cloned = patch.city.clone();
    let state_cloned = patch.state.clone();
    let zip_cloned = patch.zip.clone();
    let updated_at_str = patch.updated_at.to_rfc3339();
    let user_for_revision = Some(user_id.clone());

    match &**pool {
        DbPoolEnum::Oracle(p) => {
            let p = p.clone();
            let charity_id_for_revision = charity_id.clone();
            let revision_payload = task::spawn_blocking(move || -> anyhow::Result<Option<(String, String)>> {
                let conn = p.get()?;
                let mut existing_rows = conn.query("SELECT name, ein, category, status, classification, nonprofit_type, deductibility, street, city, state, zip, created_at, updated_at FROM charities WHERE id = :1 AND user_id = :2", &[&charity_id, &user_id])?;
                let Some(existing) = existing_rows.next().transpose()? else {
                    return Ok(None);
                };
                let existing_name: String = existing.get(0).unwrap_or_default();
                let existing_ein: Option<String> = existing.get(1).ok();
                let existing_category: Option<String> = existing.get(2).ok();
                let existing_status: Option<String> = existing.get(3).ok();
                let existing_classification: Option<String> = existing.get(4).ok();
                let existing_nonprofit_type: Option<String> = existing.get(5).ok();
                let existing_deductibility: Option<String> = existing.get(6).ok();
                let existing_street: Option<String> = existing.get(7).ok();
                let existing_city: Option<String> = existing.get(8).ok();
                let existing_state: Option<String> = existing.get(9).ok();
                let existing_zip: Option<String> = existing.get(10).ok();
                let existing_created_at: Option<String> = existing.get(11).ok();
                let existing_updated_at: Option<String> = existing.get(12).ok();

                let sql = "UPDATE charities SET name = :1, ein = :2, category = :3, status = :4, classification = :5, nonprofit_type = :6, deductibility = :7, street = :8, city = :9, state = :10, zip = :11, updated_at = TO_TIMESTAMP_TZ(:12, 'YYYY-MM-DD\"T\"HH24:MI:SS.FF TZH:TZM') WHERE id = :13 AND user_id = :14";
                if let Err(e) = conn.execute(sql, &[&name, &ein_cloned, &category_cloned, &status_cloned, &classification_cloned, &nonprofit_type_cloned, &deductibility_cloned, &street_cloned, &city_cloned, &state_cloned, &zip_cloned, &updated_at_str, &charity_id, &user_id]) {
                    tracing::error!("Failed to update charity: {}. SQL: {}", e, sql);
                    return Err(anyhow::anyhow!("Charity update failed: {}", e));
                }
                let _ = conn.commit();
                let old_values = json!({
                    "id": charity_id,
                    "user_id": user_id,
                    "name": existing_name,
                    "ein": existing_ein,
                    "category": existing_category,
                    "status": existing_status,
                    "classification": existing_classification,
                    "nonprofit_type": existing_nonprofit_type,
                    "deductibility": existing_deductibility,
                    "street": existing_street,
                    "city": existing_city,
                    "state": existing_state,
                    "zip": existing_zip,
                    "created_at": existing_created_at,
                    "updated_at": existing_updated_at
                }).to_string();
                let new_values = json!({
                    "id": charity_id,
                    "user_id": user_id,
                    "name": name,
                    "ein": ein_cloned,
                    "category": category_cloned,
                    "status": status_cloned,
                    "classification": classification_cloned,
                    "nonprofit_type": nonprofit_type_cloned,
                    "deductibility": deductibility_cloned,
                    "street": street_cloned,
                    "city": city_cloned,
                    "state": state_cloned,
                    "zip": zip_cloned,
                    "created_at": existing_created_at,
                    "updated_at": updated_at_str
                }).to_string();
                Ok(Some((old_values, new_values)))
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
            if let Some((old_values, new_values)) = revision_payload {
                let revision = RevisionLogEntry {
                    id: Uuid::new_v4().to_string(),
                    user_id: user_for_revision,
                    table_name: "charities".to_string(),
                    record_id: charity_id_for_revision,
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

pub async fn count_donations_for_charity(pool: &DbPool, user_id: &str, charity_id: &str) -> anyhow::Result<i64> {
    match &**pool {
        DbPoolEnum::Oracle(p) => {
            let p = p.clone();
            let user_id = user_id.to_string();
            let charity_id = charity_id.to_string();
            let count = task::spawn_blocking(move || -> anyhow::Result<i64> {
                let conn = p.get()?;
                let sql = "SELECT COUNT(1) FROM donations WHERE user_id = :1 AND charity_id = :2 AND deleted = 0";
                let mut rows = conn.query(sql, &[&user_id, &charity_id])?;
                if let Some(row) = rows.next().transpose()? {
                    let val: i64 = row.get(0).unwrap_or(0);
                    return Ok(val);
                }
                Ok(0)
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
            Ok(count)
        }
    }
}

pub async fn delete_charity(pool: &DbPool, user_id: &str, charity_id: &str) -> anyhow::Result<bool> {
    let count = count_donations_for_charity(pool, user_id, charity_id).await?;
    if count > 0 {
        return Ok(false);
    }

    let user_for_revision = Some(user_id.to_string());
    match &**pool {
        DbPoolEnum::Oracle(p) => {
            let p = p.clone();
            let user_id = user_id.to_string();
            let charity_id = charity_id.to_string();
            let charity_id_for_revision = charity_id.clone();
            let revision_payload = task::spawn_blocking(move || -> anyhow::Result<Option<String>> {
                let conn = p.get()?;
                let mut existing_rows = conn.query("SELECT name, ein, category, status, classification, nonprofit_type, deductibility, street, city, state, zip, created_at, updated_at FROM charities WHERE id = :1 AND user_id = :2", &[&charity_id, &user_id])?;
                let Some(existing) = existing_rows.next().transpose()? else {
                    return Ok(None);
                };
                let existing_name: String = existing.get(0).unwrap_or_default();
                let existing_ein: Option<String> = existing.get(1).ok();
                let existing_category: Option<String> = existing.get(2).ok();
                let existing_status: Option<String> = existing.get(3).ok();
                let existing_classification: Option<String> = existing.get(4).ok();
                let existing_nonprofit_type: Option<String> = existing.get(5).ok();
                let existing_deductibility: Option<String> = existing.get(6).ok();
                let existing_street: Option<String> = existing.get(7).ok();
                let existing_city: Option<String> = existing.get(8).ok();
                let existing_state: Option<String> = existing.get(9).ok();
                let existing_zip: Option<String> = existing.get(10).ok();
                let existing_created_at: Option<String> = existing.get(11).ok();
                let existing_updated_at: Option<String> = existing.get(12).ok();

                let sql = "DELETE FROM charities WHERE id = :1 AND user_id = :2";
                conn.execute(sql, &[&charity_id, &user_id])?;
                let _ = conn.commit();
                let old_values = json!({
                    "id": charity_id,
                    "user_id": user_id,
                    "name": existing_name,
                    "ein": existing_ein,
                    "category": existing_category,
                    "status": existing_status,
                    "classification": existing_classification,
                    "nonprofit_type": existing_nonprofit_type,
                    "deductibility": existing_deductibility,
                    "street": existing_street,
                    "city": existing_city,
                    "state": existing_state,
                    "zip": existing_zip,
                    "created_at": existing_created_at,
                    "updated_at": existing_updated_at
                }).to_string();
                Ok(Some(old_values))
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
            if let Some(old_values) = revision_payload {
                let revision = RevisionLogEntry {
                    id: Uuid::new_v4().to_string(),
                    user_id: user_for_revision,
                    table_name: "charities".to_string(),
                    record_id: charity_id_for_revision,
                    operation: "delete".to_string(),
                    old_values: Some(old_values),
                    new_values: None,
                };
                log_revision(pool, &revision).await?;
                Ok(true)
            } else {
                Ok(false)
            }
        }
    }
}