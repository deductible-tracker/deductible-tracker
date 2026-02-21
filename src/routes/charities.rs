use axum::{
    extract::{Query, State, Path, Json},
    response::{IntoResponse, Json as AxumJson},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::OnceLock;
use std::env;
use crate::{auth::AuthenticatedUser, AppState};
use uuid::Uuid;

static SEARCH_HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
static PROPUBLICA_BASE_URL: OnceLock<Option<String>> = OnceLock::new();

#[derive(Debug, Deserialize)]
struct ProPublicaResponse {
    organizations: Vec<ProPublicaSearchOrg>,
}

#[derive(Debug, Deserialize)]
struct ProPublicaSearchOrg {
    ein: Option<i64>,
    strein: Option<String>,
    name: String,
    city: Option<String>,
    state: Option<String>,
    ntee_code: Option<String>,
    subseccd: Option<i64>,
    score: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct ProPublicaOrganizationResponse {
    organization: ProPublicaOrganization,
}

#[derive(Debug, Deserialize)]
struct ProPublicaOrganization {
    ein: Option<i64>,
    strein: Option<String>,
    name: Option<String>,
    address: Option<String>,
    city: Option<String>,
    state: Option<String>,
    zipcode: Option<String>,
    ntee_code: Option<String>,
    deductibility_code: Option<i64>,
    subsection_code: Option<i64>,
    foundation_code: Option<i64>,
    exempt_organization_status_code: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
struct EnrichedCharityData {
    name: Option<String>,
    ein: Option<String>,
    category: Option<String>,
    status: Option<String>,
    classification: Option<String>,
    nonprofit_type: Option<String>,
    deductibility: Option<String>,
    street: Option<String>,
    city: Option<String>,
    state: Option<String>,
    zip: Option<String>,
}

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
    Query(params): Query<HashMap<String, String>>
) -> impl IntoResponse {
    let query = params.get("q").map(|s| s.as_str()).unwrap_or("");
    let Some(base) = propublica_base_url() else {
        tracing::error!("PROPUBLICA_API_BASE_URL is not configured");
        return (StatusCode::INTERNAL_SERVER_ERROR, "Server misconfiguration: PROPUBLICA_API_BASE_URL is required").into_response();
    };
    
    let url = format!(
        "{}/search.json?q={}",
        base,
        url::form_urlencoded::byte_serialize(query.as_bytes()).collect::<String>()
    );

    let client = charity_search_client();
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
                    let ein = propublica_ein_from_search(&org).unwrap_or_default();
                    let loc = match (org.city, org.state) {
                        (Some(c), Some(s)) => format!("{}, {}", c, s),
                        (Some(c), None) => c,
                        (None, Some(s)) => s,
                        (None, None) => "Unknown".to_string(),
                    };
                    
                    CharityResult {
                        ein,
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

pub async fn lookup_charity_by_ein(
    Path(ein): Path<String>,
    _user: AuthenticatedUser,
) -> impl IntoResponse {
    let normalized_ein = normalize_ein(&ein);
    if normalized_ein.is_empty() {
        return (StatusCode::BAD_REQUEST, "Valid EIN required").into_response();
    }

    match fetch_charity_details_by_ein(&normalized_ein).await {
        Some(details) => (StatusCode::OK, AxumJson(json!({ "charity": details }))).into_response(),
        None => (StatusCode::NOT_FOUND, "No organization found for EIN").into_response(),
    }
}

fn charity_search_client() -> &'static reqwest::Client {
    SEARCH_HTTP_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(3))
            .timeout(std::time::Duration::from_secs(8))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    })
}

fn propublica_base_url() -> Option<&'static str> {
    PROPUBLICA_BASE_URL
        .get_or_init(|| {
            env::var("PROPUBLICA_API_BASE_URL")
                .ok()
                .map(|v| v.trim().trim_end_matches('/').to_string())
                .filter(|v| !v.is_empty())
        })
        .as_deref()
}

