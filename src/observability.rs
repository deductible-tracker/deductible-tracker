use anyhow::{Context, Result};
use opentelemetry::trace::TracerProvider as _;
use opentelemetry::{global, KeyValue};
use opentelemetry_otlp::{SpanExporter, WithExportConfig, WithTonicConfig};
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_sdk::Resource;
use std::env;
use std::time::Duration;
use tonic::metadata::{Ascii, MetadataKey, MetadataMap, MetadataValue};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

const DEFAULT_NEW_RELIC_OTLP_ENDPOINT: &str = "https://otlp.nr-data.net:4317";
const DEFAULT_SERVICE_NAME: &str = "deductible-tracker";

pub struct ObservabilityGuard {
    tracer_provider: Option<SdkTracerProvider>,
}

impl Drop for ObservabilityGuard {
    fn drop(&mut self) {
        if let Some(tracer_provider) = self.tracer_provider.take() {
            let _ = tracer_provider.shutdown();
        }
    }
}

pub fn init_tracing() -> Result<ObservabilityGuard> {
    let env_filter = tracing_subscriber::EnvFilter::new(
        env::var("RUST_LOG")
            .unwrap_or_else(|_| "deductible_tracker=info,tower_http=info".to_string()),
    );
    let fmt_layer = tracing_subscriber::fmt::layer();

    if !telemetry_enabled() {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .init();
        return Ok(ObservabilityGuard {
            tracer_provider: None,
        });
    }

    let tracer_provider = build_tracer_provider()?;
    let tracer = tracer_provider.tracer(service_name());
    global::set_text_map_propagator(TraceContextPropagator::new());
    global::set_tracer_provider(tracer_provider.clone());

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .with(tracing_opentelemetry::layer().with_tracer(tracer))
        .init();

    Ok(ObservabilityGuard {
        tracer_provider: Some(tracer_provider),
    })
}

fn build_tracer_provider() -> Result<SdkTracerProvider> {
    let exporter = SpanExporter::builder()
        .with_tonic()
        .with_endpoint(otlp_endpoint())
        .with_timeout(Duration::from_secs(5))
        .with_metadata(otlp_metadata()?)
        .build()
        .context("failed to build OTLP span exporter")?;

    Ok(SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(resource())
        .build())
}

fn resource() -> Resource {
    let mut builder = Resource::builder().with_service_name(service_name());

    builder = builder.with_attributes([
        KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
        KeyValue::new("deployment.environment.name", deployment_environment()),
        KeyValue::new("service.namespace", DEFAULT_SERVICE_NAME),
        KeyValue::new("cloud.provider", "oracle"),
    ]);

    if let Ok(hostname) = env::var("HOSTNAME") {
        if !hostname.trim().is_empty() {
            builder = builder.with_attribute(KeyValue::new("service.instance.id", hostname));
        }
    }

    builder.build()
}

fn service_name() -> String {
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
        || explicit_otlp_headers().is_some()
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

fn otlp_metadata() -> Result<MetadataMap> {
    let mut metadata = MetadataMap::new();

    if let Some(headers) = explicit_otlp_headers() {
        return parse_otlp_headers(&headers);
    }

    if !uses_new_relic_otlp_endpoint() {
        return Ok(metadata);
    }

    let license_key = env::var("NEW_RELIC_LICENSE_KEY")
        .context("NEW_RELIC_LICENSE_KEY must be set when OpenTelemetry export is enabled")?;
    let parsed = MetadataValue::try_from(license_key.trim())
        .context("NEW_RELIC_LICENSE_KEY contains invalid characters for OTLP metadata")?;
    metadata.insert("api-key", parsed);

    Ok(metadata)
}

fn explicit_otlp_headers() -> Option<String> {
    env::var("OTEL_EXPORTER_OTLP_TRACES_HEADERS")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            env::var("OTEL_EXPORTER_OTLP_HEADERS")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
}

fn uses_new_relic_otlp_endpoint() -> bool {
    otlp_endpoint().contains("nr-data.net")
}

fn parse_otlp_headers(headers: &str) -> Result<MetadataMap> {
    let mut metadata = MetadataMap::new();

    for raw_header in headers.split(',') {
        let header = raw_header.trim();
        if header.is_empty() {
            continue;
        }

        let (key, value) = header
            .split_once('=')
            .context("OTLP headers must be a comma-separated list of key=value pairs")?;

        let key = key.trim();
        let value = value.trim();
        let parsed_key: MetadataKey<Ascii> = key
            .parse()
            .with_context(|| format!("invalid OTLP metadata key '{key}'"))?;
        let parsed_value = MetadataValue::try_from(value)
            .with_context(|| format!("invalid OTLP metadata value for header '{key}'"))?;

        metadata.insert(parsed_key, parsed_value);
    }

    Ok(metadata)
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
