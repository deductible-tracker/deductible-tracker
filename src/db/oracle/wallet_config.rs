use anyhow::{anyhow, Context};
use std::fs;
use std::path::Path;

const ENCRYPTED_PRIVATE_KEY_MARKER: &str = "-----BEGIN ENCRYPTED PRIVATE KEY-----";

pub(crate) fn validate_wallet_password(
    wallet_dir: &Path,
    wallet_password: Option<&str>,
) -> anyhow::Result<()> {
    let wallet_pem_path = wallet_dir.join("ewallet.pem");
    if !wallet_pem_path.is_file() {
        return Ok(());
    }

    let wallet_pem = fs::read_to_string(&wallet_pem_path).with_context(|| {
        format!(
            "failed to read Oracle wallet PEM at {}",
            wallet_pem_path.display()
        )
    })?;

    if wallet_contains_encrypted_private_key(&wallet_pem) && wallet_password.is_none() {
        return Err(anyhow!(
            "Oracle wallet at {} contains an encrypted private key; set DB_WALLET_PASSWORD or ORACLE_WALLET_PASSWORD",
            wallet_pem_path.display()
        ));
    }

    Ok(())
}

pub(crate) fn wallet_contains_encrypted_private_key(wallet_pem: &str) -> bool {
    wallet_pem.contains(ENCRYPTED_PRIVATE_KEY_MARKER)
}
