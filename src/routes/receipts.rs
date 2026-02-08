use axum::{
    extract::{State, Json},
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