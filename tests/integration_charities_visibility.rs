use deadpool_oracle::PoolBuilder;
use deductible_tracker::db;
use deductible_tracker::db::models::{NewCharity, UserProfileUpsert};
use oracle_rs::Config as OracleConfig;
use oracle_rs::Value;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::time::sleep;
use tokio::time::timeout;
use uuid::Uuid;

fn charity_visibility_test_mutex() -> &'static tokio::sync::Mutex<()> {
    static MUTEX: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    MUTEX.get_or_init(|| tokio::sync::Mutex::new(()))
}

fn oracle_step_timeout() -> Duration {
    Duration::from_secs(30)
}

async fn init_test_pool() -> db::DbPool {
    std::env::set_var("RUST_ENV", "development");
    std::env::remove_var("DB_WALLET_DIR");
    std::env::remove_var("MY_WALLET_DIRECTORY");
    std::env::remove_var("TNS_ADMIN");

    let username = std::env::var("DEV_ORACLE_USER").expect("DEV_ORACLE_USER must be set");
    let password = std::env::var("DEV_ORACLE_PASSWORD").expect("DEV_ORACLE_PASSWORD must be set");
    let connect_string =
        std::env::var("DEV_ORACLE_CONNECT_STRING").expect("DEV_ORACLE_CONNECT_STRING must be set");

    for _ in 0..5 {
        let mut driver_config =
            OracleConfig::from_str(&connect_string).expect("valid DEV_ORACLE_CONNECT_STRING");
        driver_config.set_username(username.clone());
        driver_config.set_password(password.clone());

        match PoolBuilder::new(driver_config)
            .max_size(8)
            .wait_timeout(Some(Duration::from_secs(15)))
            .create_timeout(Some(Duration::from_secs(15)))
            .recycle_timeout(Some(Duration::from_secs(5)))
            .build()
        {
            Ok(pool) => return Arc::new(db::DbPoolEnum::Oracle(pool)),
            Err(error)
                if error
                    .to_string()
                    .contains("Timeout occurred while creating a new object")
                    || error.to_string().contains("connection not ready") =>
            {
                sleep(Duration::from_millis(250)).await;
            }
            Err(error) => panic!("init pool: {error}"),
        }
    }

    panic!("init pool: local Oracle never became ready after retries");
}

async fn create_test_user(pool: &db::DbPool, suffix: &str) -> String {
    let user_id = format!("charity-visibility-user-{suffix}");
    let input = UserProfileUpsert {
        user_id: user_id.clone(),
        email: format!("{suffix}@example.test"),
        name: format!("Charity Visibility {suffix}"),
        provider: "local".to_string(),
        filing_status: None,
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
    user_id
}

async fn insert_test_charity(pool: &db::DbPool, charity: &NewCharity) {
    match &**pool {
        db::DbPoolEnum::Oracle(oracle_pool) => {
            let conn = oracle_pool.get().await.expect("checkout oracle connection");
            conn.execute(
                    "INSERT INTO charities (id, user_id, name, ein, category, status, classification, nonprofit_type, deductibility, street, city, state, zip, created_at) VALUES (:1, :2, :3, :4, :5, :6, :7, :8, :9, :10, :11, :12, :13, TO_TIMESTAMP_TZ(:14, 'YYYY-MM-DD\"T\"HH24:MI:SS.FF TZH:TZM'))",
                    &vec![
                        Value::from(charity.id.clone()),
                        Value::from(charity.user_id.clone()),
                        Value::from(charity.name.clone()),
                        Value::from(charity.ein.clone()),
                        Value::from(charity.category.clone()),
                        Value::from(charity.status.clone()),
                        Value::from(charity.classification.clone()),
                        Value::from(charity.nonprofit_type.clone()),
                        Value::from(charity.deductibility.clone()),
                        Value::from(charity.street.clone()),
                        Value::from(charity.city.clone()),
                        Value::from(charity.state.clone()),
                        Value::from(charity.zip.clone()),
                        Value::from(charity.created_at.to_rfc3339()),
                    ],
                )
                .await
                .expect("insert charity row");
            conn.commit().await.expect("commit charity row");
        }
    }
}

#[tokio::test]
async fn created_charity_is_visible_in_same_users_list() {
    let _guard = charity_visibility_test_mutex().lock().await;

    let pool = init_test_pool().await;

    let suffix = Uuid::new_v4().to_string();
    let user_id = timeout(oracle_step_timeout(), create_test_user(&pool, &suffix))
        .await
        .expect("timed out creating first test user");
    let other_user_id = timeout(
        oracle_step_timeout(),
        create_test_user(&pool, &format!("other-{suffix}")),
    )
    .await
    .expect("timed out creating second test user");

    let charity_id = format!("charity-visibility-{suffix}");
    let charity_name = format!("Visibility Charity {suffix}");
    let charity = NewCharity {
        id: charity_id.clone(),
        user_id: user_id.clone(),
        name: charity_name.clone(),
        ein: None,
        category: Some("community".to_string()),
        status: Some("active".to_string()),
        classification: Some("charitable".to_string()),
        nonprofit_type: None,
        deductibility: Some("deductible".to_string()),
        street: Some("1 Main St".to_string()),
        city: Some("Austin".to_string()),
        state: Some("TX".to_string()),
        zip: Some("78701".to_string()),
        is_encrypted: None,
        encrypted_payload: None,
        created_at: chrono::Utc::now(),
    };

    timeout(oracle_step_timeout(), insert_test_charity(&pool, &charity))
        .await
        .expect("timed out inserting first charity");

    let other_charity = NewCharity {
        id: format!("charity-visibility-other-{suffix}"),
        user_id: other_user_id.clone(),
        name: format!("Other User Charity {suffix}"),
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
        is_encrypted: None,
        encrypted_payload: None,
        created_at: chrono::Utc::now(),
    };

    timeout(
        oracle_step_timeout(),
        insert_test_charity(&pool, &other_charity),
    )
    .await
    .expect("timed out inserting second charity");

    let charities = timeout(
        oracle_step_timeout(),
        db::charities::list_charities(&pool, &user_id),
    )
    .await
    .expect("timed out listing charities")
    .expect("list charities");
    let created = charities
        .iter()
        .find(|item| item.id == charity_id)
        .expect("created charity present in list");

    assert_eq!(created.name, charity_name);
    assert_eq!(created.category.as_deref(), Some("community"));
    assert_eq!(created.status.as_deref(), Some("active"));
    assert!(charities.iter().all(|item| item.user_id == user_id));
    assert!(charities.iter().all(|item| item.id != other_charity.id));

    timeout(
        oracle_step_timeout(),
        db::users::delete_user_data(&pool, &user_id),
    )
    .await
    .expect("timed out cleaning up first user")
    .expect("cleanup first user");
    timeout(
        oracle_step_timeout(),
        db::users::delete_user_data(&pool, &other_user_id),
    )
    .await
    .expect("timed out cleaning up second user")
    .expect("cleanup second user");
}
