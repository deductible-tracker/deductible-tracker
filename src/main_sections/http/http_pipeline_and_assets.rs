#[cfg(feature = "asset-pipeline")]
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
    let prebuilt_tailwind = public_assets_root().join("tailwind.css");

    if !manifest_dir.join("node_modules").exists() && prebuilt_tailwind.exists() {
        tracing::info!(
            "Skipping Tailwind build: using prebuilt CSS at {}",
            prebuilt_tailwind.display()
        );
        return Ok(());
    }

    tracing::info!("Running Tailwind build...");
    
    // 1. Try local node_modules binary directly
    let local_tailwind = manifest_dir.join("node_modules").join(".bin").join("tailwindcss");
    if local_tailwind.exists() {
        tracing::debug!("Found local tailwindcss at {}", local_tailwind.display());
        let status = Command::new(&local_tailwind)
            .args([
                "-i", "./static/css/input.css",
                "-o", "./public/assets/tailwind.css",
                "--minify"
            ])
            .current_dir(&manifest_dir)
            .status();

        if let Ok(s) = status {
            if s.success() {
                tracing::info!("Tailwind build completed successfully via local binary");
                return Ok(());
            }
        }
    }

    // 2. Fallback: Try npm run tailwind:build
    let status = Command::new("npm")
        .args(["run", "tailwind:build"])
        .current_dir(&manifest_dir)
        .status();

    if let Ok(s) = status {
        if s.success() {
            tracing::info!("Tailwind build completed successfully via npm run");
            return Ok(());
        }
    }

    // 3. Last resort: Try via npx
    let npx_status = Command::new("npx")
        .args([
            "tailwindcss",
            "-i", "./static/css/input.css",
            "-o", "./public/assets/tailwind.css",
            "--minify"
        ])
        .current_dir(&manifest_dir)
        .status();

    if let Ok(s) = npx_status {
        if s.success() {
            tracing::info!("Tailwind build completed successfully via npx");
            return Ok(());
        }
    }

    let node_modules_exists = manifest_dir.join("node_modules").exists();
    anyhow::bail!(
        "Tailwind build failed. (node_modules exists: {}, prebuilt CSS exists: {}). Ensure Node.js and tailwindcss are installed and NODE_ENV is not 'production' during build.",
        node_modules_exists,
        prebuilt_tailwind.exists()
    );
}

