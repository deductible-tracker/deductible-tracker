use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DonationReceiptSuggestion {
    pub date_of_donation: Option<NaiveDate>,
    pub organization_name: Option<String>,
    pub donation_type: String,
    pub item_name: Option<String>,
    pub amount_usd: Option<f64>,
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

#[cfg(feature = "ocr")]
mod real {
    use anyhow::{anyhow, Context};
    use chrono::NaiveDate;
    use image::DynamicImage;
    use ocrs::{ImageSource, OcrEngine, OcrEngineParams};
    use regex::Regex;
    use reqwest::Client;
    use rten::Model;
    use serde_json::{json, Value};
    use std::path::{Path, PathBuf};

    use crate::AppState;

    use super::{DonationReceiptSuggestion, ReceiptAnalysis};

    const MAX_OCR_BYTES: usize = 10 * 1024 * 1024;
    const OCRS_DETECTION_MODEL_URL: &str = "https://ocrs-models.s3-accelerate.amazonaws.com/text-detection.rten";
    const OCRS_RECOGNITION_MODEL_URL: &str = "https://ocrs-models.s3-accelerate.amazonaws.com/text-recognition.rten";

    fn detect_receipt_type(bytes: &[u8], hinted_content_type: Option<&str>) -> Option<String> {
        let hinted = hinted_content_type
            .map(str::trim)
            .filter(|value| matches!(*value, "image/jpeg" | "image/png" | "application/pdf"));
        if let Some(content_type) = hinted {
            return Some(content_type.to_string());
        }
        if bytes.len() >= 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF {
            return Some("image/jpeg".to_string());
        }
        if bytes.len() >= 8
            && bytes[0] == 0x89
            && bytes[1] == b'P'
            && bytes[2] == b'N'
            && bytes[3] == b'G'
            && bytes[4] == 0x0D
            && bytes[5] == 0x0A
            && bytes[6] == 0x1A
            && bytes[7] == 0x0A
        {
            return Some("image/png".to_string());
        }
        if bytes.starts_with(b"%PDF-") {
            return Some("application/pdf".to_string());
        }
        None
    }

    fn extract_date_and_amount(text: &str) -> (Option<NaiveDate>, Option<i64>) {
        let mut found_date = None;
        let mut found_amount = None;

        let re_iso = Regex::new(r"(\d{4}-\d{2}-\d{2})").expect("valid ISO date regex");
        if let Some(cap) = re_iso.captures(text) {
            if let Ok(date) = NaiveDate::parse_from_str(&cap[1], "%Y-%m-%d") {
                found_date = Some(date);
            }
        } else {
            let re_us = Regex::new(r"(\d{1,2}/\d{1,2}/\d{4})").expect("valid US date regex");
            if let Some(cap) = re_us.captures(text) {
                if let Ok(date) = NaiveDate::parse_from_str(&cap[1], "%m/%d/%Y") {
                    found_date = Some(date);
                }
            }
        }

        let re_amt = Regex::new(r"\$?([0-9]{1,3}(?:,[0-9]{3})*(?:\.[0-9]+)?|[0-9]+(?:\.[0-9]+)?)")
            .expect("valid amount regex");
        if let Some(cap) = re_amt.captures(text) {
            let clean = cap[1].replace(',', "");
            if let Ok(amount) = clean.parse::<f64>() {
                found_amount = Some((amount * 100.0).round() as i64);
            }
        }

        (found_date, found_amount)
    }

    fn detection_model_path(model_dir: &Path) -> PathBuf {
        model_dir.join("text-detection.rten")
    }

    fn recognition_model_path(model_dir: &Path) -> PathBuf {
        model_dir.join("text-recognition.rten")
    }

    async fn ensure_model_file(client: &Client, url: &str, path: &Path) -> anyhow::Result<()> {
        if tokio::fs::metadata(path).await.is_ok() {
            return Ok(());
        }

        let response = client
            .get(url)
            .send()
            .await
            .with_context(|| format!("downloading OCR model from {url}"))?;
        if !response.status().is_success() {
            return Err(anyhow!("failed to download OCR model from {url}: {}", response.status()));
        }

        let bytes = response
            .bytes()
            .await
            .with_context(|| format!("reading OCR model response from {url}"))?;
        tokio::fs::write(path, bytes)
            .await
            .with_context(|| format!("writing OCR model to {}", path.display()))?;
        Ok(())
    }

    async fn ensure_models(state: &AppState, client: &Client) -> anyhow::Result<(PathBuf, PathBuf)> {
        let model_dir = PathBuf::from(&state.ocrs_model_dir);
        tokio::fs::create_dir_all(&model_dir)
            .await
            .with_context(|| format!("creating OCR model directory {}", model_dir.display()))?;

        let detection_path = detection_model_path(&model_dir);
        let recognition_path = recognition_model_path(&model_dir);

        ensure_model_file(client, OCRS_DETECTION_MODEL_URL, &detection_path).await?;
        ensure_model_file(client, OCRS_RECOGNITION_MODEL_URL, &recognition_path).await?;

        Ok((detection_path, recognition_path))
    }

    fn extract_text_with_ocrs(
        detection_model_path: &Path,
        recognition_model_path: &Path,
        image: DynamicImage,
    ) -> anyhow::Result<String> {
        let detection_model = Model::load_file(detection_model_path)
            .with_context(|| format!("loading OCR detection model {}", detection_model_path.display()))?;
        let recognition_model = Model::load_file(recognition_model_path)
            .with_context(|| format!("loading OCR recognition model {}", recognition_model_path.display()))?;

        let engine = OcrEngine::new(OcrEngineParams {
            detection_model: Some(detection_model),
            recognition_model: Some(recognition_model),
            ..Default::default()
        })
        .context("initializing OCRS engine")?;

        let rgb = image.into_rgb8();
        let image_source = ImageSource::from_bytes(rgb.as_raw(), rgb.dimensions())
            .context("preparing OCR image source")?;
        let ocr_input = engine.prepare_input(image_source).context("preparing OCR input")?;
        let word_rects = engine.detect_words(&ocr_input).context("detecting OCR words")?;
        let line_rects = engine.find_text_lines(&ocr_input, &word_rects);
        let line_texts = engine
            .recognize_text(&ocr_input, &line_rects)
            .context("recognizing OCR text")?;

        let text = line_texts
            .iter()
            .flatten()
            .map(|line| line.to_string())
            .filter(|line| line.trim().len() > 1)
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_string();

        if text.is_empty() {
            return Err(anyhow!("OCR completed but no text was extracted"));
        }

        Ok(text)
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
                    "minLength": 1,
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
                    "minLength": 1,
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
            "additionalProperties": false
        })
    }

    fn mistral_instructions() -> &'static str {
        "* You are an expert document parser specializing in donation receipts.\n* Analyze the provided receipt text and extract key donation details.\n* Return structured JSON only. Do not include explanations.\n* If a field is missing, return `null`.\n* Normalize dates into ISO format (`YYYY-MM-DD`).\n* If the donation is monetary, set `item_name` to `null`.\n* If the donation is an item, include `item_name` and set `amount_usd` to `null` unless explicitly stated.\n* The organization name should match the official entity on the receipt.\n* Classify `donation_type` strictly as `\"money\"` or `\"item\"`."
    }

    fn extract_message_json(value: &Value) -> anyhow::Result<Value> {
        fn parse_content_value(message: &Value) -> anyhow::Result<Value> {
            match message {
                Value::String(text) => serde_json::from_str(text).context("parsing Mistral JSON response"),
                Value::Array(parts) => {
                    let text = parts
                        .iter()
                        .filter_map(|part| match part {
                            Value::String(text) => Some(text.clone()),
                            Value::Object(map) => map.get("text").and_then(Value::as_str).map(str::to_string),
                            _ => None,
                        })
                        .collect::<String>();
                    serde_json::from_str(&text).context("parsing Mistral JSON response from content parts")
                }
                Value::Object(_) => Ok(message.clone()),
                _ => Err(anyhow!("unexpected Mistral response content shape")),
            }
        }

        if let Some(content) = value
            .get("choices")
            .and_then(|choices| choices.get(0))
            .and_then(|choice| choice.get("message"))
            .and_then(|message| message.get("content"))
        {
            return parse_content_value(content);
        }

        if let Some(content) = value
            .get("output")
            .and_then(|output| output.get("message"))
            .and_then(|message| message.get("content"))
        {
            return parse_content_value(content);
        }

        if let Some(content) = value
            .get("outputs")
            .and_then(Value::as_array)
            .and_then(|outputs| outputs.first())
            .and_then(|output| output.get("content"))
        {
            return parse_content_value(content);
        }

        if let Some(content) = value
            .get("conversation")
            .and_then(|conversation| conversation.get("output"))
            .and_then(|output| output.get("message"))
            .and_then(|message| message.get("content"))
        {
            return parse_content_value(content);
        }

        Err(anyhow!("Mistral response did not include assistant content"))
    }

    async fn infer_donation_suggestion(
        state: &AppState,
        client: &Client,
        ocr_text: &str,
    ) -> anyhow::Result<Option<DonationReceiptSuggestion>> {
        let Some(api_key) = state.mistral_api_key.as_ref() else {
            return Ok(None);
        };

        // Validate the endpoint to prevent SSRF (#9)
        if !state.mistral_api_endpoint.starts_with("https://api.mistral.ai/")
            && !state
                .mistral_api_endpoint
                .starts_with("https://chat.mistral.ai/")
        {
            return Err(anyhow!("Untrusted Mistral API endpoint: {}", state.mistral_api_endpoint));
        }

        let response = client
            .post(&state.mistral_api_endpoint)
            .bearer_auth(api_key)
            .json(&json!({
                "model": state.mistral_model,
                "inputs": [
                    {
                        "role": "user",
                        "content": ocr_text
                    }
                ],
                "tools": [],
                "completion_args": {
                    "temperature": 0.7,
                    "max_tokens": 2048,
                    "top_p": 1,
                    "response_format": {
                        "type": "json_schema",
                        "json_schema": {
                            "name": "response_schema",
                            "schema": schema_json()
                        }
                    }
                },
                "instructions": mistral_instructions()
            }))
            .send()
            .await
            .context("calling Mistral conversations API")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("Mistral request failed with status {status}: {body}"));
        }

        let value = response
            .json::<Value>()
            .await
            .context("decoding Mistral response body")?;
        let payload = extract_message_json(&value)?;
        let suggestion = serde_json::from_value::<DonationReceiptSuggestion>(payload)
            .context("deserializing Mistral donation suggestion")?;
        Ok(Some(suggestion))
    }

    pub async fn analyze_receipt(
        state: &AppState,
        key: &str,
        hinted_content_type: Option<&str>,
    ) -> anyhow::Result<ReceiptAnalysis> {
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

        let content_type = detect_receipt_type(bytes.as_ref(), hinted_content_type)
            .ok_or_else(|| anyhow!("unsupported receipt file type for OCR"))?;
        if content_type == "application/pdf" {
            return Ok(ReceiptAnalysis {
                ocr_text: None,
                ocr_date: None,
                ocr_amount_cents: None,
                ocr_status: "skipped_non_image".to_string(),
                suggestion: None,
                warning: Some("Only image receipts are analyzed for automatic prefill.".to_string()),
            });
        }

        let image = image::load_from_memory(bytes.as_ref()).context("decoding uploaded image")?;
        let (detection_model_path, recognition_model_path) = ensure_models(state, &client).await?;
        let text = extract_text_with_ocrs(&detection_model_path, &recognition_model_path, image)?;
        let (ocr_date, ocr_amount_cents) = extract_date_and_amount(&text);
        let suggestion = infer_donation_suggestion(state, &client, &text).await?;
        let warning = if state.mistral_api_key.is_none() {
            Some("Mistral enrichment is not configured; only OCR text was extracted.".to_string())
        } else {
            None
        };

        Ok(ReceiptAnalysis {
            ocr_text: Some(text),
            ocr_date,
            ocr_amount_cents,
            ocr_status: "done".to_string(),
            suggestion,
            warning,
        })
    }
}

#[cfg(not(feature = "ocr"))]
mod stub {
    use anyhow::anyhow;

    use crate::AppState;

    use super::ReceiptAnalysis;

    pub async fn analyze_receipt(
        _state: &AppState,
        _key: &str,
        _hinted_content_type: Option<&str>,
    ) -> anyhow::Result<ReceiptAnalysis> {
        Err(anyhow!("OCR feature not enabled; build with --features ocr"))
    }
}

#[cfg(feature = "ocr")]
pub use real::analyze_receipt;
#[cfg(not(feature = "ocr"))]
pub use stub::analyze_receipt;
