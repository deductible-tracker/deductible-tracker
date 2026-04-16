use anyhow::{Context, Result};
use easy_init_newrelic_opentelemetry::NewRelicSubscriberInitializer;
use std::env;
use time::macros::offset;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

const DEFAULT_NEW_RELIC_OTLP_ENDPOINT: &str = "https://otlp.nr-data.net:4317";
const DEFAULT_SERVICE_NAME: &str = "deductible-tracker";

pub struct ObservabilityGuard {
    _inner: Option<Box<dyn std::any::Any>>,
}

pub fn init_tracing() -> Result<ObservabilityGuard> {
    let env_filter = tracing_subscriber::EnvFilter::new(
        env::var("RUST_LOG")
            .unwrap_or_else(|_| "deductible_tracker=info,tower_http=info".to_string()),
    );

    if !telemetry_enabled() {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer())
            .init();
        return Ok(ObservabilityGuard { _inner: None });
    }

    let service_name = get_service_name();
    let license_key = env::var("NEW_RELIC_LICENSE_KEY")
        .context("NEW_RELIC_LICENSE_KEY must be set when OpenTelemetry export is enabled")?;
    let endpoint = otlp_endpoint();
    let hostname = env::var("HOSTNAME").unwrap_or_default();

    // Set additional resource attributes via environment variable as the crate doesn't expose a direct resource builder
    let env_name = deployment_environment();
    let current_attributes = env::var("OTEL_RESOURCE_ATTRIBUTES").unwrap_or_default();
    let additional_attributes = format!(
        "deployment.environment.name={},cloud.provider=oracle,service.namespace={}",
        env_name, DEFAULT_SERVICE_NAME
    );
    
    let new_attributes = if current_attributes.is_empty() {
        additional_attributes
    } else {
        format!("{},{}", current_attributes, additional_attributes)
    };
    env::set_var("OTEL_RESOURCE_ATTRIBUTES", new_attributes);

    let guard = NewRelicSubscriberInitializer::default()
        .newrelic_otlp_endpoint(endpoint)
        .newrelic_license_key(license_key)
        .newrelic_service_name(service_name)
        .host_name(hostname)
        .service_version(env!("CARGO_PKG_VERSION"))
        .timestamps_offset(offset!(+00:00:00))
        .init()
        .map_err(|e| anyhow::anyhow!("Failed to initialize New Relic OpenTelemetry: {}", e))?;

    Ok(ObservabilityGuard {
        _inner: Some(Box::new(guard)),
    })
}

fn get_service_name() -> String {
    env::var("OTEL_SERVICE_NAME")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            env::var("NEW_RELIC_APP_NAME")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .unwrap_or_else(|| DEFAULT_SERVICE_NAME.to_string())
}

fn deployment_environment() -> String {
    env::var("RUST_ENV")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "development".to_string())
}

fn telemetry_enabled() -> bool {
    if let Some(true) = env_flag("OTEL_SDK_DISABLED") {
        return false;
    }

    if let Some(enabled) = env_flag("NEW_RELIC_ENABLED") {
        return enabled;
    }

    has_otlp_endpoint_config()
        || env::var("NEW_RELIC_LICENSE_KEY")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .is_some()
}

fn has_otlp_endpoint_config() -> bool {
    env::var("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .is_some()
        || env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .is_some()
}

fn otlp_endpoint() -> String {
    env::var("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .or_else(|| {
            env::var("NEW_RELIC_OTLP_ENDPOINT")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .unwrap_or_else(|| DEFAULT_NEW_RELIC_OTLP_ENDPOINT.to_string())
}

fn env_flag(name: &str) -> Option<bool> {
    env::var(name)
        .ok()
        .and_then(|value| match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
        })
}
