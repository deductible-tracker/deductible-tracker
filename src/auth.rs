use std::future::Future;
use axum::{
    extract::{Path, State, Query, Json, FromRequestParts},
    response::{Redirect, IntoResponse},
    http::{StatusCode, request::Parts, HeaderValue, header},
};
use serde::{Deserialize, Serialize};
use crate::AppState;
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

const AUTH_COOKIE_NAME: &str = "auth_token";
static JWT_SECRET: OnceLock<String> = OnceLock::new();
static JWT_ISSUER: OnceLock<Option<String>> = OnceLock::new();
static JWT_AUDIENCE: OnceLock<Option<String>> = OnceLock::new();
static HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

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
    pub phone: Option<String>,
    pub tax_id: Option<String>,
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
    pub phone: Option<String>,
    pub tax_id: Option<String>,
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

    fn from_request_parts(parts: &mut Parts, _state: &S) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        async move {
            let token = extract_token(parts)
                .ok_or((StatusCode::UNAUTHORIZED, "Missing auth token".to_string()))?;

            // Delegate to shared validator
            match validate_token_str(&token) {
                Ok(u) => Ok(u),
                Err(e) => Err(e),
            }
        }
    }
}



use axum::http::HeaderMap;

// Extract token directly from headers (used by middleware)
pub fn extract_token_from_headers(headers: &HeaderMap) -> Option<String> {
    if let Some(auth_header) = headers.get(axum::http::header::AUTHORIZATION).and_then(|h| h.to_str().ok()) {
        if auth_header.starts_with("Bearer ") {
            return Some(auth_header[7..].to_string());
        }
    }

    if let Some(cookie_header) = headers.get(header::COOKIE).and_then(|h| h.to_str().ok()) {
        for cookie in cookie_header.split(';') {
            let cookie = cookie.trim();
            if let Some((k, v)) = cookie.split_once('=') {
                if k == AUTH_COOKIE_NAME {
                    return Some(v.to_string());
                }
            }
        }
    }
    None
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

    Ok(AuthenticatedUser {
        id: token_data.claims.sub,
        email: token_data.claims.email,
        name: token_data.claims.name,
        provider: token_data.claims.provider,
    })
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

    let (authorize_url, _csrf_state) = client
        .authorize_url(|| oauth2::CsrfToken::new(state))
        .url();

    Redirect::to(authorize_url.as_str()).into_response()
}

pub async fn callback(
    Path(provider): Path<String>,
    Query(params): Query<AuthCallback>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let cfg = match load_provider_config(&provider) {
        Ok(c) => c,
        Err(e) => return (StatusCode::BAD_REQUEST, e).into_response(),
    };

    if let Err(e) = validate_state_token(&params.state, &provider) {
        tracing::warn!("OAuth state invalid: {}", e);
        return (StatusCode::UNAUTHORIZED, "Invalid state").into_response();
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
        phone: None,
        tax_id: None,
        filing_status: None,
        agi: None,
        marginal_tax_rate: None,
        itemize_deductions: None,
        provider,
    };

    let _ = crate::db::upsert_user_profile(
        &state.db,
        &user.id,
        &user.email,
        &user.name,
        &user.provider,
        &user.phone,
        &user.tax_id,
        &user.filing_status,
        &user.agi,
        &user.marginal_tax_rate,
        &user.itemize_deductions,
    ).await;

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
    response
}

