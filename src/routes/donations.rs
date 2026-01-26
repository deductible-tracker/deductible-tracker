use axum::{
    extract::{State, Query, Json},
    response::{IntoResponse, Json as AxumJson},
    http::StatusCode,
};
use serde::Deserialize;
use crate::AppState;
use crate::db::models::Donation;
use crate::auth::AuthenticatedUser;
use uuid::Uuid;
use chrono::{NaiveDate, Datelike};

#[derive(Deserialize)]
pub struct CreateDonationRequest {
    pub date: String, // YYYY-MM-DD
    pub charity_name: String,
    pub charity_id: String,
    pub notes: Option<String>,
}

#[derive(Deserialize)]
pub struct ListParams {
    year: Option<i32>,
}

pub async fn create_donation(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(req): Json<CreateDonationRequest>
) -> impl IntoResponse {
    let user_id = user.id;
    
    let date = NaiveDate::parse_from_str(&req.date, "%Y-%m-%d")
        .unwrap_or_else(|_| chrono::Utc::now().date_naive());
    
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now();
    let year = date.year();

    let conn = state.db.get();
    if conn.is_err() {
         return (StatusCode::INTERNAL_SERVER_ERROR, "DB Connection Error").into_response();
    }
    let conn = conn.unwrap();

    // Oracle SQL
    let sql = "INSERT INTO donations (id, user_id, donation_year, donation_date, charity_id, charity_name, notes, created_at) VALUES (:1, :2, :3, :4, :5, :6, :7, :8)";

    let res = conn.execute(sql, &[
        &id,
        &user_id,
        &year,
        &date, // Might need formatting for Oracle DATE/TIMESTAMP
        &req.charity_id,
        &req.charity_name,
        &req.notes,
        &now
    ]);

    match res {
        Ok(_) => {
            let _ = conn.commit();
            (StatusCode::CREATED, AxumJson(serde_json::json!({ "status": "created", "id": id }))).into_response()
        },
        Err(e) => {
            tracing::error!("DB Error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Database Error").into_response()
        }
    }
}

pub async fn list_donations(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Query(params): Query<ListParams>,
) -> impl IntoResponse {
    let user_id = user.id;
    
    let conn = state.db.get();
    if conn.is_err() {
         return (StatusCode::INTERNAL_SERVER_ERROR, "DB Connection Error").into_response();
    }
    let conn = conn.unwrap();

    let sql = if let Some(_y) = params.year {
        "SELECT id, user_id, donation_year, donation_date, charity_id, charity_name, notes FROM donations WHERE user_id = :1 AND donation_year = :2"
    } else {
        "SELECT id, user_id, donation_year, donation_date, charity_id, charity_name, notes FROM donations WHERE user_id = :1"
    };

    let rows_result = if let Some(y) = params.year {
        conn.query(sql, &[&user_id, &y])
    } else {
         conn.query(sql, &[&user_id])
    };

    let mut donations = Vec::new();
    match rows_result {
        Ok(rows) => {
            for row_result in rows {
                if let Ok(row) = row_result {
                    // Manual mapping to Donation struct
                    let d = Donation {
                        id: row.get(0).unwrap_or_default(),
                        user_id: row.get(1).unwrap_or_default(),
                        year: row.get(2).unwrap_or_default(),
                        date: row.get(3).unwrap_or_else(|_| chrono::Local::now().date_naive()),
                        charity_id: row.get(4).unwrap_or_default(),
                        charity_name: row.get(5).unwrap_or_default(),
                        charity_ein: None,
                        notes: row.get(6).ok(),
                        shared_with: None,
                        created_at: chrono::Utc::now(), // Mocked as it's not in the select
                        updated_at: chrono::Utc::now(),
                        deleted: false,
                    };
                    donations.push(d);
                }
            }
            AxumJson(serde_json::json!({ "donations": donations })).into_response()
        },
        Err(e) => {
             tracing::error!("DB Query Error: {}", e);
             (StatusCode::INTERNAL_SERVER_ERROR, "Database Error").into_response()
        }
    }
}