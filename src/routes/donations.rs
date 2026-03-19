use axum::{
    extract::{State, Query, Json, Path},
    response::{IntoResponse, Json as AxumJson},
    http::StatusCode,
};
use serde::Deserialize;
use crate::AppState;
// Donation model is provided via DB helper responses; no direct import needed here.
use crate::auth::AuthenticatedUser;
use uuid::Uuid;
use chrono::{NaiveDate, Datelike, DateTime, Utc};
use crate::db::models::{DonationPatch, NewCharity, NewDonation};

fn normalize_category(input: &Option<String>) -> Option<String> {
    input.as_ref().and_then(|value| {
        let normalized = value.trim().to_lowercase();
        match normalized.as_str() {
            "items" | "money" | "mileage" => Some(normalized),
            "" => None,
            _ => Some("money".to_string()),
        }
    })
}

#[derive(Deserialize)]
pub struct CreateDonationRequest {
    pub date: String, // YYYY-MM-DD
    pub charity_name: String,
    pub charity_id: Option<String>,
    pub charity_ein: Option<String>,
    pub category: Option<String>,
    pub amount: Option<f64>,
    pub notes: Option<String>,
    pub id: Option<String>,
}

#[derive(serde::Deserialize)]
pub struct ImportCsvRequest {
    pub csv: String,
}

pub async fn import_donations(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(req): Json<ImportCsvRequest>
) -> impl IntoResponse {
    // Parse CSV from provided string payload (expects header row)
    let mut reader = csv::ReaderBuilder::new().has_headers(true).from_reader(req.csv.as_bytes());
    let mut imported = 0usize;
    for record in reader.records() {
        match record {
            Ok(rec) => {
                // Columns expected: id?, date (YYYY-MM-DD), charity_name, charity_id?, charity_ein?, notes?, amount?[, category?]
                let id = rec.get(0).map(|s| s.to_string()).filter(|s| !s.is_empty()).unwrap_or_else(|| Uuid::new_v4().to_string());
                let date_str = rec.get(1).unwrap_or("");
                let charity_name = rec.get(2).unwrap_or("").to_string();
                let charity_id = rec.get(3).map(|s| s.to_string()).filter(|s| !s.is_empty());
                let charity_ein = rec.get(4).map(|s| s.to_string()).filter(|s| !s.is_empty());
                let notes = rec.get(5).map(|s| s.to_string()).filter(|s| !s.is_empty());
                let amount = rec.get(6).and_then(|s| {
                    let trimmed = s.trim();
                    if trimmed.is_empty() { None } else { trimmed.parse::<f64>().ok() }
                });
                let category_raw = rec.get(7).map(|s| s.to_string()).filter(|s| !s.trim().is_empty());
                let category = normalize_category(&category_raw);

                let date = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d").unwrap_or_else(|_| chrono::Utc::now().date_naive());
                let year = date.year();
                let now = chrono::Utc::now();

                let resolved_charity_id = if let Some(cid) = charity_id {
                    cid
                } else {
                    match crate::db::charities::find_charity_by_name_or_ein(&state.db, &user.id, &charity_name, &charity_ein).await {
                        Ok(Some(existing)) => existing.id,
                        Ok(None) => {
                            let new_id = Uuid::new_v4().to_string();
                            let new_charity = NewCharity {
                                id: new_id.clone(),
                                user_id: user.id.clone(),
                                name: charity_name.clone(),
                                ein: charity_ein.clone(),
                                category: None,
                                status: None,
                                classification: None,
                                nonprofit_type: None,
                                deductibility: None,
                                street: None,
                                city: None,
                                state: None,
                                zip: None,
                                created_at: now,
                            };
                            if let Err(e) = crate::db::charities::create_charity(&state.db, &new_charity).await {
                                tracing::error!("Import charity create failed: {}", e);
                                continue;
                            }
                            new_id
                        }
                        Err(e) => {
                            tracing::error!("Import charity lookup failed: {}", e);
                            continue;
                        }
                    }
                };

                let new_donation = NewDonation {
                    id: id.clone(),
                    user_id: user.id.clone(),
                    year,
                    date,
                    category: category.clone(),
                    charity_id: resolved_charity_id.clone(),
                    amount,
                    notes: notes.clone(),
                    created_at: now,
                };
                if let Err(e) = crate::db::donations::add_donation(&state.db, &new_donation).await {
                    tracing::error!("Import add_donation failed: {}", e);
                } else {
                    imported += 1;
                    let audit_id = Uuid::new_v4().to_string();
                    let details = Some(format!("Imported donation id={}", id));
                    let _ = crate::db::audit::log_audit(&state.db, &audit_id, &user.id, "import", "donations", &Some(id.clone()), &details).await;
                }
            }
            Err(e) => tracing::error!("CSV parse error: {}", e),
        }
    }

    (StatusCode::OK, AxumJson(serde_json::json!({ "imported": imported }))).into_response()
}

