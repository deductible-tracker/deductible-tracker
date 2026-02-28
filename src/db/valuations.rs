use crate::db::DbPool;

pub async fn suggest_valuations(
    pool: &DbPool,
    query: &str,
) -> anyhow::Result<Vec<(String, Option<i64>, Option<i64>)>> {
    super::suggest_valuations(pool, query).await
}

pub async fn seed_valuations(pool: &DbPool) -> anyhow::Result<()> {
    super::seed_valuations(pool).await
}
