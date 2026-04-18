use deductible_tracker::db;
use deductible_tracker::db::models::UserProfileUpsert;
use oracle_rs::Value;
use std::sync::OnceLock;
use uuid::Uuid;

fn auth_profile_test_mutex() -> &'static tokio::sync::Mutex<()> {
    static MUTEX: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    MUTEX.get_or_init(|| tokio::sync::Mutex::new(()))
}

#[tokio::test]
async fn oracle_user_profile_round_trip() {
    let _guard = auth_profile_test_mutex().lock().await;
    std::env::set_var("RUST_ENV", "development");

    let pool = db::init_pool().await.expect("init pool");

    let suffix = Uuid::new_v4().to_string();
    let user_id = format!("auth-profile-{suffix}");
    let email = format!("{suffix}@example.test");
    let input = UserProfileUpsert {
        user_id: user_id.clone(),
        email: email.clone(),
        name: "Auth Profile Test".to_string(),
        provider: "local".to_string(),
        filing_status: Some("single".to_string()),
        agi: Some(123_456.78),
        marginal_tax_rate: Some(24.0),
        itemize_deductions: Some(true),
        is_encrypted: None,
        encrypted_payload: None,
        vault_credential_id: None,
    };

    db::users::upsert_user_profile(&pool, &input)
        .await
        .expect("upsert user profile");

    let profile_by_id = db::users::get_user_profile(&pool, &user_id)
        .await
        .expect("get user profile by id")
        .expect("profile exists by id");
    assert_eq!(profile_by_id.0, input.email);
    assert_eq!(profile_by_id.1, input.name);
    assert_eq!(profile_by_id.2, input.provider);
    assert_eq!(profile_by_id.3, input.filing_status);
    assert_eq!(profile_by_id.4, input.agi);
    assert_eq!(profile_by_id.5, input.marginal_tax_rate);
    assert_eq!(profile_by_id.6, input.itemize_deductions);

    let profile_by_email = db::users::get_user_profile_by_email(&pool, &email)
        .await
        .expect("get user profile by email")
        .expect("profile exists by email");
    assert_eq!(profile_by_email.0, user_id);
    assert_eq!(profile_by_email.1 .0, input.email);
    assert_eq!(profile_by_email.1 .1, input.name);
    assert_eq!(profile_by_email.1 .2, input.provider);
    assert_eq!(profile_by_email.1 .3, input.filing_status);
    assert_eq!(profile_by_email.1 .4, input.agi);
    assert_eq!(profile_by_email.1 .5, input.marginal_tax_rate);
    assert_eq!(profile_by_email.1 .6, input.itemize_deductions);

    db::users::delete_user_data(&pool, &user_id)
        .await
        .expect("delete user profile test data");
}

#[tokio::test]
async fn oracle_repeated_profile_text_query_stays_stable() {
    let _guard = auth_profile_test_mutex().lock().await;
    std::env::set_var("RUST_ENV", "development");

    let pool = db::init_pool().await.expect("init pool");

    let suffix = Uuid::new_v4().to_string();
    let user_id = format!("auth-profile-repeat-{suffix}");
    let email = format!("repeat-{suffix}@example.test");
    let input = UserProfileUpsert {
        user_id: user_id.clone(),
        email: email.clone(),
        name: "Repeat Query Test".to_string(),
        provider: "local".to_string(),
        filing_status: Some("married_joint".to_string()),
        agi: Some(80_000.0),
        marginal_tax_rate: Some(0.12),
        itemize_deductions: Some(false),
        is_encrypted: None,
        encrypted_payload: None,
        vault_credential_id: None,
    };

    db::users::upsert_user_profile(&pool, &input)
        .await
        .expect("upsert user profile");

    match &*pool {
        db::DbPoolEnum::Oracle(oracle_pool) => {
            let conn = oracle_pool.get().await.expect("checkout oracle connection");
            let sql = "SELECT email, name, provider FROM users WHERE id = :1";

            for _ in 0..3 {
                let rows = conn
                    .query(sql, &[Value::from(user_id.clone())])
                    .await
                    .expect("repeat profile query");
                let row = rows.first().expect("profile row present");
                assert_eq!(row.get_string(0), Some(email.as_str()));
                assert_eq!(row.get_string(1), Some("Repeat Query Test"));
                assert_eq!(row.get_string(2), Some("local"));
            }
        }
    }

    db::users::delete_user_data(&pool, &user_id)
        .await
        .expect("delete repeated-query test data");
}
