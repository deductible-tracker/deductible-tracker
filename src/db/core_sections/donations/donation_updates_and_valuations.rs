pub async fn batch_sync(pool: &DbPool, user_id: &str, req: crate::db::models::BatchSyncRequest) -> anyhow::Result<()> {
    let user_id = user_id.to_string();
    let req = req.clone();

    match &**pool {
        DbPoolEnum::Oracle(pool_inner) => {
            let conn = pool_inner.get().await?;

            for donation in req.donations {
                let id = donation.id.clone();
                let action = donation.action.clone();

                if action == "delete" {
                    let sql = "UPDATE donations SET deleted = 1, updated_at = CURRENT_TIMESTAMP WHERE id = :1 AND user_id = :2";
                    conn.execute(sql, &crate::oracle_params![id, user_id.clone()]).await?;
                    continue;
                }

                let now = chrono::Utc::now();
                let donation_date = donation.date.unwrap_or_else(|| now.date_naive());
                let donation_year = donation.year.unwrap_or(donation_date.year());
                let incoming_updated_at = donation.updated_at.unwrap_or(now).to_rfc3339();
                let created_at = now.to_rfc3339();
                let is_encrypted = donation.is_encrypted.map(|v| if v { 1 } else { 0 });
                let sql = "MERGE INTO donations d USING (SELECT :1 AS id, :2 AS user_id, TO_DATE(:3, 'YYYY-MM-DD') AS donation_date, :4 AS donation_year, :5 AS donation_category, :6 AS donation_amount, :7 AS charity_id, :8 AS notes, :9 AS is_encrypted, :10 AS encrypted_payload, TO_TIMESTAMP_TZ(:11, 'YYYY-MM-DD\"T\"HH24:MI:SS.FF TZH:TZM') AS incoming_updated_at, TO_TIMESTAMP_TZ(:12, 'YYYY-MM-DD\"T\"HH24:MI:SS.FF TZH:TZM') AS incoming_created_at FROM dual) s ON (d.id = s.id AND d.user_id = s.user_id) WHEN MATCHED THEN UPDATE SET d.donation_date = s.donation_date, d.donation_year = s.donation_year, d.donation_category = s.donation_category, d.donation_amount = s.donation_amount, d.charity_id = s.charity_id, d.notes = s.notes, d.is_encrypted = s.is_encrypted, d.encrypted_payload = s.encrypted_payload, d.updated_at = s.incoming_updated_at, d.deleted = 0 WHERE d.updated_at IS NULL OR d.updated_at < s.incoming_updated_at OR s.incoming_updated_at IS NULL WHEN NOT MATCHED THEN INSERT (id, user_id, donation_date, donation_year, donation_category, donation_amount, charity_id, notes, is_encrypted, encrypted_payload, created_at, updated_at, deleted) VALUES (s.id, s.user_id, s.donation_date, s.donation_year, s.donation_category, s.donation_amount, s.charity_id, s.notes, s.is_encrypted, s.encrypted_payload, s.incoming_created_at, s.incoming_updated_at, 0)";
                conn.execute(
                    sql,
                    &crate::oracle_params![
                        id,
                        user_id.clone(),
                        donation_date.format("%Y-%m-%d").to_string(),
                        donation_year,
                        donation.category,
                        donation.amount,
                        donation.charity_id,
                        donation.notes,
                        is_encrypted,
                        donation.encrypted_payload,
                        incoming_updated_at,
                        created_at,
                    ],
                )
                .await?;
            }

            for receipt in req.receipts {
                if receipt.action != "create" {
                    continue;
                }

                let is_encrypted = receipt.is_encrypted.map(|v| if v { 1 } else { 0 });
                let sql = "MERGE INTO receipts r USING (SELECT :1 AS id, :2 AS donation_id, :3 AS receipt_key, :4 AS file_name, :5 AS content_type, :6 AS receipt_size, :7 AS is_encrypted, :8 AS encrypted_payload, TO_TIMESTAMP_TZ(:9, 'YYYY-MM-DD\"T\"HH24:MI:SS.FF TZH:TZM') AS created_at FROM dual) s ON (r.id = s.id) WHEN NOT MATCHED THEN INSERT (id, donation_id, receipt_key, file_name, content_type, receipt_size, is_encrypted, encrypted_payload, created_at) VALUES (s.id, s.donation_id, s.receipt_key, s.file_name, s.content_type, s.receipt_size, s.is_encrypted, s.encrypted_payload, s.created_at)";
                conn.execute(
                    sql,
                    &crate::oracle_params![
                        receipt.id,
                        receipt.donation_id,
                        receipt.key,
                        receipt.file_name,
                        receipt.content_type,
                        receipt.size,
                        is_encrypted,
                        receipt.encrypted_payload,
                        chrono::Utc::now().to_rfc3339(),
                    ],
                )
                .await?;
            }

            conn.commit().await?;
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
            let conn = p.get().await?;
            let user_id = patch.user_id.clone();
            let donation_id = patch.donation_id.clone();
            let donation_id_for_revision = donation_id.clone();
            let incoming = patch.incoming_updated_at.map(|d| d.to_rfc3339());
            let date_opt = patch.date_opt;
            let year_opt = patch.year_opt;
            let amount_opt = patch.amount_opt;
            let notes_cloned = patch.notes.clone();
            let is_encrypted_opt = patch.is_encrypted;
            let encrypted_payload_opt = patch.encrypted_payload.clone();
            let rows = conn
                .query(
                    "SELECT donation_date, donation_year, donation_category, donation_amount, charity_id, notes, updated_at, is_encrypted, encrypted_payload FROM donations WHERE id = :1 AND user_id = :2",
                    &crate::oracle_params![donation_id.clone(), user_id.clone()],
                )
                .await?;
            let revision_payload = if let Some(row) = rows.first() {
                let existing_updated = crate::db::oracle::row_opt_string(row, 6);
                let existing_is_encrypted = crate::db::oracle::row_bool(row, 7);
                let existing_encrypted_payload = crate::db::oracle::row_opt_string(row, 8);

                if let (Some(inc), Some(ex)) = (incoming.clone(), existing_updated.clone()) {
                    if inc <= ex {
                        None
                    } else {
                        let existing_date = crate::db::oracle::row_naive_date(row, 0);
                        let existing_year = crate::db::oracle::row_i64(row, 1).map(|value| value as i32);
                        let existing_category = crate::db::oracle::row_opt_string(row, 2);
                        let existing_amount = crate::db::oracle::row_f64(row, 3);
                        let existing_charity_id = crate::db::oracle::row_opt_string(row, 4);
                        let existing_notes = crate::db::oracle::row_opt_string(row, 5);

                        let new_date = date_opt.unwrap_or(existing_date.unwrap_or_else(|| chrono::Utc::now().date_naive()));
                        let new_year = year_opt.unwrap_or(existing_year.unwrap_or(new_date.year()));
                        let new_category = category_owned.clone().or(existing_category.clone());
                        let new_amount = amount_opt.or(existing_amount);
                        let new_charity_id = charity_id_owned.clone().or(existing_charity_id.clone()).unwrap_or_default();
                        let new_notes = notes_cloned.clone().or(existing_notes.clone());
                        let new_is_encrypted = is_encrypted_opt.or(existing_is_encrypted);
                        let new_encrypted_payload = encrypted_payload_opt.clone().or(existing_encrypted_payload.clone());
                        let new_updated_at = incoming.clone().unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

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
                            is_encrypted: existing_is_encrypted,
                            encrypted_payload: existing_encrypted_payload.clone(),
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
                            is_encrypted: new_is_encrypted,
                            encrypted_payload: new_encrypted_payload.clone(),
                            deleted: false,
                            updated_at: Some(new_updated_at.clone()),
                        });

                        let sql = "UPDATE donations SET donation_date = TO_DATE(:1, 'YYYY-MM-DD'), donation_year = :2, donation_category = :3, donation_amount = :4, charity_id = :5, notes = :6, is_encrypted = :7, encrypted_payload = :8, updated_at = TO_TIMESTAMP_TZ(:9, 'YYYY-MM-DD\"T\"HH24:MI:SS.FF TZH:TZM') WHERE id = :10 AND user_id = :11";
                        let is_enc_val = new_is_encrypted.map(|v| if v { 1 } else { 0 });
                        if let Err(e) = conn
                            .execute(
                                sql,
                                &crate::oracle_params![
                                    new_date.format("%Y-%m-%d").to_string(),
                                    new_year,
                                    new_category,
                                    new_amount,
                                    new_charity_id,
                                    new_notes,
                                    is_enc_val,
                                    new_encrypted_payload,
                                    new_updated_at,
                                    donation_id.clone(),
                                    user_id.clone(),
                                ],
                            )
                            .await
                        {
                            tracing::error!("Failed to update donation: {}. SQL: {}", e, sql);
                            return Err(anyhow::anyhow!("Donation update failed: {}", e));
                        }
                        conn.commit().await?;
                        Some((old_values, new_values))
                    }
                } else {
                    let existing_date = crate::db::oracle::row_naive_date(row, 0);
                    let existing_year = crate::db::oracle::row_i64(row, 1).map(|value| value as i32);
                    let existing_category = crate::db::oracle::row_opt_string(row, 2);
                    let existing_amount = crate::db::oracle::row_f64(row, 3);
                    let existing_charity_id = crate::db::oracle::row_opt_string(row, 4);
                    let existing_notes = crate::db::oracle::row_opt_string(row, 5);

                    let new_date = date_opt.unwrap_or(existing_date.unwrap_or_else(|| chrono::Utc::now().date_naive()));
                    let new_year = year_opt.unwrap_or(existing_year.unwrap_or(new_date.year()));
                    let new_category = category_owned.clone().or(existing_category.clone());
                    let new_amount = amount_opt.or(existing_amount);
                    let new_charity_id = charity_id_owned.clone().or(existing_charity_id.clone()).unwrap_or_default();
                    let new_notes = notes_cloned.clone().or(existing_notes.clone());
                    let new_is_encrypted = is_encrypted_opt.or(existing_is_encrypted);
                    let new_encrypted_payload = encrypted_payload_opt.clone().or(existing_encrypted_payload.clone());
                    let new_updated_at = incoming.clone().unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

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
                        is_encrypted: existing_is_encrypted,
                        encrypted_payload: existing_encrypted_payload.clone(),
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
                        is_encrypted: new_is_encrypted,
                        encrypted_payload: new_encrypted_payload.clone(),
                        deleted: false,
                        updated_at: Some(new_updated_at.clone()),
                    });

                    let sql = "UPDATE donations SET donation_date = TO_DATE(:1, 'YYYY-MM-DD'), donation_year = :2, donation_category = :3, donation_amount = :4, charity_id = :5, notes = :6, is_encrypted = :7, encrypted_payload = :8, updated_at = TO_TIMESTAMP_TZ(:9, 'YYYY-MM-DD\"T\"HH24:MI:SS.FF TZH:TZM') WHERE id = :10 AND user_id = :11";
                    let is_enc_val = new_is_encrypted.map(|v| if v { 1 } else { 0 });
                    if let Err(e) = conn
                        .execute(
                            sql,
                            &crate::oracle_params![
                                new_date.format("%Y-%m-%d").to_string(),
                                new_year,
                                new_category,
                                new_amount,
                                new_charity_id,
                                new_notes,
                                is_enc_val,
                                new_encrypted_payload,
                                new_updated_at,
                                donation_id.clone(),
                                user_id.clone(),
                            ],
                        )
                        .await
                    {
                        tracing::error!("Failed to update donation: {}. SQL: {}", e, sql);
                        return Err(anyhow::anyhow!("Donation update failed: {}", e));
                    }
                    conn.commit().await?;
                    Some((old_values, new_values))
                }
            } else {
                None
            };
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
    let normalized_query = query.trim().to_ascii_lowercase();
    if normalized_query.is_empty() {
        return Ok(Vec::new());
    }
    let pattern = format!("{}%", escape_like_pattern(&normalized_query));
    match &**pool {
        DbPoolEnum::Oracle(p) => {
            let conn = p.get().await?;
            let rows = conn
                .query(
                    "SELECT name, suggested_min, suggested_max FROM val_items WHERE LOWER(name) LIKE :1 ESCAPE '\\' ORDER BY name FETCH FIRST 20 ROWS ONLY",
                    &crate::oracle_params![pattern],
                )
                .await?;
            let mut out = Vec::new();
            for row in &rows.rows {
                out.push((
                    crate::db::oracle::row_string(row, 0),
                    crate::db::oracle::row_f64(row, 1).map(|value| value as i64),
                    crate::db::oracle::row_f64(row, 2).map(|value| value as i64),
                ));
            }
            Ok(out)
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
            let conn = p.get().await?;
            let mut categories = Vec::new();
            let rows = conn
                .query(
                    "SELECT c.id, c.name, i.name, i.suggested_min, i.suggested_max FROM val_categories c LEFT JOIN val_items i ON i.category_id = c.id ORDER BY c.name, i.name",
                    &[],
                )
                .await?;
            let mut current_category_id = None::<String>;
            let mut current_category_name = String::new();
            let mut current_items = Vec::new();

            for row in &rows.rows {
                let category_id = crate::db::oracle::row_string(row, 0);
                let category_name = crate::db::oracle::row_string(row, 1);

                if current_category_id.as_deref() != Some(category_id.as_str()) {
                    if let Some(previous_category_id) = current_category_id.replace(category_id.clone()) {
                        categories.push(json!({
                            "id": previous_category_id,
                            "name": current_category_name,
                            "items": current_items,
                        }));
                        current_items = Vec::new();
                    }
                    current_category_name = category_name;
                }

                if let Some(item_name) = crate::db::oracle::row_opt_string(row, 2) {
                    current_items.push(json!({
                        "name": item_name,
                        "min": crate::db::oracle::row_f64(row, 3),
                        "max": crate::db::oracle::row_f64(row, 4),
                    }));
                }
            }

            if let Some(category_id) = current_category_id {
                categories.push(json!({
                    "id": category_id,
                    "name": current_category_name,
                    "items": current_items,
                }));
            }
            Ok(json!(categories))
        }
    }
}

pub async fn seed_valuations(pool: &DbPool) -> anyhow::Result<()> {
    match &**pool {
        DbPoolEnum::Oracle(p) => {
            let conn = p.get().await?;
            let rows = conn
                .query("SELECT 1 FROM val_items FETCH FIRST 1 ROWS ONLY", &[])
                .await?;
            if rows.first().is_some() {
                return Ok(());
            }

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
                let _ = conn
                    .execute(
                        "INSERT INTO val_categories (id, name) VALUES (:1, :2)",
                        &crate::oracle_params![id.to_string(), name.to_string()],
                    )
                    .await;
            }

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
                let _ = conn
                    .execute(
                        "INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES (:1,:2,:3,:4,:5)",
                        &crate::oracle_params![
                            id.to_string(),
                            cat.to_string(),
                            name.to_string(),
                            low,
                            high,
                        ],
                    )
                    .await;
            }
            conn.commit().await?;
            Ok(())
        }
    }
}

fn escape_like_pattern(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '%' | '_' | '\\' => {
                escaped.push('\\');
                escaped.push(ch);
            }
            _ => escaped.push(ch),
        }
    }
    escaped
}
