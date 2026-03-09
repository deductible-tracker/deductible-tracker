use axum::{
    extract::Query,
    response::{IntoResponse, Json as AxumJson},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use crate::auth::AuthenticatedUser;

#[derive(Deserialize)]
pub struct MarginalRateQuery {
    pub filing_status: Option<String>,
    pub agi: Option<f64>,
}

#[derive(Serialize, Clone, Copy)]
pub struct TaxBracket {
    pub rate: f64,
    pub min: f64,
    pub max: Option<f64>,
}

#[derive(Serialize)]
pub struct MarginalRateResponse {
    pub filing_status: String,
    pub agi: Option<f64>,
    pub selected_rate: Option<f64>,
    pub brackets: Vec<TaxBracket>,
}

fn normalize_filing_status(status: Option<&str>) -> &'static str {
    match status.unwrap_or("single").trim().to_lowercase().as_str() {
        "single" => "single",
        "married_joint" => "married_joint",
        "married_separate" => "married_separate",
        "head_household" => "head_household",
        _ => "single",
    }
}

fn brackets_for_status(filing_status: &str) -> &'static [TaxBracket] {
    const SINGLE: [TaxBracket; 7] = [
        TaxBracket { rate: 0.10, min: 0.0, max: Some(11_925.0) },
        TaxBracket { rate: 0.12, min: 11_925.0, max: Some(48_475.0) },
        TaxBracket { rate: 0.22, min: 48_475.0, max: Some(103_350.0) },
        TaxBracket { rate: 0.24, min: 103_350.0, max: Some(197_300.0) },
        TaxBracket { rate: 0.32, min: 197_300.0, max: Some(250_525.0) },
        TaxBracket { rate: 0.35, min: 250_525.0, max: Some(626_350.0) },
        TaxBracket { rate: 0.37, min: 626_350.0, max: None },
    ];
    const MARRIED_JOINT: [TaxBracket; 7] = [
        TaxBracket { rate: 0.10, min: 0.0, max: Some(23_850.0) },
        TaxBracket { rate: 0.12, min: 23_850.0, max: Some(96_950.0) },
        TaxBracket { rate: 0.22, min: 96_950.0, max: Some(206_700.0) },
        TaxBracket { rate: 0.24, min: 206_700.0, max: Some(394_600.0) },
        TaxBracket { rate: 0.32, min: 394_600.0, max: Some(501_050.0) },
        TaxBracket { rate: 0.35, min: 501_050.0, max: Some(751_600.0) },
        TaxBracket { rate: 0.37, min: 751_600.0, max: None },
    ];
    const MARRIED_SEPARATE: [TaxBracket; 7] = [
        TaxBracket { rate: 0.10, min: 0.0, max: Some(11_925.0) },
        TaxBracket { rate: 0.12, min: 11_925.0, max: Some(48_475.0) },
        TaxBracket { rate: 0.22, min: 48_475.0, max: Some(103_350.0) },
        TaxBracket { rate: 0.24, min: 103_350.0, max: Some(197_300.0) },
        TaxBracket { rate: 0.32, min: 197_300.0, max: Some(250_525.0) },
        TaxBracket { rate: 0.35, min: 250_525.0, max: Some(375_800.0) },
        TaxBracket { rate: 0.37, min: 375_800.0, max: None },
    ];
    const HEAD_HOUSEHOLD: [TaxBracket; 7] = [
        TaxBracket { rate: 0.10, min: 0.0, max: Some(17_000.0) },
        TaxBracket { rate: 0.12, min: 17_000.0, max: Some(64_850.0) },
        TaxBracket { rate: 0.22, min: 64_850.0, max: Some(103_350.0) },
        TaxBracket { rate: 0.24, min: 103_350.0, max: Some(197_300.0) },
        TaxBracket { rate: 0.32, min: 197_300.0, max: Some(250_500.0) },
        TaxBracket { rate: 0.35, min: 250_500.0, max: Some(626_350.0) },
        TaxBracket { rate: 0.37, min: 626_350.0, max: None },
    ];

    match filing_status {
        "married_joint" => &MARRIED_JOINT,
        "married_separate" => &MARRIED_SEPARATE,
        "head_household" => &HEAD_HOUSEHOLD,
        _ => &SINGLE,
    }
}

fn marginal_rate_from_brackets(brackets: &[TaxBracket], agi: f64) -> Option<f64> {
    if !agi.is_finite() || agi < 0.0 {
        return None;
    }

    for bracket in brackets {
        if agi >= bracket.min && bracket.max.map(|max| agi <= max).unwrap_or(true) {
            return Some(bracket.rate);
        }
    }

    brackets.last().map(|b| b.rate)
}

pub async fn marginal_rate(
    _user: AuthenticatedUser,
    Query(query): Query<MarginalRateQuery>,
) -> impl IntoResponse {
    let filing_status = normalize_filing_status(query.filing_status.as_deref());
    let brackets = brackets_for_status(filing_status);
    let agi = query.agi.filter(|value| value.is_finite() && *value >= 0.0);
    let selected_rate = agi.and_then(|value| marginal_rate_from_brackets(brackets, value));

    if let Some(raw_agi) = query.agi {
        if !raw_agi.is_finite() || raw_agi < 0.0 {
            return (StatusCode::BAD_REQUEST, "AGI must be a non-negative number").into_response();
        }
    }

    AxumJson(MarginalRateResponse {
        filing_status: filing_status.to_string(),
        agi,
        selected_rate,
        brackets: brackets.to_vec(),
    }).into_response()
}
