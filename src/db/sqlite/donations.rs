use anyhow::anyhow;
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager as R2SqliteManager;
use rusqlite::params;
use tokio::task;

use crate::db::models::Donation as DonationModel;
use crate::db::models::NewDonation;

pub(crate) async fn add_donation(
    pool: &Pool<R2SqliteManager>,
    input: &NewDonation,
    created_at: &str,
) -> anyhow::Result<()> {
    let p = pool.clone();
    let input = input.clone();
    let created_at = created_at.to_string();
    task::spawn_blocking(move || -> anyhow::Result<()> {
        let conn = p.get()?;
        let sql = "INSERT INTO donations (id, user_id, donation_year, donation_date, donation_category, donation_amount, charity_id, notes, created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)";
        let date_str = input.date.format("%Y-%m-%d").to_string();
        conn.execute(sql, params![input.id, input.user_id, input.year, date_str, input.category, input.amount, input.charity_id, input.notes, created_at])?;
        Ok(())
    })
    .await
    .map_err(|e| anyhow!("DB task join error: {}", e))??;
    Ok(())
}

pub(crate) async fn list_donations(
    pool: &Pool<R2SqliteManager>,
    user_id: &str,
    year: Option<i32>,
) -> anyhow::Result<Vec<DonationModel>> {
    let p = pool.clone();
    let user_id = user_id.to_string();
    let rows = task::spawn_blocking(move || -> anyhow::Result<Vec<DonationModel>> {
        let conn = p.get()?;
        let sql_with_year = "SELECT d.id, d.user_id, d.donation_year, d.donation_date, d.donation_category, d.donation_amount, d.charity_id, c.name, c.ein, d.notes FROM donations d JOIN charities c ON c.id = d.charity_id WHERE d.user_id = ?1 AND d.donation_year = ?2 AND d.deleted = 0";
        let sql_no_year = "SELECT d.id, d.user_id, d.donation_year, d.donation_date, d.donation_category, d.donation_amount, d.charity_id, c.name, c.ein, d.notes FROM donations d JOIN charities c ON c.id = d.charity_id WHERE d.user_id = ?1 AND d.deleted = 0";

        let mut out = Vec::new();
        if let Some(y) = year {
            let mut stmt = conn.prepare(sql_with_year)?;
            let rows_iter = stmt.query_map(params![user_id, y], |row| {
                let date_str: Option<String> = row.get(3)?;
                let date = date_str
                    .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok())
                    .unwrap_or_else(|| Utc::now().date_naive());
                Ok(DonationModel {
                    id: row.get(0)?,
                    user_id: row.get(1)?,
                    year: row.get(2)?,
                    date,
                    category: row.get(4).ok(),
                    amount: row.get(5).ok(),
                    charity_id: row.get(6)?,
                    charity_name: row.get(7)?,
                    charity_ein: row.get(8).ok(),
                    notes: row.get(9).ok(),
                    shared_with: None,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                    deleted: false,
                })
            })?;
            for r in rows_iter {
                out.push(r?);
            }
        } else {
            let mut stmt = conn.prepare(sql_no_year)?;
            let rows_iter = stmt.query_map(params![user_id], |row| {
                let date_str: Option<String> = row.get(3)?;
                let date = date_str
                    .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok())
                    .unwrap_or_else(|| Utc::now().date_naive());
                Ok(DonationModel {
                    id: row.get(0)?,
                    user_id: row.get(1)?,
                    year: row.get(2)?,
                    date,
                    category: row.get(4).ok(),
                    amount: row.get(5).ok(),
                    charity_id: row.get(6)?,
                    charity_name: row.get(7)?,
                    charity_ein: row.get(8).ok(),
                    notes: row.get(9).ok(),
                    shared_with: None,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                    deleted: false,
                })
            })?;
            for r in rows_iter {
                out.push(r?);
            }
        }
        Ok(out)
    })
    .await
    .map_err(|e| anyhow!("DB task join error: {}", e))??;
    Ok(rows)
}

pub(crate) async fn list_donations_since(
    pool: &Pool<R2SqliteManager>,
    user_id: &str,
    since: &str,
) -> anyhow::Result<Vec<DonationModel>> {
    let p = pool.clone();
    let user_id = user_id.to_string();
    let since = since.to_string();
    let rows = task::spawn_blocking(move || -> anyhow::Result<Vec<DonationModel>> {
        let conn = p.get()?;
        let sql = "SELECT d.id, d.user_id, d.donation_year, d.donation_date, d.donation_category, d.donation_amount, d.charity_id, c.name, c.ein, d.notes, d.created_at, d.updated_at, d.deleted FROM donations d JOIN charities c ON c.id = d.charity_id WHERE d.user_id = ?1 AND (d.updated_at > ?2 OR d.created_at > ?2)";
        let mut stmt = conn.prepare(sql)?;
        let mut out = Vec::new();
        let rows_iter = stmt.query_map(params![user_id, since], |row| {
            let date_str: Option<String> = row.get(3)?;
            let date = date_str
                .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok())
                .unwrap_or_else(|| Utc::now().date_naive());
            let created_at_str: Option<String> = row.get(10)?;
            let updated_at_str: Option<String> = row.get(11)?;
            let created_at = created_at_str
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(Utc::now);
            let updated_at = updated_at_str
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(Utc::now);
            Ok(DonationModel {
                id: row.get(0)?,
                user_id: row.get(1)?,
                year: row.get(2)?,
                date,
                category: row.get(4).ok(),
                amount: row.get(5).ok(),
                charity_id: row.get(6)?,
                charity_name: row.get(7)?,
                charity_ein: row.get(8).ok(),
                notes: row.get(9).ok(),
                shared_with: None,
                created_at,
                updated_at,
                deleted: row.get::<usize, i64>(12)? != 0,
            })
        })?;
        for r in rows_iter {
            out.push(r?);
        }
        Ok(out)
    })
    .await
    .map_err(|e| anyhow!("DB task join error: {}", e))??;
    Ok(rows)
}
