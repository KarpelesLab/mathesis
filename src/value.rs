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

use puremp::{Complex, Float, Int, Rational, RoundingMode};

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
    /// An exact complex number with rational parts (a Gaussian rational). The
    /// imaginary part is guaranteed nonzero — a purely real value collapses back
    /// to `Int`/`Ratio` (see [`from_complex`]).
    Cplx(Complex<Rational>),
    /// An inexact complex number (float parts) — the result once an irrational
    /// or transcendental enters a complex computation (`Pi*I`, `Sqrt[-2]`,
    /// `Exp[I*Pi]`). Imaginary part guaranteed nonzero (see [`from_complex_float`]).
    CplxReal(Complex<Float>),
    Bool(bool),
    /// Opaque plain text — a string literal, or verbatim solver output from
    /// `SMT[..]`. Rendered as monospace, never typeset as math.
    Text(String),
    /// A serialized graphics payload (JSON) produced by `Plot`/`Plot3D`,
    /// rendered by the frontend rather than typeset.
    Graphics(String),
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

/// Collapse a complex number to a real one when its imaginary part vanishes.
pub fn from_complex(c: Complex<Rational>) -> Value {
    if c.is_real() {
        from_rational(c.re)
    } else {
        Value::Cplx(c)
    }
}

fn rat(n: i64) -> Rational {
    Rational::from_integer(Int::from(n))
}

/// Collapse an inexact complex to a real when its imaginary part vanishes; a
/// non-finite component becomes an error.
pub fn from_complex_float(c: Complex<Float>) -> EResult<Value> {
    if c.re.is_nan() || c.im.is_nan() {
        return err("result is undefined or not a number");
    }
    if c.im.is_zero() {
        real(c.re)
    } else {
        Ok(Value::CplxReal(c))
    }
}

fn is_complex(v: &Value) -> bool {
    matches!(v, Value::Cplx(_) | Value::CplxReal(_))
}

/// Can this value take the *exact* (Gaussian-rational) complex path?
fn exact_complex_ok(v: &Value) -> bool {
    matches!(v, Value::Int(_) | Value::Ratio(_) | Value::Cplx(_))
}

/// A real value as a complex float (`Float` has no `Default`, so `from_real` is
/// unavailable — build with an explicit zero imaginary part).
fn cf_real(f: Float) -> Complex<Float> {
    Complex::new(f, Float::zero(WORK_BITS))
}

/// View any number as an inexact complex `Complex<Float>`.
pub fn complex_float(v: &Value) -> EResult<Complex<Float>> {
    Ok(match v {
        Value::Int(n) => cf_real(Float::from_int(n, WORK_BITS, NEAR)),
        Value::Ratio(r) => cf_real(Float::from_rational(r, WORK_BITS, NEAR)),
        Value::Real(f) => cf_real(f.clone()),
        Value::Sym { val, .. } => cf_real(val.clone()),
        Value::Cplx(c) => Complex::new(
            Float::from_rational(&c.re, WORK_BITS, NEAR),
            Float::from_rational(&c.im, WORK_BITS, NEAR),
        ),
        Value::CplxReal(c) => c.clone(),
        _ => return err("expected a number"),
    })
}

