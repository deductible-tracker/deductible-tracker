use anyhow::anyhow;
use chrono::Utc;
use r2d2::Pool;
use tokio::task;

use crate::db::oracle::OracleConnectionManager;
use crate::db::models::Donation as DonationModel;
use crate::db::models::NewDonation;

pub fn parse_utc_from_opt_string(value: Option<String>) -> chrono::DateTime<Utc> {
    value
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(Utc::now)
}

pub(crate) async fn add_donation(
    pool: &Pool<OracleConnectionManager>,
    input: &NewDonation,
    created_at: &str,
) -> anyhow::Result<()> {
    let p = pool.clone();
    let input = input.clone();
    let created_at = created_at.to_string();
    task::spawn_blocking(move || -> anyhow::Result<()> {
        let conn = p.get()?;
        let sql = "INSERT INTO donations (id, user_id, donation_year, donation_date, donation_category, donation_amount, charity_id, notes, created_at) VALUES (:1, :2, :3, :4, :5, :6, :7, :8, TO_TIMESTAMP_TZ(:9, 'YYYY-MM-DD\"T\"HH24:MI:SS.FF TZH:TZM'))";
        conn.execute(sql, &[&input.id, &input.user_id, &input.year, &input.date, &input.category, &input.amount, &input.charity_id, &input.notes, &created_at])?;
        let _ = conn.commit();
        Ok(())
    })
    .await
    .map_err(|e| anyhow!("DB task join error: {}", e))??;
    Ok(())
}

pub(crate) async fn list_donations(
    pool: &Pool<OracleConnectionManager>,
    user_id: &str,
    year: Option<i32>,
) -> anyhow::Result<Vec<DonationModel>> {
    let p = pool.clone();
    let user_id = user_id.to_string();
    let rows = task::spawn_blocking(move || -> anyhow::Result<Vec<DonationModel>> {
        let conn = p.get()?;
        let sql = if year.is_some() {
            "SELECT d.id, d.user_id, d.donation_year, d.donation_date, d.donation_category, d.donation_amount, d.charity_id, c.name, c.ein, d.notes, d.created_at, d.updated_at FROM donations d JOIN charities c ON c.id = d.charity_id WHERE d.user_id = :1 AND d.donation_year = :2 AND d.deleted = 0"
        } else {
            "SELECT d.id, d.user_id, d.donation_year, d.donation_date, d.donation_category, d.donation_amount, d.charity_id, c.name, c.ein, d.notes, d.created_at, d.updated_at FROM donations d JOIN charities c ON c.id = d.charity_id WHERE d.user_id = :1 AND d.deleted = 0"
        };
        let rows = if let Some(y) = year {
            conn.query(sql, &[&user_id, &y])?
        } else {
            conn.query(sql, &[&user_id])?
        };
        let mut out = Vec::new();
        for row in rows.flatten() {
            out.push(DonationModel {
                id: row.get(0).unwrap_or_default(),
                user_id: row.get(1).unwrap_or_default(),
                year: row.get(2).unwrap_or_default(),
                date: row.get(3).unwrap_or_else(|_| Utc::now().date_naive()),
                category: row.get(4).ok(),
                amount: row.get(5).ok(),
                charity_id: row.get(6).unwrap_or_default(),
                charity_name: row.get(7).unwrap_or_default(),
                charity_ein: row.get(8).ok(),
                notes: row.get(9).ok(),
                shared_with: None,
                created_at: parse_utc_from_opt_string(row.get(10).ok()),
                updated_at: parse_utc_from_opt_string(row.get(11).ok()),
                deleted: false,
            });
        }
        Ok(out)
    })
    .await
    .map_err(|e| anyhow!("DB task join error: {}", e))??;
    Ok(rows)
}

pub(crate) async fn list_donations_since(
    pool: &Pool<OracleConnectionManager>,
    user_id: &str,
    since: &str,
) -> anyhow::Result<Vec<DonationModel>> {
    let p = pool.clone();
    let user_id = user_id.to_string();
    let since = since.to_string();
    let rows = task::spawn_blocking(move || -> anyhow::Result<Vec<DonationModel>> {
        let conn = p.get()?;
        let parse_utc = |value: Option<String>| {
            value
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(Utc::now)
        };
        let sql = "SELECT d.id, d.user_id, d.donation_year, d.donation_date, d.donation_category, d.donation_amount, d.charity_id, c.name, c.ein, d.notes, d.created_at, d.updated_at, d.deleted FROM donations d JOIN charities c ON c.id = d.charity_id WHERE d.user_id = :1 AND (d.updated_at > :2 OR d.created_at > :2)";
        let rows = conn.query(sql, &[&user_id, &since])?;
        let mut out = Vec::new();
        for row in rows.flatten() {
            out.push(DonationModel {
                id: row.get(0).unwrap_or_default(),
                user_id: row.get(1).unwrap_or_default(),
                year: row.get(2).unwrap_or_default(),
                date: row.get(3).unwrap_or_else(|_| Utc::now().date_naive()),
                category: row.get(4).ok(),
                amount: row.get(5).ok(),
                charity_id: row.get(6).unwrap_or_default(),
                charity_name: row.get(7).unwrap_or_default(),
                charity_ein: row.get(8).ok(),
                notes: row.get(9).ok(),
                shared_with: None,
                created_at: parse_utc(row.get(10).ok()),
                updated_at: parse_utc(row.get(11).ok()),
                deleted: row.get::<usize, i64>(12).unwrap_or(0) != 0,
            });
        }
        Ok(out)
    })
    .await
    .map_err(|e| anyhow!("DB task join error: {}", e))??;
    Ok(rows)
}
