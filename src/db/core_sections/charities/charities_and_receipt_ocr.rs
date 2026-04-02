pub async fn set_receipt_ocr(
    pool: &DbPool,
    id: &str,
    text: &Option<String>,
    date: &Option<chrono::NaiveDate>,
    amount: &Option<f64>,
    status: &Option<String>,
) -> anyhow::Result<()> {
    match &**pool {
        DbPoolEnum::Oracle(p) => crate::db::oracle::receipts::set_receipt_ocr(p, id, text, date, amount, status).await,
    }
}
