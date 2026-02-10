use axum::{
    routing::{get, post},
    Router,
    http::HeaderValue,
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

mod db;
mod routes;
mod auth;

use db::DbPool;

#[derive(Clone)]
pub struct AppState {
    pub db: DbPool,
    pub storage: Operator,
    pub bucket_name: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env if it exists
    dotenvy::dotenv().ok();

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
    };

    let governor_config = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(5)
            .burst_size(20)
            .finish()
            .expect("governor config"),
    );

    // CORS configuration (no permissive mode)
    let cors = {
        let env_mode = env::var("RUST_ENV").unwrap_or_else(|_| "development".to_string());
        let origins = env::var("ALLOWED_ORIGINS")
            .ok()
            .map(|v| {
                v.split(',')
                    .map(|s| s.trim().parse::<HeaderValue>().unwrap())
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
            ])
            .allow_credentials(true)
    };

    // Router Setup
    let app = Router::new()
        .route("/health", get(health_check))
        // API Routes
        .route("/api/donations", get(routes::donations::list_donations).post(routes::donations::create_donation))
        .route("/api/charities/search", get(routes::charities::search_charities))
        .route("/api/receipts/upload", post(routes::receipts::generate_upload_url))
        .route("/api/me", get(auth::me))
        // Auth Routes
        .route("/auth/login/:provider", get(auth::login))
        .route("/auth/callback/:provider", get(auth::callback))
        .route("/auth/logout", post(auth::logout))
        // Dev only login
        .route("/auth/dev/login", post(auth::dev_login))
        .fallback_service(ServeDir::new("static"))
        .layer(cors)
        .layer(GovernorLayer { config: governor_config })
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
            HeaderValue::from_static("default-src 'self'; script-src 'self' https://cdn.jsdelivr.net https://cdn.tailwindcss.com https://unpkg.com; script-src-elem 'self' https://cdn.jsdelivr.net https://cdn.tailwindcss.com https://unpkg.com; style-src 'self' 'unsafe-inline' https://cdn.jsdelivr.net https://cdn.tailwindcss.com; img-src 'self' data:; connect-src 'self';"),
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

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("signal received, starting graceful shutdown");
}

async fn health_check() -> &'static str {
    "OK"
}
