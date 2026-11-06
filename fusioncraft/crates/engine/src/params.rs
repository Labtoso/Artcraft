//! Reading command parameters from JSON. Every accessor returns a readable error instead of
//! panicking on missing or malformed input.

use crate::EngineError;
use fusioncraft_doc::Expr;
use fusioncraft_geom::{Vec2, Vec3};
use serde_json::Value;

fn bad(msg: String) -> EngineError {
    EngineError::BadParams(msg)
}

pub fn get<'a>(p: &'a Value, key: &str) -> Option<&'a Value> {
    p.get(key).filter(|v| !v.is_null())
}

pub fn f64_of(v: &Value, key: &str) -> Result<f64, EngineError> {
    let x = v.as_f64().ok_or_else(|| bad(format!("'{key}' must be a number")))?;
    if !x.is_finite() || x.abs() > 1e9 {
        return Err(bad(format!("'{key}' is out of range")));
    }
    Ok(x)
}

pub fn num(p: &Value, key: &str) -> Result<f64, EngineError> {
    f64_of(get(p, key).ok_or_else(|| bad(format!("missing '{key}'")))?, key)
}

pub fn num_or(p: &Value, key: &str, default: f64) -> Result<f64, EngineError> {
    get(p, key).map_or(Ok(default), |v| f64_of(v, key))
}

pub fn u64_req(p: &Value, key: &str) -> Result<u64, EngineError> {
    get(p, key).and_then(Value::as_u64).ok_or_else(|| bad(format!("'{key}' must be a whole number")))
}

pub fn u32_or(p: &Value, key: &str, default: u32, max: u32) -> Result<u32, EngineError> {
    match get(p, key) {
        None => Ok(default),
        Some(v) => {
            let n = v.as_u64().ok_or_else(|| bad(format!("'{key}' must be a whole number")))?;
            u32::try_from(n).ok().filter(|n| *n <= max).ok_or_else(|| bad(format!("'{key}' must be at most {max}")))
        }
    }
}

pub fn bool_or(p: &Value, key: &str, default: bool) -> bool {
    get(p, key).and_then(Value::as_bool).unwrap_or(default)
}

pub fn str_req<'a>(p: &'a Value, key: &str) -> Result<&'a str, EngineError> {
    get(p, key).and_then(Value::as_str).ok_or_else(|| bad(format!("'{key}' must be text")))
}

pub fn str_opt<'a>(p: &'a Value, key: &str) -> Option<&'a str> {
    get(p, key).and_then(Value::as_str)
}

/// An expression: a number or text such as "width / 2".
pub fn expr(p: &Value, key: &str) -> Result<Expr, EngineError> {
    match get(p, key) {
        Some(Value::String(s)) => Ok(Expr::new(s.clone())),
        Some(v) => Ok(Expr::num(f64_of(v, key)?)),
        None => Err(bad(format!("missing '{key}'"))),
    }
}

pub fn expr_or(p: &Value, key: &str, default: &str) -> Result<Expr, EngineError> {
    if get(p, key).is_some() { expr(p, key) } else { Ok(Expr::new(default)) }
}

pub fn vec2(p: &Value, key: &str) -> Result<Vec2, EngineError> {
    let v = get(p, key).ok_or_else(|| bad(format!("missing '{key}'")))?;
    let arr = v.as_array().filter(|a| a.len() == 2).ok_or_else(|| bad(format!("'{key}' must be [x, y]")))?;
    match arr.as_slice() {
        [x, y] => Ok(Vec2::new(f64_of(x, key)?, f64_of(y, key)?)),
        _ => Err(bad(format!("'{key}' must be [x, y]"))),
    }
}

pub fn vec3_or(p: &Value, key: &str, default: Vec3) -> Result<Vec3, EngineError> {
    let Some(v) = get(p, key) else { return Ok(default) };
    let arr = v.as_array().filter(|a| a.len() == 3).ok_or_else(|| bad(format!("'{key}' must be [x, y, z]")))?;
    match arr.as_slice() {
        [x, y, z] => Ok(Vec3::new(f64_of(x, key)?, f64_of(y, key)?, f64_of(z, key)?)),
        _ => Err(bad(format!("'{key}' must be [x, y, z]"))),
    }
}

pub fn index_list(p: &Value, key: &str) -> Result<Vec<usize>, EngineError> {
    match get(p, key) {
        None => Ok(Vec::new()),
        Some(Value::Array(a)) => {
            if a.len() > 10_000 {
                return Err(bad(format!("'{key}' is too long")));
            }
            a.iter()
                .map(|v| v.as_u64().and_then(|n| usize::try_from(n).ok()).ok_or_else(|| bad(format!("'{key}' must list whole numbers"))))
                .collect()
        }
        Some(_) => Err(bad(format!("'{key}' must be a list"))),
    }
}