pub async fn list_charities(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> impl IntoResponse {
    match crate::db::list_charities(&state.db, &user.id).await {
        Ok(list) => {
            let out: Vec<CharityResponse> = list.into_iter().map(|c| CharityResponse {
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
            }).collect();
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
    let mut resolved_nonprofit_type: Option<String> = None;
    let mut resolved_deductibility: Option<String> = None;
    let mut resolved_street: Option<String> = None;
    let mut resolved_city: Option<String> = None;
    let mut resolved_state: Option<String> = None;
    let mut resolved_zip: Option<String> = None;

    if resolved_nonprofit_type.is_none() {
        resolved_nonprofit_type = clean_opt_string(req.nonprofit_type);
    }
    if resolved_deductibility.is_none() {
        resolved_deductibility = clean_opt_string(req.deductibility);
    }
    if resolved_street.is_none() {
        resolved_street = clean_opt_string(req.street);
    }
    if resolved_city.is_none() {
        resolved_city = clean_opt_string(req.city);
    }
    if resolved_state.is_none() {
        resolved_state = clean_opt_string(req.state);
    }
    if resolved_zip.is_none() {
        resolved_zip = clean_opt_string(req.zip);
    }

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
        resolved_ein = resolved_ein.and_then(|e| {
            let normalized = normalize_ein(&e);
            if normalized.is_empty() { None } else { Some(normalized) }
        });
    }

    if resolved_city.as_ref().map(|v| v.trim().is_empty()).unwrap_or(true) {
        return (StatusCode::BAD_REQUEST, "City required").into_response();
    }
    if resolved_state.as_ref().map(|v| v.trim().is_empty()).unwrap_or(true) {
        return (StatusCode::BAD_REQUEST, "State required").into_response();
    }

    match crate::db::find_charity_by_name_or_ein(&state.db, &user.id, &resolved_name, &resolved_ein).await {
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
            return (StatusCode::OK, AxumJson(json!({ "charity": payload, "created": false }))).into_response();
        }
        Ok(None) => {}
        Err(e) => {
            tracing::error!("Charity lookup error: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Database Error").into_response();
        }
    }

    let id = Uuid::new_v4().to_string();
    if let Err(e) = crate::db::create_charity(
        &state.db,
        &id,
        &user.id,
        &resolved_name,
        &resolved_ein,
        &resolved_category,
        &resolved_status,
        &resolved_classification,
        &resolved_nonprofit_type,
        &resolved_deductibility,
        &resolved_street,
        &resolved_city,
        &resolved_state,
        &resolved_zip,
        chrono::Utc::now(),
    ).await {
        tracing::error!("Charity create error: {}", e);
        return (StatusCode::INTERNAL_SERVER_ERROR, "Database Error").into_response();
    }

    let payload = CharityResponse {
        id: id.clone(),
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
    };
    (StatusCode::CREATED, AxumJson(json!({ "charity": payload, "created": true }))).into_response()
}

fn normalize_ein(value: &str) -> String {
    value.chars().filter(|c| c.is_ascii_digit()).collect::<String>()
}

fn normalize_i64_ein(value: i64) -> String {
    format!("{:09}", value)
}

fn clean_opt_string(value: Option<String>) -> Option<String> {
    value.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn map_deductibility(code: Option<i64>) -> Option<String> {
    match code {
        Some(1) => Some("Contributions are deductible".to_string()),
        Some(2) => Some("Contributions are not deductible".to_string()),
        Some(4) => Some("Contributions are deductible by treaty".to_string()),
        Some(other) => Some(format!("Code {}", other)),
        None => None,
    }
}

fn map_deductibility_from_exempt_status(exempt_status_code: Option<i64>) -> Option<String> {
    match exempt_status_code {
        Some(1) => Some("Deductible".to_string()),
        Some(2) | Some(3) | Some(4) => Some("May not be deductible".to_string()),
        Some(other) => Some(format!("Status {}", other)),
        None => None,
    }
}

fn map_nonprofit_type(subsection_code: Option<i64>) -> Option<String> {
    subsection_code.and_then(map_tax_section).map(|s| s.to_string())
}

fn map_tax_section(subsection_code: i64) -> Option<&'static str> {
    match subsection_code {
        2 => Some("501(c)(2)"),
        3 => Some("501(c)(3)"),
        4 => Some("501(c)(4)"),
        5 => Some("501(c)(5)"),
        6 => Some("501(c)(6)"),
        7 => Some("501(c)(7)"),
        8 => Some("501(c)(8)"),
        9 => Some("501(c)(9)"),
        10 => Some("501(c)(10)"),
        11 => Some("501(c)(11)"),
        12 => Some("501(c)(12)"),
        13 => Some("501(c)(13)"),
        14 => Some("501(c)(14)"),
        15 => Some("501(c)(15)"),
        16 => Some("501(c)(16)"),
        17 => Some("501(c)(17)"),
        18 => Some("501(c)(18)"),
        19 => Some("501(c)(19)"),
        21 => Some("501(c)(21)"),
        22 => Some("501(c)(22)"),
        23 => Some("501(c)(23)"),
        25 => Some("501(c)(25)"),
        26 => Some("501(c)(26)"),
        27 => Some("501(c)(27)"),
        28 => Some("501(c)(28)"),
        92 => Some("4947(a)(1)"),
        _ => None,
    }
}

fn map_category_from_ntee(ntee_code: Option<&str>) -> Option<String> {
    let letter = ntee_code
        .and_then(|code| code.chars().next())
        .map(|c| c.to_ascii_uppercase());

    let category = match letter {
        Some('A') => "Arts, Culture & Humanities",
        Some('B') => "Education",
        Some('C') | Some('D') => "Environment and Animals",
        Some('E') | Some('F') | Some('G') | Some('H') => "Health",
        Some('I') | Some('J') | Some('K') | Some('L') | Some('M') | Some('N') | Some('O') | Some('P') => "Human Services",
        Some('Q') => "International, Foreign Affairs",
        Some('R') | Some('S') | Some('T') | Some('U') | Some('V') | Some('W') => "Public, Societal Benefit",
        Some('X') => "Religion Related",
        Some('Y') => "Mutual/Membership Benefit",
        _ => "Unknown, Unclassified",
    };
    Some(category.to_string())
}

fn map_exempt_status_label(code: Option<i64>) -> Option<&'static str> {
    match code {
        Some(1) => Some("Active"),
        Some(2) => Some("Exempt"),
        Some(3) => Some("Revoked"),
        Some(4) => Some("Terminated"),
        _ => None,
    }
}

