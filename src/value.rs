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

use puremp::{Algebraic, Complex, Float, Int, Quadratic, Rational, RoundingMode};

use crate::error::{EResult, EvalError, err};
use crate::mpoly::MPoly;

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
    /// An exact **real algebraic number** — a root of a rational polynomial,
    /// carrying its defining polynomial and an isolating interval (from `Solve`
    /// of a polynomial, `Eigenvalues`, …). Rendered as a radical when degree ≤ 2,
    /// otherwise as a decimal. Rationals collapse to `Int`/`Ratio` (see [`alg`]).
    Alg(Algebraic),
    /// An exact complex number with rational parts (a Gaussian rational). The
    /// imaginary part is guaranteed nonzero — a purely real value collapses back
    /// to `Int`/`Ratio` (see [`from_complex`]).
    Cplx(Complex<Rational>),
    /// An inexact complex number (float parts) — the result once an irrational
    /// or transcendental enters a complex computation (`Pi*I`, `Sqrt[-2]`,
    /// `Exp[I*Pi]`). Imaginary part guaranteed nonzero (see [`from_complex_float`]).
    CplxReal(Complex<Float>),
    /// An exact multivariate polynomial over ℚ (from `D[..]`). Guaranteed
    /// non-constant — a constant collapses to `Int`/`Ratio` (see [`from_mpoly`]).
    Poly(MPoly),
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
    /// A solution / substitution set from `FindInstance`, rendered as typeset
    /// rules `{x → 3, y → ½}`.
    Rules(Vec<(String, Value)>),
    /// The full solution set from `Solve` — zero or more assignments, rendered
    /// by the frontend as a table (`truncated` marks a capped enumeration).
    Solutions { rows: Vec<Vec<(String, Value)>>, truncated: bool },
    /// The result of `Factor[..]`: an optional overall sign and (prime, power)
    /// pairs, rendered as a product.
    Factored { negative: bool, factors: Vec<(Int, u32)> },
}

/// A rendered solution set: shared variable list plus one TeX cell per variable
/// per solution row.
pub struct SolutionTable {
    pub vars: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub truncated: bool,
}

/// Wrap a polynomial, collapsing a constant one back to `Int`/`Ratio` so a
/// derivative that happens to be a number renders (and computes) as a number.
pub fn from_mpoly(p: MPoly) -> Value {
    match p.as_constant() {
        Some(c) => from_rational(c),
        None => Value::Poly(p),
    }
}

fn is_poly(v: &Value) -> bool {
    matches!(v, Value::Poly(_))
}

/// View an exact value as a polynomial (numbers become constants), for
/// arithmetic that mixes `D[..]` results with ordinary numbers.
pub(crate) fn as_mpoly(v: &Value) -> EResult<MPoly> {
    match v {
        Value::Poly(p) => Ok(p.clone()),
        Value::Int(n) => Ok(MPoly::constant(Rational::from_integer(n.clone()))),
        Value::Ratio(r) => Ok(MPoly::constant(r.clone())),
        _ => err("a polynomial can only combine with exact rational numbers"),
    }
}

/// Collapse a rational to an integer when it is one — the canonical form used
/// everywhere a numeric result is produced.
pub fn from_rational(r: Rational) -> Value {
    match r.to_integer() {
        Some(n) => Value::Int(n),
        None => Value::Ratio(r),
    }
}

/// Wrap an exact real algebraic number, collapsing a rational one back to
/// `Int`/`Ratio` so ordinary results stay in their simplest form.
pub fn alg(a: Algebraic) -> Value {
    if a.is_rational() {
        let c = a.defining_polynomial().coeffs();
        // A rational root has defining polynomial `c1·x + c0` → `x = −c0/c1`.
        if c.len() == 2 {
            return from_rational(c[0].neg().div(&c[1]));
        }
        return Value::Int(Int::from(0));
    }
    Value::Alg(a)
}

fn is_alg(v: &Value) -> bool {
    matches!(v, Value::Alg(_))
}

