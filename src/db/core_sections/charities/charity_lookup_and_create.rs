#[allow(dead_code)]
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
            let p = p.clone();
            task::spawn_blocking(move || -> anyhow::Result<()> {
                let conn = p.get()?;
                let sql = "INSERT INTO audit_revisions (id, user_id, table_name, record_id, operation, old_values, new_values, created_at) VALUES (:1,:2,:3,:4,:5,:6,:7,:8)";
                conn.execute(
                    sql,
                    &[&id, &user_id_cloned, &table_name, &record_id, &operation, &old_values_cloned, &new_values_cloned, &created_at],
                )?;
                let _ = conn.commit();
                Ok(())
            })
            .await
            .map_err(|e| anyhow!("DB task join error: {}", e))??;
            Ok(())
        }
        DbPoolEnum::Sqlite(p) => {
            let p = p.clone();
            task::spawn_blocking(move || -> anyhow::Result<()> {
                let conn = p.get()?;
                let sql = "INSERT INTO audit_revisions (id, user_id, table_name, record_id, operation, old_values, new_values, created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)";
                conn.execute(
                    sql,
                    rusqlite::params![id, user_id_cloned, table_name, record_id, operation, old_values_cloned, new_values_cloned, created_at],
                )?;
                Ok(())
            })
            .await
            .map_err(|e| anyhow!("DB task join error: {}", e))??;
            Ok(())
        }
    }
}

