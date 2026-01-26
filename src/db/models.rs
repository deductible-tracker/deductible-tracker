use serde::{Deserialize, Serialize};
use chrono::{NaiveDate, DateTime, Utc};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Donation {
    pub id: String,
    pub user_id: String,
    pub year: i32,
    pub date: NaiveDate,
    pub charity_id: String,
    pub charity_name: String,
    pub charity_ein: Option<String>,
    pub notes: Option<String>,
    pub shared_with: Option<Vec<String>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted: bool,
}