fn map_foundation_label(code: Option<i64>) -> Option<String> {
    match code {
        Some(0) => Some("Non-501(c)(3)".to_string()),
        Some(2) => Some("Private Operating (tax-exempt investment income)".to_string()),
        Some(3) => Some("Private Operating".to_string()),
        Some(4) => Some("Private Non-Operating".to_string()),
        Some(9) => Some("Suspense".to_string()),
        Some(10) => Some("Church".to_string()),
        Some(11) => Some("School".to_string()),
        Some(12) => Some("Hospital/Medical Research".to_string()),
        Some(13) => Some("Gov-Owned College/University Support".to_string()),
        Some(14) => Some("Governmental Unit".to_string()),
        Some(15) => Some("Public Support (Gov/Public)".to_string()),
        Some(16) => Some("509(a)(2)".to_string()),
        Some(17) => Some("509(a)(3) Supporting Org".to_string()),
        Some(18) => Some("509(a)(4) Public Safety Testing".to_string()),
        Some(other) => Some(format!("Foundation {}", other)),
        None => None,
    }
}

fn derive_status(exempt_status_code: Option<i64>) -> Option<String> {
    if let Some(status_label) = map_exempt_status_label(exempt_status_code) {
        Some(status_label.to_string())
    } else {
        exempt_status_code.map(|code| format!("Status {}", code))
    }
}

