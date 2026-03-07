use crate::db::DbPool;
use crate::db::models::UserProfileUpsert;
use crate::db::UserProfileRow;

pub async fn get_user_profile(
    pool: &DbPool,
    user_id: &str,
) -> anyhow::Result<Option<UserProfileRow>> {
    super::get_user_profile(pool, user_id).await
}

pub async fn get_user_profile_by_email(
    pool: &DbPool,
    email: &str,
) -> anyhow::Result<Option<(String, UserProfileRow)>> {
    super::get_user_profile_by_email(pool, email).await
}

pub async fn upsert_user_profile(
    pool: &DbPool,
    input: &UserProfileUpsert,
) -> anyhow::Result<()> {
    super::upsert_user_profile(pool, input).await
}
