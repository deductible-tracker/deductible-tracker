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
            filing_status: None,
            agi: None,
            marginal_tax_rate: None,
            itemize_deductions: None,
            provider: "local".to_string(),
        };
        let _ = crate::db::users::upsert_user_profile(&_state.db, &crate::db::models::UserProfileUpsert {
            user_id: user.id.clone(),
            email: user.email.clone(),
            name: user.name.clone(),
            provider: user.provider.clone(),
            filing_status: user.filing_status.clone(),
            agi: user.agi,
            marginal_tax_rate: user.marginal_tax_rate,
            itemize_deductions: user.itemize_deductions,
        }).await;
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

pub async fn logout(headers: HeaderMap) -> impl IntoResponse {
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

    if let Some(token) = extract_token_from_headers(&headers) {
        let _ = revoke_token_str(&token);
    }

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
    match crate::db::users::get_user_profile(&state.db, &user.id).await {
        Ok(Some((email, name, provider, filing_status, agi, marginal_tax_rate, itemize_deductions))) => Json(UserProfile {
            id: user.id,
            email,
            name,
            filing_status,
            agi,
            marginal_tax_rate,
            itemize_deductions,
            provider,
        }).into_response(),
        Ok(None) => {
            let filing_status = None;
            let agi = None;
            let marginal_tax_rate = None;
            let itemize_deductions = None;
            let _ = crate::db::users::upsert_user_profile(&state.db, &crate::db::models::UserProfileUpsert {
                user_id: user.id.clone(),
                email: user.email.clone(),
                name: user.name.clone(),
                provider: user.provider.clone(),
                filing_status: filing_status.clone(),
                agi,
                marginal_tax_rate,
                itemize_deductions,
            }).await;
            Json(UserProfile {
                id: user.id,
                email: user.email,
                name: user.name,
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
        .and_then(|value| if value.is_finite() && (0.0..=1.0).contains(&value) { Some(value) } else { None });
    let itemize_deductions = req.itemize_deductions;

    match crate::db::users::upsert_user_profile(&state.db, &crate::db::models::UserProfileUpsert {
        user_id: user.id.clone(),
        email: email.clone(),
        name: name.clone(),
        provider: user.provider.clone(),
        filing_status: filing_status.clone(),
        agi,
        marginal_tax_rate,
        itemize_deductions,
    }).await {
        Ok(_) => Json(UserProfile {
            id: user.id,
            email,
            name,
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
        jti: uuid::Uuid::new_v4().to_string(),
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
        if let Some(token) = auth_header.strip_prefix("Bearer ") {
            return Some(token.to_string());
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

