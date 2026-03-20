use crate::db::models::{NewReceipt, Receipt};
use crate::db::DbPool;

pub async fn list_receipts(
    pool: &DbPool,
    user_id: &str,
    donation_id: Option<String>,
) -> anyhow::Result<Vec<Receipt>> {
    super::list_receipts(pool, user_id, donation_id).await
}

pub async fn get_receipt(
    pool: &DbPool,
    user_id: &str,
    receipt_id: &str,
) -> anyhow::Result<Option<Receipt>> {
    super::get_receipt(pool, user_id, receipt_id).await
}

pub async fn add_receipt(pool: &DbPool, input: &NewReceipt) -> anyhow::Result<()> {
    super::add_receipt(pool, input).await
}

pub async fn set_receipt_ocr(
    pool: &DbPool,
    receipt_id: &str,
    ocr_text: &Option<String>,
    ocr_date: &Option<chrono::NaiveDate>,
    ocr_amount: &Option<f64>,
    ocr_status: &Option<String>,
) -> anyhow::Result<()> {
    super::set_receipt_ocr(pool, receipt_id, ocr_text, ocr_date, ocr_amount, ocr_status).await
}
