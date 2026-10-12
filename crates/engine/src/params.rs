//! Reading untrusted command parameters.

use serde_json::Value;

use crate::CommandError;

/// Coordinates and sizes beyond this are rejected as nonsense.
pub const MAX_COORD: f64 = 1.0e6;

fn missing(key: &str) -> CommandError {
    CommandError::Param(format!("missing or invalid `{key}`"))
}

pub fn str<'a>(p: &'a Value, key: &str) -> Result<&'a str, CommandError> {
    p.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| missing(key))
}

pub fn opt_str<'a>(p: &'a Value, key: &str) -> Option<&'a str> {
    p.get(key).and_then(Value::as_str)
}

/// A finite number within ±[`MAX_COORD`].
pub fn coord(p: &Value, key: &str) -> Result<f64, CommandError> {
    let v = p
        .get(key)
        .and_then(Value::as_f64)
        .ok_or_else(|| missing(key))?;
    if v.is_finite() && v.abs() <= MAX_COORD {
        Ok(v)
    } else {
        Err(CommandError::Param(format!("`{key}` out of range: {v}")))
    }
}

/// A size in `1..=MAX_COORD`.
pub fn size(p: &Value, key: &str) -> Result<f64, CommandError> {
    let v = coord(p, key)?;
    if v >= 1.0 {
        Ok(v)
    } else {
        Err(CommandError::Param(format!("`{key}` must be at least 1")))
    }
}

pub fn ids(p: &Value, key: &str) -> Result<Vec<String>, CommandError> {
    let list = p
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| missing(key))?;
    list.iter()
        .map(|v| v.as_str().map(str::to_owned).ok_or_else(|| missing(key)))
        .collect()
}