// `Complex<Float>` arithmetic — component-wise over `Float`, since `puremp`'s
// `Complex<Float>` exposes transcendentals but not the four operators.
fn cf_add(a: &Complex<Float>, b: &Complex<Float>) -> Complex<Float> {
    Complex::new(
        a.re.add(&b.re, WORK_BITS, NEAR),
        a.im.add(&b.im, WORK_BITS, NEAR),
    )
}
fn cf_sub(a: &Complex<Float>, b: &Complex<Float>) -> Complex<Float> {
    Complex::new(
        a.re.sub(&b.re, WORK_BITS, NEAR),
        a.im.sub(&b.im, WORK_BITS, NEAR),
    )
}
fn cf_mul(a: &Complex<Float>, b: &Complex<Float>) -> Complex<Float> {
    let re = a
        .re
        .mul(&b.re, WORK_BITS, NEAR)
        .sub(&a.im.mul(&b.im, WORK_BITS, NEAR), WORK_BITS, NEAR);
    let im = a
        .re
        .mul(&b.im, WORK_BITS, NEAR)
        .add(&a.im.mul(&b.re, WORK_BITS, NEAR), WORK_BITS, NEAR);
    Complex::new(re, im)
}
fn cf_div(a: &Complex<Float>, b: &Complex<Float>) -> Complex<Float> {
    let denom = b
        .re
        .mul(&b.re, WORK_BITS, NEAR)
        .add(&b.im.mul(&b.im, WORK_BITS, NEAR), WORK_BITS, NEAR);
    let re = a
        .re
        .mul(&b.re, WORK_BITS, NEAR)
        .add(&a.im.mul(&b.im, WORK_BITS, NEAR), WORK_BITS, NEAR)
        .div(&denom, WORK_BITS, NEAR);
    let im = a
        .im
        .mul(&b.re, WORK_BITS, NEAR)
        .sub(&a.re.mul(&b.im, WORK_BITS, NEAR), WORK_BITS, NEAR)
        .div(&denom, WORK_BITS, NEAR);
    Complex::new(re, im)
}
fn cf_zero(c: &Complex<Float>) -> bool {
    c.re.is_zero() && c.im.is_zero()
}
/// Integer power of an inexact complex, by exponentiation by squaring.
fn cf_powi(base: &Complex<Float>, e: i64) -> Complex<Float> {
    let one = cf_real(Float::from_int(&Int::from(1), WORK_BITS, NEAR));
    let (mut b, mut n) = if e < 0 {
        (cf_div(&one, base), e.unsigned_abs())
    } else {
        (base.clone(), e as u64)
    };
    let mut acc = one;
    while n > 0 {
        if n & 1 == 1 {
            acc = cf_mul(&acc, &b);
        }
        n >>= 1;
        if n > 0 {
            b = cf_mul(&b, &b);
        }
    }
    acc
}

/// View any *exact* value as a complex number. Inexact operands (reals /
/// symbolic irrationals) have no exact-complex form, so they error — Mathesis
/// supports exact (Gaussian-rational) complex arithmetic only.
fn complex_rat(v: &Value) -> EResult<Complex<Rational>> {
    match v {
        Value::Int(n) => Ok(Complex::from_real(Rational::from_integer(n.clone()))),
        Value::Ratio(r) => Ok(Complex::from_real(r.clone())),
        Value::Cplx(c) => Ok(c.clone()),
        Value::Real(_) | Value::Sym { .. } => {
            err("inexact complex arithmetic is not supported yet")
        }
        _ => err("expected a number"),
    }
}

/// `base^e` for an exact complex base and integer exponent, by exponentiation
/// by squaring (negative `e` inverts first).
fn complex_pow(base: Complex<Rational>, e: i64) -> EResult<Value> {
    if e == 0 {
        return Ok(Value::Int(Int::from(1)));
    }
    let one = Complex::from_real(rat(1));
    let (mut b, mut n) = if e < 0 {
        if base.is_zero() {
            return err("division by zero (0 raised to a negative power)");
        }
        (one.div(&base), e.unsigned_abs())
    } else {
        (base, e as u64)
    };
    let mut acc = Complex::from_real(rat(1));
    while n > 0 {
        if n & 1 == 1 {
            acc = acc.mul(&b);
        }
        n >>= 1;
        if n > 0 {
            b = b.mul(&b);
        }
    }
    Ok(from_complex(acc))
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

/// A real value as an `f64` for plotting; `None` for non-real (complex) values.
pub fn to_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Int(_) | Value::Ratio(_) | Value::Real(_) | Value::Sym { .. } => {
            to_float(v).ok().map(|f| f.to_f64())
        }
        _ => None,
    }
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
    if is_complex(a) || is_complex(b) {
        if exact_complex_ok(a) && exact_complex_ok(b) {
            Ok(from_complex(complex_rat(a)?.add(&complex_rat(b)?)))
        } else {
            from_complex_float(cf_add(&complex_float(a)?, &complex_float(b)?))
        }
    } else if is_inexact(a) || is_inexact(b) {
        real(to_float(a)?.add(&to_float(b)?, WORK_BITS, NEAR))
    } else {
        Ok(from_rational(to_rational(a)?.add(&to_rational(b)?)))
    }
}

