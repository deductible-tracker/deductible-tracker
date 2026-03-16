pub async fn batch_sync(pool: &DbPool, user_id: &str, req: crate::db::models::BatchSyncRequest) -> anyhow::Result<()> {
    let p = pool.clone();
    let user_id = user_id.to_string();
    let req = req.clone();

    match &*p {
        DbPoolEnum::Oracle(pool_inner) => {
            let pool_inner = pool_inner.clone();
            task::spawn_blocking(move || -> anyhow::Result<()> {
                let conn = pool_inner.get()?;

                for donation in req.donations {
                    let id = donation.id.clone();
                    let action = donation.action.clone();

                    if action == "delete" {
                        let sql = "UPDATE donations SET deleted = 1, updated_at = CURRENT_TIMESTAMP WHERE id = :1 AND user_id = :2";
                        conn.execute(sql, &[&id, &user_id])?;
                        continue;
                    }

                    // check existing
                    let mut rows = conn.query("SELECT updated_at FROM donations WHERE id = :1 AND user_id = :2", &[&id, &user_id])?;
                    if let Some(row) = rows.next().transpose()? {
                        let existing_updated_str: Option<String> = row.get(0).ok();
                        let existing_updated = existing_updated_str.and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok().map(|dt| dt.with_timezone(&chrono::Utc)));
                        
                        if let (Some(inc), Some(ex)) = (donation.updated_at, existing_updated) {
                            if inc <= ex {
                                // skip stale update
                                continue;
                            }
                        }

                        // update
                        let updated_at_str = donation.updated_at.map(|dt| dt.to_rfc3339());
                        let sql = "UPDATE donations SET donation_date = :1, donation_year = :2, donation_category = :3, donation_amount = :4, charity_id = :5, notes = :6, updated_at = TO_TIMESTAMP_TZ(:7, 'YYYY-MM-DD\"T\"HH24:MI:SS.FF TZH:TZM'), deleted = 0 WHERE id = :8 AND user_id = :9";
                        let d_date = donation.date.unwrap_or_else(|| chrono::Utc::now().date_naive());
                        let d_year = donation.year.unwrap_or(d_date.year());
                        conn.execute(sql, &[&d_date, &d_year, &donation.category, &donation.amount, &donation.charity_id, &donation.notes, &updated_at_str, &id, &user_id])?;
                    } else {
                        // insert
                        let sql = "INSERT INTO donations (id, user_id, donation_date, donation_year, donation_category, donation_amount, charity_id, notes, created_at, updated_at, deleted) VALUES (:1, :2, :3, :4, :5, :6, :7, :8, TO_TIMESTAMP_TZ(:9, 'YYYY-MM-DD\"T\"HH24:MI:SS.FF TZH:TZM'), TO_TIMESTAMP_TZ(:10, 'YYYY-MM-DD\"T\"HH24:MI:SS.FF TZH:TZM'), 0)";
                        let now = chrono::Utc::now();
                        let created_at_str = now.to_rfc3339();
                        let updated_at_str = donation.updated_at.unwrap_or(now).to_rfc3339();
                        let d_date = donation.date.unwrap_or_else(|| now.date_naive());
                        let d_year = donation.year.unwrap_or(d_date.year());
                        conn.execute(sql, &[&id, &user_id, &d_date, &d_year, &donation.category, &donation.amount, &donation.charity_id, &donation.notes, &created_at_str, &updated_at_str])?;
                    }
                }

                for receipt in req.receipts {
                     let id = receipt.id.clone();
                     // Simple existence check
                     let mut rows = conn.query("SELECT 1 FROM receipts WHERE id = :1", &[&id])?;
                     if rows.next().transpose()?.is_none() {
                        let sql = "INSERT INTO receipts (id, donation_id, key, file_name, content_type, size, created_at) VALUES (:1, :2, :3, :4, :5, :6, TO_TIMESTAMP_TZ(:7, 'YYYY-MM-DD\"T\"HH24:MI:SS.FF TZH:TZM'))";
                        let now = chrono::Utc::now();
                        let created_at_str = now.to_rfc3339();
                        conn.execute(sql, &[&id, &receipt.donation_id, &receipt.key, &receipt.file_name, &receipt.content_type, &receipt.size, &created_at_str])?;
                     }
                }

                let _ = conn.commit();
                Ok(())
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
        }
    }

    Ok(())
}

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

                    let sql = "UPDATE donations SET donation_date = :1, donation_year = :2, donation_category = :3, donation_amount = :4, charity_id = :5, notes = :6, updated_at = TO_TIMESTAMP_TZ(:7, 'YYYY-MM-DD\"T\"HH24:MI:SS.FF TZH:TZM') WHERE id = :8 AND user_id = :9";
                    if let Err(e) = conn.execute(sql, &[&new_date, &new_year, &new_category, &new_amount, &new_charity_id, &new_notes, &new_updated_at, &donation_id, &user_id]) {
                        tracing::error!("Failed to update donation: {}. SQL: {}", e, sql);
                        return Err(anyhow::anyhow!("Donation update failed: {}", e));
                    }
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
    }
}

