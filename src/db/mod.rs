use r2d2::Pool;
use r2d2_oracle::OracleConnectionManager;
use std::env;
use anyhow::anyhow;

pub mod models;

pub type DbPool = Pool<OracleConnectionManager>;

pub async fn init_pool() -> anyhow::Result<DbPool> {
    let username = env::var("DB_USER").map_err(|e| {
        anyhow!("Environment variable DB_USER must be set to the Oracle database username (used to initialize the DB connection pool). Set DB_USER in your environment or deployment configuration before starting this service. Underlying error: {}", e)
    })?;
    let password = env::var("DB_PASSWORD").map_err(|e| {
        anyhow!("Environment variable DB_PASSWORD must be set to the Oracle database user's password (used to initialize the DB connection pool). Set DB_PASSWORD in your environment or deployment configuration before starting this service. Underlying error: {}", e)
    })?;
    let conn_str = env::var("DB_CONNECT_STRING").map_err(|e| {
        anyhow!("Environment variable DB_CONNECT_STRING must be set to the Oracle database connect string (for example, host/service or TNS name, used to initialize the DB connection pool). Set DB_CONNECT_STRING in your environment or deployment configuration before starting this service. Underlying error: {}", e)
    })?;
    
    eprintln!("[DB] Initializing Oracle connection pool");
    eprintln!("[DB] Using configured database user");
    eprintln!("[DB] Connect string length: {} chars", conn_str.len());
    
    if let Ok(tns_admin) = env::var("TNS_ADMIN") {
        eprintln!("[DB] TNS_ADMIN is set: {}", tns_admin);
        // Verify wallet files are readable (catches SELinux / mount issues early)
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
                eprintln!("[DB]   This will cause connection timeouts. Check SELinux labels (:z on volume mount).");
            }
        }
    } else {
        eprintln!("[DB] WARNING: TNS_ADMIN is not set");
    }
    if let Ok(wallet_dir) = env::var("MY_WALLET_DIRECTORY") {
        eprintln!("[DB] MY_WALLET_DIRECTORY is set: {}", wallet_dir);
        // Check that the auto-login wallet file exists
        let cwallet_path = std::path::Path::new(&wallet_dir).join("cwallet.sso");
        if cwallet_path.exists() {
            let size = std::fs::metadata(&cwallet_path).map(|m| m.len()).unwrap_or(0);
            eprintln!("[DB] cwallet.sso found ({} bytes)", size);
        } else {
            eprintln!("[DB] ERROR: cwallet.sso NOT found at {}", cwallet_path.display());
        }
    } else {
        eprintln!("[DB] WARNING: MY_WALLET_DIRECTORY is not set");
    }
    
    eprintln!("[DB] Creating connection manager...");
    let manager = OracleConnectionManager::new(&username, &password, &conn_str);
    
    eprintln!("[DB] Building pool with 60s connection timeout...");
    let pool = Pool::builder()
        // Increase pool size and timeout to tolerate transient connectivity delays
        .max_size(10)
        .connection_timeout(std::time::Duration::from_secs(60))
        .build(manager)
        .map_err(|e| {
            eprintln!("[DB] ERROR: Failed to create connection pool: {}", e);
            anyhow::anyhow!("Failed to create DB pool: {}", e)
        })?;
    
    eprintln!("[DB] Pool created successfully");
    Ok(pool)
}