pub fn sub(a: &Value, b: &Value) -> EResult<Value> {
    if is_complex(a) || is_complex(b) {
        if exact_complex_ok(a) && exact_complex_ok(b) {
            Ok(from_complex(complex_rat(a)?.sub(&complex_rat(b)?)))
        } else {
            from_complex_float(cf_sub(&complex_float(a)?, &complex_float(b)?))
        }
    } else if is_inexact(a) || is_inexact(b) {
        real(to_float(a)?.sub(&to_float(b)?, WORK_BITS, NEAR))
    } else {
        Ok(from_rational(to_rational(a)?.sub(&to_rational(b)?)))
    }
}

pub fn mul(a: &Value, b: &Value) -> EResult<Value> {
    if is_complex(a) || is_complex(b) {
        if exact_complex_ok(a) && exact_complex_ok(b) {
            Ok(from_complex(complex_rat(a)?.mul(&complex_rat(b)?)))
        } else {
            from_complex_float(cf_mul(&complex_float(a)?, &complex_float(b)?))
        }
    } else if is_inexact(a) || is_inexact(b) {
        real(to_float(a)?.mul(&to_float(b)?, WORK_BITS, NEAR))
    } else {
        Ok(from_rational(to_rational(a)?.mul(&to_rational(b)?)))
    }
}

