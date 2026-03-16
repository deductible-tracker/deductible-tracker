use crate::db::DbPool;
use crate::db::models::{NewCharity, CharityPatch, Charity};

pub async fn list_charities(
    pool: &DbPool,
    user_id: &str,
) -> anyhow::Result<Vec<Charity>> {
    super::list_charities(pool, user_id).await
}

pub async fn find_charity_by_name_or_ein(
    pool: &DbPool,
    user_id: &str,
    name: &str,
    ein: &Option<String>,
) -> anyhow::Result<Option<Charity>> {
    super::find_charity_by_name_or_ein(pool, user_id, name, ein).await
}

pub async fn create_charity(
    pool: &DbPool,
    input: &NewCharity,
) -> anyhow::Result<()> {
    super::create_charity(pool, input).await
}

pub async fn update_charity(
    pool: &DbPool,
    patch: &CharityPatch,
) -> anyhow::Result<bool> {
    super::update_charity(pool, patch).await
}

pub async fn delete_charity(
    pool: &DbPool,
    user_id: &str,
    charity_id: &str,
) -> anyhow::Result<bool> {
    super::delete_charity(pool, user_id, charity_id).await
}

pub async fn count_donations_for_charity(
    pool: &DbPool,
    user_id: &str,
    charity_id: &str,
) -> anyhow::Result<i64> {
    super::count_donations_for_charity(pool, user_id, charity_id).await
}
