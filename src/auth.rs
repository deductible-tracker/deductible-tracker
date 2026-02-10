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

const AUTH_COOKIE_NAME: &str = "auth_token";

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
            let secret = env::var("JWT_SECRET").map_err(|_| {
                tracing::error!("JWT_SECRET not set");
                (StatusCode::INTERNAL_SERVER_ERROR, "Server configuration error".to_string())
            })?;
            
            let mut validation = Validation::default();
            validation.validate_exp = true;
            if let Ok(issuer) = env::var("JWT_ISSUER") {
                validation.set_issuer(&[issuer.as_str()]);
            }
            if let Ok(audience) = env::var("JWT_AUDIENCE") {
                validation.set_audience(&[audience.as_str()]);
            }

            let token_data = decode::<Claims>(
                &token,
                &DecodingKey::from_secret(secret.as_ref()),
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
    }
}

pub async fn login(Path(provider): Path<String>) -> impl IntoResponse {
    // In a real app, you would have a map of clients for each provider.
    // Here is a simplified example for one provider or generic logic.
    
    let cfg = match load_provider_config(&provider) {
        Ok(c) => c,
        Err(e) => return (StatusCode::BAD_REQUEST, e).into_response(),
    };

    let client = BasicClient::new(ClientId::new(cfg.client_id))
        .set_client_secret(ClientSecret::new(cfg.client_secret))
        .set_auth_uri(AuthUrl::new(cfg.auth_url).unwrap())
        .set_token_uri(TokenUrl::new(cfg.token_url).unwrap())
        .set_redirect_uri(RedirectUrl::new(cfg.redirect_url).unwrap());

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
    State(_state): State<AppState>,
) -> impl IntoResponse {
    let cfg = match load_provider_config(&provider) {
        Ok(c) => c,
        Err(e) => return (StatusCode::BAD_REQUEST, e).into_response(),
    };

    if let Err(e) = validate_state_token(&params.state, &provider) {
        tracing::warn!("OAuth state invalid: {}", e);
        return (StatusCode::UNAUTHORIZED, "Invalid state").into_response();
    }

    let client = BasicClient::new(ClientId::new(cfg.client_id))
        .set_client_secret(ClientSecret::new(cfg.client_secret))
        .set_auth_uri(AuthUrl::new(cfg.auth_url).unwrap())
        .set_token_uri(TokenUrl::new(cfg.token_url).unwrap())
        .set_redirect_uri(RedirectUrl::new(cfg.redirect_url).unwrap());

    let http_client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("failed to build reqwest client");

    let token_result = client
        .exchange_code(AuthorizationCode::new(params.code.clone()))
        .request_async(&http_client)
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
        provider,
    };

    let token = match create_jwt(&user) {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("JWT creation failed: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Auth failed").into_response();
        }
    };

    let cookie = build_auth_cookie(&token);
    let mut response = Redirect::to("/").into_response();
    response.headers_mut().insert(header::SET_COOKIE, HeaderValue::from_str(&cookie).unwrap());
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
            provider: "local".to_string(),
        };
        match create_jwt(&user) {
            Ok(token) => {
                let cookie = build_auth_cookie(&token);
                let mut response = Json(AuthResponse { user }).into_response();
                response.headers_mut().insert(header::SET_COOKIE, HeaderValue::from_str(&cookie).unwrap());
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
    let cookie = clear_auth_cookie();
    let mut response = (StatusCode::OK, "OK").into_response();
    response.headers_mut().insert(header::SET_COOKIE, HeaderValue::from_str(&cookie).unwrap());
    response
}

pub async fn me(user: AuthenticatedUser) -> impl IntoResponse {
    let profile = UserProfile {
        id: user.id,
        email: user.email,
        name: user.name,
        provider: user.provider,
    };
    Json(profile)
}

fn create_jwt(user: &UserProfile) -> anyhow::Result<String> {
    let expiration = Utc::now()
        .checked_add_signed(Duration::days(1))
        .expect("valid timestamp")
        .timestamp();

    let issuer = env::var("JWT_ISSUER").ok();
    let audience = env::var("JWT_AUDIENCE").ok();

    let claims = Claims {
        sub: user.id.clone(),
        email: user.email.clone(),
        provider: user.provider.clone(),
        name: user.name.clone(),
        exp: expiration as usize,
        iss: issuer,
        aud: audience,
    };

    let secret = env::var("JWT_SECRET")
        .map_err(|_| anyhow::anyhow!("JWT_SECRET environment variable not set"))?;
    let token = encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_ref()))?;

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

fn clear_auth_cookie() -> String {
    let secure = env::var("RUST_ENV").unwrap_or_else(|_| "development".to_string()) == "production";
    let mut cookie = format!(
        "{}=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0",
        AUTH_COOKIE_NAME
    );
    if secure {
        cookie.push_str("; Secure");
    }
    cookie
}

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
        .expect("valid timestamp")
        .timestamp();
    let state = StateClaims {
        exp: expiration as usize,
        provider: provider.to_string(),
        nonce: uuid::Uuid::new_v4().to_string(),
    };
    let secret = env::var("JWT_SECRET")
        .map_err(|_| anyhow::anyhow!("JWT_SECRET environment variable not set"))?;
    let token = encode(&Header::default(), &state, &EncodingKey::from_secret(secret.as_ref()))?;
    Ok(token)
}

fn validate_state_token(token: &str, provider: &str) -> anyhow::Result<()> {
    let secret = env::var("JWT_SECRET")
        .map_err(|_| anyhow::anyhow!("JWT_SECRET environment variable not set"))?;
    let mut validation = Validation::default();
    validation.validate_exp = true;
    let data = decode::<StateClaims>(
        token,
        &DecodingKey::from_secret(secret.as_ref()),
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
    let client = reqwest::Client::new();
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