pub type ValuationSuggestion = (String, Option<i64>, Option<i64>);

pub async fn suggest_valuations(pool: &DbPool, query: &str) -> anyhow::Result<Vec<ValuationSuggestion>> {
    let pattern = format!("%{}%", query.to_lowercase());
    match &**pool {
        DbPoolEnum::Oracle(p) => {
            let p = p.clone();
            let pat = pattern.clone();
            let suggestions = task::spawn_blocking(move || -> anyhow::Result<Vec<ValuationSuggestion>> {
                let conn = p.get()?;
                let mut out = Vec::new();
                let rows = conn.query("SELECT name, suggested_min, suggested_max FROM val_items WHERE LOWER(name) LIKE :1 ORDER BY name", &[&pat])?;
                for row in rows.flatten() {
                    let name: String = row.get::<_, String>(0).unwrap_or_default();
                    let min: Option<f64> = row.get::<_, Option<f64>>(1).ok().flatten();
                    let max: Option<f64> = row.get::<_, Option<f64>>(2).ok().flatten();
                    out.push((name, min.map(|v| v as i64), max.map(|v| v as i64)));
                }
                Ok(out)
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
            Ok(suggestions)
        }
    }
}

pub async fn list_donations_since(pool: &DbPool, user_id: &str, since: chrono::DateTime<chrono::Utc>) -> anyhow::Result<Vec<DonationModel>> {
    let since_str = since.to_rfc3339();
    match &**pool {
        DbPoolEnum::Oracle(p) => crate::db::oracle::donations::list_donations_since(p, user_id, &since_str).await,
    }
}

pub async fn list_valuation_tree(pool: &DbPool) -> anyhow::Result<serde_json::Value> {
    match &**pool {
        DbPoolEnum::Oracle(p) => {
            let p = p.clone();
            let tree = task::spawn_blocking(move || -> anyhow::Result<serde_json::Value> {
                let conn = p.get()?;
                let mut categories = Vec::new();
                let cat_rows = conn.query("SELECT id, name FROM val_categories ORDER BY name", &[])?;
                for cat_row in cat_rows.flatten() {
                    let cat_id: String = cat_row.get(0).unwrap_or_default();
                    let cat_name: String = cat_row.get(1).unwrap_or_default();
                    let mut items = Vec::new();
                    let item_rows = conn.query("SELECT name, suggested_min, suggested_max FROM val_items WHERE category_id = :1 ORDER BY name", &[&cat_id])?;
                    for item_row in item_rows.flatten() {
                        items.push(json!({
                            "name": item_row.get::<_, String>(0).unwrap_or_default(),
                            "min": item_row.get::<_, Option<f64>>(1).ok(),
                            "max": item_row.get::<_, Option<f64>>(2).ok(),
                        }));
                    }
                    categories.push(json!({
                        "id": cat_id,
                        "name": cat_name,
                        "items": items
                    }));
                }
                Ok(json!(categories))
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
            Ok(tree)
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
                    ("cat_appliances", "Appliances"),
                    ("cat_childrens_clothing", "Children's Clothing"),
                    ("cat_furniture", "Furniture"),
                    ("cat_household_goods", "Household Goods"),
                    ("cat_mens_clothing", "Men's Clothing"),
                    ("cat_womens_clothing", "Women's Clothing"),
                    ("cat_electronics", "Electronics & Computers"),
                    ("cat_miscellaneous", "Miscellaneous"),
                ];
                for (id, name) in cats {
                    let _ = conn.execute("INSERT INTO val_categories (id, name) VALUES (:1, :2)", &[&id, &name]);
                }
                // Insert items
                let items = vec![
                    // Appliances
                    ("app_ac", "cat_appliances", "Air Conditioner", 21, 93),
                    ("app_dryer", "cat_appliances", "Dryer", 47, 93),
                    ("app_stove_elec", "cat_appliances", "Electric Stove", 78, 156),
                    ("app_freezer", "cat_appliances", "Freezer", 25, 100),
                    ("app_stove_gas", "cat_appliances", "Gas Stove", 52, 130),
                    ("app_heater", "cat_appliances", "Heater", 8, 23),
                    ("app_microwave", "cat_appliances", "Microwave", 10, 50),
                    ("app_refrigerator", "cat_appliances", "Refrigerator (Working)", 78, 259),
                    ("app_washer", "cat_appliances", "Washing Machine", 41, 156),
                    ("app_coffeemaker", "cat_appliances", "Coffee Maker", 4, 16),
                    ("app_iron", "cat_appliances", "Iron", 3, 10),

                    // Children's Clothing
                    ("child_blouse", "cat_childrens_clothing", "Blouse", 2, 8),
                    ("child_boots", "cat_childrens_clothing", "Boots", 3, 21),
                    ("child_coat", "cat_childrens_clothing", "Coat", 5, 21),
                    ("child_dress", "cat_childrens_clothing", "Dress", 2, 12),
                    ("child_jacket", "cat_childrens_clothing", "Jacket", 3, 26),
                    ("child_jeans", "cat_childrens_clothing", "Jeans", 4, 12),
                    ("child_pants", "cat_childrens_clothing", "Pants", 3, 12),
                    ("child_shirt", "cat_childrens_clothing", "Shirt", 2, 10),
                    ("child_shoes", "cat_childrens_clothing", "Shoes", 3, 10),
                    ("child_snowsuit", "cat_childrens_clothing", "Snowsuit", 4, 20),
                    ("child_sweater", "cat_childrens_clothing", "Sweater", 2, 10),

                    // Furniture
                    ("furn_bed_full", "cat_furniture", "Bed (full, queen, king)", 52, 176),
                    ("furn_bed_single", "cat_furniture", "Bed (single)", 36, 104),
                    ("furn_chair_uph", "cat_furniture", "Chair (upholstered)", 26, 104),
                    ("furn_chest", "cat_furniture", "Chest", 26, 99),
                    ("furn_china", "cat_furniture", "China Cabinet", 89, 311),
                    ("furn_coffee_table", "cat_furniture", "Coffee Table", 15, 100),
                    ("furn_desk", "cat_furniture", "Desk", 26, 145),
                    ("furn_dresser", "cat_furniture", "Dresser", 20, 104),
                    ("furn_end_table", "cat_furniture", "End Table", 10, 75),
                    ("furn_kitchen_set", "cat_furniture", "Kitchen Set", 35, 176),
                    ("furn_sofa", "cat_furniture", "Sofa", 36, 395),

                    // Household Goods
                    ("house_blanket", "cat_household_goods", "Blanket", 3, 16),
                    ("house_curtains", "cat_household_goods", "Curtains", 2, 12),
                    ("house_lamp_floor", "cat_household_goods", "Lamp, Floor", 6, 52),
                    ("house_lamp_table", "cat_household_goods", "Lamp, Table", 3, 20),
                    ("house_pillow", "cat_household_goods", "Pillow", 2, 8),
                    ("house_rug_area", "cat_household_goods", "Area Rug", 2, 93),
                    ("house_sheets", "cat_household_goods", "Sheets", 2, 9),

                    // Men's Clothing
                    ("men_jacket", "cat_mens_clothing", "Jacket", 8, 45),
                    ("men_suit", "cat_mens_clothing", "Suit (2pc)", 5, 96),
                    ("men_shirt", "cat_mens_clothing", "Shirt", 3, 12),
                    ("men_pants", "cat_mens_clothing", "Pants", 4, 23),
                    ("men_shoes", "cat_mens_clothing", "Shoes", 3, 30),
                    ("men_sweater", "cat_mens_clothing", "Sweater", 3, 12),

                    // Women's Clothing
                    ("women_suit", "cat_womens_clothing", "Suit (2pc)", 10, 96),
                    ("women_blouse", "cat_womens_clothing", "Blouse", 3, 12),
                    ("women_dress", "cat_womens_clothing", "Dress", 4, 28),
                    ("women_pants", "cat_womens_clothing", "Pants", 4, 23),
                    ("women_shoes", "cat_womens_clothing", "Shoes", 2, 30),
                    ("women_sweater", "cat_womens_clothing", "Sweater", 4, 13),

                    // Electronics & Computers
                    ("elec_desktop", "cat_electronics", "Desktop Computer", 20, 415),
                    ("elec_laptop", "cat_electronics", "Laptop", 25, 415),
                    ("elec_monitor", "cat_electronics", "Monitor", 5, 51),
                    ("elec_printer", "cat_electronics", "Printer", 1, 155),
                    ("elec_tablet", "cat_electronics", "Tablet", 25, 150),
                    ("elec_tv", "cat_electronics", "TV (Color Working)", 78, 233),

                    // Miscellaneous
                    ("misc_bicycle", "cat_miscellaneous", "Bicycle", 5, 83),
                    ("misc_books_hard", "cat_miscellaneous", "Book (hardback)", 1, 3),
                    ("misc_books_paper", "cat_miscellaneous", "Book (paperback)", 1, 2),
                    ("misc_luggage", "cat_miscellaneous", "Luggage", 5, 16),
                    ("misc_vacuum", "cat_miscellaneous", "Vacuum Cleaner", 5, 67),
                ];
                for (id, cat, name, low, high) in items {
                    let _ = conn.execute("INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES (:1,:2,:3,:4,:5)", &[&id, &cat, &name, &low, &high]);
                }
                Ok(())
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
            Ok(())
        }
    }
}
