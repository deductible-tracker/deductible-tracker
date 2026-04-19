use crate::auth::AuthenticatedUser;
use crate::db;
use crate::db::models::{BatchSyncRequest, DonationSyncItem, ReceiptSyncItem};
use crate::AppState;
use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::IntoResponse,
};

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

    // Encrypted donations still require a valid charity_id for DB foreign key constraints.
    // The client should have created an encrypted charity first if needed.
    if item.charity_id.trim().is_empty() {
        return Err("Donation charity_id is required even for encrypted sync");
    }

    // Skip further detailed validation for encrypted items (amount, category, etc)
    if item.is_encrypted.unwrap_or(false) {
        return Ok(());
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::DonationSyncItem;

    #[test]
    fn test_validate_encrypted_donation_sync_item() {
        let item = DonationSyncItem {
            action: "create".to_string(),
            id: "test-id".to_string(),
            date: None,
            year: None,
            category: None,
            amount: None,
            charity_id: "char-123".to_string(),
            notes: None,
            is_encrypted: Some(true),
            encrypted_payload: Some("payload".to_string()),
            updated_at: None,
        };

        // Should pass with charity_id even if other fields are missing
        assert!(validate_donation_sync_item(&item).is_ok());

        let invalid = DonationSyncItem {
            charity_id: "".to_string(),
            ..item
        };
        // Should fail if charity_id is missing even if encrypted
        assert!(validate_donation_sync_item(&invalid).is_err());
    }

    #[test]
    fn test_validate_unencrypted_donation_sync_item_fails_missing_fields() {
        let item = DonationSyncItem {
            action: "create".to_string(),
            id: "test-id".to_string(),
            date: None,
            year: None,
            category: Some("money".to_string()),
            amount: None, // Missing amount for money donation
            charity_id: "".to_string(), // Missing charity_id
            notes: None,
            is_encrypted: Some(false),
            encrypted_payload: None,
            updated_at: None,
        };

        assert!(validate_donation_sync_item(&item).is_err());
    }
}
