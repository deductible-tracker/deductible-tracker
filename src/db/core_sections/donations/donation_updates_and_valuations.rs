pub async fn update_donation(pool: &DbPool, patch: &crate::db::models::DonationPatch) -> anyhow::Result<bool> {
    let patch = patch.clone();
    let category_owned = patch.category_opt.clone();
    let charity_id_owned = patch.charity_id_opt.clone();
    let user_for_revision = Some(patch.user_id.clone());

    match &**pool {
        DbPoolEnum::Oracle(p) => {
            let p = p.clone();
            let user_id = patch.user_id.clone();
            let donation_id = patch.donation_id.clone();
            let donation_id_for_revision = donation_id.clone();
            let incoming = patch.incoming_updated_at.map(|d| d.to_rfc3339());
            let date_opt = patch.date_opt;
            let year_opt = patch.year_opt;
            let amount_opt = patch.amount_opt;
            let notes_cloned = patch.notes.clone();
            let revision_payload = task::spawn_blocking(move || -> anyhow::Result<Option<(String, String)>> {
                let conn = p.get()?;
                // fetch existing row
                let mut rows = conn.query("SELECT donation_date, donation_year, donation_category, donation_amount, charity_id, notes, updated_at FROM donations WHERE id = :1 AND user_id = :2", &[&donation_id, &user_id])?;
                if let Some(row) = rows.next().transpose()? {
                    let existing_updated: Option<String> = row.get(6).ok();
                    if let (Some(inc), Some(ex)) = (incoming.clone(), existing_updated.clone()) {
                        if inc <= ex { return Ok(None); }
                    }

                    // determine new values
                    let existing_date: Option<chrono::NaiveDate> = row.get(0).ok();
                    let existing_year: Option<i32> = row.get(1).ok();
                    let existing_category: Option<String> = row.get(2).ok();
                    let existing_amount: Option<f64> = row.get(3).ok();
                    let existing_charity_id: Option<String> = row.get(4).ok();
                    let existing_notes: Option<String> = row.get(5).ok();

                    let new_date = date_opt.unwrap_or(existing_date.unwrap_or_else(|| chrono::Utc::now().date_naive()));
                    let new_year = year_opt.unwrap_or(existing_year.unwrap_or(new_date.year()));
                    let new_category = category_owned.clone().or(existing_category.clone());
                    let new_amount = amount_opt.or(existing_amount);
                    let new_charity_id = charity_id_owned.clone().or(existing_charity_id.clone()).unwrap_or_default();
                    let new_notes = notes_cloned.clone().or(existing_notes.clone());
                    let new_updated_at = incoming.unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

                    let existing_date_str = existing_date
                        .map(|d| d.format("%Y-%m-%d").to_string())
                        .unwrap_or_else(|| chrono::Utc::now().date_naive().format("%Y-%m-%d").to_string());
                    let old_values = build_donation_revision_json(&DonationRevisionSnapshot {
                        donation_id: donation_id.clone(),
                        user_id: user_id.clone(),
                        donation_date: existing_date_str,
                        donation_year: existing_year.unwrap_or(0),
                        donation_category: existing_category.clone(),
                        donation_amount: existing_amount,
                        charity_id: existing_charity_id.clone().unwrap_or_default(),
                        notes: existing_notes.clone(),
                        deleted: false,
                        updated_at: existing_updated,
                    });
                    let new_values = build_donation_revision_json(&DonationRevisionSnapshot {
                        donation_id: donation_id.clone(),
                        user_id: user_id.clone(),
                        donation_date: new_date.format("%Y-%m-%d").to_string(),
                        donation_year: new_year,
                        donation_category: new_category.clone(),
                        donation_amount: new_amount,
                        charity_id: new_charity_id.clone(),
                        notes: new_notes.clone(),
                        deleted: false,
                        updated_at: Some(new_updated_at.clone()),
                    });

                    let sql = "UPDATE donations SET donation_date = :1, donation_year = :2, donation_category = :3, donation_amount = :4, charity_id = :5, notes = :6, updated_at = :7 WHERE id = :8 AND user_id = :9";
                    conn.execute(sql, &[&new_date, &new_year, &new_category, &new_amount, &new_charity_id, &new_notes, &new_updated_at, &donation_id, &user_id])?;
                    let _ = conn.commit();
                    return Ok(Some((old_values, new_values)));
                }
                Ok(None)
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
            if let Some((old_values, new_values)) = revision_payload {
                let revision = RevisionLogEntry {
                    id: Uuid::new_v4().to_string(),
                    user_id: user_for_revision,
                    table_name: "donations".to_string(),
                    record_id: donation_id_for_revision,
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
        DbPoolEnum::Sqlite(p) => {
            let p = p.clone();
            let user_id = patch.user_id.clone();
            let donation_id = patch.donation_id.clone();
            let donation_id_for_revision = donation_id.clone();
            let incoming = patch.incoming_updated_at.map(|d| d.to_rfc3339());
            let date_opt = patch.date_opt;
            let year_opt = patch.year_opt;
            let amount_opt = patch.amount_opt;
            let notes_cloned = patch.notes.clone();
            let revision_payload = task::spawn_blocking(move || -> anyhow::Result<Option<(String, String)>> {
                let conn = p.get()?;
                let sql_sel = "SELECT donation_date, donation_year, donation_category, donation_amount, charity_id, notes, updated_at FROM donations WHERE id = ?1 AND user_id = ?2";
                let mut stmt = conn.prepare(sql_sel)?;
                let mut rows = stmt.query(rusqlite::params![donation_id, user_id])?;
                if let Some(row) = rows.next()? {
                    let existing_date_str: Option<String> = row.get(0)?;
                    let existing_date = existing_date_str.clone()
                        .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok());
                    let existing_year: Option<i32> = row.get(1)?;
                    let existing_category: Option<String> = row.get(2).ok();
                    let existing_amount: Option<f64> = row.get(3).ok();
                    let existing_charity_id: Option<String> = row.get(4)?;
                    let existing_notes: Option<String> = row.get(5).ok();
                    let existing_updated_at_str: Option<String> = row.get(6).ok();

                    if let (Some(inc), Some(ex)) = (incoming.clone(), existing_updated_at_str.clone()) {
                        if inc <= ex { return Ok(None); }
                    }

                    let new_date = date_opt.unwrap_or(existing_date.unwrap_or_else(|| chrono::Utc::now().date_naive()));
                    let new_year = year_opt.unwrap_or(existing_year.unwrap_or(new_date.year()));
                    let new_category = category_owned.clone().or(existing_category.clone());
                    let new_amount = amount_opt.or(existing_amount);
                    let new_charity_id = charity_id_owned.clone().or(existing_charity_id.clone()).unwrap_or_default();
                    let new_notes = notes_cloned.clone().or(existing_notes.clone());
                    let new_updated_at = incoming.clone().unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

                    let old_values = build_donation_revision_json(&DonationRevisionSnapshot {
                        donation_id: donation_id.clone(),
                        user_id: user_id.clone(),
                        donation_date: existing_date_str.clone().unwrap_or_default(),
                        donation_year: existing_year.unwrap_or(0),
                        donation_category: existing_category.clone(),
                        donation_amount: existing_amount,
                        charity_id: existing_charity_id.clone().unwrap_or_default(),
                        notes: existing_notes.clone(),
                        deleted: false,
                        updated_at: existing_updated_at_str,
                    });
                    let new_values = build_donation_revision_json(&DonationRevisionSnapshot {
                        donation_id: donation_id.clone(),
                        user_id: user_id.clone(),
                        donation_date: new_date.format("%Y-%m-%d").to_string(),
                        donation_year: new_year,
                        donation_category: new_category.clone(),
                        donation_amount: new_amount,
                        charity_id: new_charity_id.clone(),
                        notes: new_notes.clone(),
                        deleted: false,
                        updated_at: Some(new_updated_at.clone()),
                    });

                    let sql_upd = "UPDATE donations SET donation_date = ?1, donation_year = ?2, donation_category = ?3, donation_amount = ?4, charity_id = ?5, notes = ?6, updated_at = ?7 WHERE id = ?8 AND user_id = ?9";
                    let date_str = new_date.format("%Y-%m-%d").to_string();
                    let rows = conn.execute(sql_upd, rusqlite::params![date_str, new_year, new_category, new_amount, new_charity_id, new_notes, new_updated_at, donation_id, user_id])?;
                    if rows > 0 {
                        return Ok(Some((old_values, new_values)));
                    }
                    return Ok(None);
                }
                Ok(None)
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
            if let Some((old_values, new_values)) = revision_payload {
                let revision = RevisionLogEntry {
                    id: Uuid::new_v4().to_string(),
                    user_id: user_for_revision,
                    table_name: "donations".to_string(),
                    record_id: donation_id_for_revision,
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

type ValSuggestion = (String, Option<i64>, Option<i64>);

pub async fn list_donations_since(pool: &DbPool, user_id: &str, since: chrono::DateTime<chrono::Utc>) -> anyhow::Result<Vec<DonationModel>> {
    let since_str = since.to_rfc3339();
    match &**pool {
        DbPoolEnum::Oracle(p) => crate::db::oracle::donations::list_donations_since(p, user_id, &since_str).await,
        DbPoolEnum::Sqlite(p) => crate::db::sqlite::donations::list_donations_since(p, user_id, &since_str).await,
    }
}

pub async fn suggest_valuations(pool: &DbPool, query: &str) -> anyhow::Result<Vec<ValSuggestion>> {
    let q = format!("%{}%", query.to_lowercase());
    match &**pool {
        DbPoolEnum::Oracle(p) => {
            let p = p.clone();
            let q = q.clone();
            let rows = task::spawn_blocking(move || -> anyhow::Result<Vec<ValSuggestion>> {
                let conn = p.get()?;
                let sql = "SELECT name, suggested_min, suggested_max FROM val_items WHERE LOWER(name) LIKE :1";
                let mut out = Vec::new();
                let rows_iter = conn.query(sql, &[&q])?;
                for row in rows_iter.flatten() {
                    out.push((row.get(0).unwrap_or_default(), row.get(1).ok(), row.get(2).ok()));
                }
                Ok(out)
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
            Ok(rows)
        }
        DbPoolEnum::Sqlite(p) => {
            let p = p.clone();
            let q = q.clone();
            let rows = task::spawn_blocking(move || -> anyhow::Result<Vec<ValSuggestion>> {
                let conn = p.get()?;
                let sql = "SELECT name, suggested_min, suggested_max FROM val_items WHERE lower(name) LIKE ?1";
                let mut stmt = conn.prepare(sql)?;
                let mut out = Vec::new();
                let rows_iter = stmt.query_map(rusqlite::params![q], |row| {
                    Ok((row.get(0)?, row.get(1).ok(), row.get(2).ok()))
                })?;
                for r in rows_iter { out.push(r?); }
                Ok(out)
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
            Ok(rows)
        }
    }
}

pub async fn seed_valuations(pool: &DbPool) -> anyhow::Result<()> {
    match &**pool {
        DbPoolEnum::Oracle(p) => {
            let p = p.clone();
            task::spawn_blocking(move || -> anyhow::Result<()> {
                let conn = p.get()?;
                // If there are already items, do nothing
                if let Ok(row) = conn.query_row("SELECT COUNT(1) FROM val_items", &[]) {
                    let count: i64 = row.get(0).unwrap_or(0);
                    if count > 0 {
                        return Ok(());
                    }
                }
                // Insert categories
                let cats = vec![
                    ("cat_clothing", "Clothing"),
                    ("cat_mens", "Men's Clothing"),
                    ("cat_womens", "Women's Clothing"),
                    ("cat_household", "Household Goods"),
                ];
                for (id, name) in cats {
                    let _ = conn.execute("INSERT INTO val_categories (id, name) VALUES (:1, :2)", &[&id, &name]);
                }
                // Insert items
                let items = vec![
                    ("item_1", "cat_mens", "Shirt, Dress", 3i64, 6i64),
                    ("item_2", "cat_mens", "Slacks", 5i64, 10i64),
                    ("item_3", "cat_womens", "Dress, Casual", 6i64, 12i64),
                    ("item_4", "cat_household", "Lamp, Floor", 10i64, 20i64),
                    ("item_5", "cat_household", "Toaster", 4i64, 8i64),
                ];
                for (id, cat, name, low, high) in items {
                    let _ = conn.execute("INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES (:1,:2,:3,:4,:5)", &[&id, &cat, &name, &low, &high]);
                }
                Ok(())
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
            Ok(())
        }
        DbPoolEnum::Sqlite(p) => {
            let p = p.clone();
            task::spawn_blocking(move || -> anyhow::Result<()> {
                let mut conn = p.get()?;
                if let Ok(count) = conn.query_row("SELECT COUNT(1) FROM val_items", rusqlite::params![], |r| r.get::<usize, i64>(0)) {
                    if count > 0 { return Ok(()); }
                }

                let tx = conn.transaction()?;
                tx.execute("INSERT OR IGNORE INTO val_categories (id, name, description) VALUES (?1,?2,?3)", rusqlite::params!["cat_clothing", "Clothing", Option::<String>::None])?;
                tx.execute("INSERT OR IGNORE INTO val_categories (id, name, description) VALUES (?1,?2,?3)", rusqlite::params!["cat_mens", "Men's Clothing", Option::<String>::None])?;
                tx.execute("INSERT OR IGNORE INTO val_categories (id, name, description) VALUES (?1,?2,?3)", rusqlite::params!["cat_womens", "Women's Clothing", Option::<String>::None])?;
                tx.execute("INSERT OR IGNORE INTO val_categories (id, name, description) VALUES (?1,?2,?3)", rusqlite::params!["cat_household", "Household Goods", Option::<String>::None])?;

                tx.execute("INSERT OR IGNORE INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES (?1,?2,?3,?4,?5)", rusqlite::params!["item_1", "cat_mens", "Shirt, Dress", 3i64, 6i64])?;
                tx.execute("INSERT OR IGNORE INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES (?1,?2,?3,?4,?5)", rusqlite::params!["item_2", "cat_mens", "Slacks", 5i64, 10i64])?;
                tx.execute("INSERT OR IGNORE INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES (?1,?2,?3,?4,?5)", rusqlite::params!["item_3", "cat_womens", "Dress, Casual", 6i64, 12i64])?;
                tx.execute("INSERT OR IGNORE INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES (?1,?2,?3,?4,?5)", rusqlite::params!["item_4", "cat_household", "Lamp, Floor", 10i64, 20i64])?;
                tx.execute("INSERT OR IGNORE INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES (?1,?2,?3,?4,?5)", rusqlite::params!["item_5", "cat_household", "Toaster", 4i64, 8i64])?;

                tx.commit()?;
                Ok(())
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
            Ok(())
        }
    }
}

