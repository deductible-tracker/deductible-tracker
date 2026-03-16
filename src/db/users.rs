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

pub async fn delete_user_data(
    pool: &DbPool,
    user_id: &str,
) -> anyhow::Result<()> {
    super::delete_user_data(pool, user_id).await
}

pub async fn import_data(
    pool: &DbPool,
    user_id: &str,
    profile: &UserProfileUpsert,
    charities: &[crate::db::models::Charity],
    donations: &[crate::db::models::Donation],
    receipts: &[crate::db::models::Receipt],
) -> anyhow::Result<()> {
    super::import_data(pool, user_id, profile, charities, donations, receipts).await
}
