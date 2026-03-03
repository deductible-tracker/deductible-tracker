use axum::{
    extract::{Path, State, Query, Json, FromRequestParts},
    response::{Redirect, IntoResponse},
    http::{StatusCode, request::Parts, HeaderValue, header},
};
use serde::{Deserialize, Serialize};
use crate::AppState;
use crate::db::models::UserProfileUpsert;
use oauth2::{
    basic::BasicClient, AuthUrl, ClientId, ClientSecret, RedirectUrl, TokenUrl,
    AuthorizationCode,
};
use oauth2::TokenResponse;
use std::env;
use jsonwebtoken::{encode, decode, EncodingKey, DecodingKey, Header, Validation};
use chrono::{Utc, Duration};
use serde_json::Value;
use std::sync::OnceLock;
use std::sync::Mutex;
use std::collections::HashMap;

const AUTH_COOKIE_NAME: &str = "auth_token";
static JWT_SECRET: OnceLock<String> = OnceLock::new();
static JWT_ISSUER: OnceLock<Option<String>> = OnceLock::new();
static JWT_AUDIENCE: OnceLock<Option<String>> = OnceLock::new();
static HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
static REVOKED_JTIS: OnceLock<Mutex<HashMap<String, usize>>> = OnceLock::new();

#[derive(Deserialize)]
pub struct AuthCallback {
    code: String,
    state: String,
}

#[derive(Deserialize)]
pub struct DevLoginRequest {
    username: String,
    password: String,
}

#[derive(Serialize)]
pub struct AuthResponse {
    user: UserProfile,
}

#[derive(Serialize, Clone)]
pub struct UserProfile {
    pub id: String,
    pub email: String,
    pub name: String,
    pub provider: String,
    pub filing_status: Option<String>,
    pub agi: Option<f64>,
    pub marginal_tax_rate: Option<f64>,
    pub itemize_deductions: Option<bool>,
}

#[derive(Deserialize)]
pub struct UpdateMeRequest {
    pub email: String,
    pub name: String,
    pub filing_status: Option<String>,
    pub agi: Option<f64>,
    pub marginal_tax_rate: Option<f64>,
    pub itemize_deductions: Option<bool>,
}

// Claims for our JWT
#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    exp: usize,
    jti: String,
    email: String,
    provider: String,
    name: String,
    iss: Option<String>,
    aud: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct StateClaims {
    exp: usize,
    provider: String,
    nonce: String,
}

pub struct AuthenticatedUser {
    pub id: String,
    pub email: String,
    pub name: String,
    pub provider: String,
}

impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync + 'static,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let token = extract_token(parts)
            .ok_or((StatusCode::UNAUTHORIZED, "Missing auth token".to_string()))?;

        validate_token_str(&token)
    }
}



use axum::http::HeaderMap;

fn extract_cookie_by_name(headers: &HeaderMap, cookie_name: &str) -> Option<String> {
    let cookie_header = headers.get(header::COOKIE).and_then(|h| h.to_str().ok())?;
    for cookie in cookie_header.split(';') {
        let cookie = cookie.trim();
        if let Some((k, v)) = cookie.split_once('=') {
            if k == cookie_name {
                return Some(v.to_string());
            }
        }
    }
    None
}

// Extract token directly from headers (used by middleware)
pub fn extract_token_from_headers(headers: &HeaderMap) -> Option<String> {
    if let Some(auth_header) = headers.get(axum::http::header::AUTHORIZATION).and_then(|h| h.to_str().ok()) {
        if let Some(token) = auth_header.strip_prefix("Bearer ") {
            return Some(token.to_string());
        }
    }

    extract_cookie_by_name(headers, AUTH_COOKIE_NAME)
}

