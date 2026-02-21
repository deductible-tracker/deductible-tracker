use axum::{
    extract::State,
    routing::{get, post, delete},
    Router,
    middleware::{from_fn, Next},
    http::{HeaderValue, StatusCode},
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

mod db;
mod routes;
mod auth;
mod ocr;

use db::DbPool;

#[derive(Clone)]
pub struct AppState {
    pub db: DbPool,
    pub storage: Operator,
    pub bucket_name: String,
    pub index_template: String,
    pub asset_entrypoints: AssetEntrypoints,
}

#[derive(Clone)]
pub struct AssetEntrypoints {
    pub app: String,
    pub upload: String,
    pub valuations: String,
    pub css_rewrites: HashMap<String, String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env if it exists
    dotenvy::dotenv().ok();

    run_tailwind_build_if_needed()?;

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

    // Generate fingerprinted JS assets for long-lived browser caching.
    let asset_entrypoints = prepare_fingerprinted_assets()?;
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
                    .unwrap_or(1200),
            )
            .burst_size(
                env::var("RATE_LIMIT_BURST")
                    .ok()
                    .and_then(|v| v.parse::<u32>().ok())
                    .unwrap_or(2400),
            )
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
            ])
            .allow_credentials(true)
    };

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
        .route("/api/reports/years", get(routes::reports::list_available_years))
        .route("/api/reports/export", get(routes::reports::export_csv))
        .route("/api/reports/export/txf", get(routes::reports::export_tax_txf))
        .route("/api/reports/audit", get(routes::reports::export_audit_csv))
        .route("/api/me", get(auth::me).put(auth::update_me))
        // Auth Routes
        .route("/auth/login/{provider}", get(auth::login))
        .route("/auth/callback/{provider}", get(auth::callback))
        .route("/auth/logout", post(auth::logout))
        // Dev only login
        .route("/auth/dev/login", post(auth::dev_login))
        .nest_service("/assets", ServeDir::new("static/assets"))
        .nest_service("/js", ServeDir::new("static/js"))
        .nest_service("/css", ServeDir::new("static/css"))
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
            HeaderValue::from_static("default-src 'self'; script-src 'self' https://cdn.jsdelivr.net https://cdn.tailwindcss.com https://unpkg.com; script-src-elem 'self' https://cdn.jsdelivr.net https://cdn.tailwindcss.com https://unpkg.com; style-src 'self' 'unsafe-inline' https://cdn.jsdelivr.net https://cdn.tailwindcss.com https://fonts.googleapis.com; font-src 'self' https://fonts.gstatic.com data:; img-src 'self' data:; connect-src 'self' https://unpkg.com;"),
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

fn run_tailwind_build_if_needed() -> anyhow::Result<()> {
    let env_mode = env::var("RUST_ENV").unwrap_or_else(|_| "development".to_string());
    if env_mode != "development" {
        return Ok(());
    }

    if env::var("SKIP_TAILWIND_BUILD")
        .map(|v| v.eq_ignore_ascii_case("1") || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        return Ok(());
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let status = Command::new("bash")
        .arg("-lc")
        .arg("if [ -s \"$HOME/.nvm/nvm.sh\" ]; then source \"$HOME/.nvm/nvm.sh\" && nvm use 24.13.1 >/dev/null 2>&1 || true; fi; npm run tailwind:build")
        .current_dir(&manifest_dir)
        .status()?;

    if !status.success() {
        anyhow::bail!(
            "Tailwind build failed. Run `make tailwind-build` and ensure Node v24.13.1 is available."
        );
    }

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

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn db_pool_initializes() {
        std::env::set_var("RUST_ENV", "development");
        let pool = crate::db::init_pool().await.expect("init pool");
        // Ensure we got a pool back (type check only)
        match &*pool {
            crate::db::DbPoolEnum::Sqlite(_) => assert!(true),
            crate::db::DbPoolEnum::Oracle(_) => assert!(true),
        }
    }
}

async fn require_auth(req: Request<Body>, next: Next) -> impl IntoResponse {
    // Guard only API endpoints here; non-API routes are SPA/document requests.
    let path = req.uri().path();
    if req.method() == axum::http::Method::OPTIONS || !path.starts_with("/api/") {
        return next.run(req).await;
    }

    // Check headers for token
    let headers: &HeaderMap = req.headers();
    if let Some(token) = auth::extract_token_from_headers(headers) {
        if auth::validate_token_str(&token).is_ok() {
            return next.run(req).await;
        }
    }

    // Not authenticated: API routes get 401.
    (axum::http::StatusCode::UNAUTHORIZED, "Unauthorized").into_response()
}

async fn serve_index(State(state): State<AppState>) -> impl IntoResponse {
    let mut html = state
        .index_template
        .replace("/assets/app.js", &state.asset_entrypoints.app)
        .replace("/assets/upload.js", &state.asset_entrypoints.upload)
        .replace("/assets/valuations.js", &state.asset_entrypoints.valuations);
    for (original, fingerprinted) in &state.asset_entrypoints.css_rewrites {
        html = html.replace(original, fingerprinted);
    }
    Html(html)
}

async fn spa_fallback(State(state): State<AppState>, req: Request<Body>) -> impl IntoResponse {
    let path = req.uri().path();
    if path.starts_with("/api/") {
        return StatusCode::NOT_FOUND.into_response();
    }
    serve_index(State(state)).await.into_response()
}

async fn static_cache_control(req: Request<Body>, next: Next) -> impl IntoResponse {
    let path = req.uri().path().to_string();
    let mut response = next.run(req).await;

    // Fingerprinted assets can be cached for a year.
    if path.starts_with("/assets/")
        && (path.ends_with(".js") || path.ends_with(".css"))
        && path.contains('-')
        && response.status() == StatusCode::OK
    {
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=31536000, immutable"),
        );
        return response;
    }

    // Loader assets and HTML should revalidate so users pick up new fingerprint mappings.
    if ((path.starts_with("/assets/") && (path.ends_with(".js") || path.ends_with(".css"))) || path == "/" || path == "/index.html")
        && response.status() == StatusCode::OK
    {
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-cache"),
        );
    }

    response
}