/// Lift an exact real value to an algebraic number (for mixed arithmetic).
fn as_alg(v: &Value) -> EResult<Algebraic> {
    match v {
        Value::Int(n) => Ok(Algebraic::from_int(n.clone())),
        Value::Ratio(r) => Ok(Algebraic::from_rational(r.clone())),
        Value::Alg(a) => Ok(a.clone()),
        _ => err("expected an algebraic number"),
    }
}

/// An algebraic base raised to an integer power, by repeated multiplication.
fn alg_pow(a: &Algebraic, e: i64) -> EResult<Value> {
    if e == 0 {
        return Ok(Value::Int(Int::from(1)));
    }
    let n = e.unsigned_abs();
    if n > 4096 {
        return err("exponent too large for an algebraic base");
    }
    let mut acc = Algebraic::from_int(Int::from(1));
    for _ in 0..n {
        acc = acc.mul(a);
    }
    if e < 0 {
        if acc.signum() == 0 {
            return err("division by zero (0 raised to a negative power)");
        }
        acc = acc.recip();
    }
    Ok(alg(acc))
}

/// If `a` has degree ≤ 2, express it as the exact radical `p + q·√d`.
fn alg_quadratic(a: &Algebraic) -> Option<Quadratic> {
    let c = a.defining_polynomial().coeffs();
    if c.len() != 3 {
        return None; // not a quadratic irrational
    }
    let (c0, c1, c2) = (&c[0], &c[1], &c[2]); // c2·x² + c1·x + c0
    let disc = c1.mul(c1).sub(&rat(4).mul(c2).mul(c0)); // b² − 4ac
    if !disc.is_positive() {
        return None;
    }
    let two_c2 = rat(2).mul(c2);
    let p = c1.neg().div(&two_c2); // rational part −b/2a
    // √disc = √(dn·dd) / dd, so root = p ± √(dn·dd) / (dd·2a).
    let dn = disc.numerator().clone();
    let dd = disc.denominator().clone();
    let radicand = dn.mul(&dd);
    let denom = Rational::from_integer(dd).mul(&two_c2);
    let mut coeff = rat(1).div(&denom).abs();
    // Pick the ± branch by comparing this root's interval midpoint to p.
    let (lo, hi) = a.interval();
    let mid = lo.add(hi).div(&rat(2));
    if !mid.sub(&p).is_positive() {
        coeff = coeff.neg();
    }
    Some(Quadratic::new(p, coeff, radicand))
}

