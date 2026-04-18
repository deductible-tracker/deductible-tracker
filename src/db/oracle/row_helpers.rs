use chrono::{DateTime, NaiveDate, Utc};
use oracle_rs::{Row, Value};

pub(crate) fn row_string(row: &Row, index: usize) -> String {
    row_opt_string(row, index).unwrap_or_default()
}

pub(crate) fn row_opt_string(row: &Row, index: usize) -> Option<String> {
    row.get_string(index)
        .map(ToOwned::to_owned)
        .or_else(|| row.get(index).and_then(value_to_string))
}

pub(crate) fn row_i64(row: &Row, index: usize) -> Option<i64> {
    row.get(index).and_then(|value| {
        value
            .as_i64()
            .or_else(|| value_to_string(value)?.parse::<i64>().ok())
    })
}

pub(crate) fn row_f64(row: &Row, index: usize) -> Option<f64> {
    row.get(index).and_then(|value| {
        value
            .as_f64()
            .or_else(|| value_to_string(value)?.parse::<f64>().ok())
    })
}

pub(crate) fn row_bool(row: &Row, index: usize) -> Option<bool> {
    row.get(index).and_then(|value| {
        value.as_bool().or_else(|| {
            value_to_string(value).and_then(|text| {
                match text.trim().to_ascii_lowercase().as_str() {
                    "1" | "true" | "y" | "yes" => Some(true),
                    "0" | "false" | "n" | "no" => Some(false),
                    _ => None,
                }
            })
        })
    })
}

pub(crate) fn row_naive_date(row: &Row, index: usize) -> Option<NaiveDate> {
    row.get(index)
        .and_then(|value| value_to_string(value).and_then(|text| parse_naive_date_text(&text)))
}

pub(crate) fn row_datetime_utc(row: &Row, index: usize) -> Option<DateTime<Utc>> {
    row.get(index)
        .and_then(|value| value_to_string(value).and_then(|text| parse_datetime_text(&text)))
}

pub(crate) fn parse_utc_from_opt_string(value: Option<String>) -> DateTime<Utc> {
    value
        .and_then(|text| parse_datetime_text(&text))
        .unwrap_or_else(Utc::now)
}

fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        _ => Some(value.to_string()),
    }
}

fn parse_naive_date_text(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .ok()
        .or_else(|| {
            value
                .split_whitespace()
                .next()
                .and_then(|prefix| NaiveDate::parse_from_str(prefix, "%Y-%m-%d").ok())
        })
        .or_else(|| parse_datetime_text(value).map(|value| value.date_naive()))
}

fn parse_datetime_text(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|value| value.with_timezone(&Utc))
        .or_else(|| {
            DateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S%.f %:z")
                .ok()
                .map(|value| value.with_timezone(&Utc))
        })
        .or_else(|| {
            DateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S %:z")
                .ok()
                .map(|value| value.with_timezone(&Utc))
        })
        .or_else(|| {
            chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S%.f")
                .ok()
                .map(|value| DateTime::from_naive_utc_and_offset(value, Utc))
        })
        .or_else(|| {
            chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S")
                .ok()
                .map(|value| DateTime::from_naive_utc_and_offset(value, Utc))
        })
}
