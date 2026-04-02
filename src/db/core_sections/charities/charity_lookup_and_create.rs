use oracle_rs::Row;

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
    let record_id = record_id.clone();
    let details = details.clone();
    let created_at = chrono::Utc::now().to_rfc3339();

    match &**pool {
        DbPoolEnum::Oracle(p) => {
            let conn = p.get().await?;
            // Truncate details to VARCHAR2-safe length to avoid CLOB binding issues
            let details_truncated = details.map(|v| {
                if v.len() > 4000 { v[..4000].to_string() } else { v }
            });
            let sql = "INSERT INTO audit_logs (id, user_id, action, table_name, record_id, details, created_at) VALUES (:1, :2, :3, :4, :5, :6, TO_TIMESTAMP_TZ(:7, 'YYYY-MM-DD\"T\"HH24:MI:SS.FF TZH:TZM'))";
            conn.execute(
                sql,
                &crate::oracle_params![id.clone(), user_id, action, table_name, record_id, details_truncated, created_at],
            )
            .await?;

            conn.commit().await?;
            Ok(())
        }
    }
}

pub async fn log_revision(
    pool: &DbPool,
    entry: &RevisionLogEntry,
) -> anyhow::Result<()> {
    let id = entry.id.clone();
    let user_id_cloned = entry.user_id.clone();
    let table_name = entry.table_name.clone();
    let record_id = entry.record_id.clone();
    let operation = entry.operation.clone();
    let old_values_cloned = entry.old_values.clone();
    let new_values_cloned = entry.new_values.clone();
    let created_at = chrono::Utc::now().to_rfc3339();

    match &**pool {
        DbPoolEnum::Oracle(p) => {
            let conn = p.get().await?;

            // Cap values at 252 bytes to stay within oracle-rs's safe
            // single-byte length encoding (MAX_SHORT = 252). The chunked
            // encoding path for longer strings triggers a protocol error
            // with Oracle Free.
            const MAX_BIND_LEN: usize = 252;
            let old_val_safe = match old_values_cloned {
                Some(v) if v.len() > MAX_BIND_LEN => format!("{}…", &v[..MAX_BIND_LEN - 3]),
                Some(v) => v,
                None => String::new(),
            };
            let new_val_safe = match new_values_cloned {
                Some(v) if v.len() > MAX_BIND_LEN => format!("{}…", &v[..MAX_BIND_LEN - 3]),
                Some(v) => v,
                None => String::new(),
            };

            let sql = "INSERT INTO audit_revisions (id, user_id, table_name, record_id, operation, old_values, new_values, created_at) VALUES (:1, :2, :3, :4, :5, NULLIF(:6, ''), NULLIF(:7, ''), TO_TIMESTAMP_TZ(:8, 'YYYY-MM-DD\"T\"HH24:MI:SS.FF TZH:TZM'))";
            if let Err(e) = conn
                .execute(
                    sql,
                    &crate::oracle_params![
                        id.clone(),
                        user_id_cloned,
                        table_name,
                        record_id,
                        operation,
                        old_val_safe,
                        new_val_safe,
                        created_at,
                    ],
                )
                .await
            {
                tracing::error!("Failed to insert audit revision: {}", e);
                return Err(anyhow::anyhow!("Audit revision insertion failed: {}", e));
            }

            conn.commit().await?;
            Ok(())
        }
    }
}

pub async fn list_audit_logs(pool: &DbPool, user_id: &str, since: Option<chrono::DateTime<chrono::Utc>>) -> anyhow::Result<Vec<crate::db::models::AuditLog>> {
    match &**pool {
        DbPoolEnum::Oracle(p) => {
            let conn = p.get().await?;
            let since_str = since.map(|d| d.to_rfc3339());
            let sql = if since_str.is_some() {
                "SELECT id, user_id, action, table_name, record_id, details, created_at FROM audit_logs WHERE user_id = :1 AND created_at > TO_TIMESTAMP_TZ(:2, 'YYYY-MM-DD\"T\"HH24:MI:SS.FF TZH:TZM') ORDER BY created_at DESC"
            } else {
                "SELECT id, user_id, action, table_name, record_id, details, created_at FROM audit_logs WHERE user_id = :1 ORDER BY created_at DESC"
            };
            let rows = if let Some(since_str) = since_str {
                conn.query(sql, &crate::oracle_params![user_id.to_string(), since_str])
                    .await?
            } else {
                conn.query(sql, &crate::oracle_params![user_id.to_string()]).await?
            };
            let mut out = Vec::new();
            for row in &rows.rows {
                out.push(crate::db::models::AuditLog {
                    id: crate::db::oracle::row_string(row, 0),
                    user_id: crate::db::oracle::row_string(row, 1),
                    action: crate::db::oracle::row_string(row, 2),
                    table_name: crate::db::oracle::row_string(row, 3),
                    record_id: crate::db::oracle::row_opt_string(row, 4),
                    details: crate::db::oracle::row_opt_string(row, 5),
                    created_at: crate::db::oracle::row_datetime_utc(row, 6)
                        .unwrap_or_else(chrono::Utc::now),
                });
            }
            Ok(out)
        }
    }
}

