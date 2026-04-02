mod mistral_endpoint_test {
    use std::sync::{Mutex, OnceLock};

    include!("../src/ocr/endpoint.rs");

    fn env_lock() -> &'static Mutex<()> {
        static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        ENV_LOCK.get_or_init(|| Mutex::new(()))
    }

    fn clear_env() {
        std::env::remove_var("MISTRAL_API_ENDPOINT");
    }

    #[test]
    fn uses_canonical_endpoint_when_unset() {
        let _guard = env_lock().lock().expect("lock env");
        clear_env();

        let endpoint = load_mistral_api_endpoint().expect("endpoint");

        assert_eq!(endpoint.as_str(), MISTRAL_OCR_API_ENDPOINT);
    }

    #[test]
    fn accepts_canonical_override() {
        let _guard = env_lock().lock().expect("lock env");
        clear_env();
        std::env::set_var("MISTRAL_API_ENDPOINT", MISTRAL_OCR_API_ENDPOINT);

        let endpoint = load_mistral_api_endpoint().expect("endpoint");

        assert_eq!(endpoint.as_str(), MISTRAL_OCR_API_ENDPOINT);
        clear_env();
    }

    #[test]
    fn rejects_non_canonical_override() {
        let _guard = env_lock().lock().expect("lock env");
        clear_env();
        std::env::set_var("MISTRAL_API_ENDPOINT", "http://169.254.169.254/latest/meta-data");

        let error = load_mistral_api_endpoint().expect_err("non-canonical endpoint should fail");

        assert!(error
            .to_string()
            .contains("MISTRAL_API_ENDPOINT must resolve to"));
        clear_env();
    }
}