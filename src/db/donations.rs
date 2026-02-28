use crate::db::DbPool;
use crate::db::models::Donation as DonationModel;
use crate::db::models::{DonationPatch, NewDonation};

pub async fn add_donation(
    pool: &DbPool,
    input: &NewDonation,
) -> anyhow::Result<()> {
    super::add_donation(pool, input).await
}

pub async fn list_donations(
    pool: &DbPool,
    user_id: &str,
    year: Option<i32>,
) -> anyhow::Result<Vec<DonationModel>> {
    super::list_donations(pool, user_id, year).await
}

pub async fn list_donations_since(
    pool: &DbPool,
    user_id: &str,
    since: chrono::DateTime<chrono::Utc>,
) -> anyhow::Result<Vec<DonationModel>> {
    super::list_donations_since(pool, user_id, since).await
}

pub async fn update_donation(
    pool: &DbPool,
    patch: &DonationPatch,
) -> anyhow::Result<bool> {
    super::update_donation(pool, patch).await
}

pub async fn soft_delete_donation(
    pool: &DbPool,
    user_id: &str,
    donation_id: &str,
) -> anyhow::Result<bool> {
    super::soft_delete_donation(pool, user_id, donation_id).await
}

