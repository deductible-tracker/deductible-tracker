use anyhow::{anyhow, Context};
use chrono::{DateTime, NaiveDate, Utc};
use deadpool_oracle::{Pool, PoolBuilder};
use oracle_rs::{Config as OracleDriverConfig, Connection, Row, Value};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use crate::db::core::{DbPool, DbPoolEnum, RuntimeMode, UserProfileRow};
use crate::db::models::UserProfileUpsert;

pub(crate) mod charities;
pub mod donations;
pub(crate) mod receipts;
mod wallet_config;

use wallet_config::validate_wallet_password;

fn first_present_env(keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| env::var(key).ok().filter(|value| !value.is_empty()))
}

#[macro_export]
macro_rules! oracle_params {
    ($($value:expr),* $(,)?) => {{
        vec![$($crate::db::oracle::to_value($value)),*]
    }};
}

pub(crate) fn to_value<T>(value: T) -> Value
where
    T: Into<Value>,
{
    value.into()
}

pub struct OracleConfig {
    pub username: String,
    pub password: String,
    pub connect_string: String,
    pub wallet_dir: Option<PathBuf>,
    pub wallet_password: Option<String>,
    pub statement_cache_size: usize,
}

pub fn load_config(runtime_mode: RuntimeMode) -> anyhow::Result<OracleConfig> {
    let (username, password, connect_string) = match runtime_mode {
        RuntimeMode::Production => {
            let username = env::var("DB_USER").map_err(|e| {
                anyhow!(
                    "Environment variable DB_USER must be set. Underlying error: {}",
                    e
                )
            })?;
            let password = env::var("DB_PASSWORD").map_err(|e| {
                anyhow!(
                    "Environment variable DB_PASSWORD must be set. Underlying error: {}",
                    e
                )
            })?;
            let connect_string = env::var("DB_CONNECT_STRING").map_err(|e| {
                anyhow!(
                    "Environment variable DB_CONNECT_STRING must be set. Underlying error: {}",
                    e
                )
            })?;
            (username, password, connect_string)
        }
        RuntimeMode::Development => {
            let username = first_present_env(&["DEV_ORACLE_USER", "ORACLE_PDB_USER"]).ok_or_else(|| {
                anyhow!("In development, set DEV_ORACLE_USER or ORACLE_PDB_USER environment variable")
            })?;
            let password = first_present_env(&["DEV_ORACLE_PASSWORD", "ORACLE_PWD"]).ok_or_else(|| {
                anyhow!("In development, set DEV_ORACLE_PASSWORD or ORACLE_PWD environment variable")
            })?;
            let connect_string = first_present_env(&["DEV_ORACLE_CONNECT_STRING", "ORACLE_PDB_CONNECT_STRING"]).ok_or_else(|| {
                anyhow!("In development, set DEV_ORACLE_CONNECT_STRING or ORACLE_PDB_CONNECT_STRING environment variable")
            })?;
            (username, password, connect_string)
        }
    };

    let wallet_dir = first_present_env(&["DB_WALLET_DIR", "MY_WALLET_DIRECTORY", "TNS_ADMIN"])
        .map(PathBuf::from)
        .or_else(|| {
            extract_descriptor_value(&connect_string, "MY_WALLET_DIRECTORY").map(PathBuf::from)
        })
        .or_else(|| {
            if runtime_mode == RuntimeMode::Production {
                env::var("TNS_ADMIN").ok().map(PathBuf::from)
            } else {
                None
            }
        });

    Ok(OracleConfig {
        username,
        password,
        connect_string,
        wallet_dir,
        wallet_password: first_present_env(&["DB_WALLET_PASSWORD", "ORACLE_WALLET_PASSWORD"]),
        statement_cache_size: env::var("DB_STATEMENT_CACHE_SIZE")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(20),
    })
}

pub async fn connect_once(runtime_mode: RuntimeMode) -> anyhow::Result<Connection> {
    install_rustls_provider();
    let config = load_config(runtime_mode)?;
    let driver_config = build_driver_config(&config)?;
    Connection::connect_with_config(driver_config)
        .await
        .context("failed to connect to Oracle with oracle-rs")
}

