use axum::{
    extract::{Json, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json as AxumJson},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;
use crate::db::models::{CharityPatch, NewCharity};

use crate::{auth::AuthenticatedUser, AppState};

use super::api_fetch::{
    fetch_charity_details_by_ein, fetch_charity_from_propublica, search_charities_by_query,
    SearchError,
};
use super::charity_enrichment::{clean_opt_string, normalize_ein};

#[derive(Debug, Serialize)]
struct CharityResult {
    ein: String,
    name: String,
    location: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateCharityRequest {
    pub name: String,
    pub ein: Option<String>,
    pub category: Option<String>,
    pub status: Option<String>,
    pub classification: Option<String>,
    pub nonprofit_type: Option<String>,
    pub deductibility: Option<String>,
    pub street: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub zip: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCharityRequest {
    pub name: String,
    pub ein: Option<String>,
    pub category: Option<String>,
    pub status: Option<String>,
    pub classification: Option<String>,
    pub nonprofit_type: Option<String>,
    pub deductibility: Option<String>,
    pub street: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub zip: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CharityResponse {
    pub id: String,
    pub name: String,
    pub ein: Option<String>,
    pub category: Option<String>,
    pub status: Option<String>,
    pub classification: Option<String>,
    pub nonprofit_type: Option<String>,
    pub deductibility: Option<String>,
    pub street: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub zip: Option<String>,
}

pub async fn search_charities(
    _user: AuthenticatedUser,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let query = params.get("q").map(|s| s.as_str()).unwrap_or("");

    match search_charities_by_query(query).await {
        Ok(hits) => {
            let results: Vec<CharityResult> = hits
                .into_iter()
                .map(|hit| {
                    let location = match (hit.city, hit.state) {
                        (Some(city), Some(state)) => format!("{}, {}", city, state),
                        (Some(city), None) => city,
                        (None, Some(state)) => state,
                        (None, None) => "Unknown".to_string(),
                    };

                    CharityResult {
                        ein: hit.ein,
                        name: hit.name,
                        location,
                    }
                })
                .collect();

            (StatusCode::OK, AxumJson(json!({ "results": results }))).into_response()
        }
        Err(SearchError::MissingConfig) => {
            tracing::error!("PROPUBLICA_API_BASE_URL is not configured");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Server misconfiguration: PROPUBLICA_API_BASE_URL is required",
            )
                .into_response()
        }
        Err(SearchError::Upstream) => (StatusCode::BAD_GATEWAY, "Upstream API error").into_response(),
        Err(SearchError::Transport) => {
            tracing::error!("Charity Search Error: request failed");
            (StatusCode::INTERNAL_SERVER_ERROR, "Search Error").into_response()
        }
    }
}

pub async fn lookup_charity_by_ein(Path(ein): Path<String>, _user: AuthenticatedUser) -> impl IntoResponse {
    let normalized_ein = normalize_ein(&ein);
    if normalized_ein.is_empty() {
        return (StatusCode::BAD_REQUEST, "Valid EIN required").into_response();
    }

    match fetch_charity_details_by_ein(&normalized_ein).await {
        Some(details) => (StatusCode::OK, AxumJson(json!({ "charity": details }))).into_response(),
        None => (StatusCode::NOT_FOUND, "No organization found for EIN").into_response(),
    }
}

pub async fn list_charities(State(state): State<AppState>, user: AuthenticatedUser) -> impl IntoResponse {
    match crate::db::charities::list_charities(&state.db, &user.id).await {
        Ok(list) => {
            let out: Vec<CharityResponse> = list
                .into_iter()
                .map(|c| CharityResponse {
                    id: c.id,
                    name: c.name,
                    ein: c.ein,
                    category: c.category,
                    status: c.status,
                    classification: c.classification,
                    nonprofit_type: c.nonprofit_type,
                    deductibility: c.deductibility,
                    street: c.street,
                    city: c.city,
                    state: c.state,
                    zip: c.zip,
                })
                .collect();
            (StatusCode::OK, AxumJson(json!({ "charities": out }))).into_response()
        }
        Err(e) => {
            tracing::error!("List charities error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Database Error").into_response()
        }
    }
}

pub async fn create_charity(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(req): Json<CreateCharityRequest>,
) -> impl IntoResponse {
    let name = req.name.trim();
    if name.is_empty() {
        return (StatusCode::BAD_REQUEST, "Name required").into_response();
    }

    let requested_ein = req
        .ein
        .as_ref()
        .map(|e| normalize_ein(e))
        .filter(|e| !e.is_empty());

    let fetched = if let Some(ref ein) = requested_ein {
        if let Some(by_ein) = fetch_charity_details_by_ein(ein).await {
            Some(by_ein)
        } else {
            fetch_charity_from_propublica(name).await
        }
    } else {
        fetch_charity_from_propublica(name).await
    };

    let mut resolved_name = name.to_string();
    let mut resolved_ein = requested_ein;
    let mut resolved_category: Option<String> = clean_opt_string(req.category);
    let mut resolved_status: Option<String> = clean_opt_string(req.status);
    let mut resolved_classification: Option<String> = clean_opt_string(req.classification);
    let mut resolved_nonprofit_type: Option<String> = clean_opt_string(req.nonprofit_type);
    let mut resolved_deductibility: Option<String> = clean_opt_string(req.deductibility);
    let mut resolved_street: Option<String> = clean_opt_string(req.street);
    let mut resolved_city: Option<String> = clean_opt_string(req.city);
    let mut resolved_state: Option<String> = clean_opt_string(req.state);
    let mut resolved_zip: Option<String> = clean_opt_string(req.zip);

    if let Some(org) = fetched {
        if let Some(n) = org.name.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
            resolved_name = n.to_string();
        }
        if let Some(ein) = org.ein.as_ref().map(|s| normalize_ein(s)).filter(|s| !s.is_empty()) {
            resolved_ein = Some(ein);
        }
        if resolved_category.is_none() {
            resolved_category = org.category;
        }
        if resolved_status.is_none() {
            resolved_status = org.status;
        }
        if resolved_classification.is_none() {
            resolved_classification = org.classification;
        }
        if resolved_nonprofit_type.is_none() {
            resolved_nonprofit_type = org.nonprofit_type;
        }
        if resolved_deductibility.is_none() {
            resolved_deductibility = org.deductibility;
        }
        if resolved_street.is_none() {
            resolved_street = clean_opt_string(org.street);
        }
        if resolved_city.is_none() {
            resolved_city = clean_opt_string(org.city);
        }
        if resolved_state.is_none() {
            resolved_state = clean_opt_string(org.state);
        }
        if resolved_zip.is_none() {
            resolved_zip = clean_opt_string(org.zip);
        }
    } else {
        resolved_ein = resolved_ein.and_then(|ein| {
            let normalized = normalize_ein(&ein);
            if normalized.is_empty() {
                None
            } else {
                Some(normalized)
            }
        });
    }

    match crate::db::charities::find_charity_by_name_or_ein(
        &state.db,
        &user.id,
        &resolved_name,
        &resolved_ein,
    )
    .await
    {
        Ok(Some(existing)) => {
            let payload = CharityResponse {
                id: existing.id,
                name: existing.name,
                ein: existing.ein,
                category: existing.category,
                status: existing.status,
                classification: existing.classification,
                nonprofit_type: existing.nonprofit_type,
                deductibility: existing.deductibility,
                street: existing.street,
                city: existing.city,
                state: existing.state,
                zip: existing.zip,
            };
            return (
                StatusCode::OK,
                AxumJson(json!({ "charity": payload, "created": false })),
            )
                .into_response();
        }
        Ok(None) => {}
        Err(e) => {
            tracing::error!("Charity lookup error: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Database Error").into_response();
        }
    }

    let id = Uuid::new_v4().to_string();
    let new_charity = NewCharity {
        id,
        user_id: user.id.clone(),
        name: resolved_name,
        ein: resolved_ein,
        category: resolved_category,
        status: resolved_status,
        classification: resolved_classification,
        nonprofit_type: resolved_nonprofit_type,
        deductibility: resolved_deductibility,
        street: resolved_street,
        city: resolved_city,
        state: resolved_state,
        zip: resolved_zip,
        created_at: chrono::Utc::now(),
    };

    if let Err(e) = crate::db::charities::create_charity(&state.db, &new_charity).await
    {
        tracing::error!("Charity create error: {}", e);
        return (StatusCode::INTERNAL_SERVER_ERROR, "Database Error").into_response();
    }

    let payload = CharityResponse {
        id: new_charity.id.clone(),
        name: new_charity.name.clone(),
        ein: new_charity.ein.clone(),
        category: new_charity.category.clone(),
        status: new_charity.status.clone(),
        classification: new_charity.classification.clone(),
        nonprofit_type: new_charity.nonprofit_type.clone(),
        deductibility: new_charity.deductibility.clone(),
        street: new_charity.street.clone(),
        city: new_charity.city.clone(),
        state: new_charity.state.clone(),
        zip: new_charity.zip.clone(),
    };
    (
        StatusCode::CREATED,
        AxumJson(json!({ "charity": payload, "created": true })),
    )
        .into_response()
}

pub async fn delete_charity(
    Path(charity_id): Path<String>,
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> impl IntoResponse {
    match crate::db::charities::count_donations_for_charity(&state.db, &user.id, &charity_id).await {
        Ok(count) if count > 0 => {
            return (StatusCode::CONFLICT, "Charity has donations").into_response();
        }
        Ok(_) => {}
        Err(e) => {
            tracing::error!("Charity delete check error: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Database Error").into_response();
        }
    }

    match crate::db::charities::delete_charity(&state.db, &user.id, &charity_id).await {
        Ok(true) => (StatusCode::OK, "Deleted").into_response(),
        Ok(false) => (StatusCode::NOT_FOUND, "Not found").into_response(),
        Err(e) => {
            tracing::error!("Charity delete error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Database Error").into_response()
        }
    }
}

pub async fn update_charity(
    Path(charity_id): Path<String>,
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(req): Json<UpdateCharityRequest>,
) -> impl IntoResponse {
    let name = req.name.trim();
    if name.is_empty() {
        return (StatusCode::BAD_REQUEST, "Name required").into_response();
    }

    let name_owned = name.to_string();

    let normalized_ein = req
        .ein
        .as_ref()
        .map(|value| normalize_ein(value))
        .filter(|value| !value.is_empty());

    let category = clean_opt_string(req.category);
    let status = clean_opt_string(req.status);
    let classification = clean_opt_string(req.classification);
    let nonprofit_type = clean_opt_string(req.nonprofit_type);
    let deductibility = clean_opt_string(req.deductibility);
    let street = clean_opt_string(req.street);
    let city = clean_opt_string(req.city);
    let state_code = clean_opt_string(req.state);
    let zip = clean_opt_string(req.zip);

    let patch = CharityPatch {
        charity_id: charity_id.clone(),
        user_id: user.id.clone(),
        name: name_owned.clone(),
        ein: normalized_ein.clone(),
        category: category.clone(),
        status: status.clone(),
        classification: classification.clone(),
        nonprofit_type: nonprofit_type.clone(),
        deductibility: deductibility.clone(),
        street: street.clone(),
        city: city.clone(),
        state: state_code.clone(),
        zip: zip.clone(),
        updated_at: chrono::Utc::now(),
    };

    match crate::db::charities::update_charity(&state.db, &patch).await
    {
        Ok(true) => {
            let payload = CharityResponse {
                id: charity_id,
                name: name_owned,
                ein: normalized_ein,
                category,
                status,
                classification,
                nonprofit_type,
                deductibility,
                street,
                city,
                state: state_code,
                zip,
            };
            (
                StatusCode::OK,
                AxumJson(json!({ "charity": payload, "updated": true })),
            )
                .into_response()
        }
        Ok(false) => (StatusCode::NOT_FOUND, "Not found").into_response(),
        Err(e) => {
            tracing::error!("Charity update error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Database Error").into_response()
        }
    }
}
