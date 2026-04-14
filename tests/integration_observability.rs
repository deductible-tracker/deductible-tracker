mod observability_test {
    #![allow(dead_code)]

    use std::sync::{Mutex, OnceLock};

    include!("../src/observability.rs");

    fn env_lock() -> &'static Mutex<()> {
        static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        ENV_LOCK.get_or_init(|| Mutex::new(()))
    }

    fn clear_otel_env() {
        std::env::remove_var("NEW_RELIC_LICENSE_KEY");
        std::env::remove_var("OTEL_EXPORTER_OTLP_HEADERS");
        std::env::remove_var("OTEL_EXPORTER_OTLP_TRACES_HEADERS");
    }

    #[test]
    fn otlp_metadata_uses_new_relic_license_key() {
        let _guard = env_lock().lock().expect("lock env");
        clear_otel_env();
        std::env::set_var("NEW_RELIC_LICENSE_KEY", "test-license-key");

        let metadata = otlp_metadata().expect("metadata");
        let api_key = metadata
            .get("api-key")
            .expect("api-key metadata")
            .to_str()
            .expect("api-key metadata str");

        assert_eq!(api_key, "test-license-key");

        clear_otel_env();
    }

    #[test]
    fn otlp_metadata_prefers_explicit_headers() {
        let _guard = env_lock().lock().expect("lock env");
        clear_otel_env();
        std::env::set_var("NEW_RELIC_LICENSE_KEY", "ignored-license-key");
        std::env::set_var(
            "OTEL_EXPORTER_OTLP_HEADERS",
            "api-key=header-license,x-test-header=trace-demo",
        );

        let metadata = otlp_metadata().expect("metadata");
        let api_key = metadata
            .get("api-key")
            .expect("api-key metadata")
            .to_str()
            .expect("api-key metadata str");
        let custom = metadata
            .get("x-test-header")
            .expect("custom metadata")
            .to_str()
            .expect("custom metadata str");

        assert_eq!(api_key, "header-license");
        assert_eq!(custom, "trace-demo");

        clear_otel_env();
    }
}