pub async fn dev_login(
    State(_state): State<AppState>,
    Json(payload): Json<DevLoginRequest>,
) -> impl IntoResponse {
    let env_mode = env::var("RUST_ENV").unwrap_or_else(|_| "development".to_string());
    // Only allow dev login in development and if explicitly enabled
    if env_mode == "production" || env::var("ALLOW_DEV_LOGIN").unwrap_or_default() != "true" {
        return (StatusCode::FORBIDDEN, "Dev login disabled").into_response();
    }

    let dev_user = env::var("DEV_USERNAME").unwrap_or_else(|_| "admin".to_string());
    let dev_pass = env::var("DEV_PASSWORD").unwrap_or_else(|_| "password".to_string());

    if dev_pass == "password" {
        tracing::warn!("Default DEV_PASSWORD is not allowed");
        return (StatusCode::FORBIDDEN, "Dev login misconfigured").into_response();
    }

    if payload.username == dev_user && payload.password == dev_pass {
        let user = UserProfile {
            id: "dev-1".to_string(),
            email: "dev@local".to_string(),
            name: "Developer".to_string(),
            phone: None,
            tax_id: None,
            filing_status: None,
            agi: None,
            marginal_tax_rate: None,
            itemize_deductions: None,
            provider: "local".to_string(),
        };
        let _ = crate::db::upsert_user_profile(
            &_state.db,
            &user.id,
            &user.email,
            &user.name,
            &user.provider,
            &user.phone,
            &user.tax_id,
            &user.filing_status,
            &user.agi,
            &user.marginal_tax_rate,
            &user.itemize_deductions,
        ).await;
        match create_jwt(&user) {
            Ok(token) => {
                let cookie = build_auth_cookie(&token);
                let mut response = Json(AuthResponse { user }).into_response();
                if let Ok(header_value) = HeaderValue::from_str(&cookie) {
                    response.headers_mut().insert(header::SET_COOKIE, header_value);
                }
                response
            },
            Err(e) => {
                tracing::error!("JWT creation failed: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "failed to create token").into_response()
            }
        }
    } else {
        (StatusCode::UNAUTHORIZED, "Invalid credentials").into_response()
    }
}

pub async fn logout() -> impl IntoResponse {
    // Emit Set-Cookie headers that clear the auth cookie. Some browsers are picky
    // about SameSite/Secure attributes when clearing cookies, so send variants
    // that cover common cases.
    let secure = env::var("RUST_ENV").unwrap_or_else(|_| "development".to_string()) == "production";

    // Strict variant (matches how we normally set the cookie)
    let mut cookie_strict = format!(
        "{}=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0; Expires=Thu, 01 Jan 1970 00:00:00 GMT",
        AUTH_COOKIE_NAME
    );
    if secure {
        cookie_strict.push_str("; Secure");
    }

    let mut response = (StatusCode::OK, "OK").into_response();
    if let Ok(header_value) = HeaderValue::from_str(&cookie_strict) {
        response.headers_mut().append(header::SET_COOKIE, header_value);
    }

    // If running in a secure context, also emit a None+Secure variant which
    // some clients require when SameSite=None was used previously.
    if secure {
        let cookie_none = format!(
            "{}=; HttpOnly; SameSite=None; Path=/; Max-Age=0; Expires=Thu, 01 Jan 1970 00:00:00 GMT; Secure",
            AUTH_COOKIE_NAME
        );
        if let Ok(header_value) = HeaderValue::from_str(&cookie_none) {
            response.headers_mut().append(header::SET_COOKIE, header_value);
        }
    }

    response
}

pub async fn me(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> impl IntoResponse {
    match crate::db::get_user_profile(&state.db, &user.id).await {
        Ok(Some((email, name, provider, phone, tax_id, filing_status, agi, marginal_tax_rate, itemize_deductions))) => Json(UserProfile {
            id: user.id,
            email,
            name,
            phone,
            tax_id,
            filing_status,
            agi,
            marginal_tax_rate,
            itemize_deductions,
            provider,
        }).into_response(),
        Ok(None) => {
            let phone = None;
            let tax_id = None;
            let filing_status = None;
            let agi = None;
            let marginal_tax_rate = None;
            let itemize_deductions = None;
            let _ = crate::db::upsert_user_profile(&state.db, &user.id, &user.email, &user.name, &user.provider, &phone, &tax_id, &filing_status, &agi, &marginal_tax_rate, &itemize_deductions).await;
            Json(UserProfile {
                id: user.id,
                email: user.email,
                name: user.name,
                phone,
                tax_id,
                filing_status,
                agi,
                marginal_tax_rate,
                itemize_deductions,
                provider: user.provider,
            }).into_response()
        }
        Err(e) => {
            tracing::error!("Failed loading profile: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Database Error").into_response()
        }
    }
}

pub async fn update_me(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(req): Json<UpdateMeRequest>,
) -> impl IntoResponse {
    let email = req.email.trim().to_string();
    let name = req.name.trim().to_string();
    if email.is_empty() || name.is_empty() {
        return (StatusCode::BAD_REQUEST, "Name and email are required").into_response();
    }

    let phone = req.phone.and_then(|v| {
        let trimmed = v.trim().to_string();
        if trimmed.is_empty() { None } else { Some(trimmed) }
    });
    let tax_id = req.tax_id.and_then(|v| {
        let trimmed = v.trim().to_string();
        if trimmed.is_empty() { None } else { Some(trimmed) }
    });
    let filing_status = req.filing_status.and_then(|value| {
        let normalized = value.trim().to_lowercase();
        if normalized.is_empty() {
            None
        } else {
            Some(normalized)
        }
    });
    let agi = req.agi.and_then(|value| if value.is_finite() && value >= 0.0 { Some(value) } else { None });
    let marginal_tax_rate = req
        .marginal_tax_rate
        .and_then(|value| if value.is_finite() && value >= 0.0 && value <= 1.0 { Some(value) } else { None });
    let itemize_deductions = req.itemize_deductions;

    match crate::db::upsert_user_profile(&state.db, &user.id, &email, &name, &user.provider, &phone, &tax_id, &filing_status, &agi, &marginal_tax_rate, &itemize_deductions).await {
        Ok(_) => Json(UserProfile {
            id: user.id,
            email,
            name,
            phone,
            tax_id,
            filing_status,
            agi,
            marginal_tax_rate,
            itemize_deductions,
            provider: user.provider,
        }).into_response(),
        Err(e) => {
            tracing::error!("Failed saving profile: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Database Error").into_response()
        }
    }
}

fn create_jwt(user: &UserProfile) -> anyhow::Result<String> {
    let expiration = Utc::now()
        .checked_add_signed(Duration::days(1))
        .ok_or_else(|| anyhow::anyhow!("failed to compute expiration timestamp"))?
        .timestamp();

    let issuer = jwt_issuer();
    let audience = jwt_audience();

    let claims = Claims {
        sub: user.id.clone(),
        email: user.email.clone(),
        provider: user.provider.clone(),
        name: user.name.clone(),
        exp: expiration as usize,
        iss: issuer,
        aud: audience,
    };

    let secret = jwt_secret()?;
    let token = encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_bytes()))?;

    Ok(token)
}

