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
            let charity_id_for_revision = charity_id.clone();
            let conn = p.get().await?;
            let existing_rows = conn
                .query(
                    "SELECT name, ein, category, status, classification, nonprofit_type, deductibility, street, city, state, zip, created_at, updated_at FROM charities WHERE id = :1 AND user_id = :2",
                    &crate::oracle_params![charity_id.clone(), user_id.clone()],
                )
                .await?;
            let revision_payload = if let Some(existing) = existing_rows.first() {
                let existing_name = crate::db::oracle::row_string(existing, 0);
                let existing_ein = crate::db::oracle::row_opt_string(existing, 1);
                let existing_category = crate::db::oracle::row_opt_string(existing, 2);
                let existing_status = crate::db::oracle::row_opt_string(existing, 3);
                let existing_classification = crate::db::oracle::row_opt_string(existing, 4);
                let existing_nonprofit_type = crate::db::oracle::row_opt_string(existing, 5);
                let existing_deductibility = crate::db::oracle::row_opt_string(existing, 6);
                let existing_street = crate::db::oracle::row_opt_string(existing, 7);
                let existing_city = crate::db::oracle::row_opt_string(existing, 8);
                let existing_state = crate::db::oracle::row_opt_string(existing, 9);
                let existing_zip = crate::db::oracle::row_opt_string(existing, 10);
                let existing_created_at = crate::db::oracle::row_opt_string(existing, 11);
                let existing_updated_at = crate::db::oracle::row_opt_string(existing, 12);

                let sql = "UPDATE charities SET name = :1, ein = :2, category = :3, status = :4, classification = :5, nonprofit_type = :6, deductibility = :7, street = :8, city = :9, state = :10, zip = :11, updated_at = TO_TIMESTAMP_TZ(:12, 'YYYY-MM-DD\"T\"HH24:MI:SS.FF TZH:TZM') WHERE id = :13 AND user_id = :14";
                if let Err(e) = conn
                    .execute(
                        sql,
                        &crate::oracle_params![
                            name.clone(),
                            ein_cloned.clone(),
                            category_cloned.clone(),
                            status_cloned.clone(),
                            classification_cloned.clone(),
                            nonprofit_type_cloned.clone(),
                            deductibility_cloned.clone(),
                            street_cloned.clone(),
                            city_cloned.clone(),
                            state_cloned.clone(),
                            zip_cloned.clone(),
                            updated_at_str.clone(),
                            charity_id.clone(),
                            user_id.clone(),
                        ],
                    )
                    .await
                {
                    tracing::error!("Failed to update charity: {}. SQL: {}", e, sql);
                    return Err(anyhow::anyhow!("Charity update failed: {}", e));
                }
                conn.commit().await?;
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
                Some((old_values, new_values))
            } else {
                None
            };
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
            let conn = p.get().await?;
            let sql = "SELECT COUNT(1) FROM donations WHERE user_id = :1 AND charity_id = :2 AND deleted = 0";
            let rows = conn
                .query(
                    sql,
                    &crate::oracle_params![user_id.to_string(), charity_id.to_string()],
                )
                .await?;
            Ok(rows
                .first()
                .and_then(|row| crate::db::oracle::row_i64(row, 0))
                .unwrap_or(0))
        }
    }
}

pub async fn delete_charity(pool: &DbPool, user_id: &str, charity_id: &str) -> anyhow::Result<bool> {
    match &**pool {
        DbPoolEnum::Oracle(p) => {
            let conn = p.get().await?;
            let active_donations = conn
                .query(
                    "SELECT 1 FROM donations WHERE user_id = :1 AND charity_id = :2 AND deleted = 0 FETCH FIRST 1 ROWS ONLY",
                    &crate::oracle_params![user_id.to_string(), charity_id.to_string()],
                )
                .await?;
            if active_donations.first().is_some() {
                return Ok(false);
            }
        }
    }

    let user_for_revision = Some(user_id.to_string());
    match &**pool {
        DbPoolEnum::Oracle(p) => {
            let conn = p.get().await?;
            let user_id = user_id.to_string();
            let charity_id = charity_id.to_string();
            let charity_id_for_revision = charity_id.clone();
            let existing_rows = conn
                .query(
                    "SELECT name, ein, category, status, classification, nonprofit_type, deductibility, street, city, state, zip, created_at, updated_at FROM charities WHERE id = :1 AND user_id = :2",
                    &crate::oracle_params![charity_id.clone(), user_id.clone()],
                )
                .await?;
            let revision_payload = if let Some(existing) = existing_rows.first() {
                let existing_name = crate::db::oracle::row_string(existing, 0);
                let existing_ein = crate::db::oracle::row_opt_string(existing, 1);
                let existing_category = crate::db::oracle::row_opt_string(existing, 2);
                let existing_status = crate::db::oracle::row_opt_string(existing, 3);
                let existing_classification = crate::db::oracle::row_opt_string(existing, 4);
                let existing_nonprofit_type = crate::db::oracle::row_opt_string(existing, 5);
                let existing_deductibility = crate::db::oracle::row_opt_string(existing, 6);
                let existing_street = crate::db::oracle::row_opt_string(existing, 7);
                let existing_city = crate::db::oracle::row_opt_string(existing, 8);
                let existing_state = crate::db::oracle::row_opt_string(existing, 9);
                let existing_zip = crate::db::oracle::row_opt_string(existing, 10);
                let existing_created_at = crate::db::oracle::row_opt_string(existing, 11);
                let existing_updated_at = crate::db::oracle::row_opt_string(existing, 12);

                let del_receipts_sql = "DELETE FROM receipts WHERE donation_id IN (SELECT id FROM donations WHERE charity_id = :1 AND user_id = :2 AND deleted = 1)";
                if let Err(e) = conn
                    .execute(
                        del_receipts_sql,
                        &crate::oracle_params![charity_id.clone(), user_id.clone()],
                    )
                    .await
                {
                    tracing::error!("Failed to delete receipts for soft-deleted donations on charity {}: {}", charity_id, e);
                    return Err(anyhow::anyhow!("Failed to clean up associated receipts: {}", e));
                }

                let del_donations_sql = "DELETE FROM donations WHERE charity_id = :1 AND user_id = :2 AND deleted = 1";
                if let Err(e) = conn
                    .execute(
                        del_donations_sql,
                        &crate::oracle_params![charity_id.clone(), user_id.clone()],
                    )
                    .await
                {
                    tracing::error!("Failed to delete soft-deleted donations for charity {}: {}", charity_id, e);
                    return Err(anyhow::anyhow!("Failed to clean up associated donations: {}", e));
                }

                let sql = "DELETE FROM charities WHERE id = :1 AND user_id = :2";
                if let Err(e) = conn
                    .execute(sql, &crate::oracle_params![charity_id.clone(), user_id.clone()])
                    .await
                {
                    tracing::error!("Failed to delete charity {}: {}", charity_id, e);
                    return Err(anyhow::anyhow!("Charity delete failed: {}", e));
                }
                conn.commit().await?;
                Some(json!({
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
                }).to_string())
            } else {
                None
            };
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