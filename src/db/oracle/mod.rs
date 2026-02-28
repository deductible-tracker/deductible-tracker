use anyhow::anyhow;
use r2d2::Pool;
use r2d2_oracle::OracleConnectionManager;
use std::env;
use std::sync::Arc;
use tokio::task;

use crate::db::core::{DbPool, DbPoolEnum, UserProfileRow};
use crate::db::models::UserProfileUpsert;

pub(crate) mod donations;
pub(crate) mod receipts;

pub(crate) async fn init_pool(
    db_pool_max: u32,
    db_pool_min: u32,
    db_pool_timeout_secs: u64,
) -> anyhow::Result<DbPool> {
    let username = env::var("DB_USER").map_err(|e| {
        anyhow!("Environment variable DB_USER must be set to the Oracle database username. Underlying error: {}", e)
    })?;
    let password = env::var("DB_PASSWORD").map_err(|e| {
        anyhow!("Environment variable DB_PASSWORD must be set to the Oracle database user's password. Underlying error: {}", e)
    })?;
    let conn_str = env::var("DB_CONNECT_STRING").map_err(|e| {
        anyhow!("Environment variable DB_CONNECT_STRING must be set to the Oracle database connect string. Underlying error: {}", e)
    })?;

    eprintln!("[DB] Initializing Oracle connection pool");
    eprintln!("[DB] Using configured database user");
    eprintln!("[DB] Connect string length: {} chars", conn_str.len());

    if let Ok(tns_admin) = env::var("TNS_ADMIN") {
        eprintln!("[DB] TNS_ADMIN is set: {}", tns_admin);
        match std::fs::read_dir(&tns_admin) {
            Ok(entries) => {
                eprintln!("[DB] Wallet directory contents:");
                for entry in entries.flatten() {
                    let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                    eprintln!("[DB]   {} ({} bytes)", entry.file_name().to_string_lossy(), size);
                }
            }
            Err(e) => {
                eprintln!("[DB] ERROR: Cannot read wallet directory '{}': {}", tns_admin, e);
            }
        }
    } else {
        eprintln!("[DB] WARNING: TNS_ADMIN is not set");
    }

    eprintln!("[DB] Creating connection manager...");
    let manager = OracleConnectionManager::new(&username, &password, &conn_str);

    eprintln!("[DB] Building pool...");
    let pool = Pool::builder()
        .max_size(db_pool_max)
        .min_idle(Some(db_pool_min))
        .connection_timeout(std::time::Duration::from_secs(db_pool_timeout_secs))
        .build(manager)
        .map_err(|e| {
            eprintln!("[DB] ERROR: Failed to create connection pool: {}", e);
            anyhow::anyhow!("Failed to create DB pool: {}", e)
        })?;

    {
        let pool_for_migration = pool.clone();
        let _ = task::spawn_blocking(move || {
            if let Ok(conn) = pool_for_migration.get() {
                let _ = conn.execute("ALTER TABLE users ADD (filing_status VARCHAR2(32))", &[]);
                let _ = conn.execute("ALTER TABLE users ADD (agi NUMBER(14,2))", &[]);
                let _ = conn.execute("ALTER TABLE users ADD (marginal_tax_rate NUMBER(6,4))", &[]);
                let _ = conn.execute("ALTER TABLE users ADD (itemize_deductions NUMBER(1))", &[]);
                let _ = conn.execute("ALTER TABLE users ADD (updated_at TIMESTAMP)", &[]);
                let _ = conn.execute("ALTER TABLE charities ADD (category VARCHAR2(255))", &[]);
                let _ = conn.execute("ALTER TABLE charities ADD (status VARCHAR2(255))", &[]);
                let _ = conn.execute("ALTER TABLE charities ADD (classification VARCHAR2(255))", &[]);
                let _ = conn.execute("ALTER TABLE charities ADD (nonprofit_type VARCHAR2(255))", &[]);
                let _ = conn.execute("ALTER TABLE charities ADD (deductibility VARCHAR2(64))", &[]);
                let _ = conn.execute("ALTER TABLE charities ADD (street VARCHAR2(255))", &[]);
                let _ = conn.execute("ALTER TABLE charities ADD (city VARCHAR2(120))", &[]);
                let _ = conn.execute("ALTER TABLE charities ADD (state VARCHAR2(16))", &[]);
                let _ = conn.execute("ALTER TABLE charities ADD (zip VARCHAR2(20))", &[]);
                let _ = conn.execute("ALTER TABLE donations ADD (donation_category VARCHAR2(32))", &[]);
                let _ = conn.execute("ALTER TABLE donations ADD (donation_amount NUMBER(12,2))", &[]);
                let _ = conn.execute("ALTER TABLE receipts ADD (updated_at TIMESTAMP)", &[]);
                let _ = conn.execute("ALTER TABLE audit_logs ADD (updated_at TIMESTAMP)", &[]);
                let _ = conn.execute("ALTER TABLE val_categories ADD (created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP)", &[]);
                let _ = conn.execute("ALTER TABLE val_categories ADD (updated_at TIMESTAMP)", &[]);
                let _ = conn.execute("ALTER TABLE val_items ADD (created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP)", &[]);
                let _ = conn.execute("ALTER TABLE val_items ADD (updated_at TIMESTAMP)", &[]);
                let _ = conn.execute("CREATE TABLE audit_revisions (id VARCHAR2(255) PRIMARY KEY, user_id VARCHAR2(255), table_name VARCHAR2(255) NOT NULL, record_id VARCHAR2(255) NOT NULL, operation VARCHAR2(16) NOT NULL, old_values CLOB, new_values CLOB, created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP, updated_at TIMESTAMP, CONSTRAINT fk_audit_revisions_user FOREIGN KEY (user_id) REFERENCES users(id))", &[]);
                let _ = conn.execute("CREATE INDEX idx_audit_revisions_table_record ON audit_revisions(table_name, record_id, created_at)", &[]);
                let _ = conn.commit();
            }
        })
        .await;
    }

    eprintln!("[DB] Pool created successfully (Oracle)");
    Ok(Arc::new(DbPoolEnum::Oracle(pool)))
}

