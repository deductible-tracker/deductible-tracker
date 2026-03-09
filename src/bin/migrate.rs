use std::env;
use std::fs;
use std::path::Path;
use deductible_tracker::db::RuntimeMode;
use deductible_tracker::db::oracle::{initialize_client, load_config, OracleConnectionManager};
use r2d2::Pool;

fn main() -> anyhow::Result<()> {
    // Load .env if it exists
    dotenvy::dotenv().ok();

    println!("Starting database migration...");

    let runtime_mode = RuntimeMode::from_env()?;

    let config = load_config(runtime_mode)?;
    initialize_client(&config)?;

    if let Some(tns_admin) = config.tns_admin.clone() {
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
    } else if runtime_mode == RuntimeMode::Production {
        println!("WARNING: TNS_ADMIN is not set — Oracle Net cannot find wallet/sqlnet.ora");
    }

    println!("Connecting to database (60s timeout)...");
    let manager = OracleConnectionManager::new(
        &config.username,
        &config.password,
        &config.connect_string,
    );
    let pool = Pool::builder()
        .max_size(1)
        .connection_timeout(std::time::Duration::from_secs(60))
        .build(manager)
        .map_err(|e| anyhow::anyhow!("Failed to create DB pool: {}", e))?;

    let conn = pool.get()?;

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
                if err_msg.contains("ORA-00955")
                    || err_msg.contains("ORA-02275")
                    || err_msg.contains("ORA-02298")
                {
                    println!("Skipping (Table/Object already exists).");
                } else {
                    eprintln!("Error executing statement: {}", e);
                    if !err_msg.contains("ORA-00955")
                        && !err_msg.contains("ORA-02275")
                        && !err_msg.contains("ORA-02298")
                    {
                        return Err(anyhow::anyhow!("Migration failed: {}", e));
                    }
                }
            }
        }
    }

    conn.commit()?;
    println!("Migration complete and committed (Oracle).");
    Ok(())
}
