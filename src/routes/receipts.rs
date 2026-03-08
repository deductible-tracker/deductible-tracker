use axum::{
    extract::{State, Json, Query},
    response::{IntoResponse, Json as AxumJson},
    http::StatusCode,
};
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;
use uuid::Uuid;
use chrono::Datelike;
use crate::AppState;
use crate::auth::AuthenticatedUser;

use crate::db;
use crate::ocr;
use serde::Serialize;
use crate::db::models::NewReceipt;

const MAX_RECEIPT_SIZE_BYTES: i64 = 10 * 1024 * 1024;

fn allowed_ext_for_content_type(content_type: &str) -> Option<&'static str> {
    match content_type {
        "image/jpeg" => Some("jpg"),
        "image/png" => Some("png"),
        "application/pdf" => Some("pdf"),
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
    Json(req): Json<UploadRequest>
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

    let presigned_req = state.storage
        .presign_write(&key, Duration::from_secs(300))
        .await;

    match presigned_req {
        Ok(req) => {
             let resp_data = json!({
                "upload_url": req.uri().to_string(),
                "key": key,
                "expires_in": 300
            });
            (StatusCode::OK, AxumJson(resp_data)).into_response()
        },
        Err(e) => {
            tracing::error!("Storage Presign Error: {}", e);
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
    Json(req): Json<PresignReadRequest>
) -> impl IntoResponse {
    let key = req.key;
    let user_prefix = format!("receipts/{}/", user.id);
    if !key.starts_with(&user_prefix) {
        return (StatusCode::FORBIDDEN, "Forbidden").into_response();
    }

    let presigned_req = state.storage
        .presign_read(&key, Duration::from_secs(300))
        .await;

    match presigned_req {
        Ok(req) => {
            let resp_data = json!({
                "download_url": req.uri().to_string(),
                "key": key,
                "expires_in": 300
            });
            (StatusCode::OK, AxumJson(resp_data)).into_response()
        },
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
}

#[derive(Serialize)]
struct CreatedResponse {
    id: String,
}

pub async fn confirm_receipt(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(mut req): Json<ConfirmReceiptRequest>
) -> impl IntoResponse {
    req.donation_id = req.donation_id.trim().to_string();
    req.key = req.key.trim().to_string();

    let user_prefix = format!("receipts/{}/", user.id);
    if !req.key.starts_with(&user_prefix) {
        return (StatusCode::FORBIDDEN, "Forbidden").into_response();
    }

    if let Some(content_type) = req.content_type.as_deref() {
        let Some(expected_ext) = allowed_ext_for_content_type(content_type) else {
            return (StatusCode::BAD_REQUEST, "Unsupported content type").into_response();
        };

        if let Some(actual_ext) = req.key.rsplit('.').next() {
            if actual_ext != expected_ext {
                return (StatusCode::BAD_REQUEST, "File extension/content type mismatch").into_response();
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
    match db::receipts::list_receipts(&state.db, &user.id, params.donation_id.clone()).await {
        Ok(list) => AxumJson(serde_json::json!({ "receipts": list })).into_response(),
        Err(e) => {
            tracing::error!("DB Error listing receipts: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Database Error").into_response()
        }
    }
}

#[derive(Deserialize)]
pub struct OcrRequest {
    pub id: String,
}

pub async fn ocr_receipt(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(req): Json<OcrRequest>
) -> impl IntoResponse {
    // Fetch receipt
    match crate::db::receipts::get_receipt(&state.db, &user.id, &req.id).await {
        Ok(Some(receipt)) => {
            if let Some(content_type) = receipt.content_type.as_deref() {
                if allowed_ext_for_content_type(content_type).is_none() {
                    return (StatusCode::BAD_REQUEST, "Unsupported content type").into_response();
                }
            }
            if let Some(size) = receipt.size {
                if size <= 0 || size > MAX_RECEIPT_SIZE_BYTES {
                    return (StatusCode::BAD_REQUEST, "Invalid or oversized receipt").into_response();
                }
            }

            // Run local OCR using Tesseract (leptess). Requires tesseract installed on the host.
            match ocr::run_ocr(&state, &receipt.key).await {
                Ok((text, date_opt, amt_opt)) => {
                    let ocr_status = Some("done".to_string());
                    if let Err(e) = crate::db::receipts::set_receipt_ocr(&state.db, &receipt.id, &Some(text.clone()), &date_opt, &amt_opt, &ocr_status).await {
                        tracing::error!("Failed to set OCR: {}", e);
                        return (StatusCode::INTERNAL_SERVER_ERROR, "OCR Error").into_response();
                    }
                    // Also log audit
                    let audit_id = Uuid::new_v4().to_string();
                    let details = Some(format!("OCR run for receipt {}: date={:?} amount={:?}", receipt.id, date_opt, amt_opt));
                    if let Err(e) = crate::db::audit::log_audit(
                        &state.db,
                        &audit_id,
                        &user.id,
                        "ocr",
                        "receipts",
                        &Some(receipt.id.clone()),
                        &details,
                    )
                    .await
                    {
                        tracing::error!("Failed to write audit log for OCR run (audit_id={}): {}", audit_id, e);
                    }

                    (StatusCode::OK, AxumJson(serde_json::json!({ "status": "ocr_completed", "id": receipt.id }))).into_response()
                }
                Err(e) => {
                    tracing::error!("OCR run failed: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, "OCR Failure").into_response()
                }
            }
        }
        Ok(None) => (StatusCode::NOT_FOUND, "Receipt not found").into_response(),
        Err(e) => {
            tracing::error!("DB error fetching receipt for OCR: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Database Error").into_response()
        }
    }
}