fn extract_token(parts: &Parts) -> Option<String> {
    if let Some(auth_header) = parts
        .headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
    {
        if auth_header.starts_with("Bearer ") {
            return Some(auth_header[7..].to_string());
        }
    }

    if let Some(cookie_header) = parts
        .headers
        .get(header::COOKIE)
        .and_then(|h| h.to_str().ok())
    {
        for cookie in cookie_header.split(';') {
            let cookie = cookie.trim();
            if let Some((k, v)) = cookie.split_once('=') {
                if k == AUTH_COOKIE_NAME {
                    return Some(v.to_string());
                }
            }
        }
    }
    None
}

fn build_auth_cookie(token: &str) -> String {
    let secure = env::var("RUST_ENV").unwrap_or_else(|_| "development".to_string()) == "production";
    let mut cookie = format!(
        "{}={}; HttpOnly; SameSite=Strict; Path=/; Max-Age=86400",
        AUTH_COOKIE_NAME,
        token
    );
    if secure {
        cookie.push_str("; Secure");
    }
    cookie
}

// `logout()` now emits explicit Set-Cookie headers to clear the auth cookie,
// so the older helper `clear_auth_cookie` is no longer needed.

struct ProviderConfig {
    client_id: String,
    client_secret: String,
    auth_url: String,
    token_url: String,
    userinfo_url: String,
    redirect_url: String,
}

