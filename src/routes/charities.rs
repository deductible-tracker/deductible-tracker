use axum::{
    extract::Query,
    response::{IntoResponse, Json as AxumJson},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use crate::auth::AuthenticatedUser;

#[derive(Debug, Deserialize)]
struct ProPublicaResponse {
    organizations: Vec<ProPublicaOrg>,
}

#[derive(Debug, Deserialize)]
struct ProPublicaOrg {
    ein: String,
    name: String,
    city: Option<String>,
    state: Option<String>,
    _sub_name: Option<String>,
}

#[derive(Debug, Serialize)]
struct CharityResult {
    ein: String,
    name: String,
    location: String,
}

pub async fn search_charities(
    _user: AuthenticatedUser,
    Query(params): Query<HashMap<String, String>>
) -> impl IntoResponse {
    let query = params.get("q").map(|s| s.as_str()).unwrap_or("");
    
    let url = format!(
        "https://projects.propublica.org/nonprofits/api/v2/search.json?q={}",
        url::form_urlencoded::byte_serialize(query.as_bytes()).collect::<String>()
    );

    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .header("User-Agent", "DeductibleTracker/1.0")
        .send()
        .await;

    match resp {
        Ok(r) => {
            if r.status().is_success() {
                let data: ProPublicaResponse = r.json().await.unwrap_or(ProPublicaResponse { organizations: vec![] });
                
                let results: Vec<CharityResult> = data.organizations.into_iter().map(|org| {
                    let loc = match (org.city, org.state) {
                        (Some(c), Some(s)) => format!("{}, {}", c, s),
                        (Some(c), None) => c,
                        (None, Some(s)) => s,
                        (None, None) => "Unknown".to_string(),
                    };
                    
                    CharityResult {
                        ein: org.ein,
                        name: org.name,
                        location: loc,
                    }
                }).collect();

                (StatusCode::OK, AxumJson(json!({ "results": results }))).into_response()
            } else {
                 (StatusCode::BAD_GATEWAY, "Upstream API error").into_response()
            }
        }
        Err(e) => {
             tracing::error!("Charity Search Error: {}", e);
             (StatusCode::INTERNAL_SERVER_ERROR, "Search Error").into_response()
        }
    }
}