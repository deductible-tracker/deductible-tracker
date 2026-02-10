use r2d2_oracle::OracleConnectionManager;
use r2d2::Pool;
use std::env;
use std::fs;
use std::path::Path;

fn main() -> anyhow::Result<()> {
    // Load .env if it exists
    dotenvy::dotenv().ok();

    println!("Starting database migration...");

    let username = env::var("DB_USER").expect("DB_USER must be set");
    let password = env::var("DB_PASSWORD").expect("DB_PASSWORD must be set");
    let conn_str = env::var("DB_CONNECT_STRING").expect("DB_CONNECT_STRING must be set");
    
    // Debug: Check TNS_ADMIN and wallet directory
    if let Ok(tns_admin) = env::var("TNS_ADMIN") {
        println!("TNS_ADMIN: {}", tns_admin);
        if let Ok(entries) = fs::read_dir(&tns_admin) {
            println!("Wallet files:");
            for entry in entries.flatten() {
                println!("  - {}", entry.path().display());
            }
        }
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
    // In Docker, this will be at /app/migrations/init.sql
    let migration_path = env::var("MIGRATION_FILE").unwrap_or_else(|_| "migrations/init.sql".to_string());
    
    if !Path::new(&migration_path).exists() {
        println!("Migration file not found at: {}", migration_path);
        return Ok(());
    }

    let sql_content = fs::read_to_string(&migration_path)?;

    // Split by semicolon to handle multiple statements (Basic parser)
    // Oracle crate execute might not handle multiple statements in one go depending on driver
    let statements: Vec<&str> = sql_content
        .split(';')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    for sql in statements {
        println!("Executing: {:.50}...", sql);
        // Ignore "table already exists" errors (ORA-00955) for idempotency if strictly needed,
        // but explicit CREATE TABLE IF NOT EXISTS is not standard Oracle SQL (pre-23c).
        // For older Oracle versions, we usually wrap in PL/SQL block or catch error.
        
        // Simple approach: Try execute, print error but continue if it's "exists"
        match conn.execute(sql, &[]) {
            Ok(_) => println!("Success."),
            Err(e) => {
                let err_msg = e.to_string();
                if err_msg.contains("ORA-00955") {
                    println!("Skipping (Table/Object already exists).");
                } else {
                    eprintln!("Error executing statement: {}", e);
                    // Decide if we should fail or continue. Usually fail.
                    // But for "users" and "donations" colliding, we want to continue.
                    // Ideally we use a proper migration tool like refinery or diesel migrations.
                    // Given the constraints, I'll allow ORA-00955 to pass.
                    if !err_msg.contains("ORA-00955") {
                        return Err(anyhow::anyhow!("Migration failed: {}", e));
                    }
                }
            }
        }
    }

    conn.commit()?;
    println!("Migration complete and committed.");
    Ok(())
}
