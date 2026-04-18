use chrono::Utc;
use deadpool_oracle::Pool;

use crate::db::models::Donation as DonationModel;
use crate::db::models::NewDonation;

pub fn parse_utc_from_opt_string(value: Option<String>) -> chrono::DateTime<Utc> {
    crate::db::oracle::parse_utc_from_opt_string(value)
}

pub(crate) async fn add_donation(
    pool: &Pool,
    input: &NewDonation,
    created_at: &str,
) -> anyhow::Result<()> {
    let conn = pool.get().await?;
    let donation_date = input.date.format("%Y-%m-%d").to_string();
    let is_encrypted = input.is_encrypted.map(|v| if v { 1 } else { 0 });
    let sql = "INSERT INTO donations (id, user_id, donation_year, donation_date, donation_category, donation_amount, charity_id, notes, is_encrypted, encrypted_payload, created_at) VALUES (:1, :2, :3, TO_DATE(:4, 'YYYY-MM-DD'), :5, :6, :7, :8, :9, :10, TO_TIMESTAMP_TZ(:11, 'YYYY-MM-DD\"T\"HH24:MI:SS.FF TZH:TZM'))";
    conn.execute(
        sql,
        &crate::oracle_params![
            input.id.clone(),
            input.user_id.clone(),
            input.year,
            donation_date,
            input.category.clone(),
            input.amount,
            input.charity_id.clone(),
            input.notes.clone(),
            is_encrypted,
            input.encrypted_payload.clone(),
            created_at.to_string(),
        ],
    )
    .await?;
    conn.commit().await?;
    Ok(())
}

pub(crate) async fn list_donations(
    pool: &Pool,
    user_id: &str,
    year: Option<i32>,
) -> anyhow::Result<Vec<DonationModel>> {
    let conn = pool.get().await?;
    let sql = if year.is_some() {
        "SELECT d.id, d.user_id, d.donation_year, d.donation_date, d.donation_category, d.donation_amount, d.charity_id, c.name, c.ein, d.notes, d.created_at, d.updated_at, d.is_encrypted, d.encrypted_payload FROM donations d JOIN charities c ON c.id = d.charity_id WHERE d.user_id = :1 AND d.donation_year = :2 AND d.deleted = 0"
    } else {
        "SELECT d.id, d.user_id, d.donation_year, d.donation_date, d.donation_category, d.donation_amount, d.charity_id, c.name, c.ein, d.notes, d.created_at, d.updated_at, d.is_encrypted, d.encrypted_payload FROM donations d JOIN charities c ON c.id = d.charity_id WHERE d.user_id = :1 AND d.deleted = 0"
    };
    let rows = if let Some(year) = year {
        conn.query(sql, &crate::oracle_params![user_id.to_string(), year])
            .await?
    } else {
        conn.query(sql, &crate::oracle_params![user_id.to_string()])
            .await?
    };
    let mut out = Vec::new();
    for row in &rows.rows {
        out.push(DonationModel {
            id: crate::db::oracle::row_string(row, 0),
            user_id: crate::db::oracle::row_string(row, 1),
            year: crate::db::oracle::row_i64(row, 2).unwrap_or_default() as i32,
            date: crate::db::oracle::row_naive_date(row, 3)
                .unwrap_or_else(|| Utc::now().date_naive()),
            category: crate::db::oracle::row_opt_string(row, 4),
            amount: crate::db::oracle::row_f64(row, 5),
            charity_id: crate::db::oracle::row_string(row, 6),
            charity_name: crate::db::oracle::row_string(row, 7),
            charity_ein: crate::db::oracle::row_opt_string(row, 8),
            notes: crate::db::oracle::row_opt_string(row, 9),
            is_encrypted: crate::db::oracle::row_bool(row, 12),
            encrypted_payload: crate::db::oracle::row_opt_string(row, 13),
            shared_with: None,
            created_at: crate::db::oracle::row_datetime_utc(row, 10).unwrap_or_else(Utc::now),
            updated_at: crate::db::oracle::row_datetime_utc(row, 11).unwrap_or_else(Utc::now),
            deleted: false,
        });
    }
    Ok(out)
}

pub(crate) async fn list_donations_since(
    pool: &Pool,
    user_id: &str,
    since: &str,
) -> anyhow::Result<Vec<DonationModel>> {
    let conn = pool.get().await?;
    let sql = "SELECT d.id, d.user_id, d.donation_year, d.donation_date, d.donation_category, d.donation_amount, d.charity_id, c.name, c.ein, d.notes, d.created_at, d.updated_at, d.deleted, d.is_encrypted, d.encrypted_payload FROM donations d JOIN charities c ON c.id = d.charity_id WHERE d.user_id = :1 AND (d.updated_at > TO_TIMESTAMP_TZ(:2, 'YYYY-MM-DD\"T\"HH24:MI:SS.FF TZH:TZM') OR d.created_at > TO_TIMESTAMP_TZ(:2, 'YYYY-MM-DD\"T\"HH24:MI:SS.FF TZH:TZM'))";
    let rows = conn
        .query(
            sql,
            &crate::oracle_params![user_id.to_string(), since.to_string()],
        )
        .await?;
    let mut out = Vec::new();
    for row in &rows.rows {
        out.push(DonationModel {
            id: crate::db::oracle::row_string(row, 0),
            user_id: crate::db::oracle::row_string(row, 1),
            year: crate::db::oracle::row_i64(row, 2).unwrap_or_default() as i32,
            date: crate::db::oracle::row_naive_date(row, 3)
                .unwrap_or_else(|| Utc::now().date_naive()),
            category: crate::db::oracle::row_opt_string(row, 4),
            amount: crate::db::oracle::row_f64(row, 5),
            charity_id: crate::db::oracle::row_string(row, 6),
            charity_name: crate::db::oracle::row_string(row, 7),
            charity_ein: crate::db::oracle::row_opt_string(row, 8),
            notes: crate::db::oracle::row_opt_string(row, 9),
            is_encrypted: crate::db::oracle::row_bool(row, 13),
            encrypted_payload: crate::db::oracle::row_opt_string(row, 14),
            shared_with: None,
            created_at: crate::db::oracle::row_datetime_utc(row, 10).unwrap_or_else(Utc::now),
            updated_at: crate::db::oracle::row_datetime_utc(row, 11).unwrap_or_else(Utc::now),
            deleted: crate::db::oracle::row_bool(row, 12).unwrap_or(false),
        });
    }
    Ok(out)
}

pub(crate) async fn list_donation_years(pool: &Pool, user_id: &str) -> anyhow::Result<Vec<i32>> {
    let conn = pool.get().await?;
    let rows = conn
        .query(
            "SELECT DISTINCT donation_year FROM donations WHERE user_id = :1 AND deleted = 0 ORDER BY donation_year DESC",
            &crate::oracle_params![user_id.to_string()],
        )
        .await?;
    let mut out = Vec::new();
    for row in &rows.rows {
        if let Some(y) = crate::db::oracle::row_i64(row, 0) {
            out.push(y as i32);
        }
    }
    Ok(out)
}
