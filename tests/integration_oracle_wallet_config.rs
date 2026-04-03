mod oracle_wallet_config_test {
    include!("../src/db/oracle/wallet_config.rs");

    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir() -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "deductible-wallet-config-{}-{}",
            std::process::id(),
            timestamp
        ))
    }

    #[test]
    fn detects_encrypted_private_key_marker() {
        assert!(wallet_contains_encrypted_private_key(
            "-----BEGIN ENCRYPTED PRIVATE KEY-----\n..."
        ));
        assert!(!wallet_contains_encrypted_private_key(
            "-----BEGIN CERTIFICATE-----\n..."
        ));
    }

    #[test]
    fn rejects_encrypted_wallet_without_password() {
        let wallet_dir = unique_temp_dir();
        fs::create_dir_all(&wallet_dir).expect("create wallet dir");
        fs::write(
            wallet_dir.join("ewallet.pem"),
            "-----BEGIN ENCRYPTED PRIVATE KEY-----\n...",
        )
        .expect("write wallet pem");

        let result = validate_wallet_password(&wallet_dir, None);

        assert!(result.is_err());
        assert!(result
            .expect_err("missing wallet password should fail")
            .to_string()
            .contains("DB_WALLET_PASSWORD"));

        fs::remove_dir_all(&wallet_dir).expect("remove wallet dir");
    }

    #[test]
    fn accepts_encrypted_wallet_when_password_is_configured() {
        let wallet_dir = unique_temp_dir();
        fs::create_dir_all(&wallet_dir).expect("create wallet dir");
        fs::write(
            wallet_dir.join("ewallet.pem"),
            "-----BEGIN ENCRYPTED PRIVATE KEY-----\n...",
        )
        .expect("write wallet pem");

        let result = validate_wallet_password(&wallet_dir, Some("wallet-password"));

        assert!(result.is_ok());

        fs::remove_dir_all(&wallet_dir).expect("remove wallet dir");
    }
}