pub(crate) async fn get_user_profile(
    pool: &Pool<OracleConnectionManager>,
    user_id: &str,
) -> anyhow::Result<Option<UserProfileRow>> {
    let p = pool.clone();
    let user_id = user_id.to_string();
    let row = task::spawn_blocking(move || -> anyhow::Result<Option<UserProfileRow>> {
        let conn = p.get()?;
        let sql = "SELECT email, name, provider, filing_status, agi, marginal_tax_rate, itemize_deductions FROM users WHERE id = :1";
        let mut rows = conn.query(sql, &[&user_id])?;
        if let Some(r) = rows.next().transpose()? {
            let email: String = r.get(0).unwrap_or_default();
            let name: String = r.get(1).unwrap_or_default();
            let provider: String = r.get(2).unwrap_or_else(|_| "local".to_string());
            let filing_status: Option<String> = r.get(3).ok();
            let agi: Option<f64> = r.get(4).ok();
            let marginal_tax_rate: Option<f64> = r.get(5).ok();
            let itemize_deductions_raw: Option<i64> = r.get(6).ok();
            let itemize_deductions = itemize_deductions_raw.map(|v| v != 0);
            return Ok(Some((email, name, provider, filing_status, agi, marginal_tax_rate, itemize_deductions)));
        }
        Ok(None)
    })
    .await
    .map_err(|e| anyhow!("DB task join error: {}", e))??;
    Ok(row)
}

pub(crate) async fn upsert_user_profile(
    pool: &Pool<OracleConnectionManager>,
    input: &UserProfileUpsert,
) -> anyhow::Result<()> {
    let p = pool.clone();
    let input = input.clone();
    task::spawn_blocking(move || -> anyhow::Result<()> {
        let conn = p.get()?;
        let itemize_deductions = input.itemize_deductions.map(|v| if v { 1 } else { 0 });
        let sql = "MERGE INTO users u USING (SELECT :1 AS id, :2 AS email, :3 AS name, :4 AS provider, :5 AS filing_status, :6 AS agi, :7 AS marginal_tax_rate, :8 AS itemize_deductions FROM dual) s ON (u.id = s.id) WHEN MATCHED THEN UPDATE SET u.email = s.email, u.name = s.name, u.provider = s.provider, u.filing_status = s.filing_status, u.agi = s.agi, u.marginal_tax_rate = s.marginal_tax_rate, u.itemize_deductions = s.itemize_deductions WHEN NOT MATCHED THEN INSERT (id, email, name, provider, filing_status, agi, marginal_tax_rate, itemize_deductions) VALUES (s.id, s.email, s.name, s.provider, s.filing_status, s.agi, s.marginal_tax_rate, s.itemize_deductions)";
        conn.execute(sql, &[&input.user_id, &input.email, &input.name, &input.provider, &input.filing_status, &input.agi, &input.marginal_tax_rate, &itemize_deductions])?;
        let _ = conn.commit();
        Ok(())
    })
    .await
    .map_err(|e| anyhow!("DB task join error: {}", e))??;
    Ok(())
}