#[derive(Deserialize)]
pub struct UpdateDonationRequest {
    pub date: Option<String>, // YYYY-MM-DD
    pub charity_id: Option<String>,
    pub category: Option<String>,
    pub amount: Option<f64>,
    pub notes: Option<String>,
    pub updated_at: Option<String>, // RFC3339
}

#[derive(Deserialize)]
pub struct ListParams {
    year: Option<i32>,
    pub since: Option<String>,
}

fn validate_create_donation_request(req: &CreateDonationRequest) -> Result<(), &'static str> {
    let has_charity_id = req
        .charity_id
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty());

    if req.charity_name.trim().is_empty() && !has_charity_id {
        return Err("Charity required");
    }

    if let Some(amount) = req.amount {
        if amount < 0.0 {
            return Err("Donation amount cannot be negative");
        }
    }

    let category = normalize_category(&req.category).unwrap_or_else(|| "money".to_string());
    if category == "money" && req.amount.unwrap_or(0.0) <= 0.0 {
        return Err("Money donations require a positive amount");
    }

    Ok(())
}

fn validate_update_donation_request(req: &UpdateDonationRequest) -> Result<(), &'static str> {
    if let Some(charity_id) = req.charity_id.as_deref() {
        if charity_id.trim().is_empty() {
            return Err("Charity id required");
        }
    }

    if let Some(amount) = req.amount {
        if amount < 0.0 {
            return Err("Donation amount cannot be negative");
        }
    }

    if matches!(normalize_category(&req.category).as_deref(), Some("money")) && req.amount.unwrap_or(0.0) <= 0.0 {
        return Err("Money donations require a positive amount");
    }

    Ok(())
}

pub async fn create_donation(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(req): Json<CreateDonationRequest>
) -> impl IntoResponse {
    if let Err(message) = validate_create_donation_request(&req) {
        return (StatusCode::BAD_REQUEST, message).into_response();
    }

    let user_id = user.id;
    let charity_name = req.charity_name.trim().to_string();
    
    let date = NaiveDate::parse_from_str(&req.date, "%Y-%m-%d")
        .unwrap_or_else(|_| chrono::Utc::now().date_naive());
    
    let id = if let Some(provided) = req.id.clone() { provided } else { Uuid::new_v4().to_string() };
    let now = chrono::Utc::now();
    let year = date.year();
    let category = normalize_category(&req.category);
    let charity_id = if let Some(cid) = req.charity_id.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
        cid.to_string()
    } else {
        match crate::db::charities::find_charity_by_name_or_ein(&state.db, &user_id, &charity_name, &req.charity_ein).await {
            Ok(Some(existing)) => existing.id,
            Ok(None) => {
                let new_id = Uuid::new_v4().to_string();
                let new_charity = NewCharity {
                    id: new_id.clone(),
                    user_id: user_id.clone(),
                    name: charity_name.clone(),
                    ein: req.charity_ein.clone(),
                    category: None,
                    status: None,
                    classification: None,
                    nonprofit_type: None,
                    deductibility: None,
                    street: None,
                    city: None,
                    state: None,
                    zip: None,
                    created_at: now,
                };
                if let Err(e) = crate::db::charities::create_charity(&state.db, &new_charity).await {
                    tracing::error!("Charity create failed: {}", e);
                    return (StatusCode::INTERNAL_SERVER_ERROR, "Database Error").into_response();
                }
                new_id
            }
            Err(e) => {
                tracing::error!("Charity lookup failed: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, "Database Error").into_response();
            }
        }
    };

    let new_donation = NewDonation {
        id: id.clone(),
        user_id: user_id.clone(),
        year,
        date,
        category,
        charity_id: charity_id.clone(),
        amount: req.amount,
        notes: req.notes.clone(),
        created_at: now,
    };

    if let Err(e) = crate::db::donations::add_donation(&state.db, &new_donation).await {
        tracing::error!("DB Error: {}", e);
        return (StatusCode::INTERNAL_SERVER_ERROR, "Database Error").into_response();
    }

    (StatusCode::CREATED, AxumJson(serde_json::json!({ "status": "created", "id": id, "charity_id": charity_id }))).into_response()
}

