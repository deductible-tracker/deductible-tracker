use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use url::Url;

use crate::AppState;

pub fn normalize_object_key(bucket_name: &str, key: &str) -> String {
    let trimmed = key.trim().trim_start_matches('/');
    let bucket_prefix = format!("{}/", bucket_name);

    if trimmed.starts_with(&bucket_prefix) {
        trimmed[bucket_prefix.len()..].to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn user_receipt_prefix(user_id: &str) -> String {
    format!("receipts/{}/", user_id)
}

pub fn presign_url(
    state: &AppState,
    method: &str,
    key: &str,
    expires_in_secs: u64,
) -> Result<String> {
    let normalized_key = normalize_object_key(&state.bucket_name, key);
    if normalized_key.is_empty() {
        return Err(anyhow!("storage key cannot be empty"));
    }

    let endpoint = Url::parse(&state.storage_endpoint).context("invalid storage endpoint")?;
    let host = endpoint
        .host_str()
        .ok_or_else(|| anyhow!("storage endpoint is missing a host"))?;
    let host = match endpoint.port() {
        Some(port) => format!("{}:{}", host, port),
        None => host.to_string(),
    };

    let base_path = endpoint.path().trim_matches('/');
    let object_path = if base_path.is_empty() {
        format!("{}/{}", state.bucket_name, normalized_key)
    } else {
        format!("{}/{}/{}", base_path, state.bucket_name, normalized_key)
    };
    let canonical_uri = format!("/{}", encode_path(&object_path));

    let now = Utc::now();
    let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
    let date_stamp = now.format("%Y%m%d").to_string();
    let credential_scope = format!("{}/{}/s3/aws4_request", date_stamp, state.storage_region);
    let credential = format!("{}/{}", state.storage_access_key_id, credential_scope);

    let mut query_params = [
        (
            "X-Amz-Algorithm".to_string(),
            "AWS4-HMAC-SHA256".to_string(),
        ),
        ("X-Amz-Credential".to_string(), credential),
        ("X-Amz-Date".to_string(), amz_date.clone()),
        ("X-Amz-Expires".to_string(), expires_in_secs.to_string()),
        ("X-Amz-SignedHeaders".to_string(), "host".to_string()),
    ];
    query_params.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

    let canonical_query = query_params
        .iter()
        .map(|(k, v)| {
            format!(
                "{}={}",
                encode_query_component(k),
                encode_query_component(v)
            )
        })
        .collect::<Vec<_>>()
        .join("&");

    let canonical_headers = format!("host:{}\n", host);
    let canonical_request = format!(
        "{}\n{}\n{}\n{}\nhost\nUNSIGNED-PAYLOAD",
        method, canonical_uri, canonical_query, canonical_headers,
    );
    let canonical_request_hash = hex_encode(&sha256_hash(canonical_request.as_bytes()));
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{}\n{}\n{}",
        amz_date, credential_scope, canonical_request_hash,
    );

    let signing_key = signing_key(
        &state.storage_secret_access_key,
        &date_stamp,
        &state.storage_region,
        "s3",
    )?;
    let signature = hex_encode(&hmac_sha256(&signing_key, string_to_sign.as_bytes())?);

    let mut final_url = endpoint;
    final_url.set_path(&object_path);
    final_url.set_query(Some(&format!(
        "{}&X-Amz-Signature={}",
        canonical_query, signature
    )));
    Ok(final_url.to_string())
}

fn signing_key(
    secret_access_key: &str,
    date_stamp: &str,
    region: &str,
    service: &str,
) -> Result<Vec<u8>> {
    let k_date = hmac_sha256(
        format!("AWS4{}", secret_access_key).as_bytes(),
        date_stamp.as_bytes(),
    )?;
    let k_region = hmac_sha256(&k_date, region.as_bytes())?;
    let k_service = hmac_sha256(&k_region, service.as_bytes())?;
    hmac_sha256(&k_service, b"aws4_request")
}

type HmacSha256 = Hmac<Sha256>;

fn sha256_hash(data: &[u8]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().to_vec()
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Result<Vec<u8>> {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(key)
        .map_err(|e| anyhow!("failed to create HMAC key: {}", e))?;
    mac.update(data);
    Ok(mac.finalize().into_bytes().to_vec())
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push_str(&format!("{:02x}", byte));
    }
    output
}

fn encode_query_component(input: &str) -> String {
    percent_encode(input.as_bytes(), false)
}

fn encode_path(input: &str) -> String {
    percent_encode(input.as_bytes(), true)
}

fn percent_encode(input: &[u8], preserve_slash: bool) -> String {
    let mut encoded = String::new();
    for &byte in input {
        let is_unreserved =
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~');
        if is_unreserved || (preserve_slash && byte == b'/') {
            encoded.push(byte as char);
        } else {
            encoded.push('%');
            encoded.push_str(&format!("{:02X}", byte));
        }
    }
    encoded
}
