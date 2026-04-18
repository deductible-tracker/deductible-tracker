use axum::extract::Multipart;
use std::io::{Read, Write, Cursor};

#[derive(Serialize, Deserialize)]
struct BackupData {
    profile: UserProfile,
    charities: Vec<crate::db::models::Charity>,
    donations: Vec<crate::db::models::Donation>,
    receipts: Vec<crate::db::models::Receipt>,
}

pub async fn delete_me(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    headers: HeaderMap,
) -> impl IntoResponse {
    let user_id = user.id.clone();

    // 1. Get receipt keys from DB before deleting data
    let keys = match crate::db::get_user_receipt_keys(&state.db, &user_id).await {
        Ok(k) => k,
        Err(e) => {
            tracing::error!("Failed to list receipt keys for deletion: {}", e);
            Vec::new()
        }
    };

    // 2. Delete files from storage (direct HTTP DELETE)
    let client = reqwest::Client::new();
    for key in keys {
        match crate::storage::presign_url(&state, "DELETE", &key, 300) {
            Ok(url) => {
                if let Err(e) = client.delete(&url).send().await {
                    tracing::error!("Failed to delete file {} from storage: {}", key, e);
                }
            }
            Err(e) => tracing::error!("Failed to presign delete for {}: {}", key, e),
        }
    }

    // 3. Delete data from DB
    if let Err(e) = crate::db::users::delete_user_data(&state.db, &user_id).await {
        tracing::error!("Failed to delete user data from DB: {}", e);
        return (StatusCode::INTERNAL_SERVER_ERROR, "Database Error").into_response();
    }

    // 4. Logout
    logout(headers).await.into_response()
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
    let dev_pass = match env::var("DEV_PASSWORD") {
        Ok(p) if !p.is_empty() && p != "password" => p,
        _ => {
            tracing::warn!("DEV_PASSWORD must be set to a non-default value");
            return (StatusCode::FORBIDDEN, "Dev login misconfigured").into_response();
        }
    };

    if payload.username == dev_user && payload.password == dev_pass {
        let existing_user = crate::db::users::get_user_profile_by_email(&_state.db, "dev@local").await;

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
                    is_encrypted: row.7,
                    encrypted_payload: row.8,
                    vault_credential_id: row.9,
                }
            },
            _ => {
                UserProfile {
                    id: "dev-1".to_string(),
                    email: "dev@local".to_string(),
                    name: "Developer".to_string(),
                    filing_status: None,
                    agi: None,
                    marginal_tax_rate: None,
                    itemize_deductions: None,
                    is_encrypted: None,
                    encrypted_payload: None,
                    vault_credential_id: None,
                    provider: "local".to_string(),
                }
            }
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
            is_encrypted: user.is_encrypted,
            encrypted_payload: user.encrypted_payload.clone(),
            vault_credential_id: user.vault_credential_id.clone(),
        }).await;
        match create_jwt(&user) {
            Ok(token) => {
                let cookie = build_auth_cookie(&token);
                let mut response = Json(AuthResponse { user }).into_response();
                if let Ok(header_value) = HeaderValue::from_str(&cookie) {
                    response.headers_mut().insert(header::SET_COOKIE, header_value);
                }
                        // Also emit a readable CSRF cookie for client-side X-CSRF-Token usage
                        let csrf_cookie = build_csrf_cookie(&token);
                        if let Ok(header_value) = HeaderValue::from_str(&csrf_cookie) {
                            response.headers_mut().append(header::SET_COOKIE, header_value);
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

    // Clear the readable CSRF cookie as well
    let csrf_clear = clear_csrf_cookie();
    if let Ok(header_value) = HeaderValue::from_str(&csrf_clear) {
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
        Ok(Some((email, name, provider, filing_status, agi, marginal_tax_rate, itemize_deductions, is_encrypted, encrypted_payload, vault_credential_id))) => Json(UserProfile {
            id: user.id,
            email,
            name,
            filing_status,
            agi,
            marginal_tax_rate,
            itemize_deductions,
            is_encrypted,
            encrypted_payload,
            vault_credential_id,
            provider,
        }).into_response(),
        Ok(None) => {
            let filing_status = None;
            let agi = None;
            let marginal_tax_rate = None;
            let itemize_deductions = None;
            let is_encrypted = None;
            let encrypted_payload = None;
            let vault_credential_id: Option<String> = None;
            let _ = crate::db::users::upsert_user_profile(&state.db, &crate::db::models::UserProfileUpsert {
                user_id: user.id.clone(),
                email: user.email.clone(),
                name: user.name.clone(),
                provider: user.provider.clone(),
                filing_status: filing_status.clone(),
                agi,
                marginal_tax_rate,
                itemize_deductions,
                is_encrypted,
                encrypted_payload: encrypted_payload.clone(),
                vault_credential_id: vault_credential_id.clone(),
            }).await;
            Json(UserProfile {
                id: user.id,
                email: user.email,
                name: user.name,
                filing_status,
                agi,
                marginal_tax_rate,
                itemize_deductions,
                is_encrypted,
                encrypted_payload,
                vault_credential_id,
                provider: user.provider,
            }).into_response()
        }
        Err(e) => {
            tracing::error!("Failed loading profile, falling back to token claims: {}", e);
            Json(UserProfile {
                id: user.id,
                email: user.email,
                name: user.name,
                filing_status: None,
                agi: None,
                marginal_tax_rate: None,
                itemize_deductions: None,
                is_encrypted: None,
                encrypted_payload: None,
                vault_credential_id: None,
                provider: user.provider,
            }).into_response()
        }
    }
}

pub async fn get_config(headers: HeaderMap) -> impl IntoResponse {
    let allow_dev_login = std::env::var("ALLOW_DEV_LOGIN")
        .map(|v| v.to_lowercase() == "true")
        .unwrap_or(false);
    // Determine whether Google OAuth is enabled (listed in OAUTH_PROVIDERS
    // and has a client ID configured).
    let oauth_providers = std::env::var("OAUTH_PROVIDERS").unwrap_or_default();
    let providers: Vec<String> = oauth_providers
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    let google_enabled = providers.contains(&"google".to_string()) && std::env::var("GOOGLE_CLIENT_ID").is_ok();
    let google_client_id = std::env::var("GOOGLE_CLIENT_ID").ok();

    // Generate or retrieve OAuth state for CSRF protection
    let existing_state = extract_cookie_by_name(&headers, "oauth_state");
    let (state_token, set_cookie_header) = if let Some(state) = existing_state {
        (state, None)
    } else {
        let new_state = create_state_token("google").unwrap_or_default();
        let cookie = build_oauth_state_cookie(&new_state);
        (new_state, Some(cookie))
    };

    let mut response = Json(serde_json::json!({
        "allow_dev_login": allow_dev_login,
        "google_enabled": google_enabled,
        "google_client_id": google_client_id,
        "oauth_state": state_token
    })).into_response();

    if let Some(cookie) = set_cookie_header {
        if let Ok(hv) = HeaderValue::from_str(&cookie) {
            response.headers_mut().append(header::SET_COOKIE, hv);
        }
    }

    response
}

pub async fn update_me(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(req): Json<UpdateMeRequest>,
) -> impl IntoResponse {
    // 1. Fetch existing profile to fill in missing/null fields
    let existing = match crate::db::users::get_user_profile(&state.db, &user.id).await {
        Ok(Some(row)) => Some(row),
        _ => None,
    };

    let email = req.email.map(|s| s.trim().to_string())
        .or_else(|| existing.as_ref().map(|r| r.0.clone()))
        .unwrap_or_else(|| user.email.clone());

    let name = req.name.map(|s| s.trim().to_string())
        .or_else(|| existing.as_ref().map(|r| r.1.clone()))
        .unwrap_or_else(|| user.name.clone());

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
    }).or_else(|| existing.as_ref().and_then(|r| r.3.clone()));

    let agi = req.agi
        .and_then(|value| if value.is_finite() && value >= 0.0 { Some(value) } else { None })
        .or_else(|| existing.as_ref().and_then(|r| r.4));

    let marginal_tax_rate = req.marginal_tax_rate
        .and_then(|value| if value.is_finite() && (0.0..=1.0).contains(&value) { Some(value) } else { None })
        .or_else(|| existing.as_ref().and_then(|r| r.5));

    let itemize_deductions = req.itemize_deductions.or_else(|| existing.as_ref().and_then(|r| r.6));
    let is_encrypted = req.is_encrypted.or_else(|| existing.as_ref().and_then(|r| r.7));
    let encrypted_payload = req.encrypted_payload.or_else(|| existing.as_ref().and_then(|r| r.8.clone()));
    let vault_credential_id = req.vault_credential_id.or_else(|| existing.as_ref().and_then(|r| r.9.clone()));

    match crate::db::users::upsert_user_profile(&state.db, &crate::db::models::UserProfileUpsert {
        user_id: user.id.clone(),
        email: email.clone(),
        name: name.clone(),
        provider: user.provider.clone(),
        filing_status: filing_status.clone(),
        agi,
        marginal_tax_rate,
        itemize_deductions,
        is_encrypted,
        encrypted_payload: encrypted_payload.clone(),
        vault_credential_id: vault_credential_id.clone(),
    }).await {
        Ok(_) => Json(UserProfile {
            id: user.id,
            email,
            name,
            filing_status,
            agi,
            marginal_tax_rate,
            itemize_deductions,
            is_encrypted,
            encrypted_payload,
            vault_credential_id,
            provider: user.provider,
        }).into_response(),
        Err(e) => {
            tracing::error!("Failed saving profile: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Database Error").into_response()
        }
    }
}

