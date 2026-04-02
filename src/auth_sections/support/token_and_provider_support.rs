fn build_auth_cookie(token: &str) -> String {
    let secure = env::var("RUST_ENV").unwrap_or_else(|_| "development".to_string()) == "production";
    let mut cookie = format!(
        "{}={}; HttpOnly; SameSite=Strict; Path=/; Max-Age=14400",
        AUTH_COOKIE_NAME,
        token
    );
    if secure {
        cookie.push_str("; Secure");
    }
    cookie
}

fn build_oauth_state_cookie(state_token: &str) -> String {
    let secure = env::var("RUST_ENV").unwrap_or_else(|_| "development".to_string()) == "production";
    let mut cookie = format!(
        "oauth_state={}; HttpOnly; SameSite=Lax; Path=/; Max-Age=600",
        state_token
    );
    if secure {
        cookie.push_str("; Secure");
    }
    cookie
}

fn build_pkce_verifier_cookie(verifier: &str) -> String {
    let secure = env::var("RUST_ENV").unwrap_or_else(|_| "development".to_string()) == "production";
    let mut cookie = format!(
        "pkce_verifier={}; HttpOnly; SameSite=Lax; Path=/; Max-Age=600",
        verifier
    );
    if secure {
        cookie.push_str("; Secure");
    }
    cookie
}

fn clear_pkce_verifier_cookie() -> String {
    let secure = env::var("RUST_ENV").unwrap_or_else(|_| "development".to_string()) == "production";
    let mut cookie = "pkce_verifier=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0; Expires=Thu, 01 Jan 1970 00:00:00 GMT".to_string();
    if secure {
        cookie.push_str("; Secure");
    }
    cookie
}

fn clear_oauth_state_cookie() -> String {
    let secure = env::var("RUST_ENV").unwrap_or_else(|_| "development".to_string()) == "production";
    let mut cookie = "oauth_state=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0; Expires=Thu, 01 Jan 1970 00:00:00 GMT".to_string();
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

pub struct ProviderProfile {
    pub id: String,
    pub email: String,
    pub name: String,
}

pub async fn fetch_user_profile(userinfo_url: &str, access_token: &str) -> anyhow::Result<ProviderProfile> {
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