pub fn div(a: &Value, b: &Value) -> EResult<Value> {
    if is_complex(a) || is_complex(b) {
        if exact_complex_ok(a) && exact_complex_ok(b) {
            let cb = complex_rat(b)?;
            if cb.is_zero() {
                return err("division by zero");
            }
            Ok(from_complex(complex_rat(a)?.div(&cb)))
        } else {
            let cb = complex_float(b)?;
            if cf_zero(&cb) {
                return err("division by zero");
            }
            from_complex_float(cf_div(&complex_float(a)?, &cb))
        }
    } else if is_inexact(a) || is_inexact(b) {
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
    match a {
        Value::Cplx(c) => Ok(from_complex(c.neg())),
        Value::CplxReal(c) => {
            from_complex_float(Complex::new(c.re.neg(), c.im.neg()))
        }
        _ if is_inexact(a) => Ok(Value::Real(to_float(a)?.neg())),
        _ => Ok(from_rational(to_rational(a)?.neg())),
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
    // Complex base or exponent.
    if is_complex(base) || is_complex(exp) {
        // An integer exponent stays exact when the base is exact.
        if let Ok(n) = as_int(exp) {
            let e = to_i64(&n)?;
            if exact_complex_ok(base) {
                return complex_pow(complex_rat(base)?, e);
            }
            return from_complex_float(cf_powi(&complex_float(base)?, e));
        }
        // A general (fractional / complex) exponent → Complex<Float>::pow.
        let b = complex_float(base)?;
        let w = complex_float(exp)?;
        return from_complex_float(b.pow(&w));
    }

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
            Value::Cplx(c) => complex_render(c, false),
            Value::CplxReal(c) => complex_render_float(c, false),
            Value::Graphics(_) => "«graphics»".to_string(),
            Value::Text(s) => s.clone(),
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
            Value::Cplx(c) => complex_render(c, true),
            Value::CplxReal(c) => complex_render_float(c, true),
            Value::Graphics(_) => "«graphics»".to_string(),
            // Not typeset — the frontend renders text results as monospace via
            // the `plain` flag; this arm exists only for completeness.
            Value::Text(s) => s.clone(),
            Value::Bool(b) => format!("\\text{{{}}}", if *b { "True" } else { "False" }),
            Value::Decimal(s) => s.clone(),
            Value::List(xs) => match matrix_tex(xs) {
                // A rectangular list-of-lists renders as a bracketed matrix.
                Some(m) => m,
                // Everything else stays a list.
                None => {
                    let inner: Vec<String> = xs.iter().map(Value::to_tex).collect();
                    format!("\\left\\{{{}\\right\\}}", inner.join(",\\ "))
                }
            },
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

    /// For opaque text results (string literals, `SMT[..]` output), the raw
    /// text to render as monospace instead of typeset math.
    pub fn plain_text(&self) -> Option<&str> {
        match self {
            Value::Text(s) => Some(s),
            _ => None,
        }
    }

    /// For `Plot`/`Plot3D` results, the JSON graphics payload the frontend draws.
    pub fn graphics(&self) -> Option<&str> {
        match self {
            Value::Graphics(s) => Some(s),
            _ => None,
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

/// Render a complex number `a + b i`, omitting a zero real part and a unit
/// coefficient, choosing `i` (TeX) or `I` (plain, re-typable) for the unit.
fn complex_render(c: &Complex<Rational>, tex: bool) -> String {
    let unit = if tex { "i" } else { "I" };
    let num = |r: &Rational| {
        let v = from_rational(r.clone());
        if tex { v.to_tex() } else { v.to_text() }
    };
    let neg_im = c.im.is_negative();
    let im_abs = if neg_im { c.im.neg() } else { c.im.clone() };

    let mut s = String::new();
    if !c.re.is_zero() {
        s.push_str(&num(&c.re));
        s.push_str(if neg_im { " - " } else { " + " });
    } else if neg_im {
        s.push('-');
    }
    if !im_abs.is_one() {
        s.push_str(&num(&im_abs));
        s.push_str(if tex { "\\," } else { " " });
    }
    s.push_str(unit);
    s
}

/// Render an inexact complex `a + b i` with decimal parts.
fn complex_render_float(c: &Complex<Float>, tex: bool) -> String {
    let unit = if tex { "i" } else { "I" };
    let neg_im = c.im.is_sign_negative();
    let im_abs = if neg_im { c.im.neg() } else { c.im.clone() };

    let mut s = String::new();
    if !c.re.is_zero() {
        s.push_str(&real_string(&c.re));
        s.push_str(if neg_im { " - " } else { " + " });
    } else if neg_im {
        s.push('-');
    }
    let im_str = real_string(&im_abs);
    if im_str != "1" {
        s.push_str(&im_str);
        s.push_str(if tex { "\\," } else { " " });
    }
    s.push_str(unit);
    s
}

/// If `rows` is a rectangular list-of-lists of scalars, render it as a KaTeX
/// `bmatrix`; otherwise return `None` so it falls back to plain list braces.
/// (Plain vectors like `{1, 2, 3}` are lists, not matrices, and stay lists.)
fn matrix_tex(rows: &[Value]) -> Option<String> {
    if rows.is_empty() {
        return None;
    }
    let mut width: Option<usize> = None;
    for row in rows {
        let cells = match row {
            Value::List(cells) => cells,
            _ => return None, // not a list-of-lists
        };
        // Matrix cells must be scalars — bail on deeper nesting.
        if cells.iter().any(|c| matches!(c, Value::List(_))) {
            return None;
        }
        match width {
            None => width = Some(cells.len()),
            Some(w) if w == cells.len() => {}
            Some(_) => return None, // ragged
        }
    }
    if width == Some(0) {
        return None; // rows present but empty
    }

    let body = rows
        .iter()
        .map(|row| match row {
            Value::List(cells) => cells.iter().map(Value::to_tex).collect::<Vec<_>>().join(" & "),
            _ => String::new(),
        })
        .collect::<Vec<_>>()
        .join(" \\\\ ");
    Some(format!("\\begin{{bmatrix}} {body} \\end{{bmatrix}}"))
}

/// Default rendering of a real: rounded to ~16 significant digits — the familiar
/// double-precision look. `N[..]` asks for the full precision. Non-finite values
/// render via `Float`'s own `Display` (`inf` / `-inf`).
///
/// A very large or very small magnitude (e.g. `Sqrt[1000!]`) would otherwise
/// print as a wall of zeros in plain decimal; those switch to scientific
/// notation so the result stays short.
fn real_string(f: &Float) -> String {
    let r = f.round(DISPLAY_BITS, NEAR);
    let plain = r.to_string();
    if plain.len() > 24 {
        format!("{r:e}")
    } else {
        plain
    }
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
