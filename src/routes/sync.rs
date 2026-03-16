use axum::{
    extract::{State, Json},
    response::IntoResponse,
    http::StatusCode,
};
use crate::AppState;
use crate::auth::AuthenticatedUser;
use crate::db;
use crate::db::models::BatchSyncRequest;

pub async fn batch_sync(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(req): Json<BatchSyncRequest>,
) -> impl IntoResponse {
    match db::batch_sync(&state.db, &user.id, req).await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(e) => {
            tracing::error!("Batch sync error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Database Error").into_response()
        }
    }
}
