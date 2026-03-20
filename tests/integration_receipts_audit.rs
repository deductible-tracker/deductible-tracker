use deductible_tracker::db;
use deductible_tracker::db::models::{NewCharity, NewDonation, NewReceipt};
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
    let charity = NewCharity {
        id: charity_id.clone(),
        user_id: user_id.clone(),
        name: charity_name,
        ein: None,
        category: None,
        status: None,
        classification: None,
        nonprofit_type: None,
        deductibility: None,
        street: None,
        city: None,
        state: None,
        zip: None,
        created_at: now,
    };
    db::create_charity(&pool, &charity)
        .await
        .expect("create_charity");

    let donation_id = format!("test-donation-{}", Uuid::new_v4());
    let donation_date = chrono::NaiveDate::from_ymd_opt(2026, 2, 18).expect("valid date");
    let donation = NewDonation {
        id: donation_id.clone(),
        user_id: user_id.clone(),
        year: 2026,
        date: donation_date,
        category: Some("money".to_string()),
        charity_id: charity_id.clone(),
        amount: Some(123.45),
        notes: Some("integration test".to_string()),
        created_at: now,
    };
    db::add_donation(&pool, &donation)
        .await
        .expect("add_donation");

    let receipt = NewReceipt {
        id: receipt_id.clone(),
        donation_id: donation_id.clone(),
        key,
        file_name: Some("sample.png".to_string()),
        content_type: Some("image/png".to_string()),
        size: Some(123i64),
        created_at: now,
    };
    db::add_receipt(&pool, &receipt).await.expect("add_receipt");

    // Persist OCR results
    let ocr_text = Some("Sample OCR text".to_string());
    let ocr_date = None;
    let ocr_amount = Some(12345i64);
    let ocr_status = Some("done".to_string());

    let set_ok = db::set_receipt_ocr(
        &pool,
        &receipt_id,
        &ocr_text,
        &ocr_date,
        &ocr_amount,
        &ocr_status,
    )
    .await
    .expect("set_receipt_ocr");
    assert!(set_ok, "set_receipt_ocr returned false");

    // Fetch receipt and validate OCR fields
    let fetched = db::get_receipt(&pool, &user_id, &receipt_id)
        .await
        .expect("get_receipt");
    assert!(fetched.is_some(), "receipt not found");
    let r = fetched.unwrap();
    assert_eq!(r.ocr_text.unwrap_or_default(), "Sample OCR text");
    assert_eq!(r.ocr_amount.unwrap_or_default(), 12345i64);
    assert_eq!(r.ocr_status.unwrap_or_default(), "done");

    // Log an audit entry and ensure it's retrievable
    let audit_id = format!("audit-test-{}", Uuid::new_v4());
    let details = Some("Test audit entry".to_string());
    db::log_audit(
        &pool,
        &audit_id,
        &user_id,
        "test_action",
        "receipts",
        &Some(receipt_id.clone()),
        &details,
    )
    .await
    .expect("log_audit");

    let logs = db::list_audit_logs(&pool, &user_id, None)
        .await
        .expect("list_audit_logs");
    assert!(!logs.is_empty(), "expected at least one audit log");
}