pub async fn export_me(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> impl IntoResponse {
    let user_id = user.id.clone();

    // 1. Fetch metadata
    let profile_res = crate::db::users::get_user_profile(&state.db, &user_id).await;
    let profile = match profile_res {
        Ok(Some((email, name, provider, filing_status, agi, marginal_tax_rate, itemize_deductions, is_encrypted, encrypted_payload, vault_credential_id))) => {
            UserProfile { id: user_id.clone(), email, name, provider, filing_status, agi, marginal_tax_rate, itemize_deductions, is_encrypted, encrypted_payload, vault_credential_id }
        }
        _ => return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load profile").into_response(),
    };

    let charities = crate::db::charities::list_charities(&state.db, &user_id).await.unwrap_or_default();
    let donations = crate::db::donations::list_donations(&state.db, &user_id, None).await.unwrap_or_default();
    let receipts = crate::db::receipts::list_receipts(&state.db, &user_id, None).await.unwrap_or_default();

    let backup_data = BackupData { profile, charities, donations, receipts };
    let json_data = serde_json::to_vec(&backup_data).unwrap_or_default();

    // 2. Setup streaming pipe
    let (reader, mut writer) = tokio::io::duplex(64 * 1024);

    tokio::spawn(async move {
        // ZipArchive needs a Seekable writer, which duplex is not for ZipWriter 0.8+
        // But ZipWriter can use a non-seekable writer if we don't need central directory 
        // updates until finish. However, standard ZipWriter needs Seek.
        // For truly streaming ZIP we typically use a wrapper or a seekable memory buffer.
        // Since we need to support multi-GB, we'll use a temporary file for the ZIP creation
        // to stay off the heap, and stream that file to the response.
        
        let mut temp = match tempfile::tempfile() {
            Ok(f) => f,
            Err(_) => return,
        };

        {
            let mut zip = zip::ZipWriter::new(&mut temp);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);

            let _ = zip.start_file("data.json", options);
            let _ = zip.write_all(&json_data);

            let client = reqwest::Client::new();
            for receipt in &backup_data.receipts {
                let url = match crate::storage::presign_url(&state, "GET", &receipt.key, 300) {
                    Ok(url) => url,
                    Err(e) => {
                        tracing::error!("Backup: Failed to presign {}: {}", receipt.key, e);
                        continue;
                    }
                };

                if let Ok(resp) = client.get(&url).send().await {
                    let status = resp.status();
                    if status.is_success() {
                        let mut stream = resp.bytes_stream();
                        let file_name = format!("receipts/{}", receipt.file_name.as_deref().unwrap_or(&receipt.id));
                        if zip.start_file(&file_name, options).is_ok() {
                            while let Some(chunk_res) = futures::StreamExt::next(&mut stream).await {
                                if let Ok(chunk) = chunk_res {
                                    let _ = zip.write_all(chunk.as_ref());
                                }
                            }
                            tracing::info!("Backup: Added {} to zip", file_name);
                        }
                    } else {
                        tracing::error!("Backup: Failed to fetch receipt from storage: {} (Status: {})", url, status);
                    }
                } else {
                    tracing::error!("Backup: Internal error fetching receipt: {}", url);
                }
            }
            if let Err(e) = zip.finish() {
                tracing::error!("Backup: Failed to finish zip: {}", e);
            }
        }

        // Now stream the temp file back through the duplex pipe
        use std::io::{Seek, SeekFrom};
        let _ = temp.seek(SeekFrom::Start(0));
        let mut tokio_temp = tokio::fs::File::from_std(temp);
        let _ = tokio::io::copy(&mut tokio_temp, &mut writer).await;
    });

    let filename = format!("backup-{}-{}.zip", user_id, Utc::now().format("%Y%m%d"));
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/zip"));
    let disp = format!("attachment; filename=\"{}\"", filename);
    if let Ok(h) = HeaderValue::from_str(&disp) {
        headers.insert(header::CONTENT_DISPOSITION, h);
    }

    let stream = tokio_util::io::ReaderStream::new(reader);
    (headers, axum::body::Body::from_stream(stream)).into_response()
}