pub(crate) async fn init_pool(
    runtime_mode: RuntimeMode,
    db_pool_max: u32,
    db_pool_min: u32,
    db_pool_timeout_secs: u64,
) -> anyhow::Result<DbPool> {
    install_rustls_provider();
    let config = load_config(runtime_mode)?;
    let driver_config = build_driver_config(&config)?;

    eprintln!("[DB] Initializing Oracle connection pool");
    eprintln!("[DB] Using configured database user");
    eprintln!(
        "[DB] Connect string length: {} chars",
        config.connect_string.len()
    );

    if let Some(wallet_dir) = config.wallet_dir.as_deref() {
        log_wallet_directory(wallet_dir);
    } else {
        eprintln!("[DB] Wallet directory is not configured");
    }

    eprintln!("[DB] Building pool...");
    if db_pool_min > 0 {
        eprintln!(
            "[DB] DB_POOL_MIN_IDLE={} ignored by deadpool-oracle (connections are created lazily)",
            db_pool_min
        );
    }
    let pool = PoolBuilder::new(driver_config)
        .max_size(db_pool_max as usize)
        .wait_timeout(Some(Duration::from_secs(db_pool_timeout_secs)))
        .create_timeout(Some(Duration::from_secs(db_pool_timeout_secs)))
        .recycle_timeout(Some(Duration::from_secs(5)))
        .build()
        .map_err(|e| {
            eprintln!("[DB] ERROR: Failed to create connection pool: {}", e);
            eprintln!("[DB] ERROR DEBUG: {:?}", e);
            anyhow::anyhow!("Failed to create DB pool: {}", e)
        })?;

    run_bootstrap_ddl(&pool).await?;

    eprintln!("[DB] Pool created successfully (Oracle)");
    Ok(Arc::new(DbPoolEnum::Oracle(pool)))
}

pub(crate) async fn get_user_profile_by_email(
    pool: &Pool,
    email: &str,
) -> anyhow::Result<Option<(String, UserProfileRow)>> {
    let conn = pool.get().await?;
    let sql = "SELECT id, email, name, provider, filing_status, agi, marginal_tax_rate, itemize_deductions, is_encrypted, encrypted_payload, vault_credential_id FROM users WHERE email = :1";
    let rows = conn
        .query(sql, &crate::oracle_params![email.to_string()])
        .await?;
    let Some(row) = rows.first() else {
        return Ok(None);
    };

    Ok(Some((
        row_string(row, 0),
        user_profile_row_from_row(row, 1),
    )))
}

pub(crate) async fn get_user_profile(
    pool: &Pool,
    user_id: &str,
) -> anyhow::Result<Option<UserProfileRow>> {
    let conn = pool.get().await?;
    let sql = "SELECT email, name, provider, filing_status, agi, marginal_tax_rate, itemize_deductions, is_encrypted, encrypted_payload, vault_credential_id FROM users WHERE id = :1";
    let rows = conn
        .query(sql, &crate::oracle_params![user_id.to_string()])
        .await?;
    let Some(row) = rows.first() else {
        return Ok(None);
    };

    Ok(Some(user_profile_row_from_row(row, 0)))
}

pub(crate) async fn upsert_user_profile(
    pool: &Pool,
    input: &UserProfileUpsert,
) -> anyhow::Result<()> {
    let conn = pool.get().await?;
    let itemize_deductions = input
        .itemize_deductions
        .map(|value| if value { 1 } else { 0 });
    let is_encrypted = input
        .is_encrypted
        .map(|value| if value { 1 } else { 0 });
    let sql = "MERGE INTO users u USING (SELECT :1 AS id, :2 AS email, :3 AS name, :4 AS provider, :5 AS filing_status, :6 AS agi, :7 AS marginal_tax_rate, :8 AS itemize_deductions, :9 AS is_encrypted, :10 AS encrypted_payload, :11 AS vault_credential_id FROM dual) s ON (u.id = s.id) WHEN MATCHED THEN UPDATE SET u.email = s.email, u.name = s.name, u.provider = s.provider, u.filing_status = s.filing_status, u.agi = s.agi, u.marginal_tax_rate = s.marginal_tax_rate, u.itemize_deductions = s.itemize_deductions, u.is_encrypted = s.is_encrypted, u.encrypted_payload = s.encrypted_payload, u.vault_credential_id = s.vault_credential_id WHEN NOT MATCHED THEN INSERT (id, email, name, provider, filing_status, agi, marginal_tax_rate, itemize_deductions, is_encrypted, encrypted_payload, vault_credential_id) VALUES (s.id, s.email, s.name, s.provider, s.filing_status, s.agi, s.marginal_tax_rate, s.itemize_deductions, s.is_encrypted, s.encrypted_payload, s.vault_credential_id)";
    conn.execute(
        sql,
        &crate::oracle_params![
            input.user_id.clone(),
            input.email.clone(),
            input.name.clone(),
            input.provider.clone(),
            input.filing_status.clone(),
            input.agi,
            input.marginal_tax_rate,
            itemize_deductions,
            is_encrypted,
            input.encrypted_payload.clone(),
            input.vault_credential_id.clone(),
        ],
    )
    .await?;
    conn.commit().await?;
    Ok(())
}

