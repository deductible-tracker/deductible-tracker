use r2d2::Pool;
use r2d2_oracle::OracleConnectionManager;
use std::env;

pub mod models;

pub type DbPool = Pool<OracleConnectionManager>;

pub async fn init_pool() -> anyhow::Result<DbPool> {
    let username = env::var("DB_USER").expect("DB_USER must be set");
    let password = env::var("DB_PASSWORD").expect("DB_PASSWORD must be set");
    let conn_str = env::var("DB_CONNECT_STRING").expect("DB_CONNECT_STRING must be set");
    
    let manager = OracleConnectionManager::new(&username, &password, &conn_str);
    let pool = Pool::builder()
        // Increase pool size and timeout to tolerate transient connectivity delays
        .max_size(10)
        .connection_timeout(std::time::Duration::from_secs(60))
        .build(manager)
        .map_err(|e| anyhow::anyhow!("Failed to create DB pool: {}", e))?;
    
    Ok(pool)
}