/// Render one surd term `coeff·√d` (or the whole `p + coeff·√d`), in text or TeX.
fn quad_render(q: &Quadratic, tex: bool) -> String {
    let a = q.rational_part();
    let b = q.surd_coefficient();
    let d = q.radicand();
    let surd = if tex { format!("\\sqrt{{{d}}}") } else { format!("√{d}") };
    let mag = b.abs();
    let term = if mag.is_one() {
        surd
    } else {
        let m = if tex { from_rational(mag).to_tex() } else { from_rational(mag).to_text() };
        format!("{m} {surd}")
    };
    if a.is_zero() {
        return if b.is_negative() { format!("-{term}") } else { term };
    }
    let ap = if tex { from_rational(a.clone()).to_tex() } else { from_rational(a.clone()).to_text() };
    let sign = if b.is_negative() { "-" } else { "+" };
    format!("{ap} {sign} {term}")
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
        Value::Alg(a) => Ok(a.to_float(WORK_BITS, NEAR)),
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
        Value::Int(_) | Value::Ratio(_) | Value::Real(_) | Value::Sym { .. } | Value::Alg(_) => {
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
    if is_poly(a) || is_poly(b) {
        Ok(from_mpoly(as_mpoly(a)?.add(&as_mpoly(b)?)))
    } else if is_complex(a) || is_complex(b) {
        if exact_complex_ok(a) && exact_complex_ok(b) {
            Ok(from_complex(complex_rat(a)?.add(&complex_rat(b)?)))
        } else {
            from_complex_float(cf_add(&complex_float(a)?, &complex_float(b)?))
        }
    } else if is_inexact(a) || is_inexact(b) {
        real(to_float(a)?.add(&to_float(b)?, WORK_BITS, NEAR))
    } else if is_alg(a) || is_alg(b) {
        Ok(alg(as_alg(a)?.add(&as_alg(b)?)))
    } else {
        Ok(from_rational(to_rational(a)?.add(&to_rational(b)?)))
    }
}

pub fn sub(a: &Value, b: &Value) -> EResult<Value> {
    if is_poly(a) || is_poly(b) {
        Ok(from_mpoly(as_mpoly(a)?.sub(&as_mpoly(b)?)))
    } else if is_complex(a) || is_complex(b) {
        if exact_complex_ok(a) && exact_complex_ok(b) {
            Ok(from_complex(complex_rat(a)?.sub(&complex_rat(b)?)))
        } else {
            from_complex_float(cf_sub(&complex_float(a)?, &complex_float(b)?))
        }
    } else if is_inexact(a) || is_inexact(b) {
        real(to_float(a)?.sub(&to_float(b)?, WORK_BITS, NEAR))
    } else if is_alg(a) || is_alg(b) {
        Ok(alg(as_alg(a)?.sub(&as_alg(b)?)))
    } else {
        Ok(from_rational(to_rational(a)?.sub(&to_rational(b)?)))
    }
}

pub fn mul(a: &Value, b: &Value) -> EResult<Value> {
    if is_poly(a) || is_poly(b) {
        Ok(from_mpoly(as_mpoly(a)?.mul(&as_mpoly(b)?)))
    } else if is_complex(a) || is_complex(b) {
        if exact_complex_ok(a) && exact_complex_ok(b) {
            Ok(from_complex(complex_rat(a)?.mul(&complex_rat(b)?)))
        } else {
            from_complex_float(cf_mul(&complex_float(a)?, &complex_float(b)?))
        }
    } else if is_inexact(a) || is_inexact(b) {
        real(to_float(a)?.mul(&to_float(b)?, WORK_BITS, NEAR))
    } else if is_alg(a) || is_alg(b) {
        Ok(alg(as_alg(a)?.mul(&as_alg(b)?)))
    } else {
        Ok(from_rational(to_rational(a)?.mul(&to_rational(b)?)))
    }
}

pub fn div(a: &Value, b: &Value) -> EResult<Value> {
    if is_poly(b) {
        // `Value::Poly` is never constant, so this is a genuine polynomial
        // divisor — the quotient would not be a polynomial.
        return err("division by a polynomial is not supported");
    }
    if is_poly(a) {
        let rb = to_rational(b)?;
        if rb.is_zero() {
            return err("division by zero");
        }
        return Ok(from_mpoly(as_mpoly(a)?.scalar_mul(&Rational::ONE.div(&rb))));
    }
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
    } else if is_alg(a) || is_alg(b) {
        let bb = as_alg(b)?;
        if bb.signum() == 0 {
            return err("division by zero");
        }
        Ok(alg(as_alg(a)?.div(&bb)))
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
        Value::Poly(p) => Ok(Value::Poly(p.neg())),
        Value::Cplx(c) => Ok(from_complex(c.neg())),
        Value::CplxReal(c) => {
            from_complex_float(Complex::new(c.re.neg(), c.im.neg()))
        }
        Value::Alg(x) => Ok(alg(x.neg())),
        _ if is_inexact(a) => Ok(Value::Real(to_float(a)?.neg())),
        _ => Ok(from_rational(to_rational(a)?.neg())),
    }
}

pub fn abs(v: &Value) -> EResult<Value> {
    if let Value::Alg(a) = v {
        return Ok(alg(if a.signum() < 0 { a.neg() } else { a.clone() }));
    }
    if is_inexact(v) {
        Ok(Value::Real(to_float(v)?.abs()))
    } else {
        Ok(from_rational(to_rational(v)?.abs()))
    }
}

