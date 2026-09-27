//! One module per source. Each turns the source's answer into candidates;
//! `lookup` in the crate root decides which to ask.

pub mod arxiv;
pub mod comicvine;
pub mod crossref;
pub mod google;
pub mod isbndb;
pub mod openalex;
pub mod openlibrary;
pub mod semantic;

use serde_json::Value;

/// A string field, or None if missing or empty.
pub(crate) fn s(v: &Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
}

/// A list of strings (non-strings are skipped).
pub(crate) fn strings(v: &Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// A list of objects' `field` ("authors": [{"name": …}]).
pub(crate) fn names(v: &Value, key: &str, field: &str) -> Vec<String> {
    v.get(key)
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|x| s(x, field)).collect())
        .unwrap_or_default()
}

/// A number that may come as a number or a string.
pub(crate) fn num(v: &Value, key: &str) -> Option<u32> {
    match v.get(key)? {
        Value::Number(n) => n.as_u64().and_then(|n| u32::try_from(n).ok()),
        Value::String(t) => t.trim().parse().ok(),
        _ => None,
    }
}

/// A value that may be a number or a string, as text.
pub(crate) fn text(v: &Value, key: &str) -> Option<String> {
    match v.get(key)? {
        Value::Number(n) => Some(n.to_string()),
        Value::String(t) => Some(t.trim().to_owned()).filter(|t| !t.is_empty()),
        _ => None,
    }
}
