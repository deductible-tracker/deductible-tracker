use r2d2::Pool;
use r2d2_oracle::OracleConnectionManager;
use std::env;

pub mod models;

pub type DbPool = Pool<OracleConnectionManager>;

pub async fn init_pool() -> anyhow::Result<DbPool> {
    let username = env::var("DB_USER").expect("DB_USER must be set");
    let password = env::var("DB_PASSWORD").expect("DB_PASSWORD must be set");
    let conn_str = env::var("DB_CONNECT_STRING").expect("DB_CONNECT_STRING must be set");
    
    eprintln!("[DB] Initializing Oracle connection pool");
    eprintln!("[DB] Username: {}", username);
    eprintln!("[DB] Connect string length: {} chars", conn_str.len());
    
    if let Ok(tns_admin) = env::var("TNS_ADMIN") {
        eprintln!("[DB] TNS_ADMIN: {}", tns_admin);
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