pub async fn delete_donation(
    Path(id): Path<String>,
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> impl IntoResponse {
    match crate::db::donations::soft_delete_donation(&state.db, &user.id, &id).await {
        Ok(true) => (StatusCode::OK, "Deleted").into_response(),
        Ok(false) => (StatusCode::NOT_FOUND, "Not found").into_response(),
        Err(e) => {
            tracing::error!("Delete donation error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Database Error").into_response()
        }
    }
}

pub async fn update_donation(
    Path(id): Path<String>,
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(req): Json<UpdateDonationRequest>
) -> impl IntoResponse {
    if let Err(message) = validate_update_donation_request(&req) {
        return (StatusCode::BAD_REQUEST, message).into_response();
    }

    let user_id = user.id;

    let (date_opt, year_opt) = if let Some(date_str) = req.date.clone() {
        let d = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d").unwrap_or_else(|_| chrono::Utc::now().date_naive());
        (Some(d), Some(d.year()))
    } else { (None, None) };

    let incoming_updated_at = req.updated_at.as_ref().and_then(|s| DateTime::parse_from_rfc3339(s).ok()).map(|dt| dt.with_timezone(&Utc));
    let normalized_category = normalize_category(&req.category);

    let patch = DonationPatch {
        user_id: user_id.clone(),
        donation_id: id.clone(),
        date_opt,
        year_opt,
        category_opt: normalized_category,
        charity_id_opt: req.charity_id.as_deref().map(str::trim).filter(|value| !value.is_empty()).map(ToString::to_string),
        amount_opt: req.amount,
        notes: req.notes.clone(),
        incoming_updated_at,
    };

    match crate::db::donations::update_donation(&state.db, &patch).await {
        Ok(true) => (StatusCode::OK, AxumJson(serde_json::json!({"status":"updated","id": id}))).into_response(),
        Ok(false) => (StatusCode::CONFLICT, "Not updated (stale or not found)").into_response(),
        Err(e) => {
            tracing::error!("Update donation error: {}", e);
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
    // support incremental pulls via ?since=<rfc3339>
    if let Some(since_str) = params.since.clone() {
        if let Ok(since_dt) = DateTime::parse_from_rfc3339(&since_str) {
            match crate::db::donations::list_donations_since(&state.db, &user_id, since_dt.with_timezone(&Utc)).await {
                Ok(donations) => return AxumJson(serde_json::json!({ "donations": donations })).into_response(),
                Err(e) => {
                    tracing::error!("DB Query Error: {}", e);
                    return (StatusCode::INTERNAL_SERVER_ERROR, "Database Error").into_response();
                }
            }
        }
    }

    match crate::db::donations::list_donations(&state.db, &user_id, params.year).await {
        Ok(donations) => AxumJson(serde_json::json!({ "donations": donations })).into_response(),
        Err(e) => {
            tracing::error!("DB Query Error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Database Error").into_response()
        }
    }
}