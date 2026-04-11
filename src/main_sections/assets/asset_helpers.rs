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

fn copy_passthrough_assets(source_root: &Path, dest_root: &Path) -> anyhow::Result<()> {
    if !source_root.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(source_root)? {
        let entry = entry?;
        let source_path = entry.path();
        let dest_path = dest_root.join(entry.file_name());

        if source_path.is_dir() {
            fs::create_dir_all(&dest_path)?;
            copy_passthrough_assets(&source_path, &dest_path)?;
            continue;
        }

        let ext = source_path.extension().and_then(|e| e.to_str());
        if ext == Some("js") || ext == Some("css") {
            continue;
        }

        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(&source_path, &dest_path)?;
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
        if ext == Some("gz") || ext == Some("br") {
            fs::remove_file(path)?;
            continue;
        }

        if ext == Some("js") || (ext == Some("css") && has_fingerprint_suffix(&path)) {
            fs::remove_file(path)?;
        }
    }

    Ok(())
}

fn write_generated_asset(path: &Path, content: &[u8]) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(path, content)?;
    write_precompressed_variants(path, content)?;
    Ok(())
}

fn write_precompressed_variants(path: &Path, content: &[u8]) -> anyhow::Result<()> {
    let ext = path.extension().and_then(|e| e.to_str());
    if ext != Some("js") && ext != Some("css") {
        return Ok(());
    }

    {
        use std::io::Write as _;

        let gz_path = PathBuf::from(format!("{}.gz", path.display()));
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
        encoder.write_all(content)?;
        let gzip_bytes = encoder.finish()?;
        fs::write(gz_path, gzip_bytes)?;
    }

    {
        use std::io::Write as _;

        let br_path = PathBuf::from(format!("{}.br", path.display()));
        let mut brotli_bytes = Vec::new();
        {
            let mut encoder = brotli::CompressorWriter::new(&mut brotli_bytes, 4096, 11, 22);
            encoder.write_all(content)?;
        }
        fs::write(br_path, brotli_bytes)?;
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

fn is_fingerprinted_asset_url(path: &str) -> bool {
    path.strip_prefix("/assets/")
        .map(|relative| has_fingerprint_suffix(Path::new(relative)))
        .unwrap_or(false)
}

fn retain_stable_precache_assets(precache_assets: &mut Vec<String>) {
    precache_assets.retain(|path| !is_fingerprinted_asset_url(path));
    precache_assets.sort();
    precache_assets.dedup();
}

fn build_service_worker_version(
    app: &str,
    upload: &str,
    dexie: &str,
    asset_rewrites: &std::collections::HashMap<String, String>,
    precache_assets: &[String],
) -> anyhow::Result<String> {
    let mut sorted_rewrites = asset_rewrites
        .iter()
        .map(|(original, rewritten)| (original.clone(), rewritten.clone()))
        .collect::<Vec<_>>();
    sorted_rewrites
        .sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));

    let version_seed = serde_json::json!({
        "app": app,
        "upload": upload,
        "dexie": dexie,
        "assetRewrites": sorted_rewrites,
        "precacheAssets": precache_assets,
    });
    let hash = blake3::hash(serde_json::to_string(&version_seed)?.as_bytes())
        .to_hex()
        .to_string();

    Ok(hash[..12].to_string())
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

