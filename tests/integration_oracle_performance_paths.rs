use chrono::{Duration, NaiveDate, Utc};
use deductible_tracker::db;
use deductible_tracker::db::models::{
    BatchSyncRequest, DonationSyncItem, NewCharity, NewDonation, NewReceipt, ReceiptSyncItem,
    UserProfileUpsert,
};
use oracle_rs::Value;
use uuid::Uuid;

async fn init_test_pool() -> db::DbPool {
    std::env::set_var("RUST_ENV", "development");
    db::init_pool().await.expect("init pool")
}

async fn create_test_user(pool: &db::DbPool) -> (String, String) {
    let suffix = Uuid::new_v4().to_string();
    let user_id = format!("oracle-opt-user-{suffix}");
    let email = format!("{suffix}@example.test");
    let input = UserProfileUpsert {
        user_id: user_id.clone(),
        email: email.clone(),
        name: format!("Oracle Test {suffix}"),
        provider: "local".to_string(),
        filing_status: Some("single".to_string()),
        agi: None,
        marginal_tax_rate: None,
        itemize_deductions: Some(false),
        is_encrypted: None,
        encrypted_payload: None,
        vault_credential_id: None,
    };

    db::users::upsert_user_profile(pool, &input)
        .await
        .expect("upsert test user");
    (user_id, email)
}

async fn create_test_charity(pool: &db::DbPool, user_id: &str, suffix: &str) -> String {
    let charity_id = format!("oracle-opt-charity-{suffix}");
    let charity = NewCharity {
        id: charity_id.clone(),
        user_id: user_id.to_string(),
        name: format!("Helping Hands {suffix}"),
        ein: Some(format!("99-{suffix}")),
        category: Some("community".to_string()),
        status: Some("active".to_string()),
        classification: None,
        nonprofit_type: None,
        deductibility: None,
        street: None,
        city: None,
        state: None,
        zip: None,
        is_encrypted: None,
        encrypted_payload: None,
        created_at: Utc::now(),
    };

    db::charities::create_charity(pool, &charity)
        .await
        .expect("create charity");
    charity_id
}

async fn ensure_minimal_valuation_data(pool: &db::DbPool) {
    match &**pool {
        db::DbPoolEnum::Oracle(oracle_pool) => {
            let conn = oracle_pool.get().await.expect("checkout oracle connection");
            conn.execute(
                "MERGE INTO val_categories c USING (SELECT :1 AS id, :2 AS name FROM dual) s ON (c.id = s.id) WHEN NOT MATCHED THEN INSERT (id, name) VALUES (s.id, s.name)",
                &[Value::from("cat_appliances".to_string()),
                    Value::from("Appliances".to_string())],
            )
            .await
            .expect("merge valuation category");

            conn.execute(
                "MERGE INTO val_items v USING (SELECT :1 AS id, :2 AS category_id, :3 AS name, :4 AS suggested_min, :5 AS suggested_max FROM dual) s ON (v.id = s.id) WHEN NOT MATCHED THEN INSERT (id, category_id, name, suggested_min, suggested_max) VALUES (s.id, s.category_id, s.name, s.suggested_min, s.suggested_max)",
                &[Value::from("app_ac".to_string()),
                    Value::from("cat_appliances".to_string()),
                    Value::from("Air Conditioner".to_string()),
                    Value::from(21),
                    Value::from(93)],
            )
            .await
            .expect("merge valuation item");
            conn.commit().await.expect("commit valuation seed");
        }
    }
}