pub async fn find_charity_by_name_or_ein(
    pool: &DbPool,
    user_id: &str,
    name: &str,
    ein: &Option<String>,
) -> anyhow::Result<Option<crate::db::models::Charity>> {
    match &**pool {
        DbPoolEnum::Oracle(p) => {
            let conn = p.get().await?;
            if let Some(ein_val) = ein.clone().filter(|value| !value.is_empty()) {
                let rows = conn
                    .query(
                        "SELECT id, user_id, name, ein, created_at, updated_at, nonprofit_type, deductibility, street, city, state, zip, category, status, classification FROM charities WHERE user_id = :1 AND ein = :2 FETCH FIRST 1 ROWS ONLY",
                        &crate::oracle_params![user_id.to_string(), ein_val],
                    )
                    .await?;
                if let Some(row) = rows.first() {
                    return Ok(Some(charity_from_row(row)));
                }
            }

            let normalized_name = name.trim().to_ascii_lowercase();
            let rows = conn
                .query(
                    "SELECT id, user_id, name, ein, created_at, updated_at, nonprofit_type, deductibility, street, city, state, zip, category, status, classification FROM charities WHERE user_id = :1 AND LOWER(name) = :2 FETCH FIRST 1 ROWS ONLY",
                    &crate::oracle_params![user_id.to_string(), normalized_name],
                )
                .await?;
            let row = rows.first().map(charity_from_row);
            Ok(row)
        }
    }
}

fn charity_from_row(row: &Row) -> crate::db::models::Charity {
    crate::db::models::Charity {
        id: crate::db::oracle::row_string(row, 0),
        user_id: crate::db::oracle::row_string(row, 1),
        name: crate::db::oracle::row_string(row, 2),
        ein: crate::db::oracle::row_opt_string(row, 3),
        created_at: crate::db::oracle::row_datetime_utc(row, 4).unwrap_or_else(chrono::Utc::now),
        updated_at: crate::db::oracle::row_datetime_utc(row, 5).unwrap_or_else(chrono::Utc::now),
        nonprofit_type: crate::db::oracle::row_opt_string(row, 6),
        deductibility: crate::db::oracle::row_opt_string(row, 7),
        street: crate::db::oracle::row_opt_string(row, 8),
        city: crate::db::oracle::row_opt_string(row, 9),
        state: crate::db::oracle::row_opt_string(row, 10),
        zip: crate::db::oracle::row_opt_string(row, 11),
        category: crate::db::oracle::row_opt_string(row, 12),
        status: crate::db::oracle::row_opt_string(row, 13),
        classification: crate::db::oracle::row_opt_string(row, 14),
    }
}

pub async fn create_charity(pool: &DbPool, input: &crate::db::models::NewCharity) -> anyhow::Result<()> {
    let input = input.clone();
    let created_at_str = input.created_at.to_rfc3339();

    match &**pool {
        DbPoolEnum::Oracle(p) => {
            let conn = p.get().await?;
            let trunc_name = input.name.chars().take(255).collect::<String>();
            let sql = "INSERT INTO charities (id, user_id, name, ein, category, status, classification, nonprofit_type, deductibility, street, city, state, zip, created_at) VALUES (:1, :2, :3, :4, :5, :6, :7, :8, :9, :10, :11, :12, :13, TO_TIMESTAMP_TZ(:14, 'YYYY-MM-DD\"T\"HH24:MI:SS.FF TZH:TZM'))";
            if let Err(e) = conn
                .execute(
                    sql,
                    &crate::oracle_params![
                        input.id.clone(),
                        input.user_id.clone(),
                        trunc_name,
                        input.ein.clone(),
                        input.category.clone(),
                        input.status.clone(),
                        input.classification.clone(),
                        input.nonprofit_type.clone(),
                        input.deductibility.clone(),
                        input.street.clone(),
                        input.city.clone(),
                        input.state.clone(),
                        input.zip.clone(),
                        created_at_str.clone(),
                    ],
                )
                .await
            {
                tracing::error!("Failed to create charity: {}. SQL: {}", e, sql);
                return Err(anyhow::anyhow!("Charity creation failed: {}", e));
            }
            conn.commit().await?;
        }
    };

    let revision = RevisionLogEntry {
        id: Uuid::new_v4().to_string(),
        user_id: Some(input.user_id.clone()),
        table_name: "charities".to_string(),
        record_id: input.id.clone(),
        operation: "create".to_string(),
        old_values: None,
        new_values: Some(json!({
            "id": input.id,
            "user_id": input.user_id,
            "name": input.name,
            "ein": input.ein,
            "category": input.category,
            "status": input.status,
            "classification": input.classification,
            "nonprofit_type": input.nonprofit_type,
            "deductibility": input.deductibility,
            "street": input.street,
            "city": input.city,
            "state": input.state,
            "zip": input.zip,
            "created_at": created_at_str
        }).to_string()),
    };
    log_revision(pool, &revision).await?;

    Ok(())
}

