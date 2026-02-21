use serde::{Deserialize, Serialize};
use chrono::{NaiveDate, DateTime, Utc};

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
    pub ocr_date: Option<DateTime<Utc>>,
    pub ocr_amount: Option<i64>,
    pub ocr_status: Option<String>,
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