#[tokio::test]
async fn batch_sync_merges_updates_and_receipts() {
    let pool = init_test_pool().await;
    let (user_id, _) = create_test_user(&pool).await;
    let suffix = Uuid::new_v4().to_string();
    let charity_id = create_test_charity(&pool, &user_id, &suffix).await;
    let donation_id = format!("oracle-opt-donation-{suffix}");
    let receipt_id = format!("oracle-opt-receipt-{suffix}");
    let first_updated_at = Utc::now() - Duration::minutes(10);
    let older_updated_at = first_updated_at - Duration::minutes(5);
    let newer_updated_at = first_updated_at + Duration::minutes(5);

    db::batch_sync(
        &pool,
        &user_id,
        BatchSyncRequest {
            donations: vec![DonationSyncItem {
                action: "create".to_string(),
                id: donation_id.clone(),
                date: Some(NaiveDate::from_ymd_opt(2026, 3, 1).expect("valid date")),
                year: Some(2026),
                category: Some("goods".to_string()),
                amount: Some(25.0),
                charity_id: charity_id.clone(),
                notes: Some("first sync".to_string()),
                updated_at: Some(first_updated_at),
                is_encrypted: None,
                encrypted_payload: None,
            }],
            receipts: vec![ReceiptSyncItem {
                action: "create".to_string(),
                id: receipt_id.clone(),
                donation_id: donation_id.clone(),
                key: format!("key-{suffix}"),
                file_name: Some("receipt-1.png".to_string()),
                content_type: Some("image/png".to_string()),
                size: Some(1200),
                is_encrypted: None,
                encrypted_payload: None,
            }],
        },
    )
    .await
    .expect("initial batch sync");

    db::batch_sync(
        &pool,
        &user_id,
        BatchSyncRequest {
            donations: vec![DonationSyncItem {
                action: "update".to_string(),
                id: donation_id.clone(),
                date: Some(NaiveDate::from_ymd_opt(2026, 3, 2).expect("valid date")),
                year: Some(2026),
                category: Some("goods".to_string()),
                amount: Some(99.0),
                charity_id: charity_id.clone(),
                notes: Some("older sync".to_string()),
                updated_at: Some(older_updated_at),
                is_encrypted: None,
                encrypted_payload: None,
            }],
            receipts: vec![ReceiptSyncItem {
                action: "create".to_string(),
                id: receipt_id.clone(),
                donation_id: donation_id.clone(),
                key: format!("key-{suffix}"),
                file_name: Some("receipt-1.png".to_string()),
                content_type: Some("image/png".to_string()),
                size: Some(1200),
                is_encrypted: None,
                encrypted_payload: None,
            }],
        },
    )
    .await
    .expect("older batch sync");

    let donations_after_older = db::donations::list_donations(&pool, &user_id, Some(2026))
        .await
        .expect("list donations after older sync");
    assert_eq!(donations_after_older.len(), 1);
    assert_eq!(donations_after_older[0].amount, Some(25.0));
    assert_eq!(
        donations_after_older[0].notes.as_deref(),
        Some("first sync")
    );

    db::batch_sync(
        &pool,
        &user_id,
        BatchSyncRequest {
            donations: vec![DonationSyncItem {
                action: "update".to_string(),
                id: donation_id.clone(),
                date: Some(NaiveDate::from_ymd_opt(2026, 3, 3).expect("valid date")),
                year: Some(2026),
                category: Some("goods".to_string()),
                amount: Some(55.0),
                charity_id: charity_id.clone(),
                notes: Some("newer sync".to_string()),
                updated_at: Some(newer_updated_at),
                is_encrypted: None,
                encrypted_payload: None,
            }],
            receipts: vec![ReceiptSyncItem {
                action: "create".to_string(),
                id: receipt_id.clone(),
                donation_id: donation_id.clone(),
                key: format!("key-{suffix}"),
                file_name: Some("receipt-1.png".to_string()),
                content_type: Some("image/png".to_string()),
                size: Some(1200),
                is_encrypted: None,
                encrypted_payload: None,
            }],
        },
    )
    .await
    .expect("newer batch sync");

    let donations_after_newer = db::donations::list_donations(&pool, &user_id, Some(2026))
        .await
        .expect("list donations after newer sync");
    assert_eq!(donations_after_newer.len(), 1);
    assert_eq!(donations_after_newer[0].amount, Some(55.0));
    assert_eq!(
        donations_after_newer[0].notes.as_deref(),
        Some("newer sync")
    );
    assert_eq!(
        donations_after_newer[0].date,
        NaiveDate::from_ymd_opt(2026, 3, 3).expect("valid date")
    );

    let receipts = db::receipts::list_receipts(&pool, &user_id, Some(donation_id.clone()))
        .await
        .expect("list receipts");
    assert_eq!(receipts.len(), 1);

    db::users::delete_user_data(&pool, &user_id)
        .await
        .expect("cleanup test user data");
}