#[cfg(feature = "asset-pipeline")]
fn should_prepare_assets_only() -> bool {
    env::var("PREPARE_ASSETS_ONLY")
        .map(|v| v.eq_ignore_ascii_case("1") || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

#[cfg(feature = "asset-pipeline")]
fn should_skip_asset_rebuild() -> bool {
    env::var("SKIP_ASSET_REBUILD")
        .map(|v| v.eq_ignore_ascii_case("1") || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn public_assets_root() -> PathBuf {
    Path::new("public").join("assets")
}

fn asset_manifest_path() -> PathBuf {
    public_assets_root().join(".asset-manifest.json")
}

fn load_asset_manifest() -> anyhow::Result<AssetEntrypoints> {
    let manifest = fs::read_to_string(asset_manifest_path())?;
    Ok(serde_json::from_str(&manifest)?)
}

fn load_service_worker_script() -> anyhow::Result<String> {
    Ok(fs::read_to_string(Path::new("public").join("sw.js"))?)
}

#[cfg(feature = "asset-pipeline")]
fn write_asset_manifest(entrypoints: &AssetEntrypoints) -> anyhow::Result<()> {
    let manifest_path = asset_manifest_path();
    if let Some(parent) = manifest_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&manifest_path, serde_json::to_vec_pretty(entrypoints)?)?;
    Ok(())
}

#[cfg(feature = "asset-pipeline")]
pub fn prepare_runtime_assets() -> anyhow::Result<()> {
    // When assets were already built (e.g. Docker image), skip entirely.
    if should_skip_asset_rebuild() {
        return Ok(());
    }

    run_tailwind_build_if_needed()?;

    let asset_entrypoints = prepare_fingerprinted_assets()?;
    let service_worker_script = build_service_worker_script(&asset_entrypoints)?;
    let service_worker_path = Path::new("public").join("sw.js");

    if let Some(parent) = service_worker_path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(service_worker_path, service_worker_script)?;
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

async fn require_auth(req: Request<Body>, next: Next) -> impl IntoResponse {
    // Guard only API endpoints here; non-API routes are SPA/document requests.
    let path = req.uri().path();
    if req.method() == axum::http::Method::OPTIONS || !path.starts_with("/api/") || path == "/api/config" {
        return next.run(req).await;
    }

    // CSRF Protection for state-changing methods
    if matches!(
        req.method(),
        &axum::http::Method::POST | &axum::http::Method::PUT | &axum::http::Method::DELETE | &axum::http::Method::PATCH
    ) {
        let headers = req.headers();
        let auth_token = auth::extract_token_from_headers(headers);
        if let Some(_auth) = auth_token {
            // Expect the client to send X-CSRF-Token equal to the readable csrf cookie value
            let csrf_header = headers.get("X-CSRF-Token").and_then(|h| h.to_str().ok());
            let csrf_cookie = headers
                .get(header::COOKIE)
                .and_then(|h| h.to_str().ok())
                .and_then(|cookie_header| {
                    cookie_header.split(';').find_map(|cookie| {
                        let cookie = cookie.trim();
                        if let Some((k, v)) = cookie.split_once('=') {
                            if k == "csrf_token" {
                                return Some(v.to_string());
                            }
                        }
                        None
                    })
                });

            match (csrf_header, csrf_cookie) {
                (Some(hdr), Some(cookie_val)) => {
                    if hdr != cookie_val {
                        tracing::warn!("CSRF token mismatch");
                        return (StatusCode::FORBIDDEN, "CSRF token mismatch").into_response();
                    }
                }
                _ => {
                    tracing::warn!("Missing CSRF token for authenticated state-changing request");
                    return (StatusCode::FORBIDDEN, "Missing CSRF token").into_response();
                }
            }
        }
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
    let bootstrap = serde_json::json!({
        "dexie": state.asset_entrypoints.dexie,
        "serviceWorkerVersion": state.asset_entrypoints.service_worker_version,
    });
    let bootstrap_b64 = base64::engine::general_purpose::STANDARD.encode(bootstrap.to_string());
    let mut html = state
        .index_template
        .replace("/assets/app.js", &state.asset_entrypoints.app)
        .replace("/assets/upload.js", &state.asset_entrypoints.upload)
        .replace("__DT_BOOTSTRAP_B64__", &bootstrap_b64);
    for (original, fingerprinted) in &state.asset_entrypoints.asset_rewrites {
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

#[cfg(feature = "asset-pipeline")]
fn build_service_worker_script(entrypoints: &AssetEntrypoints) -> anyhow::Result<String> {
    let template = std::fs::read_to_string("static/sw.js")?;
    let cache_name = format!("dt-cache-{}", entrypoints.service_worker_version);
    let precache_assets = serde_json::to_string(&entrypoints.precache_assets)?;
    let rendered = template
        .replace("__DT_CACHE_NAME__", &cache_name)
        .replace("__DT_PRECACHE_ASSETS__", &precache_assets);
    Ok(minify_js_asset(&rendered))
}

async fn serve_service_worker(State(state): State<AppState>) -> impl IntoResponse {
    let mut response = (
        [
            (header::CONTENT_TYPE, "application/javascript"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        state.service_worker_script.clone(),
    )
        .into_response();
    response.headers_mut().insert(
        HeaderName::from_static("service-worker-allowed"),
        HeaderValue::from_static("/"),
    );
    response
}

async fn serve_tailwind_css() -> impl IntoResponse {
    // Prefer the compiled asset if present in public/assets, otherwise fall back
    // to a minimal development CSS (static input) so clients don't get 404s.
    let compiled = public_assets_root().join("tailwind.css");
    if compiled.exists() {
        if let Ok(bytes) = std::fs::read(&compiled) {
            return (
                [
                    (header::CONTENT_TYPE, "text/css"),
                    (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
                ],
                bytes,
            )
                .into_response();
        }
    }

    // Fallback: serve the source input.css so the page still gets some styles.
    if let Ok(s) = std::fs::read_to_string("static/css/input.css") {
        return (
            [
                (header::CONTENT_TYPE, "text/css"),
                (header::CACHE_CONTROL, "no-cache"),
            ],
            s,
        )
            .into_response();
    }

    // Final fallback: return empty CSS
    (
        [
            (header::CONTENT_TYPE, "text/css"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        "/* tailwind.css missing */\n".to_string(),
    )
        .into_response()
}

async fn static_cache_control(req: Request<Body>, next: Next) -> impl IntoResponse {
    let path = req.uri().path().to_string();
    let mut response = next.run(req).await;

    // Security headers (HSTS, X-Content-Type-Options, X-Frame-Options, CSP)
    // are set via SetResponseHeaderLayer on the outer Router — do NOT
    // duplicate them here. This middleware handles only Cache-Control.

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

    if path.starts_with("/fonts/")
        && (path.ends_with(".woff2") || path.ends_with(".woff"))
        && response.status() == StatusCode::OK
    {
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=31536000, immutable"),
        );
        return response;
    }

    if path.starts_with("/vendor/")
        && path.contains('-')
        && path.ends_with(".js")
        && response.status() == StatusCode::OK
    {
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=31536000, immutable"),
        );
        return response;
    }

    // Loader assets and HTML should revalidate so users pick up new fingerprint mappings.
    if path == "/sw.js" {
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-cache"),
        );
        return response;
    }

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

#[cfg(feature = "asset-pipeline")]
fn prepare_fingerprinted_assets() -> anyhow::Result<AssetEntrypoints> {
    if should_skip_asset_rebuild() {
        return load_asset_manifest();
    }

    let static_root = Path::new("static");
    let js_root = static_root.join("js");
    let vendor_root = static_root.join("vendor");
    let css_root = static_root.join("css");
    let source_assets_root = static_root.join("assets");
    let assets_root = public_assets_root();

    fs::create_dir_all(&assets_root)?;
    clear_generated_fingerprinted_assets(&assets_root)?;
    copy_passthrough_assets(&source_assets_root, &assets_root)?;

    let mut app = "/assets/app.js".to_string();
    let mut upload = "/assets/upload.js".to_string();
    let mut dexie = "/vendor/dexie-4.3.0.min.js".to_string();
    let mut asset_rewrites: HashMap<String, String> = HashMap::new();
    let mut precache_assets = vec!["/".to_string()];

    if js_root.exists() {
        let mut files = Vec::new();
        collect_js_files(&js_root, &js_root, &mut files)?;

        let mut raw_by_rel: HashMap<PathBuf, String> = HashMap::new();
        let mut fp_by_rel: HashMap<PathBuf, PathBuf> = HashMap::new();

        for rel in &files {
            let src = js_root.join(rel);
            let content = fs::read_to_string(&src)?;
            let minified = minify_js_asset(&content);

            raw_by_rel.insert(rel.clone(), minified);
        }

        let mut js_graph_seed = files
            .iter()
            .filter_map(|rel| {
                raw_by_rel
                    .get(rel)
                    .map(|content| (rel.to_string_lossy().to_string(), content.clone()))
            })
            .collect::<Vec<_>>();
        js_graph_seed.sort_by(|left, right| left.0.cmp(&right.0));
        let js_build_hash = blake3::hash(serde_json::to_string(&js_graph_seed)?.as_bytes())
            .to_hex()
            .to_string();

        for rel in &files {
            let minified = raw_by_rel
                .get(rel)
                .ok_or_else(|| anyhow::anyhow!("Missing source content for {}", rel.display()))?;
            let hash = blake3::hash(format!("{}:{}", js_build_hash, minified).as_bytes())
                .to_hex()
                .to_string();
            let short_hash = &hash[..12];

            let stem = rel
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or_else(|| anyhow::anyhow!("Invalid JS file name: {}", rel.display()))?;

            let hashed_name = format!("{}-{}.js", stem, short_hash);
            let mut fp_rel = rel.clone();
            fp_rel.set_file_name(hashed_name);

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
            write_generated_asset(&out_path, rewritten.as_bytes())?;
        }

        app = fp_by_rel
            .get(&PathBuf::from("boot.js"))
            .map(|p| format!("/assets/{}", p.to_string_lossy().replace('\\', "/")))
            .ok_or_else(|| anyhow::anyhow!("Missing fingerprinted boot.js"))?;
        upload = fp_by_rel
            .get(&PathBuf::from("upload.js"))
            .map(|p| format!("/assets/{}", p.to_string_lossy().replace('\\', "/")))
            .ok_or_else(|| anyhow::anyhow!("Missing fingerprinted upload.js"))?;
        precache_assets.push(app.clone());
    } else {
        tracing::warn!("Skipping JS fingerprinting: {} does not exist", js_root.display());
    }

    if vendor_root.exists() {
        let mut vendor_files = Vec::new();
        collect_js_files(&vendor_root, &vendor_root, &mut vendor_files)?;

        for rel in &vendor_files {
            let src = vendor_root.join(rel);
            let content = fs::read_to_string(&src)?;
            let minified = minify_js_asset(&content);
            let hash = blake3::hash(minified.as_bytes()).to_hex().to_string();
            let short_hash = &hash[..12];

            let stem = rel
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or_else(|| anyhow::anyhow!("Invalid vendor JS file name: {}", rel.display()))?;

            let hashed_name = format!("{}-{}.js", stem, short_hash);
            let mut fp_rel = PathBuf::from("vendor");
            if let Some(parent) = rel.parent() {
                fp_rel.push(parent);
            }
            fp_rel.push(hashed_name);

            let out_path = assets_root.join(&fp_rel);
            write_generated_asset(&out_path, minified.as_bytes())?;

            let original_path = format!("/vendor/{}", rel.to_string_lossy().replace('\\', "/"));
            let hashed_path = format!("/assets/{}", fp_rel.to_string_lossy().replace('\\', "/"));
            asset_rewrites.insert(original_path.clone(), hashed_path.clone());

            if rel == &PathBuf::from("dexie-4.3.0.min.js") {
                dexie = hashed_path.clone();
                precache_assets.push(hashed_path);
            }
        }
    }

    if css_root.exists() {
        let mut static_css_files = Vec::new();
        collect_css_files(&css_root, &css_root, &mut static_css_files)?;

        for rel in &static_css_files {
            if rel.file_name().and_then(|s| s.to_str()) == Some("input.css") {
                continue;
            }

            let src = css_root.join(rel);
            let content = fs::read_to_string(&src)?;
            let minified = minify_css_asset(&content);
            let hash = blake3::hash(minified.as_bytes()).to_hex().to_string();
            let short_hash = &hash[..12];

            let stem = rel
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or_else(|| anyhow::anyhow!("Invalid CSS file name: {}", rel.display()))?;

            let hashed_name = format!("{}-{}.css", stem, short_hash);
            let mut fp_rel = PathBuf::from("css");
            if let Some(parent) = rel.parent() {
                fp_rel.push(parent);
            }
            fp_rel.push(hashed_name);

            let out_path = assets_root.join(&fp_rel);
            write_generated_asset(&out_path, minified.as_bytes())?;

            let original_path = format!("/css/{}", rel.to_string_lossy().replace('\\', "/"));
            let hashed_path = format!("/assets/{}", fp_rel.to_string_lossy().replace('\\', "/"));
            asset_rewrites.insert(original_path, hashed_path.clone());

            if rel == &PathBuf::from("fonts.css") {
                precache_assets.push(hashed_path);
            }
        }
    }

    let mut css_files = Vec::new();
    collect_css_files(&assets_root, &assets_root, &mut css_files)?;

    for rel in &css_files {
        if has_fingerprint_suffix(rel) {
            continue;
        }

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
        write_generated_asset(&out_path, minified.as_bytes())?;

        let original_path = format!("/assets/{}", rel.to_string_lossy().replace('\\', "/"));
        let hashed_path = format!("/assets/{}", fp_rel.to_string_lossy().replace('\\', "/"));
        asset_rewrites.insert(original_path, hashed_path.clone());

        if rel == &PathBuf::from("tailwind.css") {
            precache_assets.push(hashed_path);
        }
    }

    // Fingerprinted /assets/* files are generated by the asset pipeline and
    // should be runtime-cached, not embedded into sw.js as install-time entries.
    retain_stable_precache_assets(&mut precache_assets);

    let service_worker_version = build_service_worker_version(
        &app,
        &upload,
        &dexie,
        &asset_rewrites,
        &precache_assets,
    )?;

    let entrypoints = AssetEntrypoints {
        app,
        upload,
        dexie,
        service_worker_version,
        asset_rewrites,
        precache_assets,
    };

    write_asset_manifest(&entrypoints)?;

    Ok(entrypoints)
}