pub(crate) async fn delete_user_data(pool: &Pool, user_id: &str) -> anyhow::Result<()> {
    let conn = pool.get().await?;

    conn.execute(
        "DELETE FROM receipts WHERE donation_id IN (SELECT id FROM donations WHERE user_id = :1)",
        &crate::oracle_params![user_id.to_string()],
    )
    .await?;
    conn.execute(
        "DELETE FROM donations WHERE user_id = :1",
        &crate::oracle_params![user_id.to_string()],
    )
    .await?;
    conn.execute(
        "DELETE FROM charities WHERE user_id = :1",
        &crate::oracle_params![user_id.to_string()],
    )
    .await?;
    conn.execute(
        "DELETE FROM audit_logs WHERE user_id = :1",
        &crate::oracle_params![user_id.to_string()],
    )
    .await?;
    conn.execute(
        "DELETE FROM audit_revisions WHERE user_id = :1",
        &crate::oracle_params![user_id.to_string()],
    )
    .await?;
    conn.execute(
        "DELETE FROM users WHERE id = :1",
        &crate::oracle_params![user_id.to_string()],
    )
    .await?;

    conn.commit().await?;
    Ok(())
}

fn build_driver_config(config: &OracleConfig) -> anyhow::Result<OracleDriverConfig> {
    let connect_string = config.connect_string.trim();
    let descriptor = if connect_string.starts_with('(') {
        Some(connect_string.to_string())
    } else if connect_string.contains('/') || connect_string.contains(':') {
        None
    } else if let Some(wallet_dir) = config.wallet_dir.as_deref() {
        Some(resolve_tns_alias(connect_string, wallet_dir)?)
    } else {
        None
    };

    let mut driver_config = if let Some(ref descriptor) = descriptor {
        build_driver_config_from_descriptor(descriptor, config)?
    } else {
        let mut parsed = OracleDriverConfig::from_str(connect_string)
            .with_context(|| format!("unsupported Oracle connect string '{}': expected EZConnect, TNS alias, or descriptor", connect_string))?;
        parsed.set_username(config.username.clone());
        parsed.set_password(config.password.clone());
        parsed
    };

    let lower_connect_string = connect_string.to_ascii_lowercase();
    let tls_required = lower_connect_string.contains("tcps")
        || descriptor
            .as_deref()
            .and_then(|value| extract_descriptor_value(value, "PROTOCOL"))
            .map(|value| value.eq_ignore_ascii_case("tcps"))
            .unwrap_or(false)
        || matches!(driver_config.port, 2484 | 1522);

    if tls_required {
        driver_config = if let Some(wallet_dir) = config.wallet_dir.as_deref() {
            validate_wallet_password(wallet_dir, config.wallet_password.as_deref())?;
            driver_config
                .with_wallet(
                    wallet_dir.to_string_lossy(),
                    config.wallet_password.as_deref(),
                )
                .with_context(|| {
                    format!(
                        "failed to configure wallet-based TLS using {}",
                        wallet_dir.display()
                    )
                })?
        } else {
            driver_config
                .with_tls()
                .context("failed to enable TLS for Oracle connection")?
        };
    }

    Ok(driver_config.with_statement_cache_size(config.statement_cache_size))
}

#[cfg(feature = "server")]
fn install_rustls_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

