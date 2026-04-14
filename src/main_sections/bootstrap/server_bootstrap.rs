use axum::{
    extract::State,
    routing::{delete, get, post},
    Router,
    middleware::{from_fn, Next},
    http::{HeaderName, HeaderValue, StatusCode},
    response::{Html, IntoResponse},
};
use base64::Engine as _;
use std::net::SocketAddr;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::trace::{DefaultOnFailure, DefaultOnRequest, DefaultOnResponse, TraceLayer};
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_governor::GovernorLayer;
use tower_governor::governor::GovernorConfigBuilder;
use std::sync::Arc;
use axum::http::header;
use std::env;
use axum::http::{Request, header::HeaderMap};
use axum::body::Body;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
#[cfg(feature = "asset-pipeline")]
use std::process::Command;
#[cfg(feature = "asset-pipeline")]
use regex::Regex;

use db::DbPool;
use url::Url;

#[derive(Clone)]
pub struct AppState {
    pub db: DbPool,
    pub storage_endpoint: String,
    pub bucket_name: String,
    pub storage_region: String,
    pub storage_access_key_id: String,
    pub storage_secret_access_key: String,
    pub mistral_api_endpoint: Url,
    pub mistral_api_key: Option<String>,
    pub mistral_model: String,
    pub index_template: String,
    pub service_worker_script: String,
    pub asset_entrypoints: AssetEntrypoints,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct AssetEntrypoints {
    pub app: String,
    pub upload: String,
    pub dexie: String,
    pub service_worker_version: String,
    pub asset_rewrites: HashMap<String, String>,
    pub precache_assets: Vec<String>,
}

pub async fn run_app() -> anyhow::Result<()> {
    // Load .env if it exists
    dotenvy::dotenv().ok();

    #[cfg(feature = "asset-pipeline")]
    prepare_runtime_assets()?;

    let asset_entrypoints = load_asset_manifest()?;
    let service_worker_script = load_service_worker_script()?;

    #[cfg(feature = "asset-pipeline")]
    if should_prepare_assets_only() {
        return Ok(());
    }

    // Ensure critical environment variables are set
    env::var("JWT_SECRET").expect("JWT_SECRET must be set");

    let _observability = crate::observability::init_tracing()?;

    tracing::info!("Starting Deductible Tracker application...");

    let index_template = fs::read_to_string("static/index.html")?;

    // Database Setup
    tracing::info!("Initializing database connection pool...");
    let db_pool = db::init_pool().await?;
    tracing::info!("Database connection pool initialized successfully");

    // Oracle Object Storage Setup
    let storage_endpoint = env::var("OBJECT_STORAGE_ENDPOINT").expect("OBJECT_STORAGE_ENDPOINT must be set");
    let bucket_name = env::var("OBJECT_STORAGE_BUCKET").expect("OBJECT_STORAGE_BUCKET must be set");
    let storage_region = env::var("OCI_REGION").expect("OCI_REGION must be set");
    let storage_access_key_id = env::var("OCI_ACCESS_KEY_ID").expect("OCI_ACCESS_KEY_ID must be set");
    let storage_secret_access_key = env::var("OCI_SECRET_ACCESS_KEY").expect("OCI_SECRET_ACCESS_KEY must be set");
    let mistral_api_endpoint = crate::ocr::load_mistral_api_endpoint()?;
    let mistral_api_key = env::var("MISTRAL_API_KEY").ok().filter(|value| !value.trim().is_empty());
    let mistral_model = env::var("MISTRAL_MODEL").unwrap_or_else(|_| "mistral-ocr-latest".to_string());

    let state = AppState {
        db: db_pool,
        storage_endpoint,
        bucket_name,
        storage_region,
        storage_access_key_id,
        storage_secret_access_key,
        mistral_api_endpoint,
        mistral_api_key,
        mistral_model,
        index_template,
        service_worker_script,
        asset_entrypoints,
    };

    let governor_config = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(
                env::var("RATE_LIMIT_PER_SECOND")
                    .ok()
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(150),
            )
            .burst_size(
                env::var("RATE_LIMIT_BURST")
                    .ok()
                    .and_then(|v| v.parse::<u32>().ok())
                    .unwrap_or(300),
            )
            .finish()
            .expect("governor config"),
    );

