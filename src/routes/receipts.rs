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

    let ext = match req.file_type.as_str() {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "application/pdf" => "pdf",
        _ => return (StatusCode::BAD_REQUEST, "Unsupported file type").into_response(),
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
    Json(req): Json<ConfirmReceiptRequest>
) -> impl IntoResponse {
    let user_prefix = format!("receipts/{}/", user.id);
    if !req.key.starts_with(&user_prefix) {
        return (StatusCode::FORBIDDEN, "Forbidden").into_response();
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

    if let Err(e) = db::add_receipt(
        &state.db,
        &id,
        &req.donation_id,
        &req.key,
        &req.file_name,
        &req.content_type,
        &req.size,
        now,
    ).await {
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
    match db::list_receipts(&state.db, &user.id, params.donation_id.clone()).await {
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
    match crate::db::get_receipt(&state.db, &user.id, &req.id).await {
        Ok(Some(receipt)) => {
            // Run local OCR using Tesseract (leptess). Requires tesseract installed on the host.
            match ocr::run_ocr(&state, &receipt.key).await {
                Ok((text, date_opt, amt_opt)) => {
                    let ocr_status = Some("done".to_string());
                    if let Err(e) = crate::db::set_receipt_ocr(&state.db, &receipt.id, &Some(text.clone()), &date_opt, &amt_opt, &ocr_status).await {
                        tracing::error!("Failed to set OCR: {}", e);
                        return (StatusCode::INTERNAL_SERVER_ERROR, "OCR Error").into_response();
                    }
                    // Also log audit
                    let audit_id = Uuid::new_v4().to_string();
                    let details = Some(format!("OCR run for receipt {}: date={:?} amount={:?}", receipt.id, date_opt, amt_opt));
                    let _ = crate::db::log_audit(&state.db, &audit_id, &user.id, "ocr", "receipts", &Some(receipt.id.clone()), &details).await;

                    (StatusCode::OK, AxumJson(serde_json::json!({ "status": "ocr_completed", "id": receipt.id }))).into_response()
                }
                Err(e) => {
                    tracing::error!("OCR run failed: {}", e);
                    return (StatusCode::INTERNAL_SERVER_ERROR, "OCR Failure").into_response();
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