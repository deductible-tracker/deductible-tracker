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

async fn require_auth(req: Request<Body>, next: Next) -> impl IntoResponse {
    // Guard only API endpoints here; non-API routes are SPA/document requests.
    let path = req.uri().path();
    if req.method() == axum::http::Method::OPTIONS || !path.starts_with("/api/") || path == "/api/config" {
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

async fn serve_service_worker() -> impl IntoResponse {
    match std::fs::read_to_string("static/sw.js") {
        Ok(content) => (
            [
                (header::CONTENT_TYPE, "application/javascript"),
                (header::CACHE_CONTROL, "no-cache"),
            ],
            content,
        )
            .into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
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

