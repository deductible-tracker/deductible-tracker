use serde::Deserialize;
use std::env;
use std::sync::OnceLock;

use super::charity_enrichment::{
    clean_opt_string, derive_status, map_category_from_ntee, map_deductibility,
    map_deductibility_from_exempt_status, map_foundation_label, map_nonprofit_type,
    normalize_ein, normalize_i64_ein,
};

static SEARCH_HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
static PROPUBLICA_BASE_URL: OnceLock<Option<String>> = OnceLock::new();

#[derive(Debug, Deserialize)]
struct ProPublicaResponse {
    organizations: Vec<ProPublicaSearchOrg>,
}

#[derive(Debug, Deserialize)]
struct ProPublicaSearchOrg {
    ein: Option<i64>,
    strein: Option<String>,
    name: String,
    city: Option<String>,
    state: Option<String>,
    ntee_code: Option<String>,
    subseccd: Option<i64>,
    score: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct ProPublicaOrganizationResponse {
    organization: ProPublicaOrganization,
}

#[derive(Debug, Deserialize)]
struct ProPublicaOrganization {
    ein: Option<i64>,
    strein: Option<String>,
    name: Option<String>,
    address: Option<String>,
    city: Option<String>,
    state: Option<String>,
    zipcode: Option<String>,
    ntee_code: Option<String>,
    deductibility_code: Option<i64>,
    subsection_code: Option<i64>,
    foundation_code: Option<i64>,
    exempt_organization_status_code: Option<i64>,
}

#[derive(Debug, Clone)]
pub(super) struct CharitySearchHit {
    pub ein: String,
    pub name: String,
    pub city: Option<String>,
    pub state: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub(super) struct EnrichedCharityData {
    pub name: Option<String>,
    pub ein: Option<String>,
    pub category: Option<String>,
    pub status: Option<String>,
    pub classification: Option<String>,
    pub nonprofit_type: Option<String>,
    pub deductibility: Option<String>,
    pub street: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub zip: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum SearchError {
    MissingConfig,
    Upstream,
    Transport,
}

pub(super) async fn search_charities_by_query(query: &str) -> Result<Vec<CharitySearchHit>, SearchError> {
    let Some(base) = propublica_base_url() else {
        return Err(SearchError::MissingConfig);
    };

    let url = format!(
        "{}/search.json?q={}",
        base,
        url::form_urlencoded::byte_serialize(query.as_bytes()).collect::<String>()
    );

    let client = charity_search_client();
    let resp = client
        .get(&url)
        .header("User-Agent", "DeductibleTracker/1.0")
        .send()
        .await
        .map_err(|_| SearchError::Transport)?;

    if !resp.status().is_success() {
        return Err(SearchError::Upstream);
    }

    let data: ProPublicaResponse = resp
        .json()
        .await
        .unwrap_or(ProPublicaResponse {
            organizations: vec![],
        });

    Ok(data
        .organizations
        .into_iter()
        .map(|org| CharitySearchHit {
            ein: propublica_ein_from_search(&org).unwrap_or_default(),
            name: org.name,
            city: org.city,
            state: org.state,
        })
        .collect())
}

pub(super) async fn fetch_charity_from_propublica(name: &str) -> Option<EnrichedCharityData> {
    let client = charity_search_client();
    let base = propublica_base_url()?;
    let term = name.trim();
    if term.len() < 2 {
        return None;
    }

    let encoded_term = url::form_urlencoded::byte_serialize(term.as_bytes()).collect::<String>();
    let search_url = format!("{}/search.json?q={}", base, encoded_term);
    if let Ok(resp) = client
        .get(&search_url)
        .header("User-Agent", "DeductibleTracker/1.0")
        .send()
        .await
    {
        if !resp.status().is_success() {
            return None;
        }

        if let Ok(payload) = resp.json::<ProPublicaResponse>().await {
            let mut results = payload.organizations;
            results.sort_by(|a, b| {
                let b_score = organization_by_name_match_score(b, term);
                let a_score = organization_by_name_match_score(a, term);
                b_score.cmp(&a_score).then_with(|| {
                    b.score
                        .unwrap_or(0.0)
                        .partial_cmp(&a.score.unwrap_or(0.0))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
            });

            if let Some(best) = results.into_iter().next() {
                let fallback = EnrichedCharityData {
                    name: Some(best.name.clone()),
                    ein: propublica_ein_from_search(&best),
                    category: map_category_from_ntee(best.ntee_code.as_deref()),
                    status: None,
                    classification: None,
                    nonprofit_type: map_nonprofit_type(best.subseccd),
                    deductibility: None,
                    street: None,
                    city: best.city,
                    state: best.state,
                    zip: None,
                };

                if let Some(ein_digits) = fallback.ein.clone() {
                    let org_url = format!("{}/organizations/{}.json", base, ein_digits);
                    if let Ok(org_resp) = client
                        .get(&org_url)
                        .header("User-Agent", "DeductibleTracker/1.0")
                        .send()
                        .await
                    {
                        if org_resp.status().is_success() {
                            if let Ok(org_payload) = org_resp.json::<ProPublicaOrganizationResponse>().await {
                                let org = org_payload.organization;
                                let org_ein = propublica_ein_from_org(&org);
                                let org_nonprofit_type = map_nonprofit_type(org.subsection_code);
                                return Some(EnrichedCharityData {
                                    name: clean_opt_string(org.name).or_else(|| fallback.name.clone()),
                                    ein: org_ein.or_else(|| fallback.ein.clone()),
                                    category: map_category_from_ntee(org.ntee_code.as_deref())
                                        .or_else(|| fallback.category.clone()),
                                    status: derive_status(org.exempt_organization_status_code)
                                        .or_else(|| fallback.status.clone()),
                                    classification: map_foundation_label(org.foundation_code)
                                        .or_else(|| fallback.classification.clone()),
                                    nonprofit_type: org_nonprofit_type
                                        .or_else(|| fallback.nonprofit_type.clone()),
                                    deductibility: map_deductibility_from_exempt_status(
                                        org.exempt_organization_status_code,
                                    )
                                    .or_else(|| map_deductibility(org.deductibility_code)),
                                    street: clean_opt_string(org.address),
                                    city: clean_opt_string(org.city).or_else(|| fallback.city.clone()),
                                    state: clean_opt_string(org.state).or_else(|| fallback.state.clone()),
                                    zip: clean_opt_string(org.zipcode),
                                });
                            }
                        }
                    }
                }

                return Some(fallback);
            }
        }
    }

    None
}

pub(super) async fn fetch_charity_details_by_ein(ein: &str) -> Option<EnrichedCharityData> {
    let client = charity_search_client();
    let base = propublica_base_url()?;
    let normalized_ein = normalize_ein(ein);
    if normalized_ein.is_empty() {
        return None;
    }

    let org_url = format!("{}/organizations/{}.json", base, normalized_ein);
    let resp = client
        .get(&org_url)
        .header("User-Agent", "DeductibleTracker/1.0")
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        return None;
    }

    let payload = resp.json::<ProPublicaOrganizationResponse>().await.ok()?;
    let org = payload.organization;
    let org_ein = propublica_ein_from_org(&org);
    let category = map_category_from_ntee(org.ntee_code.as_deref());
    let status = derive_status(org.exempt_organization_status_code);
    let classification = map_foundation_label(org.foundation_code);
    let nonprofit_type = map_nonprofit_type(org.subsection_code);
    let deductibility = map_deductibility_from_exempt_status(org.exempt_organization_status_code)
        .or_else(|| map_deductibility(org.deductibility_code));

    Some(EnrichedCharityData {
        name: clean_opt_string(org.name),
        ein: org_ein,
        category,
        status,
        classification,
        nonprofit_type,
        deductibility,
        street: clean_opt_string(org.address),
        city: clean_opt_string(org.city),
        state: clean_opt_string(org.state),
        zip: clean_opt_string(org.zipcode),
    })
}

fn charity_search_client() -> &'static reqwest::Client {
    SEARCH_HTTP_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(3))
            .timeout(std::time::Duration::from_secs(8))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    })
}