pub fn pow(base: &Value, exp: &Value) -> EResult<Value> {
    // A polynomial base — only a constant non-negative integer power stays
    // polynomial (`Value::Poly` is never constant, so a negative power can't
    // collapse to a number).
    if is_poly(base) || is_poly(exp) {
        let Value::Poly(p) = base else {
            return err("a polynomial exponent is not supported");
        };
        let n = to_i64(&as_int(exp).map_err(|_| {
            EvalError("a polynomial can only be raised to a constant integer power".into())
        })?)?;
        if !(0..=1024).contains(&n) {
            return err("a polynomial power must be between 0 and 1024");
        }
        return Ok(from_mpoly(p.pow(n as u32)));
    }
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
            if let Value::Alg(a) = base {
                return alg_pow(a, e);
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
            Value::Alg(a) => match alg_quadratic(a) {
                Some(q) => quad_render(&q, false),
                None => real_string(&a.to_float(WORK_BITS, NEAR)),
            },
            Value::Cplx(c) => complex_render(c, false),
            Value::CplxReal(c) => complex_render_float(c, false),
            Value::Poly(p) => p.to_text(),
            Value::Graphics(_) => "«graphics»".to_string(),
            Value::Text(s) => s.clone(),
            Value::Bool(b) => if *b { "True" } else { "False" }.to_string(),
            Value::Decimal(s) => s.clone(),
            Value::List(xs) => {
                let inner: Vec<String> = xs.iter().map(Value::to_text).collect();
                format!("{{{}}}", inner.join(", "))
            }
            Value::Rules(rs) => {
                let inner: Vec<String> = rs.iter().map(|(k, v)| format!("{k} -> {}", v.to_text())).collect();
                format!("{{{}}}", inner.join(", "))
            }
            Value::Solutions { rows, .. } => {
                let inner: Vec<String> = rows
                    .iter()
                    .map(|r| {
                        let c: Vec<String> = r.iter().map(|(k, v)| format!("{k} -> {}", v.to_text())).collect();
                        format!("{{{}}}", c.join(", "))
                    })
                    .collect();
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
            Value::Alg(a) => match alg_quadratic(a) {
                Some(q) => quad_render(&q, true),
                None => real_string(&a.to_float(WORK_BITS, NEAR)),
            },
            Value::Cplx(c) => complex_render(c, true),
            Value::CplxReal(c) => complex_render_float(c, true),
            Value::Poly(p) => p.to_tex(),
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
            Value::Rules(rs) => {
                if rs.is_empty() {
                    "\\left\\{\\,\\right\\}".to_string()
                } else {
                    let inner: Vec<String> = rs
                        .iter()
                        .map(|(k, v)| format!("{k} \\to {}", v.to_tex()))
                        .collect();
                    format!("\\left\\{{{}\\right\\}}", inner.join(",\\ "))
                }
            }
            Value::Solutions { rows, .. } => {
                let inner: Vec<String> = rows
                    .iter()
                    .map(|r| {
                        let c: Vec<String> =
                            r.iter().map(|(k, v)| format!("{k} \\to {}", v.to_tex())).collect();
                        format!("\\left\\{{{}\\right\\}}", c.join(",\\ "))
                    })
                    .collect();
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

    /// For a `Solve` result, a rectangular table (variables × solutions) for the
    /// frontend to render. Cells are TeX, aligned to the shared variable list.
    pub fn solutions(&self) -> Option<SolutionTable> {
        let Value::Solutions { rows, truncated } = self else {
            return None;
        };
        let mut vars: Vec<String> = Vec::new();
        for r in rows {
            for (k, _) in r {
                if !vars.contains(k) {
                    vars.push(k.clone());
                }
            }
        }
        let cells = rows
            .iter()
            .map(|r| {
                vars.iter()
                    .map(|v| r.iter().find(|(k, _)| k == v).map(|(_, val)| val.to_tex()).unwrap_or_default())
                    .collect()
            })
            .collect();
        Some(SolutionTable { vars, rows: cells, truncated: *truncated })
    }

    /// The decimal approximation to show *alongside* an exact result, when it
    /// adds information. `None` for values whose primary rendering is already
    /// their best form — integers, reals (already a decimal), booleans, lists,
    /// factorizations, and `N[..]` output.
    pub fn approx(&self) -> Option<String> {
        match self {
            Value::Ratio(r) => Some(real_string(&Float::from_rational(r, WORK_BITS, NEAR))),
            Value::Sym { val, .. } => Some(real_string(val)),
            // A radical (degree ≤ 2) shows its decimal; a degree ≥ 3 root already
            // renders as a decimal, so it needs no second copy.
            Value::Alg(a) if alg_quadratic(a).is_some() => Some(real_string(&a.to_float(WORK_BITS, NEAR))),
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