fn organization_by_name_match_score(candidate: &ProPublicaSearchOrg, target_name: &str) -> i32 {
    let target = target_name.trim().to_lowercase();
    let name = candidate.name.trim().to_lowercase();
    if name.is_empty() {
        return 0;
    }
    if name == target {
        return 3;
    }
    if name.contains(&target) || target.contains(&name) {
        return 2;
    }
    1
}

fn propublica_ein_from_search(org: &ProPublicaSearchOrg) -> Option<String> {
    if let Some(strein) = org.strein.as_ref() {
        let normalized = normalize_ein(strein);
        if !normalized.is_empty() {
            return Some(normalized);
        }
    }
    org.ein.map(normalize_i64_ein)
}

fn propublica_ein_from_org(org: &ProPublicaOrganization) -> Option<String> {
    if let Some(strein) = org.strein.as_ref() {
        let normalized = normalize_ein(strein);
        if !normalized.is_empty() {
            return Some(normalized);
        }
    }
    org.ein.map(normalize_i64_ein)
}

async fn fetch_charity_from_propublica(name: &str) -> Option<EnrichedCharityData> {
    let client = charity_search_client();
    let base = propublica_base_url()?;
    let term = name.trim();
    if term.len() < 2 {
        return None;
    }
    let encoded_term = url::form_urlencoded::byte_serialize(term.as_bytes()).collect::<String>();
    let search_url = format!("{}/search.json?q={}", base, encoded_term);
    if let Ok(resp) = client.get(&search_url).header("User-Agent", "DeductibleTracker/1.0").send().await {
        if !resp.status().is_success() {
            return None;
        }
        if let Ok(payload) = resp.json::<ProPublicaResponse>().await {
            let mut results = payload.organizations;
            results.sort_by(|a, b| {
                let b_score = organization_by_name_match_score(b, term);
                let a_score = organization_by_name_match_score(a, term);
                b_score.cmp(&a_score).then_with(|| b.score.unwrap_or(0.0).partial_cmp(&a.score.unwrap_or(0.0)).unwrap_or(std::cmp::Ordering::Equal))
            });

            if let Some(best) = results.into_iter().next() {
                let fallback = EnrichedCharityData {
                    name: Some(best.name.clone()),
                    ein: propublica_ein_from_search(&best),
                    category: map_category_from_ntee(best.ntee_code.as_deref()),
                    status: None,
                    classification: None,
                    nonprofit_type: map_nonprofit_type(best.subseccd),
                    deductibility: None,
                    street: None,
                    city: best.city,
                    state: best.state,
                    zip: None,
                };

                if let Some(ein_digits) = fallback.ein.clone() {
                    let org_url = format!("{}/organizations/{}.json", base, ein_digits);
                    if let Ok(org_resp) = client.get(&org_url).header("User-Agent", "DeductibleTracker/1.0").send().await {
                        if org_resp.status().is_success() {
                            if let Ok(org_payload) = org_resp.json::<ProPublicaOrganizationResponse>().await {
                                let org = org_payload.organization;
                                let org_ein = propublica_ein_from_org(&org);
                                let org_nonprofit_type = map_nonprofit_type(org.subsection_code);
                                return Some(EnrichedCharityData {
                                    name: clean_opt_string(org.name).or_else(|| fallback.name.clone()),
                                    ein: org_ein.or_else(|| fallback.ein.clone()),
                                    category: map_category_from_ntee(org.ntee_code.as_deref()).or_else(|| fallback.category.clone()),
                                    status: derive_status(org.exempt_organization_status_code).or_else(|| fallback.status.clone()),
                                    classification: map_foundation_label(org.foundation_code).or_else(|| fallback.classification.clone()),
                                    nonprofit_type: org_nonprofit_type.or_else(|| fallback.nonprofit_type.clone()),
                                    deductibility: map_deductibility_from_exempt_status(org.exempt_organization_status_code)
                                        .or_else(|| map_deductibility(org.deductibility_code)),
                                    street: clean_opt_string(org.address),
                                    city: clean_opt_string(org.city).or_else(|| fallback.city.clone()),
                                    state: clean_opt_string(org.state).or_else(|| fallback.state.clone()),
                                    zip: clean_opt_string(org.zipcode),
                                });
                            }
                        }
                    }
                }

                return Some(fallback);
            }
        }
    }
    None
}

