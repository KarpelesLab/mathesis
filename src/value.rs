//! Runtime values and the exact arithmetic over them.
//!
//! Mathesis owns *no* mathematics of its own here: every operation is a thin
//! shim over `puremp`'s arbitrary-precision [`Int`]/[`Rational`]. The value tower
//! keeps results exact — integers stay integers, quotients become rationals in
//! lowest terms, and a rational that happens to be whole collapses back to an
//! integer.

use puremp::{Int, Rational};

use crate::error::{EResult, EvalError, err};

#[derive(Clone)]
pub enum Value {
    Int(Int),
    /// A rational guaranteed *not* to be an integer (see [`from_rational`]).
    Ratio(Rational),
    Bool(bool),
    /// A display-only decimal string produced by `N[..]`; not fed back into
    /// exact arithmetic.
    Decimal(String),
    List(Vec<Value>),
    /// The result of `Factor[..]`: an optional overall sign and (prime, power)
    /// pairs, rendered as a product.
    Factored { negative: bool, factors: Vec<(Int, u32)> },
}

/// Collapse a rational to an integer when it is one — the canonical form used
/// everywhere a numeric result is produced.
pub fn from_rational(r: Rational) -> Value {
    match r.to_integer() {
        Some(n) => Value::Int(n),
        None => Value::Ratio(r),
    }
}

fn to_rational(v: &Value) -> EResult<Rational> {
    match v {
        Value::Int(n) => Ok(Rational::from_integer(n.clone())),
        Value::Ratio(r) => Ok(r.clone()),
        _ => err("expected a number"),
    }
}

/// Extract an exact integer, or error if the value is not a whole number.
pub fn as_int(v: &Value) -> EResult<Int> {
    match v {
        Value::Int(n) => Ok(n.clone()),
        Value::Ratio(_) => err("expected an integer, got a fraction"),
        _ => err("expected an integer"),
    }
}

/// `Int` → `i64` via its decimal rendering — robust and independent of which
/// primitive-conversion helpers `puremp` exposes.
pub fn to_i64(n: &Int) -> EResult<i64> {
    n.to_string()
        .parse::<i64>()
        .map_err(|_| EvalError("integer out of range".into()))
}

pub fn to_u64(n: &Int) -> EResult<u64> {
    n.to_string()
        .parse::<u64>()
        .map_err(|_| EvalError("expected a non-negative integer in range".into()))
}

// --- exact arithmetic -------------------------------------------------------

pub fn add(a: &Value, b: &Value) -> EResult<Value> {
    Ok(from_rational(to_rational(a)?.add(&to_rational(b)?)))
}

pub fn sub(a: &Value, b: &Value) -> EResult<Value> {
    Ok(from_rational(to_rational(a)?.sub(&to_rational(b)?)))
}

pub fn mul(a: &Value, b: &Value) -> EResult<Value> {
    Ok(from_rational(to_rational(a)?.mul(&to_rational(b)?)))
}

pub fn div(a: &Value, b: &Value) -> EResult<Value> {
    let rb = to_rational(b)?;
    if rb.is_zero() {
        return err("division by zero");
    }
    Ok(from_rational(to_rational(a)?.div(&rb)))
}

pub fn neg(a: &Value) -> EResult<Value> {
    Ok(from_rational(to_rational(a)?.neg()))
}

pub fn pow(base: &Value, exp: &Value) -> EResult<Value> {
    let e = to_i64(&as_int(exp).map_err(|_| EvalError("exponent must be an integer".into()))?)?;

    // Fast path: integer base, non-negative exponent → exact big integer.
    if let Value::Int(n) = base {
        if e >= 0 {
            let e: u32 = e
                .try_into()
                .map_err(|_| EvalError("exponent too large".into()))?;
            return Ok(Value::Int(n.pow(e)));
        }
    }

    let rb = to_rational(base)?;
    if rb.is_zero() && e < 0 {
        return err("division by zero (0 raised to a negative power)");
    }
    let e: i32 = e
        .try_into()
        .map_err(|_| EvalError("exponent out of range".into()))?;
    Ok(from_rational(rb.pow(e)))
}

pub fn abs(v: &Value) -> EResult<Value> {
    Ok(from_rational(to_rational(v)?.abs()))
}

// --- rendering --------------------------------------------------------------

impl Value {
    /// Plain-text rendering (what the user might type back in).
    pub fn to_text(&self) -> String {
        match self {
            Value::Int(n) => n.to_string(),
            Value::Ratio(r) => format!("{}/{}", r.numerator(), r.denominator()),
            Value::Bool(b) => if *b { "True" } else { "False" }.to_string(),
            Value::Decimal(s) => s.clone(),
            Value::List(xs) => {
                let inner: Vec<String> = xs.iter().map(Value::to_text).collect();
                format!("{{{}}}", inner.join(", "))
            }
            Value::Factored { negative, factors } => {
                let mut parts: Vec<String> = Vec::new();
                if *negative {
                    parts.push("-1".to_string());
                }
                for (p, e) in factors {
                    parts.push(if *e == 1 {
                        p.to_string()
                    } else {
                        format!("{p}^{e}")
                    });
                }
                if parts.is_empty() {
                    "1".to_string()
                } else {
                    parts.join(" * ")
                }
            }
        }
    }

    /// TeX rendering for KaTeX in the notebook.
    pub fn to_tex(&self) -> String {
        match self {
            Value::Int(n) => n.to_string(),
            Value::Ratio(r) => {
                let num = r.numerator();
                if num.is_negative() {
                    format!("-\\frac{{{}}}{{{}}}", num.abs(), r.denominator())
                } else {
                    format!("\\frac{{{}}}{{{}}}", num, r.denominator())
                }
            }
            Value::Bool(b) => format!("\\text{{{}}}", if *b { "True" } else { "False" }),
            Value::Decimal(s) => s.clone(),
            Value::List(xs) => {
                let inner: Vec<String> = xs.iter().map(Value::to_tex).collect();
                format!("\\left\\{{{}\\right\\}}", inner.join(",\\ "))
            }
            Value::Factored { negative, factors } => {
                let mut parts: Vec<String> = Vec::new();
                if *negative {
                    parts.push("-".to_string());
                }
                let body: Vec<String> = factors
                    .iter()
                    .map(|(p, e)| {
                        if *e == 1 {
                            p.to_string()
                        } else {
                            format!("{p}^{{{e}}}")
                        }
                    })
                    .collect();
                if body.is_empty() {
                    parts.push("1".to_string());
                } else {
                    parts.push(body.join(" \\cdot "));
                }
                parts.concat()
            }
        }
    }
}

/// Format a rational as a decimal string with `digits` fractional places
/// (rounded), used by `N[..]`.
pub fn decimal_string(r: &Rational, digits: u32) -> String {
    let mut s = String::new();
    // write_decimal only fails if the sink fails; a String never does.
    let _ = r.write_decimal(&mut s, digits, false);
    s
}