fn propublica_base_url() -> Option<&'static str> {
    PROPUBLICA_BASE_URL
        .get_or_init(|| {
            env::var("PROPUBLICA_API_BASE_URL")
                .ok()
                .map(|v| v.trim().trim_end_matches('/').to_string())
                .filter(|v| !v.is_empty())
        })
        .as_deref()
}

fn organization_by_name_match_score(candidate: &ProPublicaSearchOrg, target_name: &str) -> i32 {
    let target = target_name.trim().to_lowercase();
    let name = candidate.name.trim().to_lowercase();
    if name.is_empty() {
        return 0;
    }
    if name == target {
        return 3;
    }
    if name.contains(&target) || target.contains(&name) {
        return 2;
    }
    1
}

fn propublica_ein_from_search(org: &ProPublicaSearchOrg) -> Option<String> {
    if let Some(strein) = org.strein.as_ref() {
        let normalized = normalize_ein(strein);
        if !normalized.is_empty() {
            return Some(normalized);
        }
    }
    org.ein.map(normalize_i64_ein)
}

fn propublica_ein_from_org(org: &ProPublicaOrganization) -> Option<String> {
    if let Some(strein) = org.strein.as_ref() {
        let normalized = normalize_ein(strein);
        if !normalized.is_empty() {
            return Some(normalized);
        }
    }
    org.ein.map(normalize_i64_ein)
}
