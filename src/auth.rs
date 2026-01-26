use axum::{
    extract::{Path, State, Query, Json, FromRequestParts},
    response::{Redirect, IntoResponse},
    http::{StatusCode, request::Parts},
    async_trait,
};
use serde::{Deserialize, Serialize};
use crate::AppState;
use oauth2::{
    basic::BasicClient, AuthUrl, ClientId, ClientSecret, RedirectUrl, TokenUrl,
};
use std::env;
use jsonwebtoken::{encode, decode, EncodingKey, DecodingKey, Header, Validation};
use chrono::{Utc, Duration};

#[derive(Deserialize)]
pub struct AuthCallback {
    _code: String,
    _state: String,
}

#[derive(Deserialize)]
pub struct DevLoginRequest {
    username: String,
    password: String,
}

#[derive(Serialize)]
pub struct AuthResponse {
    token: String,
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
}

pub struct AuthenticatedUser {
    pub id: String,
}

#[async_trait]
impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .ok_or((StatusCode::UNAUTHORIZED, "Missing Authorization header".to_string()))?;

        if !auth_header.starts_with("Bearer ") {
            return Err((StatusCode::UNAUTHORIZED, "Invalid Authorization header".to_string()));
        }

        let token = &auth_header[7..];
        let secret = env::var("JWT_SECRET").unwrap_or_else(|_| "secret".to_string());
        
        let mut validation = Validation::default();
        validation.validate_exp = true;

        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(secret.as_ref()),
            &validation,
        )
        .map_err(|e| {
            tracing::error!("Token error: {}", e);
            (StatusCode::UNAUTHORIZED, "Invalid token".to_string())
        })?;

        Ok(AuthenticatedUser {
            id: token_data.claims.sub,
        })
    }
}

pub async fn login(Path(provider): Path<String>) -> impl IntoResponse {
    // In a real app, you would have a map of clients for each provider.
    // Here is a simplified example for one provider or generic logic.
    
    let client_id = env::var(format!("{}_CLIENT_ID", provider.to_uppercase()))
        .unwrap_or_else(|_| "missing-id".to_string());
    let client_secret = env::var(format!("{}_CLIENT_SECRET", provider.to_uppercase()))
        .unwrap_or_else(|_| "missing-secret".to_string());
        
    let auth_url = format!("https://{}.com/oauth/authorize", provider); // distinct per provider
    let token_url = format!("https://{}.com/oauth/token", provider);

    // This is pseudo-code for setup, real implementation requires proper discovery
    let client = BasicClient::new(
        ClientId::new(client_id),
        Some(ClientSecret::new(client_secret)),
        AuthUrl::new(auth_url).unwrap(),
        Some(TokenUrl::new(token_url).unwrap())
    )
    .set_redirect_uri(RedirectUrl::new("http://localhost:8080/auth/callback".to_string()).unwrap());

    let (authorize_url, _csrf_state) = client
        .authorize_url(oauth2::CsrfToken::new_random)
        .url();

    Redirect::to(authorize_url.as_str())
}

pub async fn callback(
    Path(provider): Path<String>,
    Query(_params): Query<AuthCallback>,
    State(_state): State<AppState>,
) -> impl IntoResponse {
    // 1. Exchange code for token (Mocked for brevity as we don't have real creds)
    // 2. Fetch User Info
    
    // MOCK USER for success path
    let user = UserProfile {
        id: "social-123".to_string(),
        email: "user@example.com".to_string(),
        name: "Social User".to_string(),
        provider,
    };

    let token = create_jwt(&user).unwrap();

    // Redirect to frontend with token
    Redirect::to(&format!("/?token={}", token))
}

pub async fn dev_login(
    State(_state): State<AppState>,
    Json(payload): Json<DevLoginRequest>,
) -> impl IntoResponse {
    let dev_user = env::var("DEV_USERNAME").unwrap_or_else(|_| "admin".to_string());
    let dev_pass = env::var("DEV_PASSWORD").unwrap_or_else(|_| "password".to_string());

    if payload.username == dev_user && payload.password == dev_pass {
        let user = UserProfile {
            id: "dev-1".to_string(),
            email: "dev@local".to_string(),
            name: "Developer".to_string(),
            provider: "local".to_string(),
        };
        let token = create_jwt(&user).unwrap();
        Json(AuthResponse { token, user }).into_response()
    } else {
        (StatusCode::UNAUTHORIZED, "Invalid credentials").into_response()
    }
}

fn create_jwt(user: &UserProfile) -> anyhow::Result<String> {
    let expiration = Utc::now()
        .checked_add_signed(Duration::days(1))
        .expect("valid timestamp")
        .timestamp();

    let claims = Claims {
        sub: user.id.clone(),
        email: user.email.clone(),
        provider: user.provider.clone(),
        exp: expiration as usize,
    };

    let secret = env::var("JWT_SECRET").unwrap_or_else(|_| "secret".to_string());
    let token = encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_ref()))?;

    Ok(token)
}