    let auth_governor_config = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(
                env::var("AUTH_RATE_LIMIT_PER_SECOND")
                    .ok()
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(5),
            )
            .burst_size(
                env::var("AUTH_RATE_LIMIT_BURST")
                    .ok()
                    .and_then(|v| v.parse::<u32>().ok())
                    .unwrap_or(20),
            )
            .finish()
            .expect("auth governor config"),
    );

    // CORS configuration (no permissive mode)
    let cors = {
        let env_mode = env::var("RUST_ENV").unwrap_or_else(|_| "development".to_string());
        let origins = env::var("ALLOWED_ORIGINS")
            .ok()
            .map(|v| {
                v.split(',')
                    .filter_map(|s| {
                        let trimmed = s.trim();
                        if trimmed.is_empty() {
                            return None;
                        }
                        match trimmed.parse::<HeaderValue>() {
                            Ok(value) => Some(value),
                            Err(_) => {
                                tracing::warn!("Ignoring invalid ALLOWED_ORIGINS entry: {}", trimmed);
                                None
                            }
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| {
                if env_mode == "production" {
                    panic!("ALLOWED_ORIGINS must be set in production")
                }
                vec![
                    HeaderValue::from_static("http://localhost:3000"),
                    HeaderValue::from_static("http://127.0.0.1:3000"),
                ]
            });

        let origins = if origins.is_empty() {
            if env_mode == "production" {
                panic!("ALLOWED_ORIGINS must contain at least one valid origin in production")
            }
            vec![
                HeaderValue::from_static("http://localhost:3000"),
                HeaderValue::from_static("http://127.0.0.1:3000"),
            ]
        } else {
            origins
        };

        CorsLayer::new()
            .allow_origin(origins)
            .allow_methods([
                axum::http::Method::GET,
                axum::http::Method::POST,
                axum::http::Method::PUT,
                axum::http::Method::DELETE,
                axum::http::Method::OPTIONS,
            ])
            .allow_headers([
                header::CONTENT_TYPE,
                header::AUTHORIZATION,
                header::ACCEPT,
                HeaderName::from_static("x-csrf-token"),
            ])
            .allow_credentials(true)
    };

    let auth_router = Router::new()
        .route("/auth/login/{provider}", get(auth::login))
        .route("/auth/callback/{provider}", get(auth::callback).post(auth::callback))
        .route("/auth/logout", post(auth::logout))
        .route("/auth/dev/login", post(auth::dev_login))
        .route("/api/auth/risc", post(auth::risc_webhook))
        .layer(GovernorLayer::new(auth_governor_config));

    let api_router = Router::new()
        .route("/api/donations", get(routes::donations::list_donations).post(routes::donations::create_donation))
        .route("/api/donations/{id}", delete(routes::donations::delete_donation).put(routes::donations::update_donation))
        .route("/api/donations/import", post(routes::donations::import_donations))
        .route("/api/charities", get(routes::charities::list_charities).post(routes::charities::create_charity))
        .route("/api/charities/{id}", delete(routes::charities::delete_charity).put(routes::charities::update_charity))
        .route("/api/charities/search", get(routes::charities::search_charities))
        .route("/api/charities/lookup/{ein}", get(routes::charities::lookup_charity_by_ein))
        .route("/api/receipts/upload", post(routes::receipts::generate_upload_url))
        .route("/api/receipts/presign", post(routes::receipts::generate_read_url))
        .route("/api/receipts/confirm", post(routes::receipts::confirm_receipt))
        .route("/api/receipts/ocr", post(routes::receipts::ocr_receipt))
        .route("/api/receipts", get(routes::receipts::list_receipts))
        .route("/api/valuations/suggest", post(routes::valuations::suggest))
        .route("/api/valuations/seed", post(routes::valuations::seed))
        .route("/api/valuations/tree", get(routes::valuations::tree))
        .route("/api/reports/years", get(routes::reports::list_available_years))
        .route("/api/reports/export", get(routes::reports::export_csv))
        .route("/api/reports/export/txf", get(routes::reports::export_tax_txf))
        .route("/api/reports/audit", get(routes::reports::export_audit_csv))
        .route("/api/tax/marginal-rate", get(routes::tax::marginal_rate))
        .route("/api/sync/batch", post(routes::sync::batch_sync))
        .route("/api/me", get(auth::me).put(auth::update_me).delete(auth::delete_me))
        .route("/api/me/export", get(auth::export_me))
        .route("/api/me/import", post(auth::import_me))
        .route("/api/config", get(auth::get_config))
        .merge(auth_router)
        .layer(from_fn(require_auth))
        .layer(cors)
        .layer(GovernorLayer::new(governor_config));

    // Router Setup
    let app = Router::new()
        .merge(api_router)
        .route("/", get(serve_index))
        .route("/index.html", get(serve_index))
        .route("/health", get(health_check))
        .route("/sw.js", get(serve_service_worker))
        .route("/assets/tailwind.css", get(serve_tailwind_css))
        .nest_service(
            "/assets",
            ServeDir::new("public/assets")
                .precompressed_br()
                .precompressed_gzip(),
        )
        .nest_service("/fonts", ServeDir::new("static/fonts"))
        .fallback(get(spa_fallback))
        .layer(from_fn(static_cache_control))
        .layer(
            TraceLayer::new_for_http()
                .on_request(DefaultOnRequest::new().level(tracing::Level::INFO))
                .on_response(DefaultOnResponse::new().level(tracing::Level::INFO))
                .on_failure(DefaultOnFailure::new().level(tracing::Level::ERROR)),
        )
        .layer(CompressionLayer::new())
        .layer(SetResponseHeaderLayer::overriding(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::X_FRAME_OPTIONS,
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::STRICT_TRANSPORT_SECURITY,
            HeaderValue::from_static("max-age=63072000; includeSubDomains; preload"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::REFERRER_POLICY,
            HeaderValue::from_static("strict-origin-when-cross-origin"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("x-permitted-cross-domain-policies"),
            HeaderValue::from_static("none"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static("default-src 'self'; script-src 'self' https://accounts.google.com; script-src-elem 'self' https://accounts.google.com; style-src 'self' 'unsafe-inline' https://accounts.google.com; font-src 'self' data:; img-src 'self' data: blob: https://*.compat.objectstorage.*.oraclecloud.com; connect-src 'self' https://accounts.google.com https://*.compat.objectstorage.*.oraclecloud.com; frame-src https://accounts.google.com; frame-ancestors 'none'; base-uri 'self'; form-action 'self'; upgrade-insecure-requests"),
        ))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    tracing::info!("listening on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

