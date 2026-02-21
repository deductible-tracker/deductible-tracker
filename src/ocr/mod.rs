// Provide two implementations: a real OCR when the `ocr` feature is enabled, and
// a stub that returns an error when it's not. This avoids linking to system
// libraries (leptonica/tesseract) on machines where they're not installed.
#[cfg(feature = "ocr")]
mod real {
    use anyhow::anyhow;
    use crate::AppState;
    use leptess::LepTess;
    use tempfile::NamedTempFile;
    use std::io::Write;
    use chrono::NaiveDate;
    use regex::Regex;

    pub async fn run_ocr(state: &AppState, key: &str) -> anyhow::Result<(String, Option<NaiveDate>, Option<i64>)> {
        // Presign read URL from storage
        let presigned = state.storage.presign_read(key, std::time::Duration::from_secs(300)).await.map_err(|e| anyhow!("presign read: {}", e))?;
        let url = presigned.uri().to_string();

        // download object
        let resp = reqwest::get(&url).await.map_err(|e| anyhow!("fetching object: {}", e))?;
        if !resp.status().is_success() {
            return Err(anyhow!("failed to download object: {}", resp.status()));
        }
        let bytes = resp.bytes().await.map_err(|e| anyhow!("reading bytes: {}", e))?;

        // write to temp file
        let mut tmp = NamedTempFile::new().map_err(|e| anyhow!("tmpfile: {}", e))?;
        tmp.write_all(&bytes).map_err(|e| anyhow!("write tmp: {}", e))?;
        let path = tmp.path().to_string_lossy().to_string();

        // run tesseract via leptess
        let mut lt = LepTess::new(None, "eng").map_err(|e| anyhow!("tesseract init: {}", e))?;
        lt.set_image(&path);
        let text = lt.get_utf8_text().map_err(|e| anyhow!("tesseract run: {}", e))?;

        // simple heuristics for date and amount
        let mut found_date: Option<NaiveDate> = None;
        let mut found_amount: Option<i64> = None;

        // date patterns YYYY-MM-DD or MM/DD/YYYY or M/D/YYYY
        let re_iso = Regex::new(r"(\d{4}-\d{2}-\d{2})").unwrap();
        if let Some(cap) = re_iso.captures(&text) {
            if let Ok(d) = NaiveDate::parse_from_str(&cap[1], "%Y-%m-%d") {
                found_date = Some(d);
            }
        } else {
            let re_us = Regex::new(r"(\d{1,2}/\d{1,2}/\d{4})").unwrap();
            if let Some(cap) = re_us.captures(&text) {
                if let Ok(d) = NaiveDate::parse_from_str(&cap[1], "%m/%d/%Y") {
                    found_date = Some(d);
                }
            }
        }

        // amount patterns $12.34 or 12.34
        let re_amt = Regex::new(r"\$?([0-9]{1,3}(?:,[0-9]{3})*(?:\.[0-9]{2})|[0-9]+\.[0-9]{2})").unwrap();
        if let Some(cap) = re_amt.captures(&text) {
            let raw = &cap[1];
            let clean = raw.replace(",", "");
            if let Ok(f) = clean.parse::<f64>() {
                let cents = (f * 100.0).round() as i64;
                found_amount = Some(cents);
            }
        }

        Ok((text, found_date, found_amount))
    }
}

#[cfg(not(feature = "ocr"))]
mod stub {
    use anyhow::anyhow;
    use crate::AppState;
    use chrono::NaiveDate;

    pub async fn run_ocr(_state: &AppState, _key: &str) -> anyhow::Result<(String, Option<NaiveDate>, Option<i64>)> {
        Err(anyhow!("OCR feature not enabled; build with --features ocr and install Tesseract/Leptonica"))
    }
}

// Re-export a unified `run_ocr` symbol
#[cfg(feature = "ocr")]
pub use real::run_ocr;
#[cfg(not(feature = "ocr"))]
pub use stub::run_ocr;
