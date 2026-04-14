use deductible_tracker::db;
use oracle_rs::Value;
use std::time::Duration;

async fn ensure_valuations_seeded(pool: &db::DbPool) {
    for _ in 0..5 {
        match &**pool {
            db::DbPoolEnum::Oracle(oracle_pool) => match oracle_pool.get().await {
                Ok(conn) => {
                    let rows = conn
                        .query("SELECT COUNT(1) FROM val_items", &[])
                        .await
                        .expect("count valuation items");
                    if rows
                        .first()
                        .and_then(|row| row.get(0))
                        .and_then(|value| {
                            value
                                .as_i64()
                                .or_else(|| value.to_string().trim().parse::<i64>().ok())
                        })
                        .unwrap_or(0)
                        > 0
                    {
                        return;
                    }

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
                    return;
                }
                Err(error) if error.to_string().contains("connection not ready") => {
                    tokio::time::sleep(Duration::from_millis(250)).await;
                }
                Err(error) => panic!("checkout valuation connection: {error}"),
            },
        }
    }

    panic!("seed valuations did not succeed after retries: connection not ready");
}

#[tokio::test]
async fn valuation_tree_returns_grouped_categories_and_items() {
    std::env::set_var("RUST_ENV", "development");

    let pool = db::init_pool().await.expect("init pool");

    ensure_valuations_seeded(&pool).await;

    let tree = db::valuations::list_valuation_tree(&pool)
        .await
        .expect("list valuation tree");

    let categories = tree.as_array().expect("valuation tree array");
    assert!(!categories.is_empty(), "expected valuation categories");

    let appliances = categories
        .iter()
        .find(|category| {
            category.get("id").and_then(|value| value.as_str()) == Some("cat_appliances")
        })
        .expect("appliances category");

    let items = appliances
        .get("items")
        .and_then(|value| value.as_array())
        .expect("appliances items array");
    assert!(!items.is_empty(), "expected valuation items in appliances");

    let air_conditioner = items
        .iter()
        .find(|item| item.get("name").and_then(|value| value.as_str()) == Some("Air Conditioner"))
        .expect("air conditioner valuation item");

    assert_eq!(
        air_conditioner.get("min").and_then(|value| value.as_f64()),
        Some(21.0)
    );
    assert_eq!(
        air_conditioner.get("max").and_then(|value| value.as_f64()),
        Some(93.0)
    );
}
