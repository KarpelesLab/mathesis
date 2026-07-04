//! Runtime values and the arithmetic over them.
//!
//! Mathesis owns *no* mathematics of its own here: every operation is a thin
//! shim over `puremp`. The value tower has two layers:
//!
//! - **exact** — [`Int`] and [`Rational`]. Integers stay integers, quotients
//!   become rationals in lowest terms, and a rational that happens to be whole
//!   collapses back to an integer.
//! - **real** — [`Float`], an arbitrary-precision approximation used for
//!   irrational results (`Pi`, `Sqrt[2]`, `Sin[1]`, …). Reals are *contagious*:
//!   any operation mixing an exact value with a real produces a real, exactly as
//!   a hand calculation drops to decimals once an irrational enters.

use puremp::{Float, Int, Rational, RoundingMode};

use crate::error::{EResult, EvalError, err};

/// Working precision (bits) for constants and transcendentals — ~308 decimal
/// digits, generous headroom so `N[Pi, 200]` is still fully backed.
pub const WORK_BITS: u64 = 1024;

/// Precision (bits) used for the *default* rendering of a real — ~15–16
/// significant digits, the familiar double-precision look. `N[..]` overrides it.
const DISPLAY_BITS: u64 = 53;

/// Round-to-nearest everywhere; the only mode a calculator surface needs.
pub const NEAR: RoundingMode = RoundingMode::Nearest;