#[cfg(not(feature = "server"))]
fn install_rustls_provider() {}

fn build_driver_config_from_descriptor(
    descriptor: &str,
    config: &OracleConfig,
) -> anyhow::Result<OracleDriverConfig> {
    let host = extract_descriptor_value(descriptor, "HOST")
        .context("Oracle connect descriptor is missing HOST")?;
    let port = extract_descriptor_value(descriptor, "PORT")
        .context("Oracle connect descriptor is missing PORT")?
        .parse::<u16>()
        .context("Oracle connect descriptor has an invalid PORT")?;

    if let Some(service_name) = extract_descriptor_value(descriptor, "SERVICE_NAME") {
        Ok(OracleDriverConfig::new(
            host,
            port,
            service_name,
            config.username.clone(),
            config.password.clone(),
        ))
    } else if let Some(sid) = extract_descriptor_value(descriptor, "SID") {
        Ok(OracleDriverConfig::with_sid(
            host,
            port,
            sid,
            config.username.clone(),
            config.password.clone(),
        ))
    } else {
        Err(anyhow!(
            "Oracle connect descriptor must contain SERVICE_NAME or SID"
        ))
    }
}

fn extract_descriptor_value(descriptor: &str, key: &str) -> Option<String> {
    let descriptor_lower = descriptor.to_ascii_lowercase();
    let pattern = format!("({}=", key.to_ascii_lowercase());
    let start = descriptor_lower.find(&pattern)? + pattern.len();
    let remainder = &descriptor[start..];
    let end = remainder.find(')')?;
    Some(remainder[..end].trim().trim_matches('"').to_string())
}

fn resolve_tns_alias(alias: &str, wallet_dir: &Path) -> anyhow::Result<String> {
    let tnsnames_path = wallet_dir.join("tnsnames.ora");
    let contents = fs::read_to_string(&tnsnames_path)
        .with_context(|| format!("failed to read {}", tnsnames_path.display()))?;
    let lower_contents = contents.to_ascii_lowercase();
    let alias_lower = alias.trim().to_ascii_lowercase();
    let pattern_with_space = format!("{} =", alias_lower);
    let pattern_without_space = format!("{}=", alias_lower);
    let alias_start = lower_contents
        .find(&pattern_with_space)
        .or_else(|| lower_contents.find(&pattern_without_space))
        .with_context(|| {
            format!(
                "TNS alias '{}' was not found in {}",
                alias,
                tnsnames_path.display()
            )
        })?;
    let paren_start = contents[alias_start..]
        .find('(')
        .map(|offset| alias_start + offset)
        .context("TNS alias does not contain a connect descriptor")?;

    let mut depth = 0usize;
    for (offset, ch) in contents[paren_start..].char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    let end = paren_start + offset + 1;
                    return Ok(contents[paren_start..end].to_string());
                }
            }
            _ => {}
        }
    }

    Err(anyhow!(
        "TNS alias '{}' in {} has unbalanced parentheses",
        alias,
        tnsnames_path.display()
    ))
}

fn log_wallet_directory(wallet_dir: &Path) {
    eprintln!("[DB] Wallet directory: {}", wallet_dir.display());
    match fs::read_dir(wallet_dir) {
        Ok(entries) => {
            eprintln!("[DB] Wallet directory contents:");
            for entry in entries.flatten() {
                let size = entry.metadata().map(|metadata| metadata.len()).unwrap_or(0);
                eprintln!(
                    "[DB]   {} ({} bytes)",
                    entry.file_name().to_string_lossy(),
                    size
                );
            }
        }
        Err(error) => {
            eprintln!(
                "[DB] ERROR: Cannot read wallet directory '{}': {}",
                wallet_dir.display(),
                error
            );
        }
    }
}

