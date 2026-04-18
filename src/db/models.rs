use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct UserProfileUpsert {
    pub user_id: String,
    pub email: String,
    pub name: String,
    pub provider: String,
    pub filing_status: Option<String>,
    pub agi: Option<f64>,
    pub marginal_tax_rate: Option<f64>,
    pub itemize_deductions: Option<bool>,
    pub is_encrypted: Option<bool>,
    pub encrypted_payload: Option<String>,
    pub vault_credential_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewDonation {
    pub id: String,
    pub user_id: String,
    pub year: i32,
    pub date: NaiveDate,
    pub category: Option<String>,
    pub charity_id: String,
    pub amount: Option<f64>,
    pub notes: Option<String>,
    pub is_encrypted: Option<bool>,
    pub encrypted_payload: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct DonationPatch {
    pub user_id: String,
    pub donation_id: String,
    pub date_opt: Option<NaiveDate>,
    pub year_opt: Option<i32>,
    pub category_opt: Option<String>,
    pub charity_id_opt: Option<String>,
    pub amount_opt: Option<f64>,
    pub notes: Option<String>,
    pub is_encrypted: Option<bool>,
    pub encrypted_payload: Option<String>,
    pub incoming_updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct NewReceipt {
    pub id: String,
    pub donation_id: String,
    pub key: String,
    pub file_name: Option<String>,
    pub content_type: Option<String>,
    pub size: Option<i64>,
    pub is_encrypted: Option<bool>,
    pub encrypted_payload: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewCharity {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub ein: Option<String>,
    pub category: Option<String>,
    pub status: Option<String>,
    pub classification: Option<String>,
    pub nonprofit_type: Option<String>,
    pub deductibility: Option<String>,
    pub street: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub zip: Option<String>,
    pub is_encrypted: Option<bool>,
    pub encrypted_payload: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CharityPatch {
    pub charity_id: String,
    pub user_id: String,
    pub name: String,
    pub ein: Option<String>,
    pub category: Option<String>,
    pub status: Option<String>,
    pub classification: Option<String>,
    pub nonprofit_type: Option<String>,
    pub deductibility: Option<String>,
    pub street: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub zip: Option<String>,
    pub is_encrypted: Option<bool>,
    pub encrypted_payload: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct RevisionLogEntry {
    pub id: String,
    pub user_id: Option<String>,
    pub table_name: String,
    pub record_id: String,
    pub operation: String,
    pub old_values: Option<String>,
    pub new_values: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BatchSyncRequest {
    pub donations: Vec<DonationSyncItem>,
    pub receipts: Vec<ReceiptSyncItem>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DonationSyncItem {
    pub action: String, // "create", "update", "delete"
    pub id: String,
    pub date: Option<chrono::NaiveDate>,
    pub year: Option<i32>,
    pub category: Option<String>,
    pub amount: Option<f64>,
    pub charity_id: String,
    pub notes: Option<String>,
    pub is_encrypted: Option<bool>,
    pub encrypted_payload: Option<String>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReceiptSyncItem {
    pub action: String, // "create"
    pub id: String,
    pub donation_id: String,
    pub key: String,
    pub file_name: Option<String>,
    pub content_type: Option<String>,
    pub size: Option<i64>,
    pub is_encrypted: Option<bool>,
    pub encrypted_payload: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Donation {
    pub id: String,
    pub user_id: String,
    pub year: i32,
    pub date: NaiveDate,
    pub category: Option<String>,
    pub amount: Option<f64>,
    pub charity_id: String,
    pub charity_name: String,
    pub charity_ein: Option<String>,
    pub notes: Option<String>,
    pub is_encrypted: Option<bool>,
    pub encrypted_payload: Option<String>,
    pub shared_with: Option<Vec<String>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Receipt {
    pub id: String,
    pub donation_id: String,
    pub key: String,
    pub file_name: Option<String>,
    pub content_type: Option<String>,
    pub size: Option<i64>,
    pub ocr_text: Option<String>,
    pub ocr_date: Option<NaiveDate>,
    pub ocr_amount: Option<i64>,
    pub ocr_status: Option<String>,
    pub is_encrypted: Option<bool>,
    pub encrypted_payload: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Charity {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub ein: Option<String>,
    pub category: Option<String>,
    pub status: Option<String>,
    pub classification: Option<String>,
    pub nonprofit_type: Option<String>,
    pub deductibility: Option<String>,
    pub street: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub zip: Option<String>,
    pub is_encrypted: Option<bool>,
    pub encrypted_payload: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AuditLog {
    pub id: String,
    pub user_id: String,
    pub action: String,
    pub table_name: String,
    pub record_id: Option<String>,
    pub details: Option<String>,
    pub created_at: DateTime<Utc>,
}