// Validate a token string and return AuthenticatedUser (used by extractor & middleware)
pub fn validate_token_str(token: &str) -> Result<AuthenticatedUser, (StatusCode, String)> {
    let secret = jwt_secret().map_err(|_| {
        tracing::error!("JWT_SECRET not set");
        (StatusCode::INTERNAL_SERVER_ERROR, "Server configuration error".to_string())
    })?;

    let mut validation = Validation::default();
    validation.validate_exp = true;
    if let Some(issuer) = jwt_issuer() {
        validation.set_issuer(&[issuer.as_str()]);
    }
    if let Some(audience) = jwt_audience() {
        validation.set_audience(&[audience.as_str()]);
    }

    let token_data = decode::<Claims>(
        &token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|e| {
        tracing::error!("Token error: {}", e);
        (StatusCode::UNAUTHORIZED, "Invalid token".to_string())
    })?;

    if is_token_revoked(&token_data.claims.jti, token_data.claims.exp) {
        return Err((StatusCode::UNAUTHORIZED, "Invalid token".to_string()));
    }

    Ok(AuthenticatedUser {
        id: token_data.claims.sub,
        email: token_data.claims.email,
        name: token_data.claims.name,
        provider: token_data.claims.provider,
    })
}

pub fn revoke_token_str(token: &str) -> Result<(), ()> {
    let secret = jwt_secret().map_err(|_| ())?;

    let mut validation = Validation::default();
    validation.validate_exp = false;
    if let Some(issuer) = jwt_issuer() {
        validation.set_issuer(&[issuer.as_str()]);
    }
    if let Some(audience) = jwt_audience() {
        validation.set_audience(&[audience.as_str()]);
    }

    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    ).map_err(|_| ())?;

    let now_ts = Utc::now().timestamp();
    let now = if now_ts > 0 { now_ts as usize } else { 0 };
    let revocations = revoked_jtis();
    let mut guard = revocations.lock().map_err(|_| ())?;
    guard.retain(|_, exp| *exp > now);
    guard.insert(token_data.claims.jti, token_data.claims.exp);
    Ok(())
}

fn revoked_jtis() -> &'static Mutex<HashMap<String, usize>> {
    REVOKED_JTIS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn is_token_revoked(jti: &str, exp: usize) -> bool {
    let now_ts = Utc::now().timestamp();
    let now = if now_ts > 0 { now_ts as usize } else { 0 };
    let revocations = revoked_jtis();
    if let Ok(mut guard) = revocations.lock() {
        guard.retain(|_, expires_at| *expires_at > now);
        if exp <= now {
            return true;
        }
        return guard.contains_key(jti);
    }
    false
}

pub async fn login(Path(provider): Path<String>) -> impl IntoResponse {
    // In a real app, you would have a map of clients for each provider.
    // Here is a simplified example for one provider or generic logic.
    
    let cfg = match load_provider_config(&provider) {
        Ok(c) => c,
        Err(e) => return (StatusCode::BAD_REQUEST, e).into_response(),
    };

    let auth_url = match AuthUrl::new(cfg.auth_url.clone()) {
        Ok(v) => v,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Invalid auth URL configuration").into_response(),
    };
    let token_url = match TokenUrl::new(cfg.token_url.clone()) {
        Ok(v) => v,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Invalid token URL configuration").into_response(),
    };
    let redirect_url = match RedirectUrl::new(cfg.redirect_url.clone()) {
        Ok(v) => v,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Invalid redirect URL configuration").into_response(),
    };

    let client = BasicClient::new(ClientId::new(cfg.client_id))
        .set_client_secret(ClientSecret::new(cfg.client_secret))
        .set_auth_uri(auth_url)
        .set_token_uri(token_url)
        .set_redirect_uri(redirect_url);

    let state = match create_state_token(&provider) {
        Ok(s) => s,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };
    let state_for_cookie = state.clone();

    let (authorize_url, _csrf_state) = client
        .authorize_url(|| oauth2::CsrfToken::new(state))
        .url();

    let mut response = Redirect::to(authorize_url.as_str()).into_response();
    let state_cookie = build_oauth_state_cookie(&state_for_cookie);
    if let Ok(header_value) = HeaderValue::from_str(&state_cookie) {
        response.headers_mut().append(header::SET_COOKIE, header_value);
    }
    response
}

