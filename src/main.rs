use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tower_http::services::ServeDir;
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

    // Initialize Tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "deductible_tracker=debug,tower_http=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Database Setup
    let db_pool = db::init_pool().await?;

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

    // Router Setup
    let app = Router::new()
        .route("/health", get(health_check))
        // API Routes
        .route("/api/donations", get(routes::donations::list_donations).post(routes::donations::create_donation))
        .route("/api/charities/search", get(routes::charities::search_charities))
        .route("/api/receipts/upload", post(routes::receipts::generate_upload_url))
        // Auth Routes
        .route("/auth/login/:provider", get(auth::login))
        .route("/auth/callback/:provider", get(auth::callback))
        // Dev only login
        .route("/auth/dev/login", post(auth::dev_login))
        .fallback_service(ServeDir::new("static"))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    tracing::info!("listening on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
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
