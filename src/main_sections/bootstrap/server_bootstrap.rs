use axum::{
    extract::State,
    routing::{get, post, delete},
    Router,
    middleware::{from_fn, Next},
    http::{HeaderValue, StatusCode, HeaderName},
    response::{Html, IntoResponse},
};
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_governor::GovernorLayer;
use tower_governor::governor::GovernorConfigBuilder;
use std::sync::Arc;
use axum::http::header;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use opendal::services::S3;
use opendal::Operator;
use std::env;
use axum::http::{Request, header::HeaderMap};
use axum::body::Body;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use regex::Regex;

use db::DbPool;

#[derive(Clone)]
pub struct AppState {
    pub db: DbPool,
    pub storage: Operator,
    pub bucket_name: String,
    pub index_template: String,
    pub asset_entrypoints: AssetEntrypoints,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct AssetEntrypoints {
    pub app: String,
    pub upload: String,
    pub valuations: String,
    pub css_rewrites: HashMap<String, String>,
}

pub async fn run_app() -> anyhow::Result<()> {
    // Load .env if it exists
    dotenvy::dotenv().ok();

    run_tailwind_build_if_needed()?;

    let asset_entrypoints = prepare_fingerprinted_assets()?;

    if should_prepare_assets_only() {
        return Ok(());
    }

    // Ensure critical environment variables are set
    env::var("JWT_SECRET").expect("JWT_SECRET must be set");

    // Initialize Tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "deductible_tracker=info,tower_http=info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting Deductible Tracker application...");

    let index_template = fs::read_to_string("static/index.html")?;

    // Database Setup
    tracing::info!("Initializing database connection pool...");
    let db_pool = db::init_pool().await?;
    tracing::info!("Database connection pool initialized successfully");

    // Oracle Object Storage Setup (Using S3 Compat via OpenDAL)
    let endpoint = env::var("OBJECT_STORAGE_ENDPOINT").expect("OBJECT_STORAGE_ENDPOINT must be set");
    let bucket_name = env::var("OBJECT_STORAGE_BUCKET").expect("OBJECT_STORAGE_BUCKET must be set");
    let region = env::var("OCI_REGION").unwrap_or_else(|_| "us-ashburn-1".to_string());
    
    // Explicitly load credentials from OCI_* env vars
    let access_key = env::var("OCI_ACCESS_KEY_ID").expect("OCI_ACCESS_KEY_ID must be set");
    let secret_key = env::var("OCI_SECRET_ACCESS_KEY").expect("OCI_SECRET_ACCESS_KEY must be set");

    let op: Operator = Operator::new(
        S3::default()
            .endpoint(&endpoint)
            .bucket(&bucket_name)
            .region(&region)
            .access_key_id(&access_key)
            .secret_access_key(&secret_key)
    )?.finish();

    let state = AppState {
        db: db_pool,
        storage: op,
        bucket_name,
        index_template,
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
        .layer(GovernorLayer::new(auth_governor_config));

    // Router Setup
    let app = Router::new()
        .route("/", get(serve_index))
        .route("/index.html", get(serve_index))
        .route("/health", get(health_check))
        // API Routes
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
        .route("/api/me", get(auth::me).put(auth::update_me))
        .route("/api/config", get(auth::get_config))
        .merge(auth_router)
        .route("/sw.js", get(serve_service_worker))
        .route("/assets/tailwind.css", get(serve_tailwind_css))
        .nest_service("/assets", ServeDir::new("public/assets"))
        .nest_service("/vendor", ServeDir::new("static/vendor"))
        .nest_service("/js", ServeDir::new("static/js"))
        .nest_service("/css", ServeDir::new("static/css"))
        .nest_service("/fonts", ServeDir::new("static/fonts"))
        .nest_service("/data", ServeDir::new("static/data"))
        .fallback(get(spa_fallback))
        .layer(from_fn(static_cache_control))
        .layer(from_fn(require_auth))
        .layer(cors)
        .layer(GovernorLayer::new(governor_config))
        .layer(TraceLayer::new_for_http())
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
            HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static("default-src 'self'; script-src 'self' https://accounts.google.com; script-src-elem 'self' https://accounts.google.com; style-src 'self' 'unsafe-inline' https://accounts.google.com; font-src 'self' https://fonts.gstatic.com data:; img-src 'self' data: blob: https://axi3e0fffvc5.compat.objectstorage.us-chicago-1.oraclecloud.com; connect-src 'self' https://accounts.google.com https://axi3e0fffvc5.compat.objectstorage.us-chicago-1.oraclecloud.com; frame-src https://accounts.google.com; frame-ancestors 'self' https://accounts.google.com;"),
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

