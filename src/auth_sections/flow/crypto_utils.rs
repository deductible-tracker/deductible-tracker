// src/auth_sections/flow/crypto_utils.rs

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce
};
use base64::prelude::*;
use anyhow::anyhow;

pub fn decrypt_payload(base64_key: &str, encrypted_b64: &str) -> anyhow::Result<Vec<u8>> {
    // 1. Decode key (should be 32 bytes for AES-256)
    let key_bytes = BASE64_STANDARD.decode(base64_key)
        .map_err(|e| anyhow!("Failed to decode key: {}", e))?;
    
    if key_bytes.len() != 32 {
        return Err(anyhow!("Invalid key length. Expected 32 bytes, got {}", key_bytes.len()));
    }

    // 2. Decode encrypted data
    let combined = BASE64_STANDARD.decode(encrypted_b64)
        .map_err(|e| anyhow!("Failed to decode encrypted payload: {}", e))?;

    if combined.len() < 12 {
        return Err(anyhow!("Payload too short"));
    }

    // 3. Split IV and ciphertext (matching frontend: 12 byte IV prefix)
    let (iv, ciphertext) = combined.split_at(12);
    let nonce = Nonce::from_slice(iv);
    
    // 4. Decrypt
    let cipher = Aes256Gcm::new_from_slice(&key_bytes)
        .map_err(|e| anyhow!("Cipher init error: {}", e))?;

    let decrypted = cipher.decrypt(nonce, ciphertext)
        .map_err(|e| anyhow!("Decryption error: {}", e))?;

    Ok(decrypted)
}