pub async fn callback(
    Path(provider): Path<String>,
    Query(params): Query<AuthCallback>,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let cfg = match load_provider_config(&provider) {
        Ok(c) => c,
        Err(e) => return (StatusCode::BAD_REQUEST, e).into_response(),
    };

    let state_cookie = extract_cookie_by_name(&headers, "oauth_state");
    if state_cookie.as_deref() != Some(params.state.as_str()) {
        tracing::warn!("OAuth state cookie mismatch or missing");
        let mut response = (StatusCode::UNAUTHORIZED, "Invalid state").into_response();
        let clear_state_cookie = clear_oauth_state_cookie();
        if let Ok(header_value) = HeaderValue::from_str(&clear_state_cookie) {
            response.headers_mut().append(header::SET_COOKIE, header_value);
        }
        return response;
    }

    if let Err(e) = validate_state_token(&params.state, &provider) {
        tracing::warn!("OAuth state invalid: {}", e);
        let mut response = (StatusCode::UNAUTHORIZED, "Invalid state").into_response();
        let clear_state_cookie = clear_oauth_state_cookie();
        if let Ok(header_value) = HeaderValue::from_str(&clear_state_cookie) {
            response.headers_mut().append(header::SET_COOKIE, header_value);
        }
        return response;
    }

    let auth_url = match AuthUrl::new(cfg.auth_url.clone()) {
        Ok(v) => v,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Invalid auth URL configuration").into_response(),
    };
    let token_url = match TokenUrl::new(cfg.token_url.clone()) {
        Ok(v) => v,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Invalid token URL configuration").into_response(),
    };
    let redirect_url = match RedirectUrl::new(cfg.redirect_url.clone()) {
        Ok(v) => v,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Invalid redirect URL configuration").into_response(),
    };

    let client = BasicClient::new(ClientId::new(cfg.client_id))
        .set_client_secret(ClientSecret::new(cfg.client_secret))
        .set_auth_uri(auth_url)
        .set_token_uri(token_url)
        .set_redirect_uri(redirect_url);

    let http_client = match oauth_http_client() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("OAuth HTTP client init failed: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Server configuration error").into_response();
        }
    };

    let token_result = client
        .exchange_code(AuthorizationCode::new(params.code.clone()))
        .request_async(&http_client.clone())
        .await;
    let token_result = match token_result {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("OAuth token exchange failed: {}", e);
            return (StatusCode::BAD_GATEWAY, "OAuth token exchange failed").into_response();
        }
    };

    let access_token = token_result.access_token().secret();
    let profile = match fetch_user_profile(&cfg.userinfo_url, access_token).await {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("Userinfo fetch failed: {}", e);
            return (StatusCode::BAD_GATEWAY, "Userinfo fetch failed").into_response();
        }
    };

    let user = UserProfile {
        id: profile.id,
        email: profile.email,
        name: profile.name,
        filing_status: None,
        agi: None,
        marginal_tax_rate: None,
        itemize_deductions: None,
        provider,
    };

    let _ = crate::db::users::upsert_user_profile(&state.db, &UserProfileUpsert {
        user_id: user.id.clone(),
        email: user.email.clone(),
        name: user.name.clone(),
        provider: user.provider.clone(),
        filing_status: user.filing_status.clone(),
        agi: user.agi,
        marginal_tax_rate: user.marginal_tax_rate,
        itemize_deductions: user.itemize_deductions,
    }).await;

    let token = match create_jwt(&user) {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("JWT creation failed: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Auth failed").into_response();
        }
    };

    let cookie = build_auth_cookie(&token);
    let mut response = Redirect::to("/").into_response();
    if let Ok(header_value) = HeaderValue::from_str(&cookie) {
        response.headers_mut().insert(header::SET_COOKIE, header_value);
    }
    let clear_state_cookie = clear_oauth_state_cookie();
    if let Ok(header_value) = HeaderValue::from_str(&clear_state_cookie) {
        response.headers_mut().append(header::SET_COOKIE, header_value);
    }
    response
}