#[derive(Clone)]
pub enum Value {
    Int(Int),
    /// A rational guaranteed *not* to be an integer (see [`from_rational`]).
    Ratio(Rational),
    /// An arbitrary-precision real approximation (irrational results).
    Real(Float),
    /// An *exact but irrational* leaf kept in symbolic form for display —
    /// `Pi` → π, `Sqrt[2]` → √2 — carrying its numeric value for the decimal
    /// approximation and for when arithmetic must drop to the real path.
    Sym { text: String, tex: String, val: Float },
    Bool(bool),
    /// A display-only decimal string produced by `N[..]`; not fed back into
    /// arithmetic.
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

pub fn to_rational(v: &Value) -> EResult<Rational> {
    match v {
        Value::Int(n) => Ok(Rational::from_integer(n.clone())),
        Value::Ratio(r) => Ok(r.clone()),
        _ => err("expected an exact (integer or rational) number"),
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

/// Convert any numeric value (exact or real) to a [`Float`] at working precision.
pub fn to_float(v: &Value) -> EResult<Float> {
    match v {
        Value::Int(n) => Ok(Float::from_int(n, WORK_BITS, NEAR)),
        Value::Ratio(r) => Ok(Float::from_rational(r, WORK_BITS, NEAR)),
        Value::Real(f) => Ok(f.clone()),
        Value::Sym { val, .. } => Ok(val.clone()),
        _ => err("expected a number"),
    }
}

/// Inexact values — reals and symbolic-but-irrational leaves — force the real
/// path in arithmetic (we keep no symbolic simplifier).
fn is_inexact(v: &Value) -> bool {
    matches!(v, Value::Real(_) | Value::Sym { .. })
}

/// Wrap a freshly computed real, turning a non-finite result (an out-of-domain
/// call such as `Log[-1]`) into a readable error instead of a `NaN`. This is the
/// canonical constructor for real results from `eval`'s transcendental builtins.
pub fn real(f: Float) -> EResult<Value> {
    if f.is_nan() {
        err("result is undefined or not a real number")
    } else {
        Ok(Value::Real(f))
    }
}

// --- arithmetic -------------------------------------------------------------
//
// Each operation takes the exact path when both operands are exact, and drops to
// the real path as soon as either operand is a [`Value::Real`].

pub fn add(a: &Value, b: &Value) -> EResult<Value> {
    if is_inexact(a) || is_inexact(b) {
        real(to_float(a)?.add(&to_float(b)?, WORK_BITS, NEAR))
    } else {
        Ok(from_rational(to_rational(a)?.add(&to_rational(b)?)))
    }
}

pub fn sub(a: &Value, b: &Value) -> EResult<Value> {
    if is_inexact(a) || is_inexact(b) {
        real(to_float(a)?.sub(&to_float(b)?, WORK_BITS, NEAR))
    } else {
        Ok(from_rational(to_rational(a)?.sub(&to_rational(b)?)))
    }
}

pub fn mul(a: &Value, b: &Value) -> EResult<Value> {
    if is_inexact(a) || is_inexact(b) {
        real(to_float(a)?.mul(&to_float(b)?, WORK_BITS, NEAR))
    } else {
        Ok(from_rational(to_rational(a)?.mul(&to_rational(b)?)))
    }
}

pub fn div(a: &Value, b: &Value) -> EResult<Value> {
    if is_inexact(a) || is_inexact(b) {
        let fb = to_float(b)?;
        if fb.is_zero() {
            return err("division by zero");
        }
        real(to_float(a)?.div(&fb, WORK_BITS, NEAR))
    } else {
        let rb = to_rational(b)?;
        if rb.is_zero() {
            return err("division by zero");
        }
        Ok(from_rational(to_rational(a)?.div(&rb)))
    }
}

pub fn neg(a: &Value) -> EResult<Value> {
    if is_inexact(a) {
        Ok(Value::Real(to_float(a)?.neg()))
    } else {
        Ok(from_rational(to_rational(a)?.neg()))
    }
}

pub fn abs(v: &Value) -> EResult<Value> {
    if is_inexact(v) {
        Ok(Value::Real(to_float(v)?.abs()))
    } else {
        Ok(from_rational(to_rational(v)?.abs()))
    }
}

pub fn pow(base: &Value, exp: &Value) -> EResult<Value> {
    // Exact fast path: an exact base raised to an *integer* exponent stays exact.
    if !is_inexact(base) && !is_inexact(exp) {
        if let Ok(n) = as_int(exp) {
            let e = to_i64(&n)?;
            if let Value::Int(b) = base {
                if e >= 0 {
                    let e: u32 =
                        e.try_into().map_err(|_| EvalError("exponent too large".into()))?;
                    return Ok(Value::Int(b.pow(e)));
                }
            }
            let rb = to_rational(base)?;
            if rb.is_zero() && e < 0 {
                return err("division by zero (0 raised to a negative power)");
            }
            let e: i32 =
                e.try_into().map_err(|_| EvalError("exponent out of range".into()))?;
            return Ok(from_rational(rb.pow(e)));
        }
    }

    // Real path: a fractional exponent, or a real operand — e.g. 2^(1/2), Pi^2.
    let fb = to_float(base)?;
    let fe = to_float(exp)?;
    if fb.is_zero() && fe.is_sign_negative() {
        return err("division by zero (0 raised to a negative power)");
    }
    real(fb.pow(&fe, WORK_BITS, NEAR))
}

// --- rendering --------------------------------------------------------------

impl Value {
    /// Plain-text rendering (what the user might type back in).
    pub fn to_text(&self) -> String {
        match self {
            Value::Int(n) => n.to_string(),
            Value::Ratio(r) => format!("{}/{}", r.numerator(), r.denominator()),
            Value::Real(f) => real_string(f),
            Value::Sym { text, .. } => text.clone(),
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
            Value::Real(f) => real_string(f),
            Value::Sym { tex, .. } => tex.clone(),
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

    /// The decimal approximation to show *alongside* an exact result, when it
    /// adds information. `None` for values whose primary rendering is already
    /// their best form — integers, reals (already a decimal), booleans, lists,
    /// factorizations, and `N[..]` output.
    pub fn approx(&self) -> Option<String> {
        match self {
            Value::Ratio(r) => Some(real_string(&Float::from_rational(r, WORK_BITS, NEAR))),
            Value::Sym { val, .. } => Some(real_string(val)),
            _ => None,
        }
    }
}

/// Default rendering of a real: rounded to ~16 significant digits — the familiar
/// double-precision look. `N[..]` asks for the full precision. Non-finite values
/// render via `Float`'s own `Display` (`inf` / `-inf`).
fn real_string(f: &Float) -> String {
    f.round(DISPLAY_BITS, NEAR).to_string()
}

/// Render a real as a decimal with `digits` fractional places, for `N[..]`.
pub fn real_decimal(f: &Float, digits: u32) -> String {
    f.to_decimal_string(digits)
}

/// Format a rational as a decimal string with `digits` fractional places
/// (rounded), used by `N[..]`.
pub fn decimal_string(r: &Rational, digits: u32) -> String {
    let mut s = String::new();
    // write_decimal only fails if the sink fails; a String never does.
    let _ = r.write_decimal(&mut s, digits, false);
    s
}
