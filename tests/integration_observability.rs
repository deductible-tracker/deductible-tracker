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
        std::env::remove_var("OTEL_EXPORTER_OTLP_ENDPOINT");
        std::env::remove_var("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT");
        std::env::remove_var("OTEL_EXPORTER_OTLP_HEADERS");
        std::env::remove_var("OTEL_EXPORTER_OTLP_TRACES_HEADERS");
        std::env::remove_var("OTEL_SDK_DISABLED");
        std::env::remove_var("NEW_RELIC_ENABLED");
    }

    #[test]
    fn telemetry_enabled_when_local_otlp_endpoint_is_configured() {
        let _guard = env_lock().lock().expect("lock env");
        clear_otel_env();
        std::env::set_var("OTEL_EXPORTER_OTLP_ENDPOINT", "http://127.0.0.1:4317");

        assert!(telemetry_enabled());

        clear_otel_env();
    }

    #[test]
    fn telemetry_disabled_by_otel_sdk_disabled() {
        let _guard = env_lock().lock().expect("lock env");
        clear_otel_env();
        std::env::set_var("OTEL_EXPORTER_OTLP_ENDPOINT", "http://127.0.0.1:4317");
        std::env::set_var("OTEL_SDK_DISABLED", "true");

        assert!(!telemetry_enabled());

        clear_otel_env();
    }

    #[test]
    fn otlp_endpoint_prefers_traces_specific_env() {
        let _guard = env_lock().lock().expect("lock env");
        clear_otel_env();
        std::env::set_var("OTEL_EXPORTER_OTLP_ENDPOINT", "http://general:4317");
        std::env::set_var("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT", "http://traces:4317");

        assert_eq!(otlp_endpoint(), "http://traces:4317");

        clear_otel_env();
    }
}
