use r2d2::Pool;
use r2d2_oracle::OracleConnectionManager;
use std::env;
use anyhow::anyhow;
use std::sync::Arc;
use tokio::task;
use serde_json::json;
use uuid::Uuid;

pub mod models;

// Add sqlite support for development environment
use rusqlite::params;
use r2d2_sqlite::SqliteConnectionManager as R2SqliteManager;

pub enum DbPoolEnum {
    Oracle(Pool<OracleConnectionManager>),
    Sqlite(r2d2::Pool<R2SqliteManager>),
}

pub type DbPool = Arc<DbPoolEnum>;

pub async fn init_pool() -> anyhow::Result<DbPool> {
    let env_mode = env::var("RUST_ENV").unwrap_or_else(|_| "development".to_string());
    let db_pool_max = env::var("DB_POOL_MAX_SIZE")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(32);
    let db_pool_min = env::var("DB_POOL_MIN_IDLE")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(4);
    let db_pool_timeout_secs = env::var("DB_POOL_CONNECTION_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(15);

    if env_mode == "production" {
        // Oracle
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

        // Best-effort schema compatibility for existing Oracle installs.
        // Ignore errors for columns that already exist.
        {
            let pool_for_migration = pool.clone();
            let _ = task::spawn_blocking(move || {
                if let Ok(conn) = pool_for_migration.get() {
                    let _ = conn.execute("ALTER TABLE users ADD (phone VARCHAR2(64))", &[]);
                    let _ = conn.execute("ALTER TABLE users ADD (tax_id VARCHAR2(64))", &[]);
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
            }).await;
        }

        eprintln!("[DB] Pool created successfully (Oracle)");
        Ok(Arc::new(DbPoolEnum::Oracle(pool)))
    } else {
        // Development - use SQLite
        eprintln!("[DB] Initializing SQLite connection pool (development mode)");
        // Use a file in the project dir named dev.db unless overridden
        let db_path = env::var("DEV_SQLITE_PATH").unwrap_or_else(|_| "dev.db".to_string());
        let manager = R2SqliteManager::file(&db_path);
        let pool = r2d2::Pool::builder()
            .max_size(db_pool_max.min(16))
            .min_idle(Some(db_pool_min.min(4)))
            .connection_timeout(std::time::Duration::from_secs(db_pool_timeout_secs))
            .build(manager)
            .map_err(|e| anyhow!("Failed to create SQLite pool: {}", e))?;

        // Run a quick migration to ensure tables exist
        let pool_clone = pool.clone();
        task::spawn_blocking(move || -> anyhow::Result<()> {
            let conn = pool_clone.get()?;
            // Create tables compatible with the Oracle schema (using SQLite types)
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS users (
                    id TEXT PRIMARY KEY,
                    email TEXT NOT NULL UNIQUE,
                    name TEXT,
                    phone TEXT,
                    tax_id TEXT,
                    filing_status TEXT,
                    agi REAL,
                    marginal_tax_rate REAL,
                    itemize_deductions INTEGER,
                    provider TEXT,
                    created_at TEXT DEFAULT (datetime('now')),
                    updated_at TEXT
                );

                CREATE TABLE IF NOT EXISTS donations (
                    id TEXT PRIMARY KEY,
                    user_id TEXT NOT NULL,
                    donation_year INTEGER,
                    donation_date TEXT,
                    donation_category TEXT,
                    donation_amount REAL,
                    charity_id TEXT NOT NULL,
                    notes TEXT,
                    created_at TEXT DEFAULT (datetime('now')),
                    updated_at TEXT,
                    deleted INTEGER DEFAULT 0,
                    FOREIGN KEY (user_id) REFERENCES users(id),
                    FOREIGN KEY (charity_id) REFERENCES charities(id)
                );

                CREATE TABLE IF NOT EXISTS charities (
                    id TEXT PRIMARY KEY,
                    user_id TEXT NOT NULL,
                    name TEXT NOT NULL,
                    ein TEXT,
                    category TEXT,
                    status TEXT,
                    classification TEXT,
                    nonprofit_type TEXT,
                    deductibility TEXT,
                    street TEXT,
                    city TEXT,
                    state TEXT,
                    zip TEXT,
                    created_at TEXT DEFAULT (datetime('now')),
                    updated_at TEXT,
                    FOREIGN KEY (user_id) REFERENCES users(id)
                );

                CREATE TABLE IF NOT EXISTS receipts (
                    id TEXT PRIMARY KEY,
                    donation_id TEXT NOT NULL,
                    key TEXT NOT NULL,
                    file_name TEXT,
                    content_type TEXT,
                    size INTEGER,
                    ocr_text TEXT,
                    ocr_date TEXT,
                    ocr_amount INTEGER,
                    ocr_status TEXT,
                    updated_at TEXT,
                    created_at TEXT DEFAULT (datetime('now')),
                    FOREIGN KEY (donation_id) REFERENCES donations(id)
                );

                CREATE INDEX IF NOT EXISTS idx_receipts_donation ON receipts(donation_id);
                CREATE INDEX IF NOT EXISTS idx_receipts_donation_key ON receipts(donation_id, key);

                CREATE TABLE IF NOT EXISTS val_categories (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    description TEXT,
                    created_at TEXT DEFAULT (datetime('now')),
                    updated_at TEXT
                );

                CREATE TABLE IF NOT EXISTS val_items (
                    id TEXT PRIMARY KEY,
                    category_id TEXT,
                    name TEXT NOT NULL,
                    suggested_min INTEGER,
                    suggested_max INTEGER,
                    created_at TEXT DEFAULT (datetime('now')),
                    updated_at TEXT,
                    FOREIGN KEY (category_id) REFERENCES val_categories(id)
                );

                CREATE TABLE IF NOT EXISTS audit_logs (
                    id TEXT PRIMARY KEY,
                    user_id TEXT NOT NULL,
                    action TEXT NOT NULL,
                    table_name TEXT NOT NULL,
                    record_id TEXT,
                    details TEXT,
                    created_at TEXT DEFAULT (datetime('now')),
                    updated_at TEXT,
                    FOREIGN KEY (user_id) REFERENCES users(id)
                );

                CREATE TABLE IF NOT EXISTS audit_revisions (
                    id TEXT PRIMARY KEY,
                    user_id TEXT,
                    table_name TEXT NOT NULL,
                    record_id TEXT NOT NULL,
                    operation TEXT NOT NULL,
                    old_values TEXT,
                    new_values TEXT,
                    created_at TEXT DEFAULT (datetime('now')),
                    updated_at TEXT,
                    FOREIGN KEY (user_id) REFERENCES users(id)
                );

                CREATE INDEX IF NOT EXISTS idx_donations_user_year ON donations(user_id, donation_year);
                CREATE INDEX IF NOT EXISTS idx_donations_user_updated_created ON donations(user_id, updated_at, created_at);
                CREATE INDEX IF NOT EXISTS idx_charities_user ON charities(user_id);
                CREATE UNIQUE INDEX IF NOT EXISTS idx_charities_user_name ON charities(user_id, lower(name));
                CREATE INDEX IF NOT EXISTS idx_audit_revisions_table_record ON audit_revisions(table_name, record_id, created_at);

                INSERT OR IGNORE INTO users (id, email, name, provider) VALUES ('dev-1','dev@local','Developer','local');
                INSERT OR IGNORE INTO users (id, email, name, provider) VALUES ('user-123','test@example.com','Test User','local');
                "
            )?;

            // Attempt to add newer columns to existing tables (safe to run repeatedly);
            // SQLite will error if the column already exists, so ignore errors here.
            let _ = conn.execute_batch("ALTER TABLE receipts ADD COLUMN ocr_text TEXT;");
            let _ = conn.execute_batch("ALTER TABLE receipts ADD COLUMN ocr_date TEXT;");
            let _ = conn.execute_batch("ALTER TABLE receipts ADD COLUMN ocr_amount INTEGER;");
            let _ = conn.execute_batch("ALTER TABLE receipts ADD COLUMN ocr_status TEXT;");
            let _ = conn.execute_batch("ALTER TABLE donations ADD COLUMN donation_category TEXT;");
            let _ = conn.execute_batch("ALTER TABLE donations ADD COLUMN donation_amount REAL;");
            let _ = conn.execute_batch("ALTER TABLE users ADD COLUMN phone TEXT;");
            let _ = conn.execute_batch("ALTER TABLE users ADD COLUMN tax_id TEXT;");
            let _ = conn.execute_batch("ALTER TABLE users ADD COLUMN filing_status TEXT;");
            let _ = conn.execute_batch("ALTER TABLE users ADD COLUMN agi REAL;");
            let _ = conn.execute_batch("ALTER TABLE users ADD COLUMN marginal_tax_rate REAL;");
            let _ = conn.execute_batch("ALTER TABLE users ADD COLUMN itemize_deductions INTEGER;");
            let _ = conn.execute_batch("ALTER TABLE users ADD COLUMN updated_at TEXT;");
            let _ = conn.execute_batch("ALTER TABLE charities ADD COLUMN category TEXT;");
            let _ = conn.execute_batch("ALTER TABLE charities ADD COLUMN status TEXT;");
            let _ = conn.execute_batch("ALTER TABLE charities ADD COLUMN classification TEXT;");
            let _ = conn.execute_batch("ALTER TABLE charities ADD COLUMN nonprofit_type TEXT;");
            let _ = conn.execute_batch("ALTER TABLE charities ADD COLUMN deductibility TEXT;");
            let _ = conn.execute_batch("ALTER TABLE charities ADD COLUMN street TEXT;");
            let _ = conn.execute_batch("ALTER TABLE charities ADD COLUMN city TEXT;");
            let _ = conn.execute_batch("ALTER TABLE charities ADD COLUMN state TEXT;");
            let _ = conn.execute_batch("ALTER TABLE charities ADD COLUMN zip TEXT;");
            let _ = conn.execute_batch("CREATE TABLE IF NOT EXISTS audit_revisions (id TEXT PRIMARY KEY, user_id TEXT, table_name TEXT NOT NULL, record_id TEXT NOT NULL, operation TEXT NOT NULL, old_values TEXT, new_values TEXT, created_at TEXT DEFAULT (datetime('now')), updated_at TEXT, FOREIGN KEY (user_id) REFERENCES users(id));");
            let _ = conn.execute_batch("ALTER TABLE receipts ADD COLUMN updated_at TEXT;");
            let _ = conn.execute_batch("ALTER TABLE audit_logs ADD COLUMN updated_at TEXT;");
            let _ = conn.execute_batch("ALTER TABLE val_categories ADD COLUMN created_at TEXT;");
            let _ = conn.execute_batch("ALTER TABLE val_categories ADD COLUMN updated_at TEXT;");
            let _ = conn.execute_batch("ALTER TABLE val_items ADD COLUMN created_at TEXT;");
            let _ = conn.execute_batch("ALTER TABLE val_items ADD COLUMN updated_at TEXT;");

            // Normalize legacy receipts schema (older dev DBs had receipts.user_id).
            // CREATE TABLE IF NOT EXISTS does not rewrite existing columns, so rebuild table when needed.
            let mut has_legacy_receipts_user_id = false;
            {
                let mut stmt = conn.prepare("PRAGMA table_info(receipts)")?;
                let mut rows = stmt.query([])?;
                while let Some(row) = rows.next()? {
                    let col_name: String = row.get(1)?;
                    if col_name.eq_ignore_ascii_case("user_id") {
                        has_legacy_receipts_user_id = true;
                        break;
                    }
                }
            }

            if has_legacy_receipts_user_id {
                conn.execute_batch(
                    "ALTER TABLE receipts RENAME TO receipts_legacy;

                    CREATE TABLE receipts (
                        id TEXT PRIMARY KEY,
                        donation_id TEXT NOT NULL,
                        key TEXT NOT NULL,
                        file_name TEXT,
                        content_type TEXT,
                        size INTEGER,
                        ocr_text TEXT,
                        ocr_date TEXT,
                        ocr_amount INTEGER,
                        ocr_status TEXT,
                        updated_at TEXT,
                        created_at TEXT DEFAULT (datetime('now')),
                        FOREIGN KEY (donation_id) REFERENCES donations(id)
                    );

                    INSERT INTO receipts (
                        id, donation_id, key, file_name, content_type, size,
                        ocr_text, ocr_date, ocr_amount, ocr_status, updated_at, created_at
                    )
                    SELECT
                        id,
                        donation_id,
                        key,
                        file_name,
                        content_type,
                        size,
                        ocr_text,
                        ocr_date,
                        ocr_amount,
                        ocr_status,
                        updated_at,
                        created_at
                    FROM receipts_legacy
                    WHERE donation_id IS NOT NULL;

                    DROP TABLE receipts_legacy;

                    CREATE INDEX IF NOT EXISTS idx_receipts_donation ON receipts(donation_id);
                    CREATE INDEX IF NOT EXISTS idx_receipts_donation_key ON receipts(donation_id, key);"
                )?;
            }

            Ok(())
        }).await.map_err(|e| anyhow!("Migration task join error: {}", e))??;

        eprintln!("[DB] SQLite pool created and migrated (path={})", db_path);
        Ok(Arc::new(DbPoolEnum::Sqlite(pool)))
    }
}

// High level helpers used by routes to avoid Oracle/SQLite API differences
use crate::db::models::Donation as DonationModel;
use chrono::Datelike;

pub async fn get_user_profile(
    pool: &DbPool,
    user_id: &str,
) -> anyhow::Result<Option<(String, String, String, Option<String>, Option<String>, Option<String>, Option<f64>, Option<f64>, Option<bool>)>> {
    match &**pool {
        DbPoolEnum::Oracle(p) => {
            let p = p.clone();
            let user_id = user_id.to_string();
            let row = task::spawn_blocking(move || -> anyhow::Result<Option<(String, String, String, Option<String>, Option<String>, Option<String>, Option<f64>, Option<f64>, Option<bool>)>> {
                let conn = p.get()?;
                let sql = "SELECT email, name, provider, phone, tax_id, filing_status, agi, marginal_tax_rate, itemize_deductions FROM users WHERE id = :1";
                let mut rows = conn.query(sql, &[&user_id])?;
                if let Some(r) = rows.next().transpose()? {
                    let email: String = r.get(0).unwrap_or_default();
                    let name: String = r.get(1).unwrap_or_default();
                    let provider: String = r.get(2).unwrap_or_else(|_| "local".to_string());
                    let phone: Option<String> = r.get(3).ok();
                    let tax_id: Option<String> = r.get(4).ok();
                    let filing_status: Option<String> = r.get(5).ok();
                    let agi: Option<f64> = r.get(6).ok();
                    let marginal_tax_rate: Option<f64> = r.get(7).ok();
                    let itemize_deductions_raw: Option<i64> = r.get(8).ok();
                    let itemize_deductions = itemize_deductions_raw.map(|v| v != 0);
                    return Ok(Some((email, name, provider, phone, tax_id, filing_status, agi, marginal_tax_rate, itemize_deductions)));
                }
                Ok(None)
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
            Ok(row)
        }
        DbPoolEnum::Sqlite(p) => {
            let p = p.clone();
            let user_id = user_id.to_string();
            let row = task::spawn_blocking(move || -> anyhow::Result<Option<(String, String, String, Option<String>, Option<String>, Option<String>, Option<f64>, Option<f64>, Option<bool>)>> {
                let conn = p.get()?;
                let sql = "SELECT email, name, provider, phone, tax_id, filing_status, agi, marginal_tax_rate, itemize_deductions FROM users WHERE id = ?1";
                let mut stmt = conn.prepare(sql)?;
                let mut rows = stmt.query(rusqlite::params![user_id])?;
                if let Some(r) = rows.next()? {
                    let email: String = r.get(0)?;
                    let name: String = r.get(1).unwrap_or_default();
                    let provider: String = r.get(2).unwrap_or_else(|_| "local".to_string());
                    let phone: Option<String> = r.get(3).ok();
                    let tax_id: Option<String> = r.get(4).ok();
                    let filing_status: Option<String> = r.get(5).ok();
                    let agi: Option<f64> = r.get(6).ok();
                    let marginal_tax_rate: Option<f64> = r.get(7).ok();
                    let itemize_deductions_raw: Option<i64> = r.get(8).ok();
                    let itemize_deductions = itemize_deductions_raw.map(|v| v != 0);
                    return Ok(Some((email, name, provider, phone, tax_id, filing_status, agi, marginal_tax_rate, itemize_deductions)));
                }
                Ok(None)
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
            Ok(row)
        }
    }
}

pub async fn upsert_user_profile(
    pool: &DbPool,
    user_id: &str,
    email: &str,
    name: &str,
    provider: &str,
    phone: &Option<String>,
    tax_id: &Option<String>,
    filing_status: &Option<String>,
    agi: &Option<f64>,
    marginal_tax_rate: &Option<f64>,
    itemize_deductions: &Option<bool>,
) -> anyhow::Result<()> {
    match &**pool {
        DbPoolEnum::Oracle(p) => {
            let p = p.clone();
            let user_id = user_id.to_string();
            let email = email.to_string();
            let name = name.to_string();
            let provider = provider.to_string();
            let phone = phone.clone();
            let tax_id = tax_id.clone();
            let filing_status = filing_status.clone();
            let agi = *agi;
            let marginal_tax_rate = *marginal_tax_rate;
            let itemize_deductions = itemize_deductions.map(|v| if v { 1 } else { 0 });
            task::spawn_blocking(move || -> anyhow::Result<()> {
                let conn = p.get()?;
                let sql = "MERGE INTO users u USING (SELECT :1 AS id, :2 AS email, :3 AS name, :4 AS provider, :5 AS phone, :6 AS tax_id, :7 AS filing_status, :8 AS agi, :9 AS marginal_tax_rate, :10 AS itemize_deductions FROM dual) s ON (u.id = s.id) WHEN MATCHED THEN UPDATE SET u.email = s.email, u.name = s.name, u.provider = s.provider, u.phone = s.phone, u.tax_id = s.tax_id, u.filing_status = s.filing_status, u.agi = s.agi, u.marginal_tax_rate = s.marginal_tax_rate, u.itemize_deductions = s.itemize_deductions WHEN NOT MATCHED THEN INSERT (id, email, name, provider, phone, tax_id, filing_status, agi, marginal_tax_rate, itemize_deductions) VALUES (s.id, s.email, s.name, s.provider, s.phone, s.tax_id, s.filing_status, s.agi, s.marginal_tax_rate, s.itemize_deductions)";
                conn.execute(sql, &[&user_id, &email, &name, &provider, &phone, &tax_id, &filing_status, &agi, &marginal_tax_rate, &itemize_deductions])?;
                let _ = conn.commit();
                Ok(())
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
            Ok(())
        }
        DbPoolEnum::Sqlite(p) => {
            let p = p.clone();
            let user_id = user_id.to_string();
            let email = email.to_string();
            let name = name.to_string();
            let provider = provider.to_string();
            let phone = phone.clone();
            let tax_id = tax_id.clone();
            let filing_status = filing_status.clone();
            let agi = *agi;
            let marginal_tax_rate = *marginal_tax_rate;
            let itemize_deductions = itemize_deductions.map(|v| if v { 1 } else { 0 });
            task::spawn_blocking(move || -> anyhow::Result<()> {
                let conn = p.get()?;
                let sql = "INSERT INTO users (id, email, name, provider, phone, tax_id, filing_status, agi, marginal_tax_rate, itemize_deductions) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10) ON CONFLICT(id) DO UPDATE SET email = excluded.email, name = excluded.name, provider = excluded.provider, phone = excluded.phone, tax_id = excluded.tax_id, filing_status = excluded.filing_status, agi = excluded.agi, marginal_tax_rate = excluded.marginal_tax_rate, itemize_deductions = excluded.itemize_deductions";
                conn.execute(sql, rusqlite::params![user_id, email, name, provider, phone, tax_id, filing_status, agi, marginal_tax_rate, itemize_deductions])?;
                Ok(())
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
            Ok(())
        }
    }
}

pub async fn add_donation(
    pool: &DbPool,
    id: &str,
    user_id: &str,
    year: i32,
    date: chrono::NaiveDate,
    category: &Option<String>,
    charity_id: &str,
    amount: &Option<f64>,
    notes: &Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
) -> anyhow::Result<()> {
    let id = id.to_string();
    let user_id = user_id.to_string();
    let category_cloned = category.clone();
    let charity_id = charity_id.to_string();
    let amount_cloned = *amount;
    let notes_cloned = notes.clone();
    let created_at_str = created_at.to_rfc3339();
    let date_str_for_audit = date.format("%Y-%m-%d").to_string();
    let id_for_revision = id.clone();
    let user_id_for_revision = user_id.clone();
    let category_for_revision = category_cloned.clone();
    let charity_id_for_revision = charity_id.clone();
    let notes_for_revision = notes_cloned.clone();
    let created_at_for_revision = created_at_str.clone();

    match &**pool {
        DbPoolEnum::Oracle(p) => {
            let p = p.clone();
            let id = id.clone();
            let user_id = user_id.clone();
            let charity_id = charity_id.clone();
            let notes = notes_cloned.clone();
            let date = date;
            let created_at_str = created_at_str.clone();
            task::spawn_blocking(move || -> anyhow::Result<()> {
                let conn = p.get()?;
                let sql = "INSERT INTO donations (id, user_id, donation_year, donation_date, donation_category, donation_amount, charity_id, notes, created_at) VALUES (:1, :2, :3, :4, :5, :6, :7, :8, :9)";
                conn.execute(sql, &[
                    &id,
                    &user_id,
                    &year,
                    &date,
                    &category_cloned,
                    &amount_cloned,
                    &charity_id,
                    &notes,
                    &created_at_str,
                ])?;
                let _ = conn.commit();
                Ok(())
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
        }
        DbPoolEnum::Sqlite(p) => {
            let p = p.clone();
            task::spawn_blocking(move || -> anyhow::Result<()> {
                let conn = p.get()?;
                let sql = "INSERT INTO donations (id, user_id, donation_year, donation_date, donation_category, donation_amount, charity_id, notes, created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)";
                let date_str = date.format("%Y-%m-%d").to_string();
                conn.execute(
                    sql,
                    params![id, user_id, year, date_str, category_cloned, amount_cloned, charity_id, notes_cloned, created_at_str],
                )?;
                Ok(())
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
        }
    };

    let revision_id = Uuid::new_v4().to_string();
    let user_for_revision = Some(user_id_for_revision.clone());
    let new_values = Some(
        json!({
            "id": id_for_revision,
            "user_id": user_id_for_revision,
            "donation_year": year,
            "donation_date": date_str_for_audit,
            "donation_category": category_for_revision,
            "donation_amount": amount_cloned,
            "charity_id": charity_id_for_revision,
            "notes": notes_for_revision,
            "created_at": created_at_for_revision,
            "deleted": false
        })
        .to_string(),
    );
    log_revision(
        pool,
        &revision_id,
        &user_for_revision,
        "donations",
        &id_for_revision,
        "create",
        &None,
        &new_values,
    )
    .await?;

    Ok(())
}

pub async fn add_receipt(
    pool: &DbPool,
    id: &str,
    donation_id: &str,
    key: &str,
    file_name: &Option<String>,
    content_type: &Option<String>,
    size: &Option<i64>,
    created_at: chrono::DateTime<chrono::Utc>,
) -> anyhow::Result<()> {
    let id = id.to_string();
    let donation_id = donation_id.to_string();
    let key = key.to_string();
    let file_name_cloned = file_name.clone();
    let content_type_cloned = content_type.clone();
    let size_cloned = *size;
    let created_at_str = created_at.to_rfc3339();
    let id_for_revision = id.clone();
    let donation_id_for_revision = donation_id.clone();
    let key_for_revision = key.clone();
    let file_name_for_revision = file_name_cloned.clone();
    let content_type_for_revision = content_type_cloned.clone();
    let created_at_for_revision = created_at_str.clone();

    let donation_owner = donation_owner_user_id(pool, &donation_id).await?;

    match &**pool {
        DbPoolEnum::Oracle(p) => {
            let p = p.clone();
            task::spawn_blocking(move || -> anyhow::Result<()> {
                let conn = p.get()?;
                let sql = "INSERT INTO receipts (id, donation_id, key, file_name, content_type, size, ocr_text, ocr_date, ocr_amount, ocr_status, created_at) VALUES (:1,:2,:3,:4,:5,:6,:7,:8,:9,:10,:11)";
                conn.execute(sql, &[
                    &id,
                    &donation_id,
                    &key,
                    &file_name_cloned,
                    &content_type_cloned,
                    &size_cloned,
                    &Option::<String>::None,
                    &Option::<String>::None,
                    &Option::<i64>::None,
                    &Option::<String>::None,
                    &created_at_str,
                ])?;
                let _ = conn.commit();
                Ok(())
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
        }
        DbPoolEnum::Sqlite(p) => {
            let p = p.clone();
            let file_name = file_name_cloned;
            let content_type = content_type_cloned;
            task::spawn_blocking(move || -> anyhow::Result<()> {
                let conn = p.get()?;
                let sql = "INSERT INTO receipts (id, donation_id, key, file_name, content_type, size, ocr_text, ocr_date, ocr_amount, ocr_status, created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)";
                conn.execute(
                    sql,
                    rusqlite::params![id, donation_id, key, file_name, content_type, size_cloned, Option::<String>::None, Option::<String>::None, Option::<i64>::None, Option::<String>::None, created_at_str],
                )?;
                Ok(())
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
        }
    };

    let revision_id = Uuid::new_v4().to_string();
    let new_values = Some(
        json!({
            "id": id_for_revision,
            "donation_id": donation_id_for_revision,
            "key": key_for_revision,
            "file_name": file_name_for_revision,
            "content_type": content_type_for_revision,
            "size": size_cloned,
            "ocr_text": null,
            "ocr_date": null,
            "ocr_amount": null,
            "ocr_status": null,
            "created_at": created_at_for_revision
        })
        .to_string(),
    );
    log_revision(
        pool,
        &revision_id,
        &donation_owner,
        "receipts",
        &id_for_revision,
        "create",
        &None,
        &new_values,
    )
    .await?;

    Ok(())
}

fn build_donation_revision_json(
    donation_id: &str,
    user_id: &str,
    donation_date: &str,
    donation_year: i32,
    donation_category: &Option<String>,
    donation_amount: &Option<f64>,
    charity_id: &str,
    notes: &Option<String>,
    deleted: bool,
    updated_at: Option<&str>,
) -> String {
    json!({
        "id": donation_id,
        "user_id": user_id,
        "donation_date": donation_date,
        "donation_year": donation_year,
        "donation_category": donation_category,
        "donation_amount": donation_amount,
        "charity_id": charity_id,
        "notes": notes,
        "deleted": deleted,
        "updated_at": updated_at,
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
        DbPoolEnum::Sqlite(p) => {
            let p = p.clone();
            let donation_id = donation_id.to_string();
            let user_id = task::spawn_blocking(move || -> anyhow::Result<Option<String>> {
                let conn = p.get()?;
                let sql = "SELECT user_id FROM donations WHERE id = ?1";
                let mut stmt = conn.prepare(sql)?;
                let mut rows = stmt.query(params![donation_id])?;
                if let Some(row) = rows.next()? {
                    let user_id: String = row.get(0)?;
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
        DbPoolEnum::Sqlite(p) => {
            let p = p.clone();
            let receipt_id = receipt_id.to_string();
            let user_id = task::spawn_blocking(move || -> anyhow::Result<Option<String>> {
                let conn = p.get()?;
                let sql = "SELECT d.user_id FROM receipts r JOIN donations d ON d.id = r.donation_id WHERE r.id = ?1";
                let mut stmt = conn.prepare(sql)?;
                let mut rows = stmt.query(params![receipt_id])?;
                if let Some(row) = rows.next()? {
                    let user_id: String = row.get(0)?;
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
        DbPoolEnum::Sqlite(p) => {
            let p = p.clone();
            let user_id = user_id.to_string();
            let donation_id = donation_id.to_string();
            let exists = task::spawn_blocking(move || -> anyhow::Result<bool> {
                let conn = p.get()?;
                let sql = "SELECT COUNT(1) FROM donations WHERE id = ?1 AND user_id = ?2";
                let count: i64 = conn.query_row(sql, params![donation_id, user_id], |row| row.get(0))?;
                Ok(count > 0)
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
            Ok(exists)
        }
    }
}

pub async fn list_receipts(pool: &DbPool, user_id: &str, donation_id: Option<String>) -> anyhow::Result<Vec<crate::db::models::Receipt>> {
    match &**pool {
        DbPoolEnum::Oracle(p) => {
            let p = p.clone();
            let user_id = user_id.to_string();
            let donation_id_cloned = donation_id.clone();
            let rows = task::spawn_blocking(move || -> anyhow::Result<Vec<crate::db::models::Receipt>> {
                let conn = p.get()?;
                let sql = if donation_id_cloned.is_some() {
                    "SELECT r.id, r.donation_id, r.key, r.file_name, r.content_type, r.size, r.ocr_text, r.ocr_date, r.ocr_amount, r.ocr_status, r.created_at FROM receipts r JOIN donations d ON d.id = r.donation_id WHERE d.user_id = :1 AND r.donation_id = :2"
                } else {
                    "SELECT r.id, r.donation_id, r.key, r.file_name, r.content_type, r.size, r.ocr_text, r.ocr_date, r.ocr_amount, r.ocr_status, r.created_at FROM receipts r JOIN donations d ON d.id = r.donation_id WHERE d.user_id = :1"
                };
                let rows_iter = if let Some(did) = donation_id_cloned {
                    conn.query(sql, &[&user_id, &did])?
                } else {
                    conn.query(sql, &[&user_id])?
                };

                let mut out = Vec::new();
                for row_result in rows_iter {
                    if let Ok(row) = row_result {
                        let r = crate::db::models::Receipt {
                            id: row.get(0).unwrap_or_default(),
                            donation_id: row.get(1).unwrap_or_default(),
                            key: row.get(2).unwrap_or_default(),
                            file_name: row.get(3).ok(),
                            content_type: row.get(4).ok(),
                            size: row.get(5).ok(),
                            ocr_text: row.get(6).ok(),
                            ocr_date: None,
                            ocr_amount: row.get(8).ok(),
                            ocr_status: row.get(9).ok(),
                            created_at: chrono::Utc::now(),
                        };
                        out.push(r);
                    }
                }
                Ok(out)
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
            Ok(rows)
        }
        DbPoolEnum::Sqlite(p) => {
            let p = p.clone();
            let user_id = user_id.to_string();
            let donation_id_cloned = donation_id.clone();
            let rows = task::spawn_blocking(move || -> anyhow::Result<Vec<crate::db::models::Receipt>> {
                let conn = p.get()?;
                let sql_with_donation = "SELECT r.id, r.donation_id, r.key, r.file_name, r.content_type, r.size, r.ocr_text, r.ocr_date, r.ocr_amount, r.ocr_status, r.created_at FROM receipts r JOIN donations d ON d.id = r.donation_id WHERE d.user_id = ?1 AND r.donation_id = ?2";
                let sql_no_donation = "SELECT r.id, r.donation_id, r.key, r.file_name, r.content_type, r.size, r.ocr_text, r.ocr_date, r.ocr_amount, r.ocr_status, r.created_at FROM receipts r JOIN donations d ON d.id = r.donation_id WHERE d.user_id = ?1";
                let mut out = Vec::new();
                if let Some(did) = donation_id_cloned {
                    let mut stmt = conn.prepare(sql_with_donation)?;
                    let rows_iter = stmt.query_map(rusqlite::params![user_id, did], |row| {
                        let created_at_str: Option<String> = row.get(10)?;
                        let created_at = created_at_str
                            .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                            .map(|dt| dt.with_timezone(&chrono::Utc))
                            .unwrap_or_else(|| chrono::Utc::now());
                        Ok(crate::db::models::Receipt {
                            id: row.get(0)?,
                            donation_id: row.get(1)?,
                            key: row.get(2)?,
                            file_name: row.get(3).ok(),
                            content_type: row.get(4).ok(),
                            size: row.get(5).ok(),
                            ocr_text: row.get(6).ok(),
                            ocr_date: None,
                            ocr_amount: row.get(8).ok(),
                            ocr_status: row.get(9).ok(),
                            created_at,
                        })
                    })?;
                    for r in rows_iter { out.push(r?); }
                } else {
                    let mut stmt = conn.prepare(sql_no_donation)?;
                    let rows_iter = stmt.query_map(rusqlite::params![user_id], |row| {
                        let created_at_str: Option<String> = row.get(10)?;
                        let created_at = created_at_str
                            .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                            .map(|dt| dt.with_timezone(&chrono::Utc))
                            .unwrap_or_else(|| chrono::Utc::now());
                        Ok(crate::db::models::Receipt {
                            id: row.get(0)?,
                            donation_id: row.get(1)?,
                            key: row.get(2)?,
                            file_name: row.get(3).ok(),
                            content_type: row.get(4).ok(),
                            size: row.get(5).ok(),
                            ocr_text: row.get(6).ok(),
                            ocr_date: None,
                            ocr_amount: row.get(8).ok(),
                            ocr_status: row.get(9).ok(),
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

pub async fn get_receipt(pool: &DbPool, user_id: &str, receipt_id: &str) -> anyhow::Result<Option<crate::db::models::Receipt>> {
    match &**pool {
        DbPoolEnum::Oracle(p) => {
            let p = p.clone();
            let user_id = user_id.to_string();
            let receipt_id = receipt_id.to_string();
            let row = task::spawn_blocking(move || -> anyhow::Result<Option<crate::db::models::Receipt>> {
                let conn = p.get()?;
                let sql = "SELECT r.id, r.donation_id, r.key, r.file_name, r.content_type, r.size, r.ocr_text, r.ocr_date, r.ocr_amount, r.ocr_status, r.created_at FROM receipts r JOIN donations d ON d.id = r.donation_id WHERE d.user_id = :1 AND r.id = :2";
                let mut rows = conn.query(sql, &[&user_id, &receipt_id])?;
                if let Some(r) = rows.next().transpose()? {
                    return Ok(Some(crate::db::models::Receipt {
                        id: r.get(0).unwrap_or_default(),
                        donation_id: r.get(1).unwrap_or_default(),
                        key: r.get(2).unwrap_or_default(),
                        file_name: r.get(3).ok(),
                        content_type: r.get(4).ok(),
                        size: r.get(5).ok(),
                        ocr_text: r.get(6).ok(),
                        ocr_date: None,
                        ocr_amount: r.get(8).ok(),
                        ocr_status: r.get(9).ok(),
                        created_at: chrono::Utc::now(),
                    }));
                }
                Ok(None)
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
            Ok(row)
        }
        DbPoolEnum::Sqlite(p) => {
            let p = p.clone();
            let user_id = user_id.to_string();
            let receipt_id = receipt_id.to_string();
            let row = task::spawn_blocking(move || -> anyhow::Result<Option<crate::db::models::Receipt>> {
                let conn = p.get()?;
                let sql = "SELECT r.id, r.donation_id, r.key, r.file_name, r.content_type, r.size, r.ocr_text, r.ocr_date, r.ocr_amount, r.ocr_status, r.created_at FROM receipts r JOIN donations d ON d.id = r.donation_id WHERE d.user_id = ?1 AND r.id = ?2";
                let mut stmt = conn.prepare(sql)?;
                let mut rows = stmt.query(rusqlite::params![user_id, receipt_id])?;
                if let Some(row) = rows.next()? {
                    let created_at_str: Option<String> = row.get(10)?;
                    let created_at = created_at_str
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(|| chrono::Utc::now());
                    return Ok(Some(crate::db::models::Receipt {
                        id: row.get(0)?,
                        donation_id: row.get(1)?,
                        key: row.get(2)?,
                        file_name: row.get(3).ok(),
                        content_type: row.get(4).ok(),
                        size: row.get(5).ok(),
                        ocr_text: row.get(6).ok(),
                        ocr_date: None,
                        ocr_amount: row.get(8).ok(),
                        ocr_status: row.get(9).ok(),
                        created_at,
                    }));
                }
                Ok(None)
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
            Ok(row)
        }
    }
}

pub async fn list_donations(pool: &DbPool, user_id: &str, year: Option<i32>) -> anyhow::Result<Vec<DonationModel>> {
    match &**pool {
        DbPoolEnum::Oracle(p) => {
            let p = p.clone();
            let user_id = user_id.to_string();
            let rows = task::spawn_blocking(move || -> anyhow::Result<Vec<_>> {
                let conn = p.get()?;
                let sql = if year.is_some() {
                    "SELECT d.id, d.user_id, d.donation_year, d.donation_date, d.donation_category, d.donation_amount, d.charity_id, c.name, c.ein, d.notes FROM donations d JOIN charities c ON c.id = d.charity_id WHERE d.user_id = :1 AND d.donation_year = :2 AND d.deleted = 0"
                } else {
                    "SELECT d.id, d.user_id, d.donation_year, d.donation_date, d.donation_category, d.donation_amount, d.charity_id, c.name, c.ein, d.notes FROM donations d JOIN charities c ON c.id = d.charity_id WHERE d.user_id = :1 AND d.deleted = 0"
                };
                let rows = if let Some(y) = year {
                    conn.query(sql, &[&user_id, &y])?
                } else {
                    conn.query(sql, &[&user_id])?
                };
                let mut out = Vec::new();
                for row_result in rows {
                    if let Ok(row) = row_result {
                        let d = DonationModel {
                            id: row.get(0).unwrap_or_default(),
                            user_id: row.get(1).unwrap_or_default(),
                            year: row.get(2).unwrap_or_default(),
                            date: row.get(3).unwrap_or_else(|_| chrono::Utc::now().date_naive()),
                            category: row.get(4).ok(),
                            amount: row.get(5).ok(),
                            charity_id: row.get(6).unwrap_or_default(),
                            charity_name: row.get(7).unwrap_or_default(),
                            charity_ein: row.get(8).ok(),
                            notes: row.get(9).ok(),
                            shared_with: None,
                            created_at: chrono::Utc::now(),
                            updated_at: chrono::Utc::now(),
                            deleted: false,
                        };
                        out.push(d);
                    }
                }
                Ok(out)
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
            Ok(rows)
        }
        DbPoolEnum::Sqlite(p) => {
            let user_id = user_id.to_string();
            let p = p.clone();
            let rows = task::spawn_blocking(move || -> anyhow::Result<Vec<DonationModel>> {
                let conn = p.get()?;
                let sql_with_year = "SELECT d.id, d.user_id, d.donation_year, d.donation_date, d.donation_category, d.donation_amount, d.charity_id, c.name, c.ein, d.notes FROM donations d JOIN charities c ON c.id = d.charity_id WHERE d.user_id = ?1 AND d.donation_year = ?2 AND d.deleted = 0";
                let sql_no_year = "SELECT d.id, d.user_id, d.donation_year, d.donation_date, d.donation_category, d.donation_amount, d.charity_id, c.name, c.ein, d.notes FROM donations d JOIN charities c ON c.id = d.charity_id WHERE d.user_id = ?1 AND d.deleted = 0";

                let mut out = Vec::new();
                if let Some(y) = year {
                    let mut stmt = conn.prepare(sql_with_year)?;
                    let rows_iter = stmt.query_map(params![user_id, y], |row| {
                        let date_str: Option<String> = row.get(3)?;
                        let date = date_str
                            .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok())
                            .unwrap_or_else(|| chrono::Utc::now().date_naive());
                        Ok(DonationModel {
                            id: row.get(0)?,
                            user_id: row.get(1)?,
                            year: row.get(2)?,
                            date,
                            category: row.get(4).ok(),
                            amount: row.get(5).ok(),
                            charity_id: row.get(6)?,
                            charity_name: row.get(7)?,
                            charity_ein: row.get(8).ok(),
                            notes: row.get(9).ok(),
                            shared_with: None,
                            created_at: chrono::Utc::now(),
                            updated_at: chrono::Utc::now(),
                            deleted: false,
                        })
                    })?;
                    for r in rows_iter {
                        out.push(r?);
                    }
                } else {
                    let mut stmt = conn.prepare(sql_no_year)?;
                    let rows_iter = stmt.query_map(params![user_id], |row| {
                        let date_str: Option<String> = row.get(3)?;
                        let date = date_str
                            .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok())
                            .unwrap_or_else(|| chrono::Utc::now().date_naive());
                        Ok(DonationModel {
                            id: row.get(0)?,
                            user_id: row.get(1)?,
                            year: row.get(2)?,
                            date,
                            category: row.get(4).ok(),
                            amount: row.get(5).ok(),
                            charity_id: row.get(6)?,
                            charity_name: row.get(7)?,
                            charity_ein: row.get(8).ok(),
                            notes: row.get(9).ok(),
                            shared_with: None,
                            created_at: chrono::Utc::now(),
                            updated_at: chrono::Utc::now(),
                            deleted: false,
                        })
                    })?;
                    for r in rows_iter {
                        out.push(r?);
                    }
                }
                Ok(out)
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
            Ok(rows)
        }
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

                let sql = "UPDATE donations SET deleted = 1, updated_at = :1 WHERE id = :2 AND user_id = :3";
                let _stmt = conn.execute(sql, &[&updated_at, &donation_id, &user_id])?;
                let _ = conn.commit();
                let mut cnt_rows = conn.query("SELECT COUNT(1) FROM donations WHERE id = :1 AND user_id = :2 AND deleted = 1", &[&donation_id, &user_id])?;
                if let Some(r) = cnt_rows.next().transpose()? {
                    let cnt: i64 = r.get(0).unwrap_or(0);
                    if cnt > 0 {
                        let old_values = build_donation_revision_json(
                            &donation_id,
                            &user_id,
                            &existing_date,
                            existing_year,
                            &existing_category,
                            &existing_amount,
                            &existing_charity_id,
                            &existing_notes,
                            existing_deleted == 1,
                            None,
                        );
                        let new_values = build_donation_revision_json(
                            &donation_id,
                            &user_id,
                            &existing_date,
                            existing_year,
                            &existing_category,
                            &existing_amount,
                            &existing_charity_id,
                            &existing_notes,
                            true,
                            Some(&updated_at),
                        );
                        return Ok(Some((old_values, new_values)));
                    }
                }
                Ok(None)
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
            if let Some((old_values, new_values)) = revision_payload {
                let revision_id = Uuid::new_v4().to_string();
                log_revision(
                    pool,
                    &revision_id,
                    &user_for_revision,
                    "donations",
                    &donation_id_for_revision,
                    "delete",
                    &Some(old_values),
                    &Some(new_values),
                )
                .await?;
                Ok(true)
            } else {
                Ok(false)
            }
        }
        DbPoolEnum::Sqlite(p) => {
            let p = p.clone();
            let user_id = user_id.to_string();
            let donation_id = donation_id.to_string();
            let donation_id_for_revision = donation_id.clone();
            let updated_at = chrono::Utc::now().to_rfc3339();
            let revision_payload = task::spawn_blocking(move || -> anyhow::Result<Option<(String, String)>> {
                let conn = p.get()?;
                let sql_sel = "SELECT donation_date, donation_year, donation_category, donation_amount, charity_id, notes, deleted FROM donations WHERE id = ?1 AND user_id = ?2";
                let mut stmt = conn.prepare(sql_sel)?;
                let mut rows = stmt.query(params![donation_id, user_id])?;
                let Some(row) = rows.next()? else {
                    return Ok(None);
                };

                let existing_date: String = row.get::<usize, Option<String>>(0)?.unwrap_or_default();
                let existing_year: i32 = row.get::<usize, Option<i32>>(1)?.unwrap_or(0);
                let existing_category: Option<String> = row.get(2).ok();
                let existing_amount: Option<f64> = row.get(3).ok();
                let existing_charity_id: String = row.get::<usize, Option<String>>(4)?.unwrap_or_default();
                let existing_notes: Option<String> = row.get(5).ok();
                let existing_deleted: i64 = row.get::<usize, Option<i64>>(6)?.unwrap_or(0);

                let sql = "UPDATE donations SET deleted = 1, updated_at = ?1 WHERE id = ?2 AND user_id = ?3";
                let rows = conn.execute(sql, params![updated_at, donation_id, user_id])?;
                if rows == 0 {
                    return Ok(None);
                }

                let old_values = build_donation_revision_json(
                    &donation_id,
                    &user_id,
                    &existing_date,
                    existing_year,
                    &existing_category,
                    &existing_amount,
                    &existing_charity_id,
                    &existing_notes,
                    existing_deleted == 1,
                    None,
                );
                let new_values = build_donation_revision_json(
                    &donation_id,
                    &user_id,
                    &existing_date,
                    existing_year,
                    &existing_category,
                    &existing_amount,
                    &existing_charity_id,
                    &existing_notes,
                    true,
                    Some(&updated_at),
                );
                Ok(Some((old_values, new_values)))
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
            if let Some((old_values, new_values)) = revision_payload {
                let revision_id = Uuid::new_v4().to_string();
                log_revision(
                    pool,
                    &revision_id,
                    &user_for_revision,
                    "donations",
                    &donation_id_for_revision,
                    "delete",
                    &Some(old_values),
                    &Some(new_values),
                )
                .await?;
                Ok(true)
            } else {
                Ok(false)
            }
        }
    }
}

pub async fn update_donation(
    pool: &DbPool,
    user_id: &str,
    donation_id: &str,
    date_opt: Option<chrono::NaiveDate>,
    year_opt: Option<i32>,
    category_opt: Option<&str>,
    charity_id_opt: Option<&str>,
    amount_opt: Option<f64>,
    notes: &Option<String>,
    incoming_updated_at: Option<chrono::DateTime<chrono::Utc>>,
) -> anyhow::Result<bool> {
    let category_owned = category_opt.map(|s| s.to_string());
    let charity_id_owned = charity_id_opt.map(|s| s.to_string());
    let user_for_revision = Some(user_id.to_string());

    match &**pool {
        DbPoolEnum::Oracle(p) => {
            let p = p.clone();
            let user_id = user_id.to_string();
            let donation_id = donation_id.to_string();
            let donation_id_for_revision = donation_id.clone();
            let incoming = incoming_updated_at.map(|d| d.to_rfc3339());
            let notes_cloned = notes.clone();
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
                    let old_values = build_donation_revision_json(
                        &donation_id,
                        &user_id,
                        &existing_date_str,
                        existing_year.unwrap_or(0),
                        &existing_category,
                        &existing_amount,
                        &existing_charity_id.clone().unwrap_or_default(),
                        &existing_notes,
                        false,
                        existing_updated.as_deref(),
                    );
                    let new_values = build_donation_revision_json(
                        &donation_id,
                        &user_id,
                        &new_date.format("%Y-%m-%d").to_string(),
                        new_year,
                        &new_category,
                        &new_amount,
                        &new_charity_id,
                        &new_notes,
                        false,
                        Some(&new_updated_at),
                    );

                    let sql = "UPDATE donations SET donation_date = :1, donation_year = :2, donation_category = :3, donation_amount = :4, charity_id = :5, notes = :6, updated_at = :7 WHERE id = :8 AND user_id = :9";
                    conn.execute(sql, &[&new_date, &new_year, &new_category, &new_amount, &new_charity_id, &new_notes, &new_updated_at, &donation_id, &user_id])?;
                    let _ = conn.commit();
                    return Ok(Some((old_values, new_values)));
                }
                Ok(None)
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
            if let Some((old_values, new_values)) = revision_payload {
                let revision_id = Uuid::new_v4().to_string();
                log_revision(
                    pool,
                    &revision_id,
                    &user_for_revision,
                    "donations",
                    &donation_id_for_revision,
                    "update",
                    &Some(old_values),
                    &Some(new_values),
                )
                .await?;
                Ok(true)
            } else {
                Ok(false)
            }
        }
        DbPoolEnum::Sqlite(p) => {
            let charity_id_owned = charity_id_opt.map(|s| s.to_string());
            let p = p.clone();
            let user_id = user_id.to_string();
            let donation_id = donation_id.to_string();
            let donation_id_for_revision = donation_id.clone();
            let incoming = incoming_updated_at.map(|d| d.to_rfc3339());
            let notes_cloned = notes.clone();
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

                    let old_values = build_donation_revision_json(
                        &donation_id,
                        &user_id,
                        &existing_date_str.clone().unwrap_or_default(),
                        existing_year.unwrap_or(0),
                        &existing_category,
                        &existing_amount,
                        &existing_charity_id.clone().unwrap_or_default(),
                        &existing_notes,
                        false,
                        existing_updated_at_str.as_deref(),
                    );
                    let new_values = build_donation_revision_json(
                        &donation_id,
                        &user_id,
                        &new_date.format("%Y-%m-%d").to_string(),
                        new_year,
                        &new_category,
                        &new_amount,
                        &new_charity_id,
                        &new_notes,
                        false,
                        Some(&new_updated_at),
                    );

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
                let revision_id = Uuid::new_v4().to_string();
                log_revision(
                    pool,
                    &revision_id,
                    &user_for_revision,
                    "donations",
                    &donation_id_for_revision,
                    "update",
                    &Some(old_values),
                    &Some(new_values),
                )
                .await?;
                Ok(true)
            } else {
                Ok(false)
            }
        }
    }
}

pub async fn list_donations_since(pool: &DbPool, user_id: &str, since: chrono::DateTime<chrono::Utc>) -> anyhow::Result<Vec<DonationModel>> {
    let since_str = since.to_rfc3339();
    match &**pool {
        DbPoolEnum::Oracle(p) => {
            let p = p.clone();
            let user_id = user_id.to_string();
            let rows = task::spawn_blocking(move || -> anyhow::Result<Vec<DonationModel>> {
                let conn = p.get()?;
                let sql = "SELECT d.id, d.user_id, d.donation_year, d.donation_date, d.donation_category, d.donation_amount, d.charity_id, c.name, c.ein, d.notes, d.created_at, d.updated_at, d.deleted FROM donations d JOIN charities c ON c.id = d.charity_id WHERE d.user_id = :1 AND (d.updated_at > :2 OR d.created_at > :2)";
                let rows = conn.query(sql, &[&user_id, &since_str])?;
                let mut out = Vec::new();
                for row_result in rows {
                    if let Ok(row) = row_result {
                        let d = DonationModel {
                            id: row.get(0).unwrap_or_default(),
                            user_id: row.get(1).unwrap_or_default(),
                            year: row.get(2).unwrap_or_default(),
                            date: row.get(3).unwrap_or_else(|_| chrono::Utc::now().date_naive()),
                            category: row.get(4).ok(),
                            amount: row.get(5).ok(),
                            charity_id: row.get(6).unwrap_or_default(),
                            charity_name: row.get(7).unwrap_or_default(),
                            charity_ein: row.get(8).ok(),
                            notes: row.get(9).ok(),
                            shared_with: None,
                            created_at: chrono::Utc::now(),
                            updated_at: chrono::Utc::now(),
                            deleted: row.get::<usize, i64>(12).unwrap_or(0) != 0,
                        };
                        out.push(d);
                    }
                }
                Ok(out)
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
            Ok(rows)
        }
        DbPoolEnum::Sqlite(p) => {
            let p = p.clone();
            let user_id = user_id.to_string();
            let rows = task::spawn_blocking(move || -> anyhow::Result<Vec<DonationModel>> {
                let conn = p.get()?;
                let sql = "SELECT d.id, d.user_id, d.donation_year, d.donation_date, d.donation_category, d.donation_amount, d.charity_id, c.name, c.ein, d.notes, d.created_at, d.updated_at, d.deleted FROM donations d JOIN charities c ON c.id = d.charity_id WHERE d.user_id = ?1 AND (d.updated_at > ?2 OR d.created_at > ?2)";
                let mut stmt = conn.prepare(sql)?;
                let mut out = Vec::new();
                let rows_iter = stmt.query_map(rusqlite::params![user_id, since_str], |row| {
                    let date_str: Option<String> = row.get(3)?;
                    let date = date_str
                        .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok())
                        .unwrap_or_else(|| chrono::Utc::now().date_naive());
                    let created_at_str: Option<String> = row.get(10)?;
                    let updated_at_str: Option<String> = row.get(11)?;
                    let created_at = created_at_str
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(|| chrono::Utc::now());
                    let updated_at = updated_at_str
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(|| chrono::Utc::now());
                    Ok(DonationModel {
                        id: row.get(0)?,
                        user_id: row.get(1)?,
                        year: row.get(2)?,
                        date,
                        category: row.get(4).ok(),
                        amount: row.get(5).ok(),
                        charity_id: row.get(6)?,
                        charity_name: row.get(7)?,
                        charity_ein: row.get(8).ok(),
                        notes: row.get(9).ok(),
                        shared_with: None,
                        created_at,
                        updated_at,
                        deleted: row.get::<usize, i64>(12)? != 0,
                    })
                })?;
                for r in rows_iter { out.push(r?); }
                Ok(out)
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
            Ok(rows)
        }
    }
}

pub async fn suggest_valuations(pool: &DbPool, query: &str) -> anyhow::Result<Vec<(String, Option<i64>, Option<i64>)>> {
    let q = format!("%{}%", query.to_lowercase());
    match &**pool {
        DbPoolEnum::Oracle(p) => {
            let p = p.clone();
            let q = q.clone();
            let rows = task::spawn_blocking(move || -> anyhow::Result<Vec<(String, Option<i64>, Option<i64>)>> {
                let conn = p.get()?;
                let sql = "SELECT name, suggested_min, suggested_max FROM val_items WHERE LOWER(name) LIKE :1";
                let mut out = Vec::new();
                let rows_iter = conn.query(sql, &[&q])?;
                for row_result in rows_iter {
                    if let Ok(row) = row_result {
                        out.push((row.get(0).unwrap_or_default(), row.get(1).ok(), row.get(2).ok()));
                    }
                }
                Ok(out)
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
            Ok(rows)
        }
        DbPoolEnum::Sqlite(p) => {
            let p = p.clone();
            let q = q.clone();
            let rows = task::spawn_blocking(move || -> anyhow::Result<Vec<(String, Option<i64>, Option<i64>)>> {
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

pub async fn list_charities(pool: &DbPool, user_id: &str) -> anyhow::Result<Vec<crate::db::models::Charity>> {
    match &**pool {
        DbPoolEnum::Oracle(p) => {
            let p = p.clone();
            let user_id = user_id.to_string();
            let rows = task::spawn_blocking(move || -> anyhow::Result<Vec<crate::db::models::Charity>> {
                let conn = p.get()?;
                let sql = "SELECT id, user_id, name, ein, category, status, classification, nonprofit_type, deductibility, street, city, state, zip, created_at, updated_at FROM charities WHERE user_id = :1";
                let rows = conn.query(sql, &[&user_id])?;
                let mut out = Vec::new();
                for row_result in rows {
                    if let Ok(row) = row_result {
                        let c = crate::db::models::Charity {
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
                            created_at: chrono::Utc::now(),
                            updated_at: chrono::Utc::now(),
                        };
                        out.push(c);
                    }
                }
                Ok(out)
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
            Ok(rows)
        }
        DbPoolEnum::Sqlite(p) => {
            let p = p.clone();
            let user_id = user_id.to_string();
            let rows = task::spawn_blocking(move || -> anyhow::Result<Vec<crate::db::models::Charity>> {
                let conn = p.get()?;
                let sql = "SELECT id, user_id, name, ein, category, status, classification, nonprofit_type, deductibility, street, city, state, zip, created_at, updated_at FROM charities WHERE user_id = ?1";
                let mut stmt = conn.prepare(sql)?;
                let rows_iter = stmt.query_map(params![user_id], |row| {
                    let created_at_str: Option<String> = row.get(13)?;
                    let updated_at_str: Option<String> = row.get(14)?;
                    let created_at = created_at_str
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(|| chrono::Utc::now());
                    let updated_at = updated_at_str
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(|| chrono::Utc::now());
                    Ok(crate::db::models::Charity {
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
                    })
                })?;
                let mut out = Vec::new();
                for r in rows_iter { out.push(r?); }
                Ok(out)
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
            Ok(rows)
        }
    }
}

pub async fn set_receipt_ocr(
    pool: &DbPool,
    receipt_id: &str,
    ocr_text: &Option<String>,
    ocr_date: &Option<chrono::NaiveDate>,
    ocr_amount: &Option<i64>,
    ocr_status: &Option<String>,
) -> anyhow::Result<bool> {
    let user_for_revision = receipt_owner_user_id(pool, receipt_id).await?;
    match &**pool {
        DbPoolEnum::Oracle(p) => {
            let p = p.clone();
            let receipt_id = receipt_id.to_string();
            let receipt_id_for_revision = receipt_id.clone();
            let text = ocr_text.clone();
            let o_date = ocr_date.map(|d| d.to_string());
            let amt = ocr_amount.clone();
            let status = ocr_status.clone();
            let revision_payload = task::spawn_blocking(move || -> anyhow::Result<Option<(String, String)>> {
                let conn = p.get()?;
                let mut existing_rows = conn.query("SELECT donation_id, key, file_name, content_type, size, ocr_text, ocr_date, ocr_amount, ocr_status, created_at FROM receipts WHERE id = :1", &[&receipt_id])?;
                let Some(existing) = existing_rows.next().transpose()? else {
                    return Ok(None);
                };

                let existing_donation_id: String = existing.get(0).unwrap_or_default();
                let existing_key: String = existing.get(1).unwrap_or_default();
                let existing_file_name: Option<String> = existing.get(2).ok();
                let existing_content_type: Option<String> = existing.get(3).ok();
                let existing_size: Option<i64> = existing.get(4).ok();
                let existing_ocr_text: Option<String> = existing.get(5).ok();
                let existing_ocr_date: Option<String> = existing.get(6).ok();
                let existing_ocr_amount: Option<i64> = existing.get(7).ok();
                let existing_ocr_status: Option<String> = existing.get(8).ok();
                let existing_created_at: Option<String> = existing.get(9).ok();

                let sql = "UPDATE receipts SET ocr_text = :1, ocr_date = :2, ocr_amount = :3, ocr_status = :4 WHERE id = :5";
                conn.execute(sql, &[&text, &o_date, &amt, &status, &receipt_id])?;
                let _ = conn.commit();
                let old_values = json!({
                    "id": receipt_id,
                    "donation_id": existing_donation_id,
                    "key": existing_key,
                    "file_name": existing_file_name,
                    "content_type": existing_content_type,
                    "size": existing_size,
                    "ocr_text": existing_ocr_text,
                    "ocr_date": existing_ocr_date,
                    "ocr_amount": existing_ocr_amount,
                    "ocr_status": existing_ocr_status,
                    "created_at": existing_created_at
                }).to_string();
                let new_values = json!({
                    "id": receipt_id,
                    "donation_id": existing_donation_id,
                    "key": existing_key,
                    "file_name": existing_file_name,
                    "content_type": existing_content_type,
                    "size": existing_size,
                    "ocr_text": text,
                    "ocr_date": o_date,
                    "ocr_amount": amt,
                    "ocr_status": status,
                    "created_at": existing_created_at
                }).to_string();
                Ok(Some((old_values, new_values)))
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
            if let Some((old_values, new_values)) = revision_payload {
                let revision_id = Uuid::new_v4().to_string();
                log_revision(
                    pool,
                    &revision_id,
                    &user_for_revision,
                    "receipts",
                    &receipt_id_for_revision,
                    "update",
                    &Some(old_values),
                    &Some(new_values),
                )
                .await?;
                Ok(true)
            } else {
                Ok(false)
            }
        }
        DbPoolEnum::Sqlite(p) => {
            let p = p.clone();
            let receipt_id = receipt_id.to_string();
            let receipt_id_for_revision = receipt_id.clone();
            let text = ocr_text.clone();
            let o_date = ocr_date.map(|d| d.format("%Y-%m-%d").to_string());
            let amt = ocr_amount.clone();
            let status = ocr_status.clone();
            let revision_payload = task::spawn_blocking(move || -> anyhow::Result<Option<(String, String)>> {
                let conn = p.get()?;
                let mut stmt = conn.prepare("SELECT donation_id, key, file_name, content_type, size, ocr_text, ocr_date, ocr_amount, ocr_status, created_at FROM receipts WHERE id = ?1")?;
                let mut rows = stmt.query(rusqlite::params![receipt_id])?;
                let Some(existing) = rows.next()? else {
                    return Ok(None);
                };

                let existing_donation_id: String = existing.get(0)?;
                let existing_key: String = existing.get(1)?;
                let existing_file_name: Option<String> = existing.get(2).ok();
                let existing_content_type: Option<String> = existing.get(3).ok();
                let existing_size: Option<i64> = existing.get(4).ok();
                let existing_ocr_text: Option<String> = existing.get(5).ok();
                let existing_ocr_date: Option<String> = existing.get(6).ok();
                let existing_ocr_amount: Option<i64> = existing.get(7).ok();
                let existing_ocr_status: Option<String> = existing.get(8).ok();
                let existing_created_at: Option<String> = existing.get(9).ok();

                let sql = "UPDATE receipts SET ocr_text = ?1, ocr_date = ?2, ocr_amount = ?3, ocr_status = ?4 WHERE id = ?5";
                let rows = conn.execute(sql, rusqlite::params![text, o_date, amt, status, receipt_id])?;
                if rows == 0 {
                    return Ok(None);
                }

                let old_values = json!({
                    "id": receipt_id,
                    "donation_id": existing_donation_id,
                    "key": existing_key,
                    "file_name": existing_file_name,
                    "content_type": existing_content_type,
                    "size": existing_size,
                    "ocr_text": existing_ocr_text,
                    "ocr_date": existing_ocr_date,
                    "ocr_amount": existing_ocr_amount,
                    "ocr_status": existing_ocr_status,
                    "created_at": existing_created_at
                }).to_string();
                let new_values = json!({
                    "id": receipt_id,
                    "donation_id": existing_donation_id,
                    "key": existing_key,
                    "file_name": existing_file_name,
                    "content_type": existing_content_type,
                    "size": existing_size,
                    "ocr_text": text,
                    "ocr_date": o_date,
                    "ocr_amount": amt,
                    "ocr_status": status,
                    "created_at": existing_created_at
                }).to_string();
                Ok(Some((old_values, new_values)))
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
            if let Some((old_values, new_values)) = revision_payload {
                let revision_id = Uuid::new_v4().to_string();
                log_revision(
                    pool,
                    &revision_id,
                    &user_for_revision,
                    "receipts",
                    &receipt_id_for_revision,
                    "update",
                    &Some(old_values),
                    &Some(new_values),
                )
                .await?;
                Ok(true)
            } else {
                Ok(false)
            }
        }
    }
}

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
    let record_id_cloned = record_id.clone();
    let details_cloned = details.clone();
    let created_at = chrono::Utc::now().to_rfc3339();
    match &**pool {
        DbPoolEnum::Oracle(p) => {
            let p = p.clone();
            task::spawn_blocking(move || -> anyhow::Result<()> {
                let conn = p.get()?;
                let sql = "INSERT INTO audit_logs (id, user_id, action, table_name, record_id, details, created_at) VALUES (:1,:2,:3,:4,:5,:6,:7)";
                conn.execute(sql, &[&id, &user_id, &action, &table_name, &record_id_cloned, &details_cloned, &created_at])?;
                let _ = conn.commit();
                Ok(())
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
            Ok(())
        }
        DbPoolEnum::Sqlite(p) => {
            let p = p.clone();
            task::spawn_blocking(move || -> anyhow::Result<()> {
                let conn = p.get()?;
                let sql = "INSERT INTO audit_logs (id, user_id, action, table_name, record_id, details, created_at) VALUES (?1,?2,?3,?4,?5,?6,?7)";
                conn.execute(sql, rusqlite::params![id, user_id, action, table_name, record_id_cloned, details_cloned, created_at])?;
                Ok(())
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
            Ok(())
        }
    }
}

#[allow(dead_code)]
pub async fn log_revision(
    pool: &DbPool,
    id: &str,
    user_id: &Option<String>,
    table_name: &str,
    record_id: &str,
    operation: &str,
    old_values: &Option<String>,
    new_values: &Option<String>,
) -> anyhow::Result<()> {
    let id = id.to_string();
    let user_id_cloned = user_id.clone();
    let table_name = table_name.to_string();
    let record_id = record_id.to_string();
    let operation = operation.to_string();
    let old_values_cloned = old_values.clone();
    let new_values_cloned = new_values.clone();
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
                let sql = if since_str.is_some() {
                    "SELECT id, user_id, action, table_name, record_id, details, created_at FROM audit_logs WHERE user_id = :1 AND created_at > :2 ORDER BY created_at DESC"
                } else {
                    "SELECT id, user_id, action, table_name, record_id, details, created_at FROM audit_logs WHERE user_id = :1 ORDER BY created_at DESC"
                };
                let rows_iter = if let Some(s) = since_str { conn.query(sql, &[&user_id, &s])? } else { conn.query(sql, &[&user_id])? };
                let mut out = Vec::new();
                for rr in rows_iter {
                    if let Ok(row) = rr {
                        out.push(crate::db::models::AuditLog {
                            id: row.get(0).unwrap_or_default(),
                            user_id: row.get(1).unwrap_or_default(),
                            action: row.get(2).unwrap_or_default(),
                            table_name: row.get(3).unwrap_or_default(),
                            record_id: row.get(4).ok(),
                            details: row.get(5).ok(),
                            created_at: chrono::Utc::now(),
                        });
                    }
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
                            .unwrap_or_else(|| chrono::Utc::now());
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
                            .unwrap_or_else(|| chrono::Utc::now());
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
                for row_result in rows {
                    if let Ok(row) = row_result {
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
                            created_at: chrono::Utc::now(),
                            updated_at: chrono::Utc::now(),
                        }));
                    }
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
                        .unwrap_or_else(|| chrono::Utc::now());
                    let updated_at = updated_at_str
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(|| chrono::Utc::now());
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

pub async fn create_charity(
    pool: &DbPool,
    id: &str,
    user_id: &str,
    name: &str,
    ein: &Option<String>,
    category: &Option<String>,
    status: &Option<String>,
    classification: &Option<String>,
    nonprofit_type: &Option<String>,
    deductibility: &Option<String>,
    street: &Option<String>,
    city: &Option<String>,
    state: &Option<String>,
    zip: &Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
) -> anyhow::Result<()> {
    let id = id.to_string();
    let user_id = user_id.to_string();
    let name = name.to_string();
    let ein_cloned = ein.clone();
    let category_cloned = category.clone();
    let status_cloned = status.clone();
    let classification_cloned = classification.clone();
    let nonprofit_type_cloned = nonprofit_type.clone();
    let deductibility_cloned = deductibility.clone();
    let street_cloned = street.clone();
    let city_cloned = city.clone();
    let state_cloned = state.clone();
    let zip_cloned = zip.clone();
    let created_at_str = created_at.to_rfc3339();
    let id_for_revision = id.clone();
    let user_id_for_revision = user_id.clone();
    let name_for_revision = name.clone();
    let ein_for_revision = ein_cloned.clone();
    let category_for_revision = category_cloned.clone();
    let status_for_revision = status_cloned.clone();
    let classification_for_revision = classification_cloned.clone();
    let nonprofit_type_for_revision = nonprofit_type_cloned.clone();
    let deductibility_for_revision = deductibility_cloned.clone();
    let street_for_revision = street_cloned.clone();
    let city_for_revision = city_cloned.clone();
    let state_for_revision = state_cloned.clone();
    let zip_for_revision = zip_cloned.clone();
    let created_at_for_revision = created_at_str.clone();

    match &**pool {
        DbPoolEnum::Oracle(p) => {
            let p = p.clone();
            task::spawn_blocking(move || -> anyhow::Result<()> {
                let conn = p.get()?;
                let sql = "INSERT INTO charities (id, user_id, name, ein, category, status, classification, nonprofit_type, deductibility, street, city, state, zip, created_at) VALUES (:1, :2, :3, :4, :5, :6, :7, :8, :9, :10, :11, :12, :13, :14)";
                conn.execute(sql, &[&id, &user_id, &name, &ein_cloned, &category_cloned, &status_cloned, &classification_cloned, &nonprofit_type_cloned, &deductibility_cloned, &street_cloned, &city_cloned, &state_cloned, &zip_cloned, &created_at_str])?;
                let _ = conn.commit();
                Ok(())
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
        }
        DbPoolEnum::Sqlite(p) => {
            let p = p.clone();
            task::spawn_blocking(move || -> anyhow::Result<()> {
                let conn = p.get()?;
                let sql = "INSERT INTO charities (id, user_id, name, ein, category, status, classification, nonprofit_type, deductibility, street, city, state, zip, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)";
                conn.execute(sql, params![id, user_id, name, ein_cloned, category_cloned, status_cloned, classification_cloned, nonprofit_type_cloned, deductibility_cloned, street_cloned, city_cloned, state_cloned, zip_cloned, created_at_str])?;
                Ok(())
            }).await.map_err(|e| anyhow!("DB task join error: {}", e))??;
        }
    };

    let revision_id = Uuid::new_v4().to_string();
    let user_for_revision = Some(user_id_for_revision.clone());
    let new_values = Some(json!({
        "id": id_for_revision,
        "user_id": user_id_for_revision,
        "name": name_for_revision,
        "ein": ein_for_revision,
        "category": category_for_revision,
        "status": status_for_revision,
        "classification": classification_for_revision,
        "nonprofit_type": nonprofit_type_for_revision,
        "deductibility": deductibility_for_revision,
        "street": street_for_revision,
        "city": city_for_revision,
        "state": state_for_revision,
        "zip": zip_for_revision,
        "created_at": created_at_for_revision
    }).to_string());
    log_revision(
        pool,
        &revision_id,
        &user_for_revision,
        "charities",
        &id_for_revision,
        "create",
        &None,
        &new_values,
    )
    .await?;

    Ok(())
}

pub async fn update_charity(
    pool: &DbPool,
    charity_id: &str,
    user_id: &str,
    name: &str,
    ein: &Option<String>,
    category: &Option<String>,
    status: &Option<String>,
    classification: &Option<String>,
    nonprofit_type: &Option<String>,
    deductibility: &Option<String>,
    street: &Option<String>,
    city: &Option<String>,
    state: &Option<String>,
    zip: &Option<String>,
    updated_at: chrono::DateTime<chrono::Utc>,
) -> anyhow::Result<bool> {
    let charity_id = charity_id.to_string();
    let user_id = user_id.to_string();
    let name = name.to_string();
    let ein_cloned = ein.clone();
    let category_cloned = category.clone();
    let status_cloned = status.clone();
    let classification_cloned = classification.clone();
    let nonprofit_type_cloned = nonprofit_type.clone();
    let deductibility_cloned = deductibility.clone();
    let street_cloned = street.clone();
    let city_cloned = city.clone();
    let state_cloned = state.clone();
    let zip_cloned = zip.clone();
    let updated_at_str = updated_at.to_rfc3339();
    let user_for_revision = Some(user_id.to_string());

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

                let sql = "UPDATE charities SET name = :1, ein = :2, category = :3, status = :4, classification = :5, nonprofit_type = :6, deductibility = :7, street = :8, city = :9, state = :10, zip = :11, updated_at = :12 WHERE id = :13 AND user_id = :14";
                conn.execute(sql, &[&name, &ein_cloned, &category_cloned, &status_cloned, &classification_cloned, &nonprofit_type_cloned, &deductibility_cloned, &street_cloned, &city_cloned, &state_cloned, &zip_cloned, &updated_at_str, &charity_id, &user_id])?;
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
                let revision_id = Uuid::new_v4().to_string();
                log_revision(
                    pool,
                    &revision_id,
                    &user_for_revision,
                    "charities",
                    &charity_id_for_revision,
                    "update",
                    &Some(old_values),
                    &Some(new_values),
                )
                .await?;
                Ok(true)
            } else {
                Ok(false)
            }
        }
        DbPoolEnum::Sqlite(p) => {
            let p = p.clone();
            let charity_id_for_revision = charity_id.clone();
            let revision_payload = task::spawn_blocking(move || -> anyhow::Result<Option<(String, String)>> {
                let conn = p.get()?;
                let mut existing_stmt = conn.prepare("SELECT name, ein, category, status, classification, nonprofit_type, deductibility, street, city, state, zip, created_at, updated_at FROM charities WHERE id = ?1 AND user_id = ?2")?;
                let mut existing_rows = existing_stmt.query(params![charity_id, user_id])?;
                let Some(existing) = existing_rows.next()? else {
                    return Ok(None);
                };
                let existing_name: String = existing.get(0)?;
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

                let sql = "UPDATE charities SET name = ?1, ein = ?2, category = ?3, status = ?4, classification = ?5, nonprofit_type = ?6, deductibility = ?7, street = ?8, city = ?9, state = ?10, zip = ?11, updated_at = ?12 WHERE id = ?13 AND user_id = ?14";
                let rows = conn.execute(
                    sql,
                    params![
                        name,
                        ein_cloned,
                        category_cloned,
                        status_cloned,
                        classification_cloned,
                        nonprofit_type_cloned,
                        deductibility_cloned,
                        street_cloned,
                        city_cloned,
                        state_cloned,
                        zip_cloned,
                        updated_at_str,
                        charity_id,
                        user_id
                    ],
                )?;
                if rows == 0 {
                    return Ok(None);
                }

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
                let revision_id = Uuid::new_v4().to_string();
                log_revision(
                    pool,
                    &revision_id,
                    &user_for_revision,
                    "charities",
                    &charity_id_for_revision,
                    "update",
                    &Some(old_values),
                    &Some(new_values),
                )
                .await?;
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
        DbPoolEnum::Sqlite(p) => {
            let p = p.clone();
            let user_id = user_id.to_string();
            let charity_id = charity_id.to_string();
            let count = task::spawn_blocking(move || -> anyhow::Result<i64> {
                let conn = p.get()?;
                let sql = "SELECT COUNT(1) FROM donations WHERE user_id = ?1 AND charity_id = ?2 AND deleted = 0";
                let val: i64 = conn.query_row(sql, params![user_id, charity_id], |row| row.get(0))?;
                Ok(val)
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
                let revision_id = Uuid::new_v4().to_string();
                log_revision(
                    pool,
                    &revision_id,
                    &user_for_revision,
                    "charities",
                    &charity_id_for_revision,
                    "delete",
                    &Some(old_values),
                    &None,
                )
                .await?;
                Ok(true)
            } else {
                Ok(false)
            }
        }
        DbPoolEnum::Sqlite(p) => {
            let p = p.clone();
            let user_id = user_id.to_string();
            let charity_id = charity_id.to_string();
            let charity_id_for_revision = charity_id.clone();
            let revision_payload = task::spawn_blocking(move || -> anyhow::Result<Option<String>> {
                let conn = p.get()?;
                let mut existing_stmt = conn.prepare("SELECT name, ein, category, status, classification, nonprofit_type, deductibility, street, city, state, zip, created_at, updated_at FROM charities WHERE id = ?1 AND user_id = ?2")?;
                let mut existing_rows = existing_stmt.query(params![charity_id, user_id])?;
                let Some(existing) = existing_rows.next()? else {
                    return Ok(None);
                };
                let existing_name: String = existing.get(0)?;
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

                let sql = "DELETE FROM charities WHERE id = ?1 AND user_id = ?2";
                let rows = conn.execute(sql, params![charity_id, user_id])?;
                if rows == 0 {
                    return Ok(None);
                }

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
                let revision_id = Uuid::new_v4().to_string();
                log_revision(
                    pool,
                    &revision_id,
                    &user_for_revision,
                    "charities",
                    &charity_id_for_revision,
                    "delete",
                    &Some(old_values),
                    &None,
                )
                .await?;
                Ok(true)
            } else {
                Ok(false)
            }
        }
    }
}