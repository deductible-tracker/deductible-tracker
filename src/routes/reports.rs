use axum::{
    extract::{State, Query},
    response::IntoResponse,
};
use crate::AppState;
use crate::auth::AuthenticatedUser;
use crate::db;
use axum::http::{HeaderValue, header};
use axum::response::Response;
use std::collections::BTreeSet;

#[derive(serde::Deserialize)]
pub struct ExportParams {
    pub year: Option<i32>,
}

#[derive(serde::Serialize)]
pub struct YearsResponse {
    pub years: Vec<i32>,
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        let escaped = s.replace('"', "\"\"");
        format!("\"{}\"", escaped)
    } else {
        s.to_string()
    }
}

fn txf_escape_line(s: &str) -> String {
    s.replace('^', " ").replace('\r', " ").replace('\n', " ")
}

pub async fn list_available_years(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> impl IntoResponse {
    match db::list_donations(&state.db, &user.id, None).await {
        Ok(list) => {
            let mut year_set: BTreeSet<i32> = BTreeSet::new();
            for d in list {
                year_set.insert(d.year);
            }
            let mut years: Vec<i32> = year_set.into_iter().collect();
            years.reverse();
            axum::Json(YearsResponse { years }).into_response()
        }
        Err(_) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "Database Error").into_response(),
    }
}

pub async fn export_csv(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Query(params): Query<ExportParams>,
) -> impl IntoResponse {
    match db::list_donations(&state.db, &user.id, params.year).await {
        Ok(list) => {
            let mut w = String::new();
            w.push_str("id,date,category,amount,charity_name,charity_id,notes\n");
            for d in list {
                let date = d.date.format("%Y-%m-%d").to_string();
                let category = d.category.clone().unwrap_or_default();
                let amount = format!("{:.2}", d.amount.unwrap_or(0.0));
                let notes = d.notes.clone().unwrap_or_default();
                w.push_str(&format!("{},{},{},{},{},{},{}\n",
                    csv_escape(&d.id),
                    csv_escape(&date),
                    csv_escape(&category),
                    csv_escape(&amount),
                    csv_escape(&d.charity_name),
                    csv_escape(&d.charity_id),
                    csv_escape(&notes),
                ));
            }

            let mut resp = Response::new(w.into());
            let headers = resp.headers_mut();
            headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("text/csv; charset=utf-8"));
            headers.insert(header::CONTENT_DISPOSITION, HeaderValue::from_static("attachment; filename=donations.csv"));
            resp
        }
        Err(_) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "Database Error").into_response(),
    }
}

pub async fn export_tax_txf(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Query(params): Query<ExportParams>,
) -> impl IntoResponse {
    match db::list_donations(&state.db, &user.id, params.year).await {
        Ok(list) => {
            let mut out = String::new();
            out.push_str("V042\n");
            out.push_str("ADeductible Tracker\n");
            out.push_str(&format!("D{}\n", chrono::Utc::now().format("%m/%d/%Y")));
            out.push_str("^\n");

            for d in list {
                let date = d.date.format("%Y-%m-%d").to_string();
                let ein = d.charity_ein.unwrap_or_default();
                let notes = d.notes.unwrap_or_default();
                let amount = d.amount.unwrap_or(0.0);
                let mut memo_parts = Vec::new();
                memo_parts.push(format!("Donation ID: {}", d.id));
                if !ein.trim().is_empty() {
                    memo_parts.push(format!("EIN: {}", ein.trim()));
                }
                if !notes.trim().is_empty() {
                    memo_parts.push(format!("Notes: {}", notes.trim()));
                }
                let memo = memo_parts.join(" | ");

                out.push_str("TD\n");
                out.push_str("N323\n");
                out.push_str("C1\n");
                out.push_str("LCharitable contributions\n");
                out.push_str(&format!("P{}\n", txf_escape_line(&d.charity_name)));
                out.push_str(&format!("D{}\n", txf_escape_line(&date)));
                out.push_str(&format!("${:.2}\n", amount));
                out.push_str(&format!("M{}\n", txf_escape_line(&memo)));
                out.push_str("^\n");
            }

            let mut resp = Response::new(out.into());
            let headers = resp.headers_mut();
            headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/octet-stream"));
            headers.insert(header::CONTENT_DISPOSITION, HeaderValue::from_static("attachment; filename=donations-tax-export.txf"));
            resp
        }
        Err(_) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "Database Error").into_response(),
    }
}

#[derive(serde::Deserialize)]
pub struct AuditExportParams {
    pub since: Option<String>,
}

pub async fn export_audit_csv(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Query(params): Query<AuditExportParams>,
) -> impl IntoResponse {
    let since_dt = params.since.as_ref().and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()).map(|dt| dt.with_timezone(&chrono::Utc));
    match db::list_audit_logs(&state.db, &user.id, since_dt).await {
        Ok(list) => {
            let mut w = String::new();
            w.push_str("id,user_id,action,table_name,record_id,details,created_at\n");
            for a in list {
                let record_id = a.record_id.unwrap_or_default();
                let details = a.details.unwrap_or_default();
                let created = a.created_at.to_rfc3339();
                w.push_str(&format!("{},{},{},{},{},{},{}\n",
                    csv_escape(&a.id),
                    csv_escape(&a.user_id),
                    csv_escape(&a.action),
                    csv_escape(&a.table_name),
                    csv_escape(&record_id),
                    csv_escape(&details),
                    csv_escape(&created),
                ));
            }

            let mut resp = Response::new(w.into());
            let headers = resp.headers_mut();
            headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("text/csv; charset=utf-8"));
            headers.insert(header::CONTENT_DISPOSITION, HeaderValue::from_static("attachment; filename=audit_logs.csv"));
            resp
        }
        Err(_) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "Database Error").into_response(),
    }
}
