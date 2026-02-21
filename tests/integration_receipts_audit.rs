use deductible_tracker::db;
use uuid::Uuid;

#[tokio::test]
async fn receipt_ocr_and_audit_flow() {
    std::env::set_var("RUST_ENV", "development");
    let pool = db::init_pool().await.expect("init pool");

    // Create a test receipt
    let receipt_id = format!("test-receipt-{}", Uuid::new_v4());
    let user_id = "dev-1".to_string();
    let key = "test-key".to_string();
    let now = chrono::Utc::now();

    let charity_id = format!("test-charity-{}", Uuid::new_v4());
    let charity_name = format!("Test Charity {}", Uuid::new_v4());
    db::create_charity(
        &pool,
        &charity_id,
        &user_id,
        &charity_name,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        now,
    )
    .await
    .expect("create_charity");

    let donation_id = format!("test-donation-{}", Uuid::new_v4());
    let donation_date = chrono::NaiveDate::from_ymd_opt(2026, 2, 18).expect("valid date");
    db::add_donation(
        &pool,
        &donation_id,
        &user_id,
        2026,
        donation_date,
        &Some("money".to_string()),
        &charity_id,
        &Some(123.45),
        &Some("integration test".to_string()),
        now,
    )
    .await
    .expect("add_donation");

    let _ = db::add_receipt(&pool, &receipt_id, &donation_id, &key, &Some("sample.png".to_string()), &Some("image/png".to_string()), &Some(123i64), now).await.expect("add_receipt");

    // Persist OCR results
    let ocr_text = Some("Sample OCR text".to_string());
    let ocr_date = None;
    let ocr_amount = Some(12345i64);
    let ocr_status = Some("done".to_string());

    let set_ok = db::set_receipt_ocr(&pool, &receipt_id, &ocr_text, &ocr_date, &ocr_amount, &ocr_status).await.expect("set_receipt_ocr");
    assert!(set_ok, "set_receipt_ocr returned false");

    // Fetch receipt and validate OCR fields
    let fetched = db::get_receipt(&pool, &user_id, &receipt_id).await.expect("get_receipt");
    assert!(fetched.is_some(), "receipt not found");
    let r = fetched.unwrap();
    assert_eq!(r.ocr_text.unwrap_or_default(), "Sample OCR text");
    assert_eq!(r.ocr_amount.unwrap_or_default(), 12345i64);
    assert_eq!(r.ocr_status.unwrap_or_default(), "done");

    // Log an audit entry and ensure it's retrievable
    let audit_id = format!("audit-test-{}", Uuid::new_v4());
    let details = Some("Test audit entry".to_string());
    db::log_audit(&pool, &audit_id, &user_id, "test_action", "receipts", &Some(receipt_id.clone()), &details).await.expect("log_audit");

    let logs = db::list_audit_logs(&pool, &user_id, None).await.expect("list_audit_logs");
    assert!(logs.len() >= 1, "expected at least one audit log");
}
