use r2d2::Pool;
use std::env;
use anyhow::anyhow;
use std::sync::Arc;
use serde_json::json;
use tokio::task;
use uuid::Uuid;

use crate::db::oracle::OracleConnectionManager;

pub enum DbPoolEnum {
    Oracle(Pool<OracleConnectionManager>),
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

}

pub async fn init_pool() -> anyhow::Result<DbPool> {
    let runtime_mode = RuntimeMode::from_env()?;
    let db_pool_max = env::var("DB_POOL_MAX_SIZE")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(20);
    let db_pool_min = env::var("DB_POOL_MIN_IDLE")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(2);
    let db_pool_timeout_secs = env::var("DB_POOL_CONNECTION_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(2);

    crate::db::oracle::init_pool(runtime_mode, db_pool_max, db_pool_min, db_pool_timeout_secs).await
}

// High level helpers used by routes.
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
    }
}

pub async fn get_user_profile(
    pool: &DbPool,
    user_id: &str,
) -> anyhow::Result<Option<UserProfileRow>> {
    match &**pool {
        DbPoolEnum::Oracle(p) => crate::db::oracle::get_user_profile(p, user_id).await,
    }
}

pub async fn upsert_user_profile(
    pool: &DbPool,
    input: &UserProfileUpsert,
) -> anyhow::Result<()> {
    match &**pool {
        DbPoolEnum::Oracle(p) => crate::db::oracle::upsert_user_profile(p, input).await,
    }
}

pub async fn delete_user_data(
    pool: &DbPool,
    user_id: &str,
) -> anyhow::Result<()> {
    match &**pool {
        DbPoolEnum::Oracle(p) => crate::db::oracle::delete_user_data(p, user_id).await,
    }
}
pub async fn get_user_receipt_keys(
    pool: &DbPool,
    user_id: &str,
) -> anyhow::Result<Vec<String>> {
    match &**pool {
        DbPoolEnum::Oracle(p) => {
            let p = p.clone();
            let user_id = user_id.to_string();
            task::spawn_blocking(move || -> anyhow::Result<Vec<String>> {
                let conn = p.get()?;
                let rows = conn.query(
                    "SELECT r.receipt_key FROM receipts r JOIN donations d ON d.id = r.donation_id WHERE d.user_id = :1",
                    &[&user_id],
                )?;
                let mut keys = Vec::new();
                for row in rows {
                    let r = row?;
                    let key: String = r.get(0).unwrap_or_default();
                    if !key.is_empty() {
                        keys.push(key);
                    }
                }
                Ok(keys)
            })
            .await
            .map_err(|e| anyhow!("DB task join error: {}", e))?
        }
    }
}
pub async fn list_charities(
    pool: &DbPool,
    user_id: &str,
) -> anyhow::Result<Vec<crate::db::models::Charity>> {
    match &**pool {
        DbPoolEnum::Oracle(p) => crate::db::oracle::charities::list_charities(p, user_id).await,
    }
}

pub async fn import_data(
    pool: &DbPool,
    user_id: &str,
    profile: &UserProfileUpsert,
    charities: &[crate::db::models::Charity],
    donations: &[crate::db::models::Donation],
    receipts: &[crate::db::models::Receipt],
) -> anyhow::Result<()> {
    // 1. Restore Profile
    upsert_user_profile(pool, profile).await?;

    // 2. Restore Charities (use upsert-like logic)
    for charity in charities {
        if charity.user_id != user_id { continue; }
        let new_charity = crate::db::models::NewCharity {
            id: charity.id.clone(),
            user_id: user_id.to_string(),
            name: charity.name.clone(),
            ein: charity.ein.clone(),
            category: charity.category.clone(),
            status: charity.status.clone(),
            classification: charity.classification.clone(),
            nonprofit_type: charity.nonprofit_type.clone(),
            deductibility: charity.deductibility.clone(),
            street: charity.street.clone(),
            city: charity.city.clone(),
            state: charity.state.clone(),
            zip: charity.zip.clone(),
            created_at: charity.created_at,
        };
        let _ = crate::db::create_charity(pool, &new_charity).await;
    }

    // 3. Restore Donations
    for donation in donations {
        if donation.user_id != user_id { continue; }
        let new_donation = NewDonation {
            id: donation.id.clone(),
            user_id: user_id.to_string(),
            year: donation.year,
            date: donation.date,
            category: donation.category.clone(),
            charity_id: donation.charity_id.clone(),
            amount: donation.amount,
            notes: donation.notes.clone(),
            created_at: donation.created_at,
        };
        let _ = add_donation(pool, &new_donation).await;
    }

    // 4. Restore Receipts
    for receipt in receipts {
        let new_receipt = NewReceipt {
            id: receipt.id.clone(),
            donation_id: receipt.donation_id.clone(),
            key: receipt.key.clone(),
            file_name: receipt.file_name.clone(),
            content_type: receipt.content_type.clone(),
            size: receipt.size,
            created_at: receipt.created_at,
        };
        let _ = add_receipt(pool, &new_receipt).await;
        // Also restore OCR results if they exist
        if receipt.ocr_status.is_some() {
            let ocr_amount_f64 = receipt.ocr_amount.map(|a| a as f64);
            let _ = match &**pool {
                DbPoolEnum::Oracle(p) => crate::db::oracle::receipts::set_receipt_ocr(
                    p,
                    &receipt.id,
                    &receipt.ocr_text,
                    &receipt.ocr_date.map(|dt| dt.naive_utc().date()),
                    &ocr_amount_f64,
                    &receipt.ocr_status,
                ).await,
            };
        }
    }

    Ok(())
}

pub async fn add_donation(
    pool: &DbPool,
    input: &NewDonation,
) -> anyhow::Result<()> {
    let input = input.clone();
    let created_at_str = input.created_at.to_rfc3339();
    let date_str_for_audit = input.date.format("%Y-%m-%d").to_string();

    match &**pool {
        DbPoolEnum::Oracle(p) => crate::db::oracle::donations::add_donation(p, &input, &created_at_str).await?,
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
        DbPoolEnum::Oracle(p) => crate::db::oracle::receipts::add_receipt(p, &input, &created_at_str).await?,
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
