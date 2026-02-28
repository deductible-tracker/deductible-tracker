use crate::db::DbPool;
use crate::db::models::UserProfileUpsert;

pub async fn get_user_profile(
    pool: &DbPool,
    user_id: &str,
) -> anyhow::Result<Option<(String, String, String, Option<String>, Option<f64>, Option<f64>, Option<bool>)>> {
    super::get_user_profile(pool, user_id).await
}

pub async fn upsert_user_profile(
    pool: &DbPool,
    input: &UserProfileUpsert,
) -> anyhow::Result<()> {
    super::upsert_user_profile(pool, input).await
}
