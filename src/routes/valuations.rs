use crate::AppState;
use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::{IntoResponse, Json as AxumJson},
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ValRequest {
    pub query: String,
}

pub async fn suggest(
    State(state): State<AppState>,
    Json(req): Json<ValRequest>,
) -> impl IntoResponse {
    match crate::db::valuations::suggest_valuations(&state.db, &req.query).await {
        Ok(list) => {
            let out: Vec<_> = list
                .into_iter()
                .map(|(name, min, max)| serde_json::json!({"name": name, "min": min, "max": max}))
                .collect();
            AxumJson(serde_json::json!({"suggestions": out})).into_response()
        }
        Err(e) => {
            tracing::error!("Valuation suggestion error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Database Error").into_response()
        }
    }
}

pub async fn seed(State(state): State<AppState>) -> impl IntoResponse {
    match crate::db::valuations::seed_valuations(&state.db).await {
        Ok(_) => (axum::http::StatusCode::OK, "seeded").into_response(),
        Err(e) => {
            tracing::error!("Valuation seed error: {}", e);
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "failed").into_response()
        }
    }
}

pub async fn tree(State(state): State<AppState>) -> impl IntoResponse {
    match crate::db::valuations::list_valuation_tree(&state.db).await {
        Ok(t) => AxumJson(t).into_response(),
        Err(e) => {
            tracing::error!("Valuation tree error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Database Error").into_response()
        }
    }
}
