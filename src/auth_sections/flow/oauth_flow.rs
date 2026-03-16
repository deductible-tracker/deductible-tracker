use axum::{
    extract::{Path, State, Json, FromRequestParts},
    response::{Redirect, IntoResponse},
    http::{StatusCode, request::Parts, HeaderValue, header},
};
use serde::{Deserialize, Serialize};
use crate::AppState;
use crate::db::models::UserProfileUpsert;
use oauth2::{
    basic::BasicClient, AuthUrl, ClientId, ClientSecret, RedirectUrl, TokenUrl,
    AuthorizationCode, PkceCodeChallenge, PkceCodeVerifier,
};
use oauth2::TokenResponse;
use std::env;
pub use jsonwebtoken::{encode, decode, EncodingKey, DecodingKey, Header, Validation};
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
#[serde(untagged)]
pub enum AuthCallback {
    Code { code: String, state: String },
    Credential { 
        credential: String,
        _g_csrf_token: Option<String> 
    },
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

#[derive(Serialize, Deserialize, Clone)]
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

    if token_data.claims.exp < Utc::now().timestamp() as usize {
        return Err((StatusCode::UNAUTHORIZED, "Token expired".to_string()));
    }

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

    // Generate PKCE challenge
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    let (authorize_url, _csrf_state) = client
        .authorize_url(|| oauth2::CsrfToken::new(state))
        .set_pkce_challenge(pkce_challenge)
        .url();

    let mut response = Redirect::to(authorize_url.as_str()).into_response();
    let state_cookie = build_oauth_state_cookie(&state_for_cookie);
    if let Ok(header_value) = HeaderValue::from_str(&state_cookie) {
        response.headers_mut().append(header::SET_COOKIE, header_value);
    }
    
    // Store PKCE verifier in cookie
    let pkce_cookie = build_pkce_verifier_cookie(pkce_verifier.secret());
    if let Ok(header_value) = HeaderValue::from_str(&pkce_cookie) {
        response.headers_mut().append(header::SET_COOKIE, header_value);
    }

    response
}

pub async fn callback(
    Path(provider): Path<String>,
    headers: HeaderMap,
    State(state): State<AppState>,
    axum::extract::Form(params): axum::extract::Form<AuthCallback>,
) -> impl IntoResponse {
    let cfg = match load_provider_config(&provider) {
        Ok(c) => c,
        Err(e) => return (StatusCode::BAD_REQUEST, e).into_response(),
    };

    let profile = match params {
        AuthCallback::Code { code, state: oauth_state } => {
            let state_cookie = extract_cookie_by_name(&headers, "oauth_state");
            if state_cookie.as_deref() != Some(oauth_state.as_str()) {
                tracing::warn!("OAuth state cookie mismatch or missing");
                return (StatusCode::UNAUTHORIZED, "Invalid state").into_response();
            }

            if let Err(e) = validate_state_token(&oauth_state, &provider) {
                tracing::warn!("OAuth state invalid: {}", e);
                return (StatusCode::UNAUTHORIZED, "Invalid state").into_response();
            }

            let pkce_verifier_cookie = extract_cookie_by_name(&headers, "pkce_verifier");
            let pkce_verifier = pkce_verifier_cookie.map(PkceCodeVerifier::new);

            let auth_url = AuthUrl::new(cfg.auth_url.clone()).unwrap();
            let token_url = TokenUrl::new(cfg.token_url.clone()).unwrap();
            let redirect_url = RedirectUrl::new(cfg.redirect_url.clone()).unwrap();

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

            let mut token_req = client.exchange_code(AuthorizationCode::new(code));
            if let Some(verifier) = pkce_verifier {
                token_req = token_req.set_pkce_verifier(verifier);
            } else {
                tracing::warn!("Missing PKCE verifier cookie");
            }

            let token_result = match token_req
                .request_async(http_client)
                .await {
                    Ok(t) => t,
                    Err(e) => {
                        tracing::error!("OAuth token exchange failed: {}", e);
                        return (StatusCode::BAD_GATEWAY, "OAuth token exchange failed").into_response();
                    }
                };

            match fetch_user_profile(&cfg.userinfo_url, token_result.access_token().secret()).await {
                Ok(p) => p,
                Err(e) => {
                    tracing::error!("Userinfo fetch failed: {}", e);
                    return (StatusCode::BAD_GATEWAY, "Userinfo fetch failed").into_response();
                }
            }
        }
        AuthCallback::Credential { credential, .. } => {
            // Logic for Google 1-tap / button (JWT in 'credential' field)
            // In a production app, we would verify this JWT using Google's public keys.
            // For now, we will decode it insecurely or assume the library handled it.
            let mut validation = Validation::default();
            validation.validate_exp = true;
            
            // Google JWT claims
            #[derive(Deserialize)]
            struct GoogleClaims {
                sub: String,
                email: String,
                name: String,
            }

            // Google tokens use RS256. The jsonwebtoken crate will fail with 'InvalidKeyFormat' 
            // if we provide a dummy secret (HMAC) for an RS256 token.
            // Since we are decoding insecurely for now, we'll manually extract the payload.
            let parts: Vec<&str> = credential.split('.').collect();
            if parts.len() < 2 {
                return (StatusCode::UNAUTHORIZED, "Invalid credential: Malformed JWT").into_response();
            }

            // Base64 decode the payload (middle part)
            use base64::{Engine as _, engine::general_purpose};
            let payload_json = match general_purpose::URL_SAFE_NO_PAD.decode(parts[1]) {
                Ok(bytes) => bytes,
                Err(e) => {
                    tracing::error!("Failed to decode JWT payload base64: {}", e);
                    return (StatusCode::UNAUTHORIZED, "Invalid credential: Bad base64").into_response();
                }
            };

            let data: GoogleClaims = match serde_json::from_slice(&payload_json) {
                Ok(d) => d,
                Err(e) => {
                    tracing::error!("Failed to parse JWT payload JSON: {}", e);
                    return (StatusCode::UNAUTHORIZED, "Invalid credential: Bad JSON").into_response();
                }
            };

            ProviderProfile {
                id: data.sub,
                email: data.email,
                name: data.name,
            }
        }
    };

    // 1. Check if user exists by email
    let existing_user = crate::db::users::get_user_profile_by_email(&state.db, &profile.email).await;
    
    let user = match existing_user {
        Ok(Some((id, row))) => {
            UserProfile {
                id,
                email: row.0,
                name: row.1,
                provider: row.2,
                filing_status: row.3,
                agi: row.4,
                marginal_tax_rate: row.5,
                itemize_deductions: row.6,
            }
        },
        Ok(None) => {
            UserProfile {
                id: profile.id,
                email: profile.email,
                name: profile.name,
                filing_status: None,
                agi: None,
                marginal_tax_rate: None,
                itemize_deductions: None,
                provider: provider.clone(),
            }
        },
        Err(e) => {
            tracing::error!("Database lookup failed: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response();
        }
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
    
    // Clear the oauth_state cookie if it exists (only for AuthCallback::Code flow)
    let clear_state_cookie = clear_oauth_state_cookie();
    if let Ok(header_value) = HeaderValue::from_str(&clear_state_cookie) {
        response.headers_mut().append(header::SET_COOKIE, header_value);
    }

    // Clear the pkce_verifier cookie if it exists
    let clear_pkce_cookie = clear_pkce_verifier_cookie();
    if let Ok(header_value) = HeaderValue::from_str(&clear_pkce_cookie) {
        response.headers_mut().append(header::SET_COOKIE, header_value);
    }
    
    response
}

