use std::env;
use std::fs;
use std::path::Path;

fn main() -> anyhow::Result<()> {
    // Load .env if it exists
    dotenvy::dotenv().ok();

    println!("Starting database migration...");

    let env_mode = env::var("RUST_ENV").unwrap_or_else(|_| "development".to_string());

    if env_mode == "production" {
        // Oracle path (existing behavior)
        use r2d2_oracle::OracleConnectionManager;
        use r2d2::Pool;

        let username = env::var("DB_USER").expect("DB_USER must be set");
        let password = env::var("DB_PASSWORD").expect("DB_PASSWORD must be set");
        let conn_str = env::var("DB_CONNECT_STRING").expect("DB_CONNECT_STRING must be set");

        if let Ok(tns_admin) = env::var("TNS_ADMIN") {
            println!("TNS_ADMIN: {}", tns_admin);
            match fs::read_dir(&tns_admin) {
                Ok(entries) => {
                    println!("Wallet files:");
                    for entry in entries.flatten() {
                        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                        println!("  - {} ({} bytes)", entry.path().display(), size);
                    }
                }
                Err(e) => {
                    println!("ERROR: Cannot read wallet directory '{}': {}", tns_admin, e);
                    println!("  This is likely an SELinux or permissions issue.");
                    println!("  Ensure the volume mount uses ':z' for SELinux relabeling.");
                }
            }
        } else {
            println!("WARNING: TNS_ADMIN is not set — Oracle Net cannot find wallet/sqlnet.ora");
        }

        println!("Connecting to database (60s timeout)...");
        let manager = OracleConnectionManager::new(&username, &password, &conn_str);
        let pool = Pool::builder()
            .max_size(1)
            .connection_timeout(std::time::Duration::from_secs(60))
            .build(manager)
            .map_err(|e| anyhow::anyhow!("Failed to create DB pool: {}", e))?;

        let conn = pool.get()?;

        // Read SQL file
        let migration_path = env::var("MIGRATION_FILE").unwrap_or_else(|_| "migrations/init.sql".to_string());
        if !Path::new(&migration_path).exists() {
            println!("Migration file not found at: {}", migration_path);
            return Ok(());
        }
        let sql_content = fs::read_to_string(&migration_path)?;

        let statements: Vec<&str> = sql_content
            .split(';')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        for sql in statements {
            println!("Executing: {:.50}...", sql);
            match conn.execute(sql, &[]) {
                Ok(_) => println!("Success."),
                Err(e) => {
                    let err_msg = e.to_string();
                    if err_msg.contains("ORA-00955") {
                        println!("Skipping (Table/Object already exists).");
                    } else {
                        eprintln!("Error executing statement: {}", e);
                        if !err_msg.contains("ORA-00955") {
                            return Err(anyhow::anyhow!("Migration failed: {}", e));
                        }
                    }
                }
            }
        }

        conn.commit()?;
        println!("Migration complete and committed (Oracle).");
        Ok(())
    } else {
        // SQLite migration for development
        use r2d2_sqlite::SqliteConnectionManager;
        use r2d2::Pool as R2Pool;

        let db_path = env::var("DEV_SQLITE_PATH").unwrap_or_else(|_| "dev.db".to_string());
        println!("Initializing SQLite DB at {}", db_path);
        let manager = SqliteConnectionManager::file(&db_path);
        let pool = R2Pool::builder().max_size(1).build(manager).map_err(|e| anyhow::anyhow!("Failed to create SQLite pool: {}", e))?;
        let conn = pool.get()?;

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
                created_at TEXT DEFAULT (datetime('now')),
                updated_at TEXT,
                FOREIGN KEY (donation_id) REFERENCES donations(id)
            );

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
            CREATE INDEX IF NOT EXISTS idx_receipts_donation ON receipts(donation_id);
            CREATE INDEX IF NOT EXISTS idx_receipts_donation_key ON receipts(donation_id, key);
            CREATE INDEX IF NOT EXISTS idx_audit_revisions_table_record ON audit_revisions(table_name, record_id, created_at);

            INSERT OR IGNORE INTO users (id, email, name, provider) VALUES ('dev-1','dev@local','Developer','local');
            INSERT OR IGNORE INTO users (id, email, name, provider) VALUES ('user-123','test@example.com','Test User','local');
            "
        )?;

        let _ = conn.execute_batch("ALTER TABLE donations ADD COLUMN donation_category TEXT;");
        let _ = conn.execute_batch("ALTER TABLE donations ADD COLUMN donation_amount REAL;");
        let _ = conn.execute_batch("ALTER TABLE users ADD COLUMN phone TEXT;");
        let _ = conn.execute_batch("ALTER TABLE users ADD COLUMN tax_id TEXT;");
        let _ = conn.execute_batch("ALTER TABLE users ADD COLUMN filing_status TEXT;");
        let _ = conn.execute_batch("ALTER TABLE users ADD COLUMN agi REAL;");
        let _ = conn.execute_batch("ALTER TABLE users ADD COLUMN marginal_tax_rate REAL;");
        let _ = conn.execute_batch("ALTER TABLE users ADD COLUMN itemize_deductions INTEGER;");
        let _ = conn.execute_batch("ALTER TABLE users ADD COLUMN updated_at TEXT;");
        let _ = conn.execute_batch("ALTER TABLE receipts ADD COLUMN updated_at TEXT;");
        let _ = conn.execute_batch("ALTER TABLE audit_logs ADD COLUMN updated_at TEXT;");
        let _ = conn.execute_batch("ALTER TABLE val_categories ADD COLUMN created_at TEXT;");
        let _ = conn.execute_batch("ALTER TABLE val_categories ADD COLUMN updated_at TEXT;");
        let _ = conn.execute_batch("ALTER TABLE val_items ADD COLUMN created_at TEXT;");
        let _ = conn.execute_batch("ALTER TABLE val_items ADD COLUMN updated_at TEXT;");

        println!("SQLite migration complete.");
        Ok(())
    }
}
