use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DonationReceiptSuggestion {
    pub date_of_donation: Option<NaiveDate>,
    pub organization_name: Option<String>,
    pub donation_type: String,
    pub item_name: Option<String>,
    pub amount_usd: Option<f64>,
    #[serde(flatten)]
    pub unknown_fields: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptAnalysis {
    pub ocr_text: Option<String>,
    pub ocr_date: Option<NaiveDate>,
    pub ocr_amount_cents: Option<i64>,
    pub ocr_status: String,
    pub suggestion: Option<DonationReceiptSuggestion>,
    pub warning: Option<String>,
}

mod real {
    use anyhow::{anyhow, Context};
    use base64::prelude::*;
    use reqwest::Client;
    use serde_json::{json, Value};

    use crate::AppState;
    use super::{DonationReceiptSuggestion, ReceiptAnalysis};

    const MAX_OCR_BYTES: usize = 10 * 1024 * 1024;

    fn get_mime_type(bytes: &[u8], hinted_content_type: Option<&str>) -> String {
        if let Some(hint) = hinted_content_type {
            return hint.to_string();
        }
        if bytes.starts_with(b"%PDF-") {
            return "application/pdf".to_string();
        }
        if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
            return "image/jpeg".to_string();
        }
        if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
            return "image/png".to_string();
        }
        "application/octet-stream".to_string()
    }

    fn is_image(mime: &str) -> bool {
        mime.starts_with("image/")
    }

    fn schema_json() -> Value {
        json!({
            "type": "object",
            "properties": {
                "date_of_donation": {
                    "type": "string",
                    "format": "date",
                    "description": "Date of donation in ISO format (YYYY-MM-DD)",
                    "examples": ["2026-03-17"]
                },
                "organization_name": {
                    "type": ["string", "null"],
                    "description": "Name of the organization receiving the donation",
                    "min_length": 1,
                    "examples": ["American Red Cross", "Local Food Bank"]
                },
                "donation_type": {
                    "enum": ["money", "item"],
                    "description": "Type of donation - must be either 'money' or 'item'",
                    "examples": ["money", "item"]
                },
                "item_name": {
                    "type": ["string", "null"],
                    "description": "Name of the donated item (null for monetary donations)",
                    "min_length": 1,
                    "examples": ["Winter Coat", "Textbooks"]
                },
                "amount_usd": {
                    "type": ["number", "null"],
                    "description": "Amount donated in USD (null for non-monetary donations)",
                    "minimum": 0,
                    "examples": [50, 100.5]
                }
            },
            "required": ["donation_type"],
            "additional_properties": false
        })
    }

    fn mistral_instructions() -> &'static str {
        "You are an expert document parser specializing in donation receipts.\nAnalyze the provided receipt text and extract key donation details.\nReturn structured JSON only. Do not include explanations.\nIf a field is missing, return null.\nNormalize dates into ISO format (YYYY-MM-DD).\nIf the donation is monetary, set item_name to null.\nIf the donation is an item, include item_name and set amount_usd to null unless explicitly stated.\nThe organization name should match the official entity on the receipt.\nClassify donation_type strictly as \"money\" or \"item\"."
    }

    pub async fn analyze_receipt(
        state: &AppState,
        key: &str,
        hinted_content_type: Option<&str>,
    ) -> anyhow::Result<ReceiptAnalysis> {
        let api_key = state.mistral_api_key.as_ref().ok_or_else(|| anyhow!("Mistral API key not configured"))?;
        
        let url = crate::storage::presign_url(state, "GET", key, 300)?;
        let client = Client::new();
        let response = client
            .get(&url)
            .send()
            .await
            .context("downloading uploaded receipt")?;
        
        if !response.status().is_success() {
            return Err(anyhow!("failed to download object: {}", response.status()));
        }

        let bytes = response.bytes().await.context("reading uploaded receipt bytes")?;
        if bytes.is_empty() || bytes.len() > MAX_OCR_BYTES {
            return Err(anyhow!("receipt size is invalid for OCR"));
        }

        let mime_type = get_mime_type(bytes.as_ref(), hinted_content_type);
        let b64_file = BASE64_STANDARD.encode(&bytes);

        let mut document = json!({
            "type": if is_image(&mime_type) { "image_url" } else { "document_url" }
        });

        if is_image(&mime_type) {
            document["image_url"] = json!(format!("data:{};base64,{}", mime_type, b64_file));
        } else {
            document["document_url"] = json!(format!("data:{};base64,{}", mime_type, b64_file));
        }

        let ocr_request = json!({
            "model": state.mistral_model,
            "document": document,
            "include_image_base64": false,
            "document_annotation_format": {
                "type": "json_schema",
                "json_schema": {
                    "name": "response_schema",
                    "schema": schema_json()
                }
            },
            "document_annotation_prompt": mistral_instructions()
        });

        let resp = client
            .post(&state.mistral_api_endpoint)
            .bearer_auth(api_key)
            .json(&ocr_request)
            .send()
            .await
            .context("calling Mistral OCR API")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err_text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Mistral OCR API error ({}): {}", status, err_text));
        }

        let ocr_result: Value = resp.json().await.context("parsing Mistral OCR response")?;
        
        // Use document_annotation directly from the root
        let suggestion_val = ocr_result.get("document_annotation")
            .ok_or_else(|| {
                tracing::error!("Mistral Response Body (missing document_annotation): {:?}", ocr_result);
                anyhow!("Failed to extract structured data from Mistral OCR response (document_annotation missing)")
            })?;

        let suggestion: DonationReceiptSuggestion = match suggestion_val {
            Value::String(s) => {
                serde_json::from_str(s).map_err(|e| {
                    tracing::error!("Failed to deserialize DonationReceiptSuggestion from string: {}. Content: {}", e, s);
                    anyhow!("deserializing donation suggestion string: {}", e)
                })?
            }
            Value::Object(_) => {
                serde_json::from_value(suggestion_val.clone()).map_err(|e| {
                    tracing::error!("Failed to deserialize DonationReceiptSuggestion from object: {}. Object: {:?}", e, suggestion_val);
                    anyhow!("deserializing donation suggestion object: {}", e)
                })?
            }
            _ => return Err(anyhow!("Unexpected type for document_annotation: {:?}", suggestion_val)),
        };

        let ocr_date = suggestion.date_of_donation;
        let ocr_amount_cents = suggestion.amount_usd.map(|a| (a * 100.0).round() as i64);

        Ok(ReceiptAnalysis {
            ocr_text: None,
            ocr_date,
            ocr_amount_cents,
            ocr_status: "done".to_string(),
            suggestion: Some(suggestion),
            warning: None,
        })
    }
}

pub use real::analyze_receipt;
