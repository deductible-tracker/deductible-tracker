use deductible_tracker::db::oracle::{connect_once, load_config};
use deductible_tracker::db::RuntimeMode;
use std::env;
use std::fs;
use std::path::Path;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env if it exists
    dotenvy::dotenv().ok();

    println!("Starting database migration...");

    let runtime_mode = RuntimeMode::from_env()?;

    let config = load_config(runtime_mode)?;

    if let Some(wallet_dir) = config.wallet_dir.as_deref() {
        println!("Wallet directory: {}", wallet_dir.display());
        match fs::read_dir(wallet_dir) {
            Ok(entries) => {
                println!("Wallet files:");
                for entry in entries.flatten() {
                    let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                    println!("  - {} ({} bytes)", entry.path().display(), size);
                }
            }
            Err(e) => {
                println!(
                    "ERROR: Cannot read wallet directory '{}': {}",
                    wallet_dir.display(),
                    e
                );
            }
        }
    } else if runtime_mode == RuntimeMode::Production {
        println!("WARNING: Oracle wallet directory is not configured");
    }

    println!("Connecting to database (60s timeout)...");
    let conn = connect_once(runtime_mode).await?;

    let args: Vec<String> = env::args().collect();
    let run_seed = args.contains(&"--seed".to_string())
        || env::var("RUN_SEED")
            .map(|value| value.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

    let migration_path =
        env::var("MIGRATION_FILE").unwrap_or_else(|_| "migrations/init.sql".to_string());
    if !Path::new(&migration_path).exists() {
        println!("Migration file not found at: {}", migration_path);
        return Ok(());
    }

    let mut files_to_run = vec![migration_path];
    if run_seed {
        let seed_path = "migrations/seed_valuations.sql";
        if Path::new(seed_path).exists() {
            println!("Including seed migration: {}", seed_path);
            files_to_run.push(seed_path.to_string());
        } else {
            println!("Warning: Seed file not found at: {}", seed_path);
        }
    }

    for file_path in files_to_run {
        println!("Running migration: {}", file_path);
        let sql_content = fs::read_to_string(&file_path)?;

        let statements: Vec<&str> = sql_content
            .split(';')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        for sql in statements {
            println!("Executing: {:.50}...", sql);
            match conn.execute(sql, &[]).await {
                Ok(_) => println!("Success."),
                Err(e) => {
                    let err_msg = e.to_string();
                    if err_msg.contains("ORA-00955")
                        || err_msg.contains("ORA-02275")
                        || err_msg.contains("ORA-02298")
                        || err_msg.contains("ORA-00001")
                    // Unique constraint violation (for seeds)
                    {
                        println!("Skipping (Table/Object already exists or duplicate seed).");
                    } else {
                        eprintln!("Error executing statement: {}", e);
                        return Err(anyhow::anyhow!("Migration failed: {}", e));
                    }
                }
            }
        }
    }

    conn.commit().await?;
    println!("Migration complete and committed (Oracle).");
    Ok(())
}