async fn run_bootstrap_ddl(pool: &Pool) -> anyhow::Result<()> {
    let conn = pool.get().await?;
    for sql in [
        "ALTER TABLE users ADD (filing_status VARCHAR2(32))",
        "ALTER TABLE users ADD (agi NUMBER(14,2))",
        "ALTER TABLE users ADD (marginal_tax_rate NUMBER(6,4))",
        "ALTER TABLE users ADD (itemize_deductions NUMBER(1))",
        "ALTER TABLE users ADD (is_encrypted NUMBER(1) DEFAULT 0)",
        "ALTER TABLE users ADD (encrypted_payload CLOB)",
        "ALTER TABLE users ADD (vault_credential_id VARCHAR2(512))",
        "ALTER TABLE users ADD (updated_at TIMESTAMP)",
        "ALTER TABLE charities ADD (category VARCHAR2(255))",
        "ALTER TABLE charities ADD (status VARCHAR2(255))",
        "ALTER TABLE charities ADD (classification VARCHAR2(255))",
        "ALTER TABLE charities ADD (nonprofit_type VARCHAR2(255))",
        "ALTER TABLE charities ADD (deductibility VARCHAR2(64))",
        "ALTER TABLE charities ADD (street VARCHAR2(255))",
        "ALTER TABLE charities ADD (city VARCHAR2(120))",
        "ALTER TABLE charities ADD (state VARCHAR2(16))",
        "ALTER TABLE charities ADD (zip VARCHAR2(20))",
        "ALTER TABLE charities ADD (is_encrypted NUMBER(1) DEFAULT 0)",
        "ALTER TABLE charities ADD (encrypted_payload CLOB)",
        "ALTER TABLE donations ADD (donation_category VARCHAR2(32))",
        "ALTER TABLE donations ADD (donation_amount NUMBER(12,2))",
        "ALTER TABLE donations ADD (is_encrypted NUMBER(1) DEFAULT 0)",
        "ALTER TABLE donations ADD (encrypted_payload CLOB)",
        "ALTER TABLE receipts ADD (is_encrypted NUMBER(1) DEFAULT 0)",
        "ALTER TABLE receipts ADD (encrypted_payload CLOB)",
        "ALTER TABLE receipts ADD (updated_at TIMESTAMP)",
        "ALTER TABLE audit_logs ADD (updated_at TIMESTAMP)",
        "ALTER TABLE val_categories ADD (created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP)",
        "ALTER TABLE val_categories ADD (updated_at TIMESTAMP)",
        "ALTER TABLE val_items ADD (created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP)",
        "ALTER TABLE val_items ADD (updated_at TIMESTAMP)",
        "CREATE TABLE audit_revisions (id VARCHAR2(255) PRIMARY KEY, user_id VARCHAR2(255), table_name VARCHAR2(255) NOT NULL, record_id VARCHAR2(255) NOT NULL, operation VARCHAR2(16) NOT NULL, old_values VARCHAR2(4000), new_values VARCHAR2(4000), created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP, updated_at TIMESTAMP, CONSTRAINT fk_audit_revisions_user FOREIGN KEY (user_id) REFERENCES users(id))",
        "CREATE INDEX idx_audit_revisions_table_record ON audit_revisions(table_name, record_id, created_at)",
        "CREATE INDEX idx_charities_user_ein ON charities(user_id, ein)",
        "CREATE INDEX idx_val_items_category_name ON val_items(category_id, name)",
        "CREATE INDEX idx_val_items_lower_name ON val_items(LOWER(name))",
    ] {
        let _ = conn.execute(sql, &[]).await;
    }
    let _ = conn.commit().await;
    Ok(())
}

pub(crate) fn parse_utc_from_opt_string(value: Option<String>) -> DateTime<Utc> {
    value
        .and_then(|text| parse_datetime_text(&text))
        .unwrap_or_else(Utc::now)
}

/// Deprecated: CLOB UPDATE binding hangs with oracle-rs 0.1.7. Bind as VARCHAR2 directly instead.
#[allow(dead_code)]
pub(crate) async fn write_clob_by_id(
    conn: &Connection,
    table_name: &str,
    id_column: &str,
    id_value: &str,
    clob_column: &str,
    value: &str,
) -> anyhow::Result<()> {
    if value.is_empty() {
        let sql =
            format!("UPDATE {table_name} SET {clob_column} = EMPTY_CLOB() WHERE {id_column} = :1");
        conn.execute(&sql, &crate::oracle_params![id_value.to_string()])
            .await?;
        return Ok(());
    }

    let sql = format!("UPDATE {table_name} SET {clob_column} = :2 WHERE {id_column} = :1");
    conn.execute(
        &sql,
        &crate::oracle_params![id_value.to_string(), value.to_string()],
    )
    .await?;
    Ok(())
}