#[tokio::test]
async fn clob_updates_round_trip_large_payloads() {
    let pool = init_test_pool().await;
    let (user_id, _) = create_test_user(&pool).await;
    let suffix = Uuid::new_v4().to_string();
    let charity_id = create_test_charity(&pool, &user_id, &suffix).await;
    let now = Utc::now();
    let donation_id = format!("oracle-clob-donation-{suffix}");
    let receipt_id = format!("oracle-clob-receipt-{suffix}");

    db::donations::add_donation(
        &pool,
        &NewDonation {
            id: donation_id.clone(),
            user_id: user_id.clone(),
            year: 2026,
            date: NaiveDate::from_ymd_opt(2026, 4, 12).expect("valid date"),
            category: Some("goods".to_string()),
            charity_id: charity_id.clone(),
            amount: Some(88.0),
            notes: Some("large clob test".to_string()),
            is_encrypted: None,
            encrypted_payload: None,
            created_at: now,
        },
    )
    .await
    .expect("add donation");

    db::receipts::add_receipt(
        &pool,
        &NewReceipt {
            id: receipt_id.clone(),
            donation_id: donation_id.clone(),
            key: format!("receipt-key-{suffix}"),
            file_name: Some("receipt.pdf".to_string()),
            content_type: Some("application/pdf".to_string()),
            size: Some(4096),
            is_encrypted: None,
            encrypted_payload: None,
            created_at: now,
        },
    )
    .await
    .expect("add receipt");

    let large_ocr_text = "OCR-LINE-".repeat(8_000);
    db::receipts::set_receipt_ocr(
        &pool,
        &receipt_id,
        &Some(large_ocr_text.clone()),
        &None,
        &Some(88.0),
        &Some("processed".to_string()),
    )
    .await
    .expect("set receipt ocr");

    let fetched_receipt = db::receipts::get_receipt(&pool, &user_id, &receipt_id)
        .await
        .expect("get receipt")
        .expect("receipt exists");
    assert_eq!(
        fetched_receipt.ocr_text.as_deref(),
        Some(large_ocr_text.as_str())
    );

    let audit_id = format!("oracle-clob-audit-{suffix}");
    let large_audit_details = "AUDIT-BLOCK-".repeat(8_000);
    db::audit::log_audit(
        &pool,
        &audit_id,
        &user_id,
        "large_payload_test",
        "receipts",
        &Some(receipt_id.clone()),
        &Some(large_audit_details.clone()),
    )
    .await
    .expect("log audit");

    let audit_logs = db::list_audit_logs(&pool, &user_id, None)
        .await
        .expect("list audit logs");
    let matching_audit = audit_logs
        .iter()
        .find(|log| log.id == audit_id)
        .expect("matching audit log");
    assert_eq!(
        matching_audit.details.as_deref(),
        Some(large_audit_details.as_str())
    );

    db::users::delete_user_data(&pool, &user_id)
        .await
        .expect("cleanup test user data");
}

#[tokio::test]
async fn valuation_prefix_search_and_charity_lookup_use_index_friendly_paths() {
    let pool = init_test_pool().await;
    let (user_id, _) = create_test_user(&pool).await;
    let suffix = Uuid::new_v4().to_string();
    let charity_id = create_test_charity(&pool, &user_id, &suffix).await;

    let lookup_by_name = db::charities::find_charity_by_name_or_ein(
        &pool,
        &user_id,
        &format!("helping hands {suffix}"),
        &None,
    )
    .await
    .expect("lookup charity by name")
    .expect("charity exists by name");
    assert_eq!(lookup_by_name.id, charity_id);

    let lookup_by_ein = db::charities::find_charity_by_name_or_ein(
        &pool,
        &user_id,
        "non matching name",
        &Some(format!("99-{suffix}")),
    )
    .await
    .expect("lookup charity by ein")
    .expect("charity exists by ein");
    assert_eq!(lookup_by_ein.id, charity_id);

    ensure_minimal_valuation_data(&pool).await;

    let prefix_matches = db::valuations::suggest_valuations(&pool, "Air")
        .await
        .expect("prefix valuation search");
    assert!(
        prefix_matches
            .iter()
            .any(|item| item.0 == "Air Conditioner"),
        "expected Air Conditioner in prefix results"
    );

    let infix_matches = db::valuations::suggest_valuations(&pool, "Conditioner")
        .await
        .expect("infix valuation search");
    assert!(
        infix_matches.iter().all(|item| item.0 != "Air Conditioner"),
        "did not expect Air Conditioner in infix results after prefix optimization"
    );

    db::users::delete_user_data(&pool, &user_id)
        .await
        .expect("cleanup test user data");
}
