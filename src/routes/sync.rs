use axum::{
    extract::{State, Json},
    response::IntoResponse,
    http::StatusCode,
};
use crate::AppState;
use crate::auth::AuthenticatedUser;
use crate::db;
use crate::db::models::{BatchSyncRequest, DonationSyncItem, ReceiptSyncItem};

fn validate_donation_sync_item(item: &DonationSyncItem) -> Result<(), &'static str> {
    let action = item.action.trim();
    if !matches!(action, "create" | "update" | "delete") {
        return Err("Invalid donation sync action");
    }

    if item.id.trim().is_empty() {
        return Err("Donation id is required for sync");
    }

    if action == "delete" {
        return Ok(());
    }

    if item.charity_id.trim().is_empty() {
        return Err("Donation charity_id is required for sync");
    }

    if let Some(amount) = item.amount {
        if amount < 0.0 {
            return Err("Donation amount cannot be negative");
        }
    }

    let category = item
        .category
        .as_deref()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "money".to_string());

    if category == "money" && item.amount.unwrap_or(0.0) <= 0.0 {
        return Err("Money donations require a positive amount");
    }

    Ok(())
}

fn validate_receipt_sync_item(item: &ReceiptSyncItem) -> Result<(), &'static str> {
    if item.action.trim() != "create" {
        return Err("Invalid receipt sync action");
    }

    if item.id.trim().is_empty() {
        return Err("Receipt id is required for sync");
    }

    if item.donation_id.trim().is_empty() {
        return Err("Receipt donation_id is required for sync");
    }

    if item.key.trim().is_empty() {
        return Err("Receipt key is required for sync");
    }

    Ok(())
}

fn validate_batch_sync_request(req: &BatchSyncRequest) -> Result<(), &'static str> {
    for donation in &req.donations {
        validate_donation_sync_item(donation)?;
    }

    for receipt in &req.receipts {
        validate_receipt_sync_item(receipt)?;
    }

    Ok(())
}

pub async fn batch_sync(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(req): Json<BatchSyncRequest>,
) -> impl IntoResponse {
    if let Err(message) = validate_batch_sync_request(&req) {
        return (StatusCode::BAD_REQUEST, message).into_response();
    }

    match db::batch_sync(&state.db, &user.id, req).await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(e) => {
            tracing::error!("Batch sync error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Database Error").into_response()
        }
    }
}