pub async fn import_me(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut data = Vec::new();
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("file") {
            if let Ok(bytes) = field.bytes().await {
                data = bytes.to_vec();
            }
            break;
        }
    }

    const MAX_UPLOAD_SIZE: usize = 100 * 1024 * 1024; // 100 MB
    if data.is_empty() {
        return (StatusCode::BAD_REQUEST, "Missing backup file").into_response();
    }
    if data.len() > MAX_UPLOAD_SIZE {
        return (StatusCode::BAD_REQUEST, "Backup file too large (max 100 MB)").into_response();
    }

    let mut archive = match zip::ZipArchive::new(Cursor::new(data)) {
        Ok(z) => z,
        Err(e) => {
            tracing::error!("Invalid ZIP: {}", e);
            return (StatusCode::BAD_REQUEST, "Invalid backup file").into_response();
        }
    };

    // 1. Read data.json to know what metadata we're restoring
    let backup: BackupData = {
        let mut data_file = match archive.by_name("data.json") {
            Ok(f) => f,
            Err(_) => return (StatusCode::BAD_REQUEST, "Missing data.json in backup").into_response(),
        };
        const MAX_JSON_SIZE: u64 = 10 * 1024 * 1024; // 10 MB
        if data_file.size() > MAX_JSON_SIZE {
            return (StatusCode::BAD_REQUEST, "data.json too large").into_response();
        }
        let mut json_buf = Vec::new();
        if data_file.read_to_end(&mut json_buf).is_err() {
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to read data.json").into_response();
        }
        match serde_json::from_slice(&json_buf) {
            Ok(b) => b,
            Err(e) => {
                tracing::error!("JSON error: {}", e);
                return (StatusCode::BAD_REQUEST, "Invalid data.json").into_response();
            }
        }
    };

    // 2. Import metadata into DB
    let profile_upsert = UserProfileUpsert {
        user_id: user.id.clone(),
        email: user.email.clone(),
        name: user.name.clone(),
        provider: user.provider.clone(),
        filing_status: backup.profile.filing_status,
        agi: backup.profile.agi,
        marginal_tax_rate: backup.profile.marginal_tax_rate,
        itemize_deductions: backup.profile.itemize_deductions,
        is_encrypted: backup.profile.is_encrypted,
        encrypted_payload: backup.profile.encrypted_payload,
        vault_credential_id: backup.profile.vault_credential_id,
    };

    if let Err(e) = crate::db::users::import_data(
        &state.db,
        &user.id,
        &profile_upsert,
        &backup.charities,
        &backup.donations,
        &backup.receipts,
    ).await {
        tracing::error!("Import DB error: {}", e);
        return (StatusCode::INTERNAL_SERVER_ERROR, "Database Restore Error").into_response();
    }

    // 3. Restore files to storage by streaming them one-by-one
    let client = reqwest::Client::new();
    for receipt in &backup.receipts {
        let file_name = format!("receipts/{}", receipt.key.split('/').next_back().unwrap_or(&receipt.id));
        if let Ok(mut zip_file) = archive.by_name(&file_name) {
            const MAX_FILE_SIZE: u64 = 50 * 1024 * 1024; // 50 MB
            if zip_file.size() > MAX_FILE_SIZE {
                tracing::warn!("Skipping oversized file in backup: {} ({} bytes)", file_name, zip_file.size());
                continue;
            }
            let mut file_data = Vec::new();
            if zip_file.read_to_end(&mut file_data).is_ok() {
                match crate::storage::presign_url(&state, "PUT", &receipt.key, 300) {
                    Ok(url) => {
                        if let Err(e) = client.put(url).body(file_data).send().await {
                            tracing::error!("Failed to restore file {} to storage: {}", receipt.key, e);
                        }
                    }
                    Err(e) => tracing::error!("Failed to presign restore for {}: {}", receipt.key, e),
                }
            }
        }
    }

    (StatusCode::OK, "Restore completed").into_response()
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
