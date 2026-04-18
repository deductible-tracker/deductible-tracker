use crate::auth::AuthenticatedUser;
use crate::AppState;
use axum::{
    extract::{Json, Query, State},
    http::{StatusCode, HeaderMap},
    response::{IntoResponse, Json as AxumJson},
};
use base64::Engine;
use chrono::Datelike;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::db;
use crate::db::models::NewReceipt;
use crate::ocr;
use serde::Serialize;

const MAX_RECEIPT_SIZE_BYTES: i64 = 10 * 1024 * 1024;
const PRESIGN_EXPIRATION_SECS: u64 = 300;

fn allowed_ext_for_content_type(content_type: &str) -> Option<&'static str> {
    match content_type {
        // Images
        "image/jpeg" => Some("jpg"),
        "image/png" => Some("png"),
        "image/avif" => Some("avif"),
        "image/tiff" => Some("tiff"),
        "image/gif" => Some("gif"),
        "image/heic" => Some("heic"),
        "image/heif" => Some("heif"),
        "image/bmp" => Some("bmp"),
        "image/webp" => Some("webp"),
        // Documents
        "application/pdf" => Some("pdf"),
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => Some("docx"),
        "application/msword" => Some("doc"),
        "application/vnd.openxmlformats-officedocument.presentationml.presentation" => Some("pptx"),
        "application/vnd.ms-powerpoint" => Some("ppt"),
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => Some("xlsx"),
        "text/csv" => Some("csv"),
        "text/plain" => Some("txt"),
        "application/epub+zip" => Some("epub"),
        "application/xml" => Some("xml"),
        "application/rtf" => Some("rtf"),
        "application/vnd.oasis.opendocument.text" => Some("odt"),
        "application/x-bibtex" => Some("bib"),
        "application/x-fictionbook+xml" => Some("fb2"),
        "application/x-ipynb+json" => Some("ipynb"),
        "application/x-tex" => Some("tex"),
        "text/x-opml" => Some("opml"),
        "text/troff" => Some("1"),
        _ => None,
    }
}

#[derive(Deserialize)]
pub struct UploadRequest {
    file_type: String, // e.g., "image/jpeg"
    _donation_id: Option<String>,
}

pub async fn generate_upload_url(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(req): Json<UploadRequest>,
) -> impl IntoResponse {
    let user_id = user.id;

    let ext = match allowed_ext_for_content_type(req.file_type.as_str()) {
        Some(ext) => ext,
        None => return (StatusCode::BAD_REQUEST, "Unsupported file type").into_response(),
    };

    let now = chrono::Utc::now();
    let year = now.year();
    let file_id = Uuid::new_v4();
    let key = format!("receipts/{}/{}/{}.{}", user_id, year, file_id, ext);

    match crate::storage::presign_url(&state, "PUT", &key, PRESIGN_EXPIRATION_SECS) {
        Ok(upload_url) => {
            let resp_data = json!({
                "upload_url": upload_url,
                "key": key,
                "expires_in": PRESIGN_EXPIRATION_SECS
            });
            (StatusCode::OK, AxumJson(resp_data)).into_response()
        }
        Err(e) => {
            tracing::error!("Storage Presign Write Error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Storage Error").into_response()
        }
    }
}

#[derive(Deserialize)]
pub struct PresignReadRequest {
    key: String,
}

pub async fn generate_read_url(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(req): Json<PresignReadRequest>,
) -> impl IntoResponse {
    let key = crate::storage::normalize_object_key(&state.bucket_name, &req.key);
    let user_prefix = crate::storage::user_receipt_prefix(&user.id);
    if !key.starts_with(&user_prefix) {
        return (StatusCode::FORBIDDEN, "Forbidden").into_response();
    }

    match crate::storage::presign_url(&state, "GET", &key, 300) {
        Ok(download_url) => {
            let resp_data = json!({
                "download_url": download_url,
                "key": key,
                "expires_in": 300
            });
            (StatusCode::OK, AxumJson(resp_data)).into_response()
        }
        Err(e) => {
            tracing::error!("Storage Presign Read Error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Storage Error").into_response()
        }
    }
}

#[derive(Deserialize)]
pub struct ConfirmReceiptRequest {
    pub key: String,
    pub file_name: Option<String>,
    pub content_type: Option<String>,
    pub size: Option<i64>,
    pub donation_id: String,
    pub is_encrypted: Option<bool>,
    pub encrypted_payload: Option<String>,
}

#[derive(Serialize)]
struct CreatedResponse {
    id: String,
}

