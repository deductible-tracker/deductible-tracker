use r2d2::Pool;
use r2d2_oracle::OracleConnectionManager;
use std::env;
use anyhow::anyhow;
use std::sync::Arc;
use tokio::task;
use serde_json::json;
use uuid::Uuid;

// Add sqlite support for development environment
use rusqlite::params;
use r2d2_sqlite::SqliteConnectionManager as R2SqliteManager;

pub enum DbPoolEnum {
    Oracle(Pool<OracleConnectionManager>),
    Sqlite(r2d2::Pool<R2SqliteManager>),
}

pub type DbPool = Arc<DbPoolEnum>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeMode {
    Production,
    Development,
}

impl RuntimeMode {
    pub fn from_env() -> anyhow::Result<Self> {
        match env::var("RUST_ENV") {
            Ok(value) if value.eq_ignore_ascii_case("production") => Ok(RuntimeMode::Production),
            Ok(value) if value.eq_ignore_ascii_case("development") => Ok(RuntimeMode::Development),
            Ok(value) => Err(anyhow!(
                "Invalid RUST_ENV='{}'. Expected 'development' or 'production'.",
                value
            )),
            Err(_) => Ok(RuntimeMode::Development),
        }
    }

    fn sqlite_path() -> String {
        env::var("SQLITE_DB_PATH")
            .or_else(|_| env::var("DEV_SQLITE_PATH"))
            .unwrap_or_else(|_| "dev.db".to_string())
    }
}

pub async fn init_pool() -> anyhow::Result<DbPool> {
    let runtime_mode = RuntimeMode::from_env()?;
    let db_pool_max = env::var("DB_POOL_MAX_SIZE")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(32);
    let db_pool_min = env::var("DB_POOL_MIN_IDLE")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(4);
    let db_pool_timeout_secs = env::var("DB_POOL_CONNECTION_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(15);

    if runtime_mode == RuntimeMode::Production {
        crate::db::oracle::init_pool(db_pool_max, db_pool_min, db_pool_timeout_secs).await
    } else {
        crate::db::sqlite::init_pool(&RuntimeMode::sqlite_path(), db_pool_max, db_pool_min, db_pool_timeout_secs).await
    }
}

// High level helpers used by routes to avoid Oracle/SQLite API differences
use crate::db::models::{
    Donation as DonationModel, NewDonation, NewReceipt, RevisionLogEntry, UserProfileUpsert,
};
use chrono::Datelike;

pub(crate) type UserProfileRow = (
    String,
    String,
    String,
    Option<String>,
    Option<f64>,
    Option<f64>,
    Option<bool>,
);

pub async fn get_user_profile_by_email(
    pool: &DbPool,
    email: &str,
) -> anyhow::Result<Option<(String, UserProfileRow)>> {
    match &**pool {
        DbPoolEnum::Oracle(p) => crate::db::oracle::get_user_profile_by_email(p, email).await,
        DbPoolEnum::Sqlite(p) => crate::db::sqlite::get_user_profile_by_email(p, email).await,
    }
}

pub async fn get_user_profile(
    pool: &DbPool,
    user_id: &str,
) -> anyhow::Result<Option<UserProfileRow>> {
    match &**pool {
        DbPoolEnum::Oracle(p) => crate::db::oracle::get_user_profile(p, user_id).await,
        DbPoolEnum::Sqlite(p) => crate::db::sqlite::get_user_profile(p, user_id).await,
    }
}

pub async fn upsert_user_profile(
    pool: &DbPool,
    input: &UserProfileUpsert,
) -> anyhow::Result<()> {
    match &**pool {
        DbPoolEnum::Oracle(p) => crate::db::oracle::upsert_user_profile(p, input).await,
        DbPoolEnum::Sqlite(p) => crate::db::sqlite::upsert_user_profile(p, input).await,
    }
}

pub async fn add_donation(
    pool: &DbPool,
    input: &NewDonation,
) -> anyhow::Result<()> {
    let input = input.clone();
    let created_at_str = input.created_at.to_rfc3339();
    let date_str_for_audit = input.date.format("%Y-%m-%d").to_string();

    match &**pool {
        DbPoolEnum::Oracle(p) => {
            crate::db::oracle::donations::add_donation(p, &input, &created_at_str).await?
        }
        DbPoolEnum::Sqlite(p) => {
            crate::db::sqlite::donations::add_donation(p, &input, &created_at_str).await?
        }
    };

    let revision = RevisionLogEntry {
        id: Uuid::new_v4().to_string(),
        user_id: Some(input.user_id.clone()),
        table_name: "donations".to_string(),
        record_id: input.id.clone(),
        operation: "create".to_string(),
        old_values: None,
        new_values: Some(
        json!({
            "id": input.id,
            "user_id": input.user_id,
            "donation_year": input.year,
            "donation_date": date_str_for_audit,
            "donation_category": input.category,
            "donation_amount": input.amount,
            "charity_id": input.charity_id,
            "notes": input.notes,
            "created_at": created_at_str,
            "deleted": false
        })
        .to_string(),
    ),
    };
    log_revision(pool, &revision).await?;

    Ok(())
}

pub async fn add_receipt(
    pool: &DbPool,
    input: &NewReceipt,
) -> anyhow::Result<()> {
    let input = input.clone();
    let created_at_str = input.created_at.to_rfc3339();

    let donation_owner = donation_owner_user_id(pool, &input.donation_id).await?;

    match &**pool {
        DbPoolEnum::Oracle(p) => {
            crate::db::oracle::receipts::add_receipt(p, &input, &created_at_str).await?
        }
        DbPoolEnum::Sqlite(p) => {
            crate::db::sqlite::receipts::add_receipt(p, &input, &created_at_str).await?
        }
    };

    let revision = RevisionLogEntry {
        id: Uuid::new_v4().to_string(),
        user_id: donation_owner,
        table_name: "receipts".to_string(),
        record_id: input.id.clone(),
        operation: "create".to_string(),
        old_values: None,
        new_values: Some(
        json!({
            "id": input.id,
            "donation_id": input.donation_id,
            "key": input.key,
            "file_name": input.file_name,
            "content_type": input.content_type,
            "size": input.size,
            "ocr_text": null,
            "ocr_date": null,
            "ocr_amount": null,
            "ocr_status": null,
            "created_at": created_at_str
        })
        .to_string(),
    ),
    };
    log_revision(pool, &revision).await?;

    Ok(())
}