fn load_provider_config(provider: &str) -> Result<ProviderConfig, String> {
    let allowed = env::var("OAUTH_PROVIDERS").unwrap_or_default();
    let allowed_list: Vec<String> = allowed.split(',').map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()).collect();
    if allowed_list.is_empty() || !allowed_list.contains(&provider.to_lowercase()) {
        return Err("OAuth provider not allowed".to_string());
    }

    let prefix = provider.to_uppercase();
    let client_id = env::var(format!("{}_CLIENT_ID", prefix)).map_err(|_| "Missing client id".to_string())?;
    let client_secret = env::var(format!("{}_CLIENT_SECRET", prefix)).map_err(|_| "Missing client secret".to_string())?;
    let auth_url = env::var(format!("{}_AUTH_URL", prefix)).map_err(|_| "Missing auth url".to_string())?;
    let token_url = env::var(format!("{}_TOKEN_URL", prefix)).map_err(|_| "Missing token url".to_string())?;
    let userinfo_url = env::var(format!("{}_USERINFO_URL", prefix)).map_err(|_| "Missing userinfo url".to_string())?;
    let redirect_url = env::var(format!("{}_REDIRECT_URL", prefix))
        .unwrap_or_else(|_| format!("http://localhost:8080/auth/callback/{}", provider));

    Ok(ProviderConfig {
        client_id,
        client_secret,
        auth_url,
        token_url,
        userinfo_url,
        redirect_url,
    })
}

fn create_state_token(provider: &str) -> anyhow::Result<String> {
    let expiration = Utc::now()
        .checked_add_signed(Duration::minutes(10))
        .ok_or_else(|| anyhow::anyhow!("failed to compute state expiration timestamp"))?
        .timestamp();
    let state = StateClaims {
        exp: expiration as usize,
        provider: provider.to_string(),
        nonce: uuid::Uuid::new_v4().to_string(),
    };
    let secret = jwt_secret()?;
    let token = encode(&Header::default(), &state, &EncodingKey::from_secret(secret.as_bytes()))?;
    Ok(token)
}


fn validate_state_token(token: &str, provider: &str) -> anyhow::Result<()> {
    let secret = jwt_secret()?;
    let mut validation = Validation::default();
    validation.validate_exp = true;
    let data = decode::<StateClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )?;
    if data.claims.provider.to_lowercase() != provider.to_lowercase() {
        return Err(anyhow::anyhow!("provider mismatch"));
    }
    Ok(())
}

struct ProviderProfile {
    id: String,
    email: String,
    name: String,
}

async fn fetch_user_profile(userinfo_url: &str, access_token: &str) -> anyhow::Result<ProviderProfile> {
    let client = oauth_http_client()?;
    let resp = client
        .get(userinfo_url)
        .bearer_auth(access_token)
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(anyhow::anyhow!("userinfo response status {}", resp.status()));
    }

    let json: Value = resp.json().await?;
    let id = json.get("sub")
        .or_else(|| json.get("id"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing user id"))?
        .to_string();
    let email = json.get("email")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown@example.com")
        .to_string();
    let name = json.get("name")
        .or_else(|| json.get("login"))
        .and_then(|v| v.as_str())
        .unwrap_or("User")
        .to_string();

    Ok(ProviderProfile { id, email, name })
}

fn jwt_secret() -> anyhow::Result<&'static str> {
    if let Some(existing) = JWT_SECRET.get() {
        return Ok(existing.as_str());
    }
    let value = env::var("JWT_SECRET")
        .map_err(|_| anyhow::anyhow!("JWT_SECRET environment variable not set"))?;
    let _ = JWT_SECRET.set(value);
    JWT_SECRET
        .get()
        .map(|s| s.as_str())
        .ok_or_else(|| anyhow::anyhow!("JWT secret unavailable"))
}

fn jwt_issuer() -> Option<String> {
    if let Some(v) = JWT_ISSUER.get() {
        return v.clone();
    }
    let value = env::var("JWT_ISSUER").ok();
    let _ = JWT_ISSUER.set(value.clone());
    value
}

fn jwt_audience() -> Option<String> {
    if let Some(v) = JWT_AUDIENCE.get() {
        return v.clone();
    }
    let value = env::var("JWT_AUDIENCE").ok();
    let _ = JWT_AUDIENCE.set(value.clone());
    value
}

fn oauth_http_client() -> anyhow::Result<&'static reqwest::Client> {
    if let Some(c) = HTTP_CLIENT.get() {
        return Ok(c);
    }
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(15))
        .build()?;
    let _ = HTTP_CLIENT.set(client);
    HTTP_CLIENT
        .get()
        .ok_or_else(|| anyhow::anyhow!("HTTP client unavailable"))
}