pub async fn confirm_receipt(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(mut req): Json<ConfirmReceiptRequest>,
) -> impl IntoResponse {
    req.donation_id = req.donation_id.trim().to_string();
    req.key = crate::storage::normalize_object_key(&state.bucket_name, &req.key);

    let user_prefix = crate::storage::user_receipt_prefix(&user.id);
    if !req.key.starts_with(&user_prefix) {
        return (StatusCode::FORBIDDEN, "Forbidden").into_response();
    }

    if let Some(content_type) = req.content_type.as_deref() {
        let Some(expected_ext) = allowed_ext_for_content_type(content_type) else {
            return (StatusCode::BAD_REQUEST, "Unsupported content type").into_response();
        };

        if let Some(actual_ext) = req.key.rsplit('.').next() {
            if actual_ext != expected_ext {
                return (
                    StatusCode::BAD_REQUEST,
                    "File extension/content type mismatch",
                )
                    .into_response();
            }
        }
    }

    if let Some(size) = req.size {
        if size <= 0 || size > MAX_RECEIPT_SIZE_BYTES {
            return (StatusCode::BAD_REQUEST, "Invalid or oversized receipt").into_response();
        }
    }

    match db::user_owns_donation(&state.db, &user.id, &req.donation_id).await {
        Ok(true) => {}
        Ok(false) => return (StatusCode::FORBIDDEN, "Donation not found for user").into_response(),
        Err(e) => {
            tracing::error!("DB Error validating donation ownership: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Database Error").into_response();
        }
    }

    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now();

    let new_receipt = NewReceipt {
        id: id.clone(),
        donation_id: req.donation_id.clone(),
        key: req.key.clone(),
        file_name: req.file_name.clone(),
        content_type: req.content_type.clone(),
        size: req.size,
        is_encrypted: req.is_encrypted,
        encrypted_payload: req.encrypted_payload.clone(),
        created_at: now,
    };

    if let Err(e) = db::receipts::add_receipt(&state.db, &new_receipt).await {
        tracing::error!("DB Error adding receipt: {}", e);
        return (StatusCode::INTERNAL_SERVER_ERROR, "Database Error").into_response();
    }

    (StatusCode::CREATED, AxumJson(CreatedResponse { id })).into_response()
}

#[derive(Deserialize)]
pub struct ListReceiptsParams {
    pub donation_id: Option<String>,
}

pub async fn list_receipts(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Query(params): Query<ListReceiptsParams>,
) -> impl IntoResponse {
    match db::receipts::list_receipt_summaries(&state.db, &user.id, params.donation_id.clone())
        .await
    {
        Ok(list) => AxumJson(serde_json::json!({ "receipts": list })).into_response(),
        Err(e) => {
            tracing::error!("DB Error listing receipts: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Database Error").into_response()
        }
    }
}

#[derive(Deserialize)]
pub struct OcrRequest {
    pub id: Option<String>,
    pub key: Option<String>,
    pub content_type: Option<String>,
    pub size: Option<i64>,
}

#[derive(Serialize)]
pub struct OcrResponse {
    pub status: String,
    pub id: Option<String>,
    pub ocr_text: Option<String>,
    pub ocr_date: Option<chrono::NaiveDate>,
    pub ocr_amount_usd: Option<f64>,
    pub suggestion: Option<ocr::DonationReceiptSuggestion>,
    pub warning: Option<String>,
}

