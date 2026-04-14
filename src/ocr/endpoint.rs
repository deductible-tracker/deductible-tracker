use anyhow::{anyhow, Context};
use std::env;
use url::Url;

pub(crate) const MISTRAL_OCR_API_ENDPOINT: &str = "https://api.mistral.ai/v1/ocr";

fn canonical_mistral_ocr_endpoint() -> Url {
    Url::parse(MISTRAL_OCR_API_ENDPOINT).expect("valid Mistral OCR endpoint")
}

fn validate_mistral_api_endpoint(configured: &Url, expected: &Url) -> anyhow::Result<()> {
    let configured_port = configured.port_or_known_default();
    let expected_port = expected.port_or_known_default();

    if configured.scheme() != expected.scheme()
        || configured.host_str() != expected.host_str()
        || configured_port != expected_port
        || configured.path() != expected.path()
        || configured.query().is_some()
        || configured.fragment().is_some()
        || !configured.username().is_empty()
        || configured.password().is_some()
    {
        return Err(anyhow!(
            "MISTRAL_API_ENDPOINT must resolve to {}",
            MISTRAL_OCR_API_ENDPOINT
        ));
    }

    Ok(())
}

pub(crate) fn load_mistral_api_endpoint() -> anyhow::Result<Url> {
    let canonical = canonical_mistral_ocr_endpoint();

    match env::var("MISTRAL_API_ENDPOINT") {
        Ok(configured) => {
            let configured = Url::parse(&configured)
                .context("MISTRAL_API_ENDPOINT must be a valid absolute URL")?;
            validate_mistral_api_endpoint(&configured, &canonical)?;
        }
        Err(env::VarError::NotPresent) => {}
        Err(env::VarError::NotUnicode(_)) => {
            return Err(anyhow!("MISTRAL_API_ENDPOINT must be valid UTF-8"));
        }
    }

    Ok(canonical)
}
