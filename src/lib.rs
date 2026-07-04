//! # Mathesis
//!
//! A Mathematica-style computational notebook for the browser. This crate is the
//! **frontend**: it owns the surface language (lexer → parser → evaluator) and
//! the rendering of results, and delegates every actual computation to
//! dependency-free pure-Rust engines — today [`puremp`] for exact
//! arbitrary-precision arithmetic, with more (e.g. `z3rs` for logic/SMT) to slot
//! in as the language grows.
//!
//! The single wasm entry point is [`evaluate`]: hand it a line of input, get
//! back a small JSON object the Vue frontend renders (plain text + TeX, or an
//! error message).

mod ast;
mod error;
mod eval;
mod lexer;
mod parser;
mod value;

use wasm_bindgen::prelude::*;

use error::{EResult, EvalError};
use value::Value;

/// Evaluate one line of Mathesis input.
///
/// Returns a JSON string, always one of:
/// - `{"ok":true,"text":"…","tex":"…"}`
/// - `{"ok":false,"error":"…"}`
#[wasm_bindgen]
pub fn evaluate(input: &str) -> String {
    match run(input) {
        Ok(v) => format!(
            "{{\"ok\":true,\"text\":{},\"tex\":{}}}",
            json_string(&v.to_text()),
            json_string(&v.to_tex()),
        ),
        Err(e) => format!("{{\"ok\":false,\"error\":{}}}", json_string(&e.0)),
    }
}

/// The crate version, surfaced in the UI.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

fn run(input: &str) -> EResult<Value> {
    if input.trim().is_empty() {
        return Err(EvalError("empty input".into()));
    }
    let expr = parser::parse(input).map_err(EvalError)?;
    let v = eval::eval(&expr)?;
    eval::set_last(v.clone());
    Ok(v)
}

/// Encode a string as a JSON string literal (quotes included). Hand-rolled to
/// avoid pulling `serde` into the wasm bundle for such a small need.
fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn out(input: &str) -> String {
        // Reset last-result between assertions is unnecessary for these cases.
        evaluate(input)
    }

    #[test]
    fn exact_integer_power() {
        assert!(out("2^128").contains("340282366920938463463374607431768211456"));
    }

    #[test]
    fn rationals_stay_exact() {
        let r = out("1/3 + 1/3 + 1/3");
        assert!(r.contains("\"text\":\"1\""), "{r}");
    }

    #[test]
    fn fraction_renders() {
        let r = out("1/2 + 1/3");
        assert!(r.contains("5/6"), "{r}");
        assert!(r.contains("\\\\frac{5}{6}") || r.contains("\\frac{5}{6}"), "{r}");
    }

    #[test]
    fn factorial_postfix() {
        assert!(out("10!").contains("3628800"));
    }

    #[test]
    fn builtins() {
        assert!(out("GCD[462, 1071]").contains("21"));
        assert!(out("Fibonacci[10]").contains("55"));
        assert!(out("PrimeQ[97]").contains("True"));
        assert!(out("Sqrt[144]").contains("12"));
        assert!(out("Factor[360]").contains("2^3"));
    }

    #[test]
    fn errors_are_reported() {
        assert!(out("1/0").contains("\"ok\":false"));
        assert!(out("Foo[1]").contains("unknown function"));
    }
}