pub async fn list_audit_logs(pool: &DbPool, user_id: &str, since: Option<chrono::DateTime<chrono::Utc>>) -> anyhow::Result<Vec<crate::db::models::AuditLog>> {
    match &**pool {
        DbPoolEnum::Oracle(p) => {
            let p = p.clone();
            let user_id = user_id.to_string();
            let since_str = since.map(|d| d.to_rfc3339());
            let rows = task::spawn_blocking(move || -> anyhow::Result<Vec<crate::db::models::AuditLog>> {
                let conn = p.get()?;
                let parse_utc = |value: Option<String>| {
                    value
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(chrono::Utc::now)
                };
                let sql = if since_str.is_some() {
                    "SELECT id, user_id, action, table_name, record_id, details, created_at FROM audit_logs WHERE user_id = :1 AND created_at > :2 ORDER BY created_at DESC"
                } else {
                    "SELECT id, user_id, action, table_name, record_id, details, created_at FROM audit_logs WHERE user_id = :1 ORDER BY created_at DESC"
                };
                let rows_iter = if let Some(s) = since_str { conn.query(sql, &[&user_id, &s])? } else { conn.query(sql, &[&user_id])? };
                let mut out = Vec::new();
                for row in rows_iter.flatten() {
                    out.push(crate::db::models::AuditLog {
                        id: row.get(0).unwrap_or_default(),
                        user_id: row.get(1).unwrap_or_default(),
                        action: row.get(2).unwrap_or_default(),
                        table_name: row.get(3).unwrap_or_default(),
                        record_id: row.get(4).ok(),
                        details: row.get(5).ok(),
                        created_at: parse_utc(row.get(6).ok()),
                    });
                }
                Ok(out)
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
            Ok(rows)
        }
        DbPoolEnum::Sqlite(p) => {
            let p = p.clone();
            let user_id = user_id.to_string();
            let rows = task::spawn_blocking(move || -> anyhow::Result<Vec<crate::db::models::AuditLog>> {
                let conn = p.get()?;
                let sql_with = "SELECT id, user_id, action, table_name, record_id, details, created_at FROM audit_logs WHERE user_id = ?1 AND created_at > ?2 ORDER BY created_at DESC";
                let sql_no = "SELECT id, user_id, action, table_name, record_id, details, created_at FROM audit_logs WHERE user_id = ?1 ORDER BY created_at DESC";
                let mut out = Vec::new();
                if let Some(since_dt) = since {
                    let since_str = since_dt.to_rfc3339();
                    let mut stmt = conn.prepare(sql_with)?;
                    let rows_iter = stmt.query_map(rusqlite::params![user_id, since_str], |row| {
                        let created_at_str: Option<String> = row.get(6)?;
                        let created_at = created_at_str
                            .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                            .map(|dt| dt.with_timezone(&chrono::Utc))
                            .unwrap_or_else(chrono::Utc::now);
                        Ok(crate::db::models::AuditLog {
                            id: row.get(0)?,
                            user_id: row.get(1)?,
                            action: row.get(2)?,
                            table_name: row.get(3)?,
                            record_id: row.get(4).ok(),
                            details: row.get(5).ok(),
                            created_at,
                        })
                    })?;
                    for r in rows_iter { out.push(r?); }
                } else {
                    let mut stmt = conn.prepare(sql_no)?;
                    let rows_iter = stmt.query_map(rusqlite::params![user_id], |row| {
                        let created_at_str: Option<String> = row.get(6)?;
                        let created_at = created_at_str
                            .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                            .map(|dt| dt.with_timezone(&chrono::Utc))
                            .unwrap_or_else(chrono::Utc::now);
                        Ok(crate::db::models::AuditLog {
                            id: row.get(0)?,
                            user_id: row.get(1)?,
                            action: row.get(2)?,
                            table_name: row.get(3)?,
                            record_id: row.get(4).ok(),
                            details: row.get(5).ok(),
                            created_at,
                        })
                    })?;
                    for r in rows_iter { out.push(r?); }
                }
                Ok(out)
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
            Ok(rows)
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
            let p = p.clone();
            let user_id = user_id.to_string();
            let name = name.to_string();
            let ein_cloned = ein.clone();
            let row = task::spawn_blocking(move || -> anyhow::Result<Option<crate::db::models::Charity>> {
                let conn = p.get()?;
                let parse_utc = |value: Option<String>| {
                    value
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(chrono::Utc::now)
                };
                let sql = if ein_cloned.as_ref().map(|s| !s.is_empty()).unwrap_or(false) {
                    "SELECT id, user_id, name, ein, category, status, classification, nonprofit_type, deductibility, street, city, state, zip, created_at, updated_at FROM charities WHERE user_id = :1 AND (ein = :2 OR LOWER(name) = LOWER(:3))"
                } else {
                    "SELECT id, user_id, name, ein, category, status, classification, nonprofit_type, deductibility, street, city, state, zip, created_at, updated_at FROM charities WHERE user_id = :1 AND LOWER(name) = LOWER(:2)"
                };
                let rows = if let Some(ein_val) = ein_cloned {
                    conn.query(sql, &[&user_id, &ein_val, &name])?
                } else {
                    conn.query(sql, &[&user_id, &name])?
                };
                if let Some(row) = rows.flatten().next() {
                    return Ok(Some(crate::db::models::Charity {
                        id: row.get(0).unwrap_or_default(),
                        user_id: row.get(1).unwrap_or_default(),
                        name: row.get(2).unwrap_or_default(),
                        ein: row.get(3).ok(),
                        category: row.get(4).ok(),
                        status: row.get(5).ok(),
                        classification: row.get(6).ok(),
                        nonprofit_type: row.get(7).ok(),
                        deductibility: row.get(8).ok(),
                        street: row.get(9).ok(),
                        city: row.get(10).ok(),
                        state: row.get(11).ok(),
                        zip: row.get(12).ok(),
                            created_at: parse_utc(row.get(13).ok()),
                            updated_at: parse_utc(row.get(14).ok()),
                    }));
                }
                Ok(None)
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
            Ok(row)
        }
        DbPoolEnum::Sqlite(p) => {
            let p = p.clone();
            let user_id = user_id.to_string();
            let name = name.to_string();
            let ein_cloned = ein.clone();
            let row = task::spawn_blocking(move || -> anyhow::Result<Option<crate::db::models::Charity>> {
                let conn = p.get()?;
                let sql = if ein_cloned.as_ref().map(|s| !s.is_empty()).unwrap_or(false) {
                    "SELECT id, user_id, name, ein, category, status, classification, nonprofit_type, deductibility, street, city, state, zip, created_at, updated_at FROM charities WHERE user_id = ?1 AND (ein = ?2 OR lower(name) = lower(?3))"
                } else {
                    "SELECT id, user_id, name, ein, category, status, classification, nonprofit_type, deductibility, street, city, state, zip, created_at, updated_at FROM charities WHERE user_id = ?1 AND lower(name) = lower(?2)"
                };
                let mut stmt = conn.prepare(sql)?;
                let mut rows_iter = if let Some(ein_val) = ein_cloned {
                    stmt.query(params![user_id, ein_val, name])?
                } else {
                    stmt.query(params![user_id, name])?
                };
                if let Some(row) = rows_iter.next()? {
                    let created_at_str: Option<String> = row.get(13)?;
                    let updated_at_str: Option<String> = row.get(14)?;
                    let created_at = created_at_str
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(chrono::Utc::now);
                    let updated_at = updated_at_str
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(chrono::Utc::now);
                    return Ok(Some(crate::db::models::Charity {
                        id: row.get(0)?,
                        user_id: row.get(1)?,
                        name: row.get(2)?,
                        ein: row.get(3).ok(),
                        category: row.get(4).ok(),
                        status: row.get(5).ok(),
                        classification: row.get(6).ok(),
                        nonprofit_type: row.get(7).ok(),
                        deductibility: row.get(8).ok(),
                        street: row.get(9).ok(),
                        city: row.get(10).ok(),
                        state: row.get(11).ok(),
                        zip: row.get(12).ok(),
                        created_at,
                        updated_at,
                    }));
                }
                Ok(None)
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
            Ok(row)
        }
    }
}

pub async fn create_charity(pool: &DbPool, input: &crate::db::models::NewCharity) -> anyhow::Result<()> {
    let input = input.clone();
    let created_at_str = input.created_at.to_rfc3339();

    match &**pool {
        DbPoolEnum::Oracle(p) => {
            let p = p.clone();
            let insert_input = input.clone();
            let insert_created_at = created_at_str.clone();
            task::spawn_blocking(move || -> anyhow::Result<()> {
                let conn = p.get()?;
                let sql = "INSERT INTO charities (id, user_id, name, ein, category, status, classification, nonprofit_type, deductibility, street, city, state, zip, created_at) VALUES (:1, :2, :3, :4, :5, :6, :7, :8, :9, :10, :11, :12, :13, :14)";
                conn.execute(sql, &[&insert_input.id, &insert_input.user_id, &insert_input.name, &insert_input.ein, &insert_input.category, &insert_input.status, &insert_input.classification, &insert_input.nonprofit_type, &insert_input.deductibility, &insert_input.street, &insert_input.city, &insert_input.state, &insert_input.zip, &insert_created_at])?;
                let _ = conn.commit();
                Ok(())
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
        }
        DbPoolEnum::Sqlite(p) => {
            let p = p.clone();
            let insert_input = input.clone();
            let insert_created_at = created_at_str.clone();
            task::spawn_blocking(move || -> anyhow::Result<()> {
                let conn = p.get()?;
                let sql = "INSERT INTO charities (id, user_id, name, ein, category, status, classification, nonprofit_type, deductibility, street, city, state, zip, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)";
                conn.execute(sql, params![insert_input.id, insert_input.user_id, insert_input.name, insert_input.ein, insert_input.category, insert_input.status, insert_input.classification, insert_input.nonprofit_type, insert_input.deductibility, insert_input.street, insert_input.city, insert_input.state, insert_input.zip, insert_created_at])?;
                Ok(())
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
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