fn user_profile_row_from_parts(
    email: String,
    name: String,
    provider: String,
    filing_status: Option<String>,
    numeric_fields: UserProfileNumericFields,
    is_encrypted: Option<bool>,
    encrypted_payload: Option<String>,
    vault_credential_id: Option<String>,
) -> UserProfileRow {
    (
        email,
        name,
        provider,
        filing_status,
        numeric_fields.agi,
        numeric_fields.marginal_tax_rate,
        numeric_fields.itemize_deductions,
        is_encrypted,
        encrypted_payload,
        vault_credential_id,
    )
}

fn user_profile_row_from_row(row: &Row, offset: usize) -> UserProfileRow {
    user_profile_row_from_parts(
        row_string(row, offset),
        row_string(row, offset + 1),
        row_opt_string(row, offset + 2).unwrap_or_else(|| "local".to_string()),
        row_opt_string(row, offset + 3),
        UserProfileNumericFields {
            agi: row_f64(row, offset + 4),
            marginal_tax_rate: row_f64(row, offset + 5),
            itemize_deductions: row_bool(row, offset + 6),
        },
        row_bool(row, offset + 7),
        row_opt_string(row, offset + 8),
        row_opt_string(row, offset + 9),
    )
}

struct UserProfileNumericFields {
    agi: Option<f64>,
    marginal_tax_rate: Option<f64>,
    itemize_deductions: Option<bool>,
}

pub(crate) fn row_string(row: &Row, index: usize) -> String {
    row_opt_string(row, index).unwrap_or_default()
}

pub(crate) fn row_opt_string(row: &Row, index: usize) -> Option<String> {
    row.get_string(index)
        .map(ToOwned::to_owned)
        .or_else(|| row.get(index).and_then(value_to_string))
}

pub(crate) fn row_i64(row: &Row, index: usize) -> Option<i64> {
    row.get(index).and_then(|value| {
        value
            .as_i64()
            .or_else(|| value_to_string(value)?.parse::<i64>().ok())
    })
}

pub(crate) fn row_f64(row: &Row, index: usize) -> Option<f64> {
    row.get(index).and_then(|value| {
        value
            .as_f64()
            .or_else(|| value_to_string(value)?.parse::<f64>().ok())
    })
}

pub(crate) fn row_bool(row: &Row, index: usize) -> Option<bool> {
    row.get(index).and_then(|value| {
        value.as_bool().or_else(|| {
            value_to_string(value).and_then(|text| {
                match text.trim().to_ascii_lowercase().as_str() {
                    "1" | "true" | "y" | "yes" => Some(true),
                    "0" | "false" | "n" | "no" => Some(false),
                    _ => None,
                }
            })
        })
    })
}

pub(crate) fn row_naive_date(row: &Row, index: usize) -> Option<NaiveDate> {
    row.get(index)
        .and_then(|value| value_to_string(value).and_then(|text| parse_naive_date_text(&text)))
}

pub(crate) fn row_datetime_utc(row: &Row, index: usize) -> Option<DateTime<Utc>> {
    row.get(index)
        .and_then(|value| value_to_string(value).and_then(|text| parse_datetime_text(&text)))
}

fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        _ => Some(value.to_string()),
    }
}

fn parse_naive_date_text(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .ok()
        .or_else(|| {
            value
                .split_whitespace()
                .next()
                .and_then(|prefix| NaiveDate::parse_from_str(prefix, "%Y-%m-%d").ok())
        })
        .or_else(|| parse_datetime_text(value).map(|value| value.date_naive()))
}

fn parse_datetime_text(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|value| value.with_timezone(&Utc))
        .or_else(|| {
            DateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S%.f %:z")
                .ok()
                .map(|value| value.with_timezone(&Utc))
        })
        .or_else(|| {
            DateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S %:z")
                .ok()
                .map(|value| value.with_timezone(&Utc))
        })
        .or_else(|| {
            chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S%.f")
                .ok()
                .map(|value| DateTime::from_naive_utc_and_offset(value, Utc))
        })
        .or_else(|| {
            chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S")
                .ok()
                .map(|value| DateTime::from_naive_utc_and_offset(value, Utc))
        })
}
