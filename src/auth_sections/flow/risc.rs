use axum::body::Bytes;
use jsonwebtoken::jwk::JwkSet;
use std::sync::RwLock;
use chrono::DateTime;

static GOOGLE_JWKS_CACHE: OnceLock<RwLock<CachedJwks>> = OnceLock::new();

struct CachedJwks {
    keys: JwkSet,
    expires_at: DateTime<Utc>,
}

pub async fn get_google_jwks(kid: &str) -> anyhow::Result<JwkSet> {
    let cache = GOOGLE_JWKS_CACHE.get_or_init(|| {
        RwLock::new(CachedJwks {
            keys: JwkSet { keys: vec![] },
            expires_at: Utc::now() - Duration::hours(1),
        })
    });

    // 1. Check if cache is valid and contains the kid
    {
        let read_guard = cache.read().map_err(|_| anyhow::anyhow!("JWKS cache lock poisoned"))?;
        if read_guard.expires_at > Utc::now() && read_guard.keys.find(kid).is_some() {
            return Ok(read_guard.keys.clone());
        }
    }

    // 2. Fetch fresh keys if expired or kid missing
    tracing::info!("Fetching fresh Google JWKS (reason: expired or missing kid '{}')", kid);
    let jwks_url = "https://www.googleapis.com/oauth2/v3/certs";
    let resp = reqwest::get(jwks_url).await?;
    let jwks: JwkSet = resp.json().await?;

    let mut write_guard = cache.write().map_err(|_| anyhow::anyhow!("JWKS cache lock poisoned"))?;
    write_guard.keys = jwks.clone();
    // Cache for 24 hours (Google keys rotate infrequently)
    write_guard.expires_at = Utc::now() + Duration::hours(24);

    Ok(jwks)
}

#[derive(Deserialize)]
pub struct RiscEvent {
    pub iss: String,
    pub sub: String,
    pub aud: String,
    pub iat: usize,
    pub jti: String,
    pub events: Value,
}

pub async fn risc_webhook(
    State(_state): State<AppState>,
    body: Bytes,
) -> impl IntoResponse {
    // 1. Get the raw token from the body
    let token_str = String::from_utf8_lossy(&body);

    // 2. Verify the JWT header to get the 'kid'
    let header = match jsonwebtoken::decode_header(token_str.as_bytes()) {
        Ok(h) => h,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    let kid = match header.kid {
        Some(k) => k,
        None => return StatusCode::BAD_REQUEST.into_response(),
    };

    // 3. Fetch Google's public keys (using cache)
    let jwks = match get_google_jwks(&kid).await {
        Ok(j) => j,
        Err(e) => {
            tracing::error!("Failed to fetch Google JWKS: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    // 4. Verify the JWT
    let jwk = match jwks.find(&kid) {
        Some(j) => j,
        None => return StatusCode::BAD_REQUEST.into_response(),
    };

    let decoding_key = match DecodingKey::from_jwk(jwk) {
        Ok(k) => k,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let mut validation = Validation::new(header.alg);
    validation.set_issuer(&["https://accounts.google.com"]);
    // In production, 'aud' MUST match your Google Client ID
    if let Ok(client_id) = std::env::var("GOOGLE_CLIENT_ID") {
        validation.set_audience(&[client_id]);
    }

    let token_data = match jsonwebtoken::decode::<RiscEvent>(
        token_str.as_bytes(),
        &decoding_key,
        &validation,
    ) {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!("RISC token verification failed: {}", e);
            return StatusCode::UNAUTHORIZED.into_response();
        }
    };

    let payload = token_data.claims;
    tracing::info!(
        "Verified RISC event: iss={}, sub={}, aud={}, iat={}, jti={}", 
        payload.iss, payload.sub, payload.aud, payload.iat, payload.jti
    );

    // 4. Handle specific events
    if payload.events.get("http://schemas.openid.net/event/risc/v1/token-revoked").is_some() 
       || payload.events.get("http://schemas.openid.net/event/risc/v1/account-disabled").is_some() {
        
        tracing::info!("RISC: Revoking all sessions for user sub: {}", payload.sub);
        // Implement your global user revocation logic here
    }

    StatusCode::ACCEPTED.into_response()
}
