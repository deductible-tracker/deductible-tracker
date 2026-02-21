use axum::{extract::{State, Json}, response::{IntoResponse, Json as AxumJson}, http::StatusCode};
use serde::Deserialize;
use crate::AppState;

#[derive(Deserialize)]
pub struct ValRequest {
    pub query: String,
}

pub async fn suggest(
    State(state): State<AppState>,
    Json(req): Json<ValRequest>
) -> impl IntoResponse {
    match crate::db::suggest_valuations(&state.db, &req.query).await {
        Ok(list) => {
            let out: Vec<_> = list.into_iter().map(|(name, min, max)| serde_json::json!({"name": name, "min": min, "max": max})).collect();
            AxumJson(serde_json::json!({"suggestions": out})).into_response()
        }
        Err(e) => {
            tracing::error!("Valuation suggestion error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Database Error").into_response()
        }
    }
}

pub async fn seed(
    State(state): State<AppState>
) -> impl IntoResponse {
    match crate::db::seed_valuations(&state.db).await {
        Ok(_) => (axum::http::StatusCode::OK, "seeded").into_response(),
        Err(e) => {
            tracing::error!("Valuation seed error: {}", e);
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "failed").into_response()
        }
    }
}
