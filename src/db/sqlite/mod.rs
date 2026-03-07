use anyhow::anyhow;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager as R2SqliteManager;
use std::sync::Arc;
use tokio::task;

use crate::db::core::{DbPool, DbPoolEnum, UserProfileRow};
use crate::db::models::UserProfileUpsert;

pub(crate) mod donations;
pub(crate) mod receipts;

pub(crate) async fn init_pool(
    db_path: &str,
    db_pool_max: u32,
    db_pool_min: u32,
    db_pool_timeout_secs: u64,
) -> anyhow::Result<DbPool> {
    eprintln!("[DB] Initializing SQLite connection pool (development mode)");
    let manager = R2SqliteManager::file(db_path);
    let pool = Pool::builder()
        .max_size(db_pool_max.min(16))
        .min_idle(Some(db_pool_min.min(4)))
        .connection_timeout(std::time::Duration::from_secs(db_pool_timeout_secs))
        .build(manager)
        .map_err(|e| anyhow!("Failed to create SQLite pool: {}", e))?;

    let pool_clone = pool.clone();
    task::spawn_blocking(move || -> anyhow::Result<()> {
        let conn = pool_clone.get()?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS users (
                id TEXT PRIMARY KEY,
                email TEXT NOT NULL UNIQUE,
                name TEXT,
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
            INSERT OR IGNORE INTO users (id, email, name, provider) VALUES ('user-123','test@example.com','Test User','local');"
        )?;

        let _ = conn.execute_batch("ALTER TABLE receipts ADD COLUMN ocr_text TEXT;");
        let _ = conn.execute_batch("ALTER TABLE receipts ADD COLUMN ocr_date TEXT;");
        let _ = conn.execute_batch("ALTER TABLE receipts ADD COLUMN ocr_amount INTEGER;");
        let _ = conn.execute_batch("ALTER TABLE receipts ADD COLUMN ocr_status TEXT;");
        let _ = conn.execute_batch("ALTER TABLE donations ADD COLUMN donation_category TEXT;");
        let _ = conn.execute_batch("ALTER TABLE donations ADD COLUMN donation_amount REAL;");
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
    })
    .await
    .map_err(|e| anyhow!("Migration task join error: {}", e))??;

    eprintln!("[DB] SQLite pool created and migrated (path={})", db_path);
    Ok(Arc::new(DbPoolEnum::Sqlite(pool)))
}

pub(crate) async fn get_user_profile_by_email(
    pool: &Pool<R2SqliteManager>,
    email: &str,
) -> anyhow::Result<Option<(String, UserProfileRow)>> {
    let p = pool.clone();
    let email = email.to_string();
    let row = task::spawn_blocking(move || -> anyhow::Result<Option<(String, UserProfileRow)>> {
        let conn = p.get()?;
        let sql = "SELECT id, email, name, provider, filing_status, agi, marginal_tax_rate, itemize_deductions FROM users WHERE email = ?1";
        let mut stmt = conn.prepare(sql)?;
        let mut rows = stmt.query(rusqlite::params![email])?;
        if let Some(r) = rows.next()? {
            let id: String = r.get(0)?;
            let email: String = r.get(1)?;
            let name: String = r.get(2).unwrap_or_default();
            let provider: String = r.get(3).unwrap_or_else(|_| "local".to_string());
            let filing_status: Option<String> = r.get(4).ok();
            let agi: Option<f64> = r.get(5).ok();
            let marginal_tax_rate: Option<f64> = r.get(6).ok();
            let itemize_deductions_raw: Option<i64> = r.get(7).ok();
            let itemize_deductions = itemize_deductions_raw.map(|v| v != 0);
            return Ok(Some((id, (email, name, provider, filing_status, agi, marginal_tax_rate, itemize_deductions))));
        }
        Ok(None)
    })
    .await
    .map_err(|e| anyhow!("DB task join error: {}", e))??;
    Ok(row)
}

pub(crate) async fn get_user_profile(
    pool: &Pool<R2SqliteManager>,
    user_id: &str,
) -> anyhow::Result<Option<UserProfileRow>> {
    let p = pool.clone();
    let user_id = user_id.to_string();
    let row = task::spawn_blocking(move || -> anyhow::Result<Option<UserProfileRow>> {
        let conn = p.get()?;
        let sql = "SELECT email, name, provider, filing_status, agi, marginal_tax_rate, itemize_deductions FROM users WHERE id = ?1";
        let mut stmt = conn.prepare(sql)?;
        let mut rows = stmt.query(rusqlite::params![user_id])?;
        if let Some(r) = rows.next()? {
            let email: String = r.get(0)?;
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
    pool: &Pool<R2SqliteManager>,
    input: &UserProfileUpsert,
) -> anyhow::Result<()> {
    let p = pool.clone();
    let input = input.clone();
    task::spawn_blocking(move || -> anyhow::Result<()> {
        let conn = p.get()?;
        let itemize_deductions = input.itemize_deductions.map(|v| if v { 1 } else { 0 });
        let sql = "INSERT INTO users (id, email, name, provider, filing_status, agi, marginal_tax_rate, itemize_deductions) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) ON CONFLICT(id) DO UPDATE SET email = excluded.email, name = excluded.name, provider = excluded.provider, filing_status = excluded.filing_status, agi = excluded.agi, marginal_tax_rate = excluded.marginal_tax_rate, itemize_deductions = excluded.itemize_deductions";
        conn.execute(sql, rusqlite::params![input.user_id, input.email, input.name, input.provider, input.filing_status, input.agi, input.marginal_tax_rate, itemize_deductions])?;
        Ok(())
    })
    .await
    .map_err(|e| anyhow!("DB task join error: {}", e))??;
    Ok(())
}
