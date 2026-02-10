use r2d2::Pool;
use r2d2_oracle::OracleConnectionManager;
use std::env;

pub mod models;

pub type DbPool = Pool<OracleConnectionManager>;

pub async fn init_pool() -> anyhow::Result<DbPool> {
    let username = env::var("DB_USER").expect("Environment variable DB_USER must be set to the Oracle database username (used to initialize the DB connection pool). Set DB_USER in your environment or deployment configuration before starting this service.");
    let password = env::var("DB_PASSWORD").expect("Environment variable DB_PASSWORD must be set to the Oracle database user's password (used to initialize the DB connection pool). Set DB_PASSWORD in your environment or deployment configuration before starting this service.");
    let conn_str = env::var("DB_CONNECT_STRING").expect("Environment variable DB_CONNECT_STRING must be set to the Oracle database connect string (for example, host/service or TNS name, used to initialize the DB connection pool). Set DB_CONNECT_STRING in your environment or deployment configuration before starting this service.");
    
    eprintln!("[DB] Initializing Oracle connection pool");
    eprintln!("[DB] Using configured database user");
    eprintln!("[DB] Connect string length: {} chars", conn_str.len());
    
    if env::var("TNS_ADMIN").is_ok() {
        eprintln!("[DB] TNS_ADMIN is set");
    }
    if let Ok(wallet_dir) = env::var("MY_WALLET_DIRECTORY") {
        eprintln!("[DB] MY_WALLET_DIRECTORY: {}", wallet_dir);
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