async fn fetch_charity_details_by_ein(ein: &str) -> Option<EnrichedCharityData> {
    let client = charity_search_client();
    let base = propublica_base_url()?;
    let normalized_ein = normalize_ein(ein);
    if normalized_ein.is_empty() {
        return None;
    }

    let org_url = format!("{}/organizations/{}.json", base, normalized_ein);
    let resp = client
        .get(&org_url)
        .header("User-Agent", "DeductibleTracker/1.0")
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        return None;
    }

    let payload = resp.json::<ProPublicaOrganizationResponse>().await.ok()?;
    let org = payload.organization;
    let org_ein = propublica_ein_from_org(&org);
    let category = map_category_from_ntee(org.ntee_code.as_deref());
    let status = derive_status(org.exempt_organization_status_code);
    let classification = map_foundation_label(org.foundation_code);
    let nonprofit_type = map_nonprofit_type(org.subsection_code);
    let deductibility = map_deductibility_from_exempt_status(org.exempt_organization_status_code)
        .or_else(|| map_deductibility(org.deductibility_code));

    Some(EnrichedCharityData {
        name: clean_opt_string(org.name),
        ein: org_ein,
        category,
        status,
        classification,
        nonprofit_type,
        deductibility,
        street: clean_opt_string(org.address),
        city: clean_opt_string(org.city),
        state: clean_opt_string(org.state),
        zip: clean_opt_string(org.zipcode),
    })
}

pub async fn delete_charity(
    Path(charity_id): Path<String>,
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> impl IntoResponse {
    match crate::db::count_donations_for_charity(&state.db, &user.id, &charity_id).await {
        Ok(count) if count > 0 => {
            return (StatusCode::CONFLICT, "Charity has donations").into_response();
        }
        Ok(_) => {}
        Err(e) => {
            tracing::error!("Charity delete check error: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Database Error").into_response();
        }
    }

    match crate::db::delete_charity(&state.db, &user.id, &charity_id).await {
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

    let normalized_ein = req.ein
        .as_ref()
        .map(|v| normalize_ein(v))
        .filter(|v| !v.is_empty());

    let category = clean_opt_string(req.category);
    let status = clean_opt_string(req.status);
    let classification = clean_opt_string(req.classification);
    let nonprofit_type = clean_opt_string(req.nonprofit_type);
    let deductibility = clean_opt_string(req.deductibility);
    let street = clean_opt_string(req.street);
    let city = clean_opt_string(req.city);
    let state_code = clean_opt_string(req.state);
    let zip = clean_opt_string(req.zip);

    match crate::db::update_charity(
        &state.db,
        &charity_id,
        &user.id,
        name,
        &normalized_ein,
        &category,
        &status,
        &classification,
        &nonprofit_type,
        &deductibility,
        &street,
        &city,
        &state_code,
        &zip,
        chrono::Utc::now(),
    ).await {
        Ok(true) => {
            let payload = CharityResponse {
                id: charity_id,
                name: name.to_string(),
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
            (StatusCode::OK, AxumJson(json!({ "charity": payload, "updated": true }))).into_response()
        }
        Ok(false) => (StatusCode::NOT_FOUND, "Not found").into_response(),
        Err(e) => {
            tracing::error!("Charity update error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Database Error").into_response()
        }
    }
}