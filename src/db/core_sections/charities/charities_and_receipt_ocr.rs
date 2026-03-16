pub async fn set_receipt_ocr(
    pool: &DbPool,
    id: &str,
    text: &Option<String>,
    date: &Option<chrono::NaiveDate>,
    amount: &Option<f64>,
    status: &Option<String>,
) -> anyhow::Result<()> {
    match &**pool {
        DbPoolEnum::Oracle(p) => {
            let p = p.clone();
            let id = id.to_string();
            let text = text.clone();
            let date = date.map(|d| d.format("%Y-%m-%d").to_string());
            let amount = *amount;
            let status = status.clone();
            let updated_at = chrono::Utc::now().to_rfc3339();

            task::spawn_blocking(move || -> anyhow::Result<()> {
                let conn = p.get()?;
                let sql = "UPDATE receipts SET ocr_text = :1, ocr_date = TO_DATE(:2, 'YYYY-MM-DD'), ocr_amount = :3, ocr_status = :4, updated_at = TO_TIMESTAMP_TZ(:5, 'YYYY-MM-DD\"T\"HH24:MI:SS.FF TZH:TZM') WHERE id = :6";
                conn.execute(sql, &[&text, &date, &amount, &status, &updated_at, &id])?;
                let _ = conn.commit();
                Ok(())
            })
            .await
            .map_err(|e| anyhow!("DB task join error: {}", e))??;
            Ok(())
        }
    }
}