pub async fn ocr_receipt(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    headers: HeaderMap,
    Json(req): Json<OcrRequest>,
) -> impl IntoResponse {
    let (receipt_id, receipt_key, content_type, size, is_encrypted) = if let Some(receipt_id) = req.id.clone() {
        match crate::db::receipts::get_receipt(&state.db, &user.id, &receipt_id).await {
            Ok(Some(receipt)) => (
                Some(receipt.id),
                receipt.key,
                receipt.content_type,
                receipt.size,
                receipt.is_encrypted.unwrap_or(false),
            ),
            Ok(None) => return (StatusCode::NOT_FOUND, "Receipt not found").into_response(),
            Err(e) => {
                tracing::error!("DB error fetching receipt for OCR: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, "Database Error").into_response();
            }
        }
    } else if let Some(key) = req.key.clone() {
        let normalized_key = crate::storage::normalize_object_key(&state.bucket_name, &key);
        let user_prefix = crate::storage::user_receipt_prefix(&user.id);
        if !normalized_key.starts_with(&user_prefix) {
            return (StatusCode::FORBIDDEN, "Forbidden").into_response();
        }
        (None, normalized_key, req.content_type.clone(), req.size, false)
    } else {
        return (StatusCode::BAD_REQUEST, "Receipt id or key is required").into_response();
    };

    if let Some(content_type_value) = content_type.as_deref() {
        if allowed_ext_for_content_type(content_type_value).is_none() {
            return (StatusCode::BAD_REQUEST, "Unsupported content type").into_response();
        }
    }
    if let Some(size_value) = size {
        if size_value <= 0 || size_value > MAX_RECEIPT_SIZE_BYTES {
            return (StatusCode::BAD_REQUEST, "Invalid or oversized receipt").into_response();
        }
    }

    let analysis_result = if is_encrypted {
        let Some(vault_key) = headers.get("X-Vault-Key").and_then(|h| h.to_str().ok()) else {
            return (StatusCode::BAD_REQUEST, "Vault key required for encrypted receipt").into_response();
        };

        // Download and decrypt transiently
        let download_url = match crate::storage::presign_url(&state, "GET", &receipt_key, 300) {
            Ok(url) => url,
            Err(e) => {
                tracing::error!("Failed to presign URL for transient OCR: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, "Storage Error").into_response();
            }
        };

        let client = reqwest::Client::new();
        let bytes = match client.get(download_url).send().await {
            Ok(resp) if resp.status().is_success() => match resp.bytes().await {
                Ok(b) => b,
                Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to read receipt bytes: {}", e)).into_response(),
            },
            Ok(resp) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to download receipt: {}", resp.status())).into_response(),
            Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Download request failed: {}", e)).into_response(),
        };

        let encrypted_b64 = base64::prelude::BASE64_STANDARD.encode(&bytes);
        let decrypted_bytes = match crate::auth::decrypt_payload(vault_key, &encrypted_b64) {
            Ok(b) => b,
            Err(e) => {
                tracing::error!("Failed to decrypt receipt for OCR: {}", e);
                return (StatusCode::UNAUTHORIZED, "Invalid vault key or corrupted data").into_response();
            }
        };

        ocr::analyze_receipt_bytes(&state, &decrypted_bytes, content_type.as_deref()).await
    } else {
        ocr::analyze_receipt(&state, &receipt_key, content_type.as_deref()).await
    };

    match analysis_result {
        Ok(analysis) => {
            if let Some(receipt_id_value) = receipt_id.clone() {
                let ocr_text = analysis.ocr_text.clone();
                let ocr_date = analysis.ocr_date;
                let ocr_amount = analysis.ocr_amount_cents.map(|value| value as f64 / 100.0);
                let ocr_status = Some(analysis.ocr_status.clone());
                if let Err(e) = crate::db::receipts::set_receipt_ocr(
                    &state.db,
                    &receipt_id_value,
                    &ocr_text,
                    &ocr_date,
                    &ocr_amount,
                    &ocr_status,
                )
                .await
                {
                    tracing::error!("Failed to persist OCR analysis: {}", e);
                    return (
                        StatusCode::OK,
                        AxumJson(OcrResponse {
                            status: "failed".to_string(),
                            id: receipt_id,
                            ocr_text: None,
                            ocr_date: None,
                            ocr_amount_usd: None,
                            suggestion: None,
                            warning: Some("Unable to prepopulate donation data".to_string()),
                        }),
                    )
                        .into_response();
                }

                let audit_id = Uuid::new_v4().to_string();
                let details = Some(format!(
                    "Receipt analysis for {}: status={} suggestion={}",
                    receipt_id_value,
                    analysis.ocr_status,
                    analysis.suggestion.is_some()
                ));
                if let Err(e) = crate::db::audit::log_audit(
                    &state.db,
                    &audit_id,
                    &user.id,
                    "ocr",
                    "receipts",
                    &Some(receipt_id_value.clone()),
                    &details,
                )
                .await
                {
                    tracing::error!(
                        "Failed to write audit log for OCR run (audit_id={}): {}",
                        audit_id,
                        e
                    );
                }
            }

            let response = OcrResponse {
                status: analysis.ocr_status,
                id: receipt_id,
                ocr_text: analysis.ocr_text,
                ocr_date: analysis.ocr_date,
                ocr_amount_usd: analysis.ocr_amount_cents.map(|value| value as f64 / 100.0),
                suggestion: analysis.suggestion,
                warning: analysis.warning,
            };
            (StatusCode::OK, AxumJson(response)).into_response()
        }
        Err(e) => {
            tracing::error!("OCR run failed: {}", e);
            (
                StatusCode::OK,
                AxumJson(OcrResponse {
                    status: "failed".to_string(),
                    id: receipt_id,
                    ocr_text: None,
                    ocr_date: None,
                    ocr_amount_usd: None,
                    suggestion: None,
                    warning: Some("Unable to prepopulate donation data".to_string()),
                }),
            )
                .into_response()
        }
    }
}