fn prepare_fingerprinted_assets() -> anyhow::Result<AssetEntrypoints> {
    let static_root = Path::new("static");
    let js_root = static_root.join("js");
    let assets_root = static_root.join("assets");

    fs::create_dir_all(&assets_root)?;
    clear_generated_fingerprinted_assets(&assets_root)?;

    let mut app = "/assets/app.js".to_string();
    let mut upload = "/assets/upload.js".to_string();
    let mut valuations = "/assets/valuations.js".to_string();

    if js_root.exists() {
        let mut files = Vec::new();
        collect_js_files(&js_root, &js_root, &mut files)?;

        let mut raw_by_rel: HashMap<PathBuf, String> = HashMap::new();
        let mut fp_by_rel: HashMap<PathBuf, PathBuf> = HashMap::new();

        for rel in &files {
            let src = js_root.join(rel);
            let content = fs::read_to_string(&src)?;
            let minified = minify_js_asset(&content);
            let hash = blake3::hash(minified.as_bytes()).to_hex().to_string();
            let short_hash = &hash[..12];

            let stem = rel
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or_else(|| anyhow::anyhow!("Invalid JS file name: {}", rel.display()))?;

            let hashed_name = format!("{}-{}.js", stem, short_hash);
            let mut fp_rel = rel.clone();
            fp_rel.set_file_name(hashed_name);

            raw_by_rel.insert(rel.clone(), minified);
            fp_by_rel.insert(rel.clone(), fp_rel);
        }

        let import_spec_re = Regex::new(r#"['\"]\.{1,2}/[^'\"]+\.js['\"]"#)?;

        for rel in &files {
            let original = raw_by_rel
                .get(rel)
                .ok_or_else(|| anyhow::anyhow!("Missing source content for {}", rel.display()))?;
            let current_dir = rel.parent().unwrap_or(Path::new(""));

            let rewritten = import_spec_re.replace_all(original, |caps: &regex::Captures| {
                let matched = caps.get(0).map(|m| m.as_str()).unwrap_or("");
                if matched.len() < 3 {
                    return matched.to_string();
                }

                let quote = &matched[0..1];
                let spec = &matched[1..matched.len() - 1];

                if let Some(target_rel) = resolve_js_relative(current_dir, spec) {
                    if let Some(target_fp_rel) = fp_by_rel.get(&target_rel) {
                        let from_dir = current_dir;
                        let new_rel = relative_path(from_dir, target_fp_rel);
                        let spec_out = new_rel
                            .to_string_lossy()
                            .replace('\\', "/");
                        let spec_out = if spec_out.starts_with("../") || spec_out.starts_with("./") {
                            spec_out
                        } else {
                            format!("./{}", spec_out)
                        };
                        return format!("{}{}{}", quote, spec_out, quote);
                    }
                }

                matched.to_string()
            });

            let out_rel = fp_by_rel
                .get(rel)
                .ok_or_else(|| anyhow::anyhow!("Missing fingerprinted path for {}", rel.display()))?;
            let out_path = assets_root.join(out_rel);
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&out_path, rewritten.as_bytes())?;
        }

        app = fp_by_rel
            .get(&PathBuf::from("app.js"))
            .map(|p| format!("/assets/{}", p.to_string_lossy().replace('\\', "/")))
            .ok_or_else(|| anyhow::anyhow!("Missing fingerprinted app.js"))?;
        upload = fp_by_rel
            .get(&PathBuf::from("upload.js"))
            .map(|p| format!("/assets/{}", p.to_string_lossy().replace('\\', "/")))
            .ok_or_else(|| anyhow::anyhow!("Missing fingerprinted upload.js"))?;
        valuations = fp_by_rel
            .get(&PathBuf::from("valuations.js"))
            .map(|p| format!("/assets/{}", p.to_string_lossy().replace('\\', "/")))
            .ok_or_else(|| anyhow::anyhow!("Missing fingerprinted valuations.js"))?;
    } else {
        tracing::warn!("Skipping JS fingerprinting: {} does not exist", js_root.display());
    }

    let mut css_files = Vec::new();
    collect_css_files(&assets_root, &assets_root, &mut css_files)?;
    let mut css_rewrites: HashMap<String, String> = HashMap::new();

    for rel in &css_files {
        let src = assets_root.join(rel);
        let content = fs::read_to_string(&src)?;
        let minified = minify_css_asset(&content);
        let hash = blake3::hash(minified.as_bytes()).to_hex().to_string();
        let short_hash = &hash[..12];

        let stem = rel
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid CSS file name: {}", rel.display()))?;

        let hashed_name = format!("{}-{}.css", stem, short_hash);
        let mut fp_rel = rel.clone();
        fp_rel.set_file_name(hashed_name);

        let out_path = assets_root.join(&fp_rel);
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&out_path, minified.as_bytes())?;

        let original_path = format!("/assets/{}", rel.to_string_lossy().replace('\\', "/"));
        let hashed_path = format!("/assets/{}", fp_rel.to_string_lossy().replace('\\', "/"));
        css_rewrites.insert(original_path, hashed_path);
    }

    Ok(AssetEntrypoints {
        app,
        upload,
        valuations,
        css_rewrites,
    })
}

fn collect_js_files(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_js_files(root, &path, out)?;
            continue;
        }

        if path.extension().and_then(|e| e.to_str()) == Some("js") {
            let rel = path
                .strip_prefix(root)
                .map_err(|e| anyhow::anyhow!("strip_prefix failed for {}: {}", path.display(), e))?
                .to_path_buf();
            out.push(rel);
        }
    }
    Ok(())
}

fn collect_css_files(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_css_files(root, &path, out)?;
            continue;
        }

        if path.extension().and_then(|e| e.to_str()) == Some("css") {
            let rel = path
                .strip_prefix(root)
                .map_err(|e| anyhow::anyhow!("strip_prefix failed for {}: {}", path.display(), e))?
                .to_path_buf();
            out.push(rel);
        }
    }
    Ok(())
}

fn clear_generated_fingerprinted_assets(dir: &Path) -> anyhow::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            clear_generated_fingerprinted_assets(&path)?;
            continue;
        }

        let ext = path.extension().and_then(|e| e.to_str());
        if ext == Some("js") || (ext == Some("css") && has_fingerprint_suffix(&path)) {
            fs::remove_file(path)?;
        }
    }

    Ok(())
}

fn minify_js_asset(content: &str) -> String {
    minifier::js::minify(content).to_string()
}

fn minify_css_asset(content: &str) -> String {
    match minifier::css::minify(content) {
        Ok(minified) => minified.to_string(),
        Err(err) => {
            tracing::warn!("Skipping CSS minification due to parse error: {}", err);
            content.to_string()
        }
    }
}

fn has_fingerprint_suffix(path: &Path) -> bool {
    let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
        return false;
    };

    let Some((_, maybe_hash)) = stem.rsplit_once('-') else {
        return false;
    };

    maybe_hash.len() == 12 && maybe_hash.chars().all(|c| c.is_ascii_hexdigit())
}

fn resolve_js_relative(current_dir: &Path, spec: &str) -> Option<PathBuf> {
    if !(spec.starts_with("./") || spec.starts_with("../")) {
        return None;
    }

    let mut combined = PathBuf::from(current_dir);
    combined.push(spec);

    let mut normalized = PathBuf::new();
    for comp in combined.components() {
        use std::path::Component;
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
            _ => return None,
        }
    }
    Some(normalized)
}

fn relative_path(from_dir: &Path, to_file: &Path) -> PathBuf {
    let from_parts: Vec<_> = from_dir.components().collect();
    let to_parts: Vec<_> = to_file.components().collect();

    let mut common = 0usize;
    while common < from_parts.len() && common < to_parts.len() && from_parts[common] == to_parts[common] {
        common += 1;
    }

    let mut rel = PathBuf::new();
    for _ in common..from_parts.len() {
        rel.push("..");
    }
    for comp in &to_parts[common..] {
        rel.push(comp.as_os_str());
    }
    rel
}
