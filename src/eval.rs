//! The evaluator: walk an [`Expr`] and produce a [`Value`], delegating every
//! real computation to `puremp`. It also owns the session's `%` (last result)
//! via a thread-local — the wasm world is single-threaded, so one cell's output
//! is available to the next.

use core::cell::RefCell;
use core::cmp::Ordering;
use std::collections::HashMap;

use puremp::{Complex, Float, Int, Matrix, Rational, RoundingMode, lll_reduce, lll_reduce_delta};

use crate::ast::{Expr, Op};
use crate::error::{EResult, EvalError, err};
use crate::value::{self, NEAR, Value, WORK_BITS};

thread_local! {
    static LAST: RefCell<Option<Value>> = const { RefCell::new(None) };
}

pub fn set_last(v: Value) {
    LAST.with(|c| *c.borrow_mut() = Some(v));
}

fn get_last() -> EResult<Value> {
    LAST.with(|c| {
        c.borrow()
            .clone()
            .ok_or_else(|| EvalError("no previous result to reference with `%`".into()))
    })
}

thread_local! {
    static ENV: RefCell<Vec<(String, Value)>> = const { RefCell::new(Vec::new()) };
}

thread_local! {
    /// Session variables bound with `=` (persist across cells, like `%`).
    static GLOBALS: RefCell<HashMap<String, Value>> = RefCell::new(HashMap::new());
}

/// Built-in names that may not be reassigned.
const RESERVED: &[&str] = &["Pi", "E", "I", "True", "False", "EulerGamma", "Catalan"];

fn global_get(name: &str) -> Option<Value> {
    GLOBALS.with(|g| g.borrow().get(name).cloned())
}

/// Bind (or rebind) a session variable, returning the stored value. Variables
/// live for the worker's lifetime (cleared on a page reload / fresh sheet, like
/// `%`).
fn assign(name: &str, value: Value) -> EResult<Value> {
    if RESERVED.contains(&name) {
        return err(format!("`{name}` is a built-in constant and cannot be reassigned"));
    }
    GLOBALS.with(|g| g.borrow_mut().insert(name.to_string(), value.clone()));
    Ok(value)
}

fn lookup(name: &str) -> Option<Value> {
    ENV.with(|e| {
        e.borrow()
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.clone())
    })
}

/// Evaluate `expr` with extra variable bindings in scope. Used by `Plot` to bind
/// the plot variable to each sample point.
pub fn eval_bound(expr: &Expr, bindings: Vec<(String, Value)>) -> EResult<Value> {
    let n = bindings.len();
    ENV.with(|e| e.borrow_mut().extend(bindings));
    let r = eval(expr);
    ENV.with(|e| {
        let mut b = e.borrow_mut();
        let len = b.len();
        b.truncate(len - n);
    });
    r
}

pub fn eval(e: &Expr) -> EResult<Value> {
    match e {
        Expr::Int(s) => Int::from_str_radix(s, 10)
            .map(Value::Int)
            .map_err(|_| EvalError(format!("invalid integer literal `{s}`"))),
        Expr::Decimal { int, frac } => decimal_literal(int, frac),
        Expr::Symbol(s) => symbol(s),
        Expr::Str(s) => Ok(Value::Text(s.clone())),
        Expr::Assign { name, value } => {
            let v = eval(value)?;
            assign(name, v)
        }
        Expr::Last => get_last(),
        Expr::Neg(x) => value::neg(&eval(x)?),
        Expr::Factorial(x) => factorial(&eval(x)?),
        Expr::Bin(op, a, b) => {
            let a = eval(a)?;
            let b = eval(b)?;
            match op {
                Op::Add => value::add(&a, &b),
                Op::Sub => value::sub(&a, &b),
                Op::Mul => value::mul(&a, &b),
                Op::Div => value::div(&a, &b),
                Op::Pow => value::pow(&a, &b),
                Op::Eq => Ok(Value::Bool(numeric_cmp(&a, &b)? == Ordering::Equal)),
                Op::Ne => Ok(Value::Bool(numeric_cmp(&a, &b)? != Ordering::Equal)),
                Op::Lt => Ok(Value::Bool(numeric_cmp(&a, &b)? == Ordering::Less)),
                Op::Le => Ok(Value::Bool(numeric_cmp(&a, &b)? != Ordering::Greater)),
                Op::Gt => Ok(Value::Bool(numeric_cmp(&a, &b)? == Ordering::Greater)),
                Op::Ge => Ok(Value::Bool(numeric_cmp(&a, &b)? != Ordering::Less)),
                Op::And => Ok(Value::Bool(as_bool(&a)? && as_bool(&b)?)),
                Op::Or => Ok(Value::Bool(as_bool(&a)? || as_bool(&b)?)),
            }
        }
        // Solver forms hold their arguments unevaluated — the constraint is
        // translated to SMT-LIB, not computed.
        Expr::Call(head, args)
            if matches!(
                head.as_str(),
                "SatisfiableQ" | "TautologyQ" | "FindInstance" | "Solve" | "Maximize" | "Minimize"
            ) =>
        {
            crate::solve::solve_form(head, args)
        }
        // Plot forms hold their arguments — the expression is sampled over a
        // range with the plot variable bound, not evaluated up front.
        Expr::Call(head, args) if matches!(head.as_str(), "Plot" | "Plot3D") => {
            crate::plot::plot_form(head, args)
        }
        Expr::Call(head, args) => {
            let vs = args.iter().map(eval).collect::<EResult<Vec<_>>>()?;
            call(head, &vs)
        }
        Expr::List(xs) => {
            let vs = xs.iter().map(eval).collect::<EResult<Vec<_>>>()?;
            Ok(Value::List(vs))
        }
    }
}

/// A decimal literal `int.frac` becomes the *exact* rational
/// `(int·10^k + frac) / 10^k` where `k = len(frac)` — never a lossy float.
fn decimal_literal(int: &str, frac: &str) -> EResult<Value> {
    let digits = format!("{int}{frac}");
    let num = Int::from_str_radix(&digits, 10)
        .map_err(|_| EvalError(format!("invalid decimal literal `{int}.{frac}`")))?;
    let den = Int::from(10).pow(frac.len() as u32);
    Ok(value::from_rational(puremp::Rational::new(num, den)))
}

/// Named constants. Irrational constants evaluate to a real at working
/// precision; `N[Pi, d]` then renders as many digits as requested.
fn symbol(name: &str) -> EResult<Value> {
    // Local (Plot) bindings shadow session variables, which shadow nothing but
    // the built-in constants below.
    if let Some(v) = lookup(name).or_else(|| global_get(name)) {
        return Ok(v);
    }
    match name {
        "Pi" => Ok(Value::Sym {
            text: "Pi".into(),
            tex: "\\pi".into(),
            val: Float::pi(WORK_BITS, NEAR),
        }),
        "E" => Ok(Value::Sym {
            text: "E".into(),
            tex: "e".into(),
            val: Float::e(WORK_BITS, NEAR),
        }),
        // The imaginary unit.
        "I" => Ok(Value::Cplx(Complex::imaginary(Rational::from_integer(Int::from(1))))),
        "True" => Ok(Value::Bool(true)),
        "False" => Ok(Value::Bool(false)),
        "EulerGamma" => Ok(Value::Sym {
            text: "EulerGamma".into(),
            tex: "\\gamma".into(),
            val: Float::euler_gamma(WORK_BITS, NEAR),
        }),
        "Catalan" => Ok(Value::Sym {
            text: "Catalan".into(),
            tex: "C".into(),
            val: Float::catalan(WORK_BITS, NEAR),
        }),
        _ => err(format!("undefined symbol `{name}`")),
    }
}

fn factorial(v: &Value) -> EResult<Value> {
    let n = value::as_int(v).map_err(|_| EvalError("factorial expects an integer".into()))?;
    if n.is_negative() {
        return err("factorial of a negative integer is undefined");
    }
    let n = value::to_u64(&n)?;
    Ok(Value::Int(Int::factorial(n)))
}

// --- builtin functions ------------------------------------------------------

fn arity(head: &str, args: &[Value], want: usize) -> EResult<()> {
    if args.len() == want {
        Ok(())
    } else {
        err(format!(
            "{head} expects {want} argument{}, got {}",
            if want == 1 { "" } else { "s" },
            args.len()
        ))
    }
}

fn call(head: &str, args: &[Value]) -> EResult<Value> {
    match head {
        "Factor" => {
            arity(head, args, 1)?;
            factor(&value::as_int(&args[0])?)
        }
        "GCD" => fold_ints(head, args, |a, b| a.gcd(b)),
        "LCM" => fold_ints(head, args, |a, b| a.lcm(b)),
        "Factorial" => {
            arity(head, args, 1)?;
            factorial(&args[0])
        }
        "Binomial" => {
            arity(head, args, 2)?;
            let n = value::to_u64(&value::as_int(&args[0])?)?;
            let k = value::to_u64(&value::as_int(&args[1])?)?;
            Ok(Value::Int(Int::binomial(n, k)))
        }
        "Fibonacci" => {
            arity(head, args, 1)?;
            let n = value::to_u64(&value::as_int(&args[0])?)?;
            Ok(Value::Int(Int::fibonacci(n)))
        }
        "PrimeQ" => {
            arity(head, args, 1)?;
            Ok(Value::Bool(value::as_int(&args[0])?.is_prime_bpsw()))
        }
        "Sqrt" => {
            arity(head, args, 1)?;
            sqrt(&args[0])
        }
        "Sin" => transcendental(head, args, Float::sin, |c| c.sin()),
        "Cos" => transcendental(head, args, Float::cos, |c| c.cos()),
        "Tan" => real_unary(head, args, Float::tan),
        "ArcSin" => real_unary(head, args, Float::asin),
        "ArcCos" => real_unary(head, args, Float::acos),
        "ArcTan" => match args.len() {
            // ArcTan[x, y] is the angle of the point (x, y) — atan2(y, x).
            2 => value::real(value::to_float(&args[1])?.atan2(&value::to_float(&args[0])?, WORK_BITS, NEAR)),
            _ => real_unary(head, args, Float::atan),
        },
        "Sinh" => real_unary(head, args, Float::sinh),
        "Cosh" => real_unary(head, args, Float::cosh),
        "Tanh" => real_unary(head, args, Float::tanh),
        "ArcSinh" => real_unary(head, args, Float::asinh),
        "ArcCosh" => real_unary(head, args, Float::acosh),
        "ArcTanh" => real_unary(head, args, Float::atanh),
        "Exp" => transcendental(head, args, Float::exp, |c| c.exp()),
        "Erf" => real_unary(head, args, Float::erf),
        "Erfc" => real_unary(head, args, Float::erfc),
        "Zeta" => real_unary(head, args, Float::zeta),
        "Gamma" => real_unary(head, args, Float::gamma),
        "LogGamma" => real_unary(head, args, Float::ln_gamma),
        "Beta" => {
            arity(head, args, 2)?;
            let a = value::to_float(&args[0])?;
            let b = value::to_float(&args[1])?;
            value::real(Float::beta(&a, &b, WORK_BITS, NEAR))
        }
        // PolyGamma[x] is the digamma ψ(x); PolyGamma[n, x] is the nth polygamma.
        "PolyGamma" => match args.len() {
            1 => value::real(value::to_float(&args[0])?.digamma(WORK_BITS, NEAR)),
            2 => {
                let n = value::to_u64(&value::as_int(&args[0])?)?;
                value::real(value::to_float(&args[1])?.polygamma(n, WORK_BITS, NEAR))
            }
            _ => err("PolyGamma expects PolyGamma[x] or PolyGamma[n, x]"),
        },
        "BesselJ" => bessel(head, args, Float::bessel_j),
        "BesselI" => bessel(head, args, Float::bessel_i),
        "BesselY" => bessel(head, args, Float::bessel_y),
        "BesselK" => bessel(head, args, Float::bessel_k),
        "Identify" => {
            arity(head, args, 1)?;
            let x = value::to_float(&args[0])?;
            match puremp::identify(&x, 256) {
                Some(id) => Ok(Value::Text(id.to_string())),
                None => err("Identify: no closed form found at this precision"),
            }
        }
        "Log" => log(head, args),
        "Log2" => {
            arity(head, args, 1)?;
            let x = value::to_float(&args[0])?.ln(WORK_BITS, NEAR);
            value::real(x.div(&Float::ln2(WORK_BITS, NEAR), WORK_BITS, NEAR))
        }
        "Log10" => {
            arity(head, args, 1)?;
            let x = value::to_float(&args[0])?.ln(WORK_BITS, NEAR);
            let ln10 = Float::from_int(&Int::from(10), WORK_BITS, NEAR).ln(WORK_BITS, NEAR);
            value::real(x.div(&ln10, WORK_BITS, NEAR))
        }
        // --- number theory (integer arguments) ---
        "PowerMod" => power_mod(head, args),
        "ModularInverse" => {
            arity(head, args, 2)?;
            let m = value::as_int(&args[1])?;
            value::as_int(&args[0])?
                .modinv(&m)
                .map(Value::Int)
                .ok_or_else(|| EvalError("ModularInverse: no inverse exists modulo m".into()))
        }
        "ExtendedGCD" => {
            arity(head, args, 2)?;
            let (g, s, t) = value::as_int(&args[0])?.extended_gcd(&value::as_int(&args[1])?);
            Ok(Value::List(vec![
                Value::Int(g),
                Value::List(vec![Value::Int(s), Value::Int(t)]),
            ]))
        }
        "JacobiSymbol" => {
            arity(head, args, 2)?;
            Ok(int_from(value::as_int(&args[0])?.jacobi(&value::as_int(&args[1])?)))
        }
        "ChineseRemainder" => {
            arity(head, args, 2)?;
            let residues = int_vec(&args[0])?;
            let moduli = int_vec(&args[1])?;
            if residues.len() != moduli.len() {
                return err("ChineseRemainder: the two lists must have equal length");
            }
            Int::crt(&residues, &moduli)
                .map(Value::Int)
                .ok_or_else(|| EvalError("ChineseRemainder: no solution (inconsistent or non-coprime moduli)".into()))
        }
        "Mod" => {
            arity(head, args, 2)?;
            let b = value::as_int(&args[1])?;
            if b.is_zero() {
                return err("Mod: the modulus must be nonzero");
            }
            Ok(Value::Int(value::as_int(&args[0])?.rem_euclid(&b)))
        }
        "Quotient" => {
            arity(head, args, 2)?;
            let b = value::as_int(&args[1])?;
            if b.is_zero() {
                return err("Quotient: the divisor must be nonzero");
            }
            Ok(Value::Int(value::as_int(&args[0])?.div_floor(&b)))
        }
        "SqrtMod" => {
            arity(head, args, 2)?;
            value::as_int(&args[0])?
                .sqrt_mod(&value::as_int(&args[1])?)
                .map(Value::Int)
                .ok_or_else(|| EvalError("SqrtMod: no square root exists".into()))
        }
        "Multinomial" => {
            if args.is_empty() {
                return err("Multinomial expects at least one argument");
            }
            let ks = args
                .iter()
                .map(|v| value::to_u64(&value::as_int(v)?))
                .collect::<EResult<Vec<_>>>()?;
            Ok(Value::Int(Int::multinomial(&ks)))
        }
        "LucasL" => {
            arity(head, args, 1)?;
            Ok(Value::Int(Int::lucas(value::to_u64(&value::as_int(&args[0])?)?)))
        }
        "NextPrime" => {
            arity(head, args, 1)?;
            Ok(Value::Int(value::as_int(&args[0])?.next_prime()))
        }
        "RandomInteger" => crate::random::random_integer(args),
        "RandomReal" => crate::random::random_real(args),
        "RandomChoice" => crate::random::random_choice(args),
        "RandomPrime" => crate::random::random_prime(args),
        "RandomBytes" => crate::random::random_bytes(args),
        "PreviousPrime" => {
            arity(head, args, 1)?;
            value::as_int(&args[0])?
                .prev_prime()
                .map(Value::Int)
                .ok_or_else(|| EvalError("PreviousPrime: there is no prime below 2".into()))
        }
        "EulerPhi" => {
            arity(head, args, 1)?;
            Ok(Value::Int(value::as_int(&args[0])?.euler_phi()))
        }
        "Divisors" => {
            arity(head, args, 1)?;
            let ds = value::as_int(&args[0])?.divisors();
            Ok(Value::List(ds.into_iter().map(Value::Int).collect()))
        }
        "DivisorSigma" => {
            arity(head, args, 2)?;
            let k = value::to_u64(&value::as_int(&args[0])?)?.min(u32::MAX as u64) as u32;
            Ok(Value::Int(value::as_int(&args[1])?.divisor_sigma(k)))
        }
        "MoebiusMu" => {
            arity(head, args, 1)?;
            Ok(int_from(value::as_int(&args[0])?.moebius_mu()))
        }
        "Radical" => {
            arity(head, args, 1)?;
            Ok(Value::Int(value::as_int(&args[0])?.radical()))
        }
        "DiscreteLog" => {
            // DiscreteLog[b, t, m] — least x ≥ 0 with b^x ≡ t (mod m).
            arity(head, args, 3)?;
            let base = value::as_int(&args[0])?;
            let target = value::as_int(&args[1])?;
            let modulus = value::as_int(&args[2])?;
            if !modulus.is_positive() || modulus.is_one() {
                return err("DiscreteLog: the modulus must be greater than 1");
            }
            let order = modulus.euler_phi();
            puremp::discrete_log(&base, &target, &modulus, &order)
                .map(Value::Int)
                .ok_or_else(|| EvalError("DiscreteLog: no solution exists".into()))
        }
        "EvenQ" => {
            arity(head, args, 1)?;
            Ok(Value::Bool(value::as_int(&args[0])?.is_even()))
        }
        "OddQ" => {
            arity(head, args, 1)?;
            Ok(Value::Bool(value::as_int(&args[0])?.is_odd()))
        }
        "IntegerQ" => {
            arity(head, args, 1)?;
            Ok(Value::Bool(matches!(&args[0], Value::Int(_))))
        }
        "Sign" => {
            arity(head, args, 1)?;
            sign(&args[0])
        }
        // --- rounding & rational conversions ---
        "Floor" => {
            arity(head, args, 1)?;
            Ok(Value::Int(as_exact_rational(&args[0])?.floor()))
        }
        "Ceiling" => {
            arity(head, args, 1)?;
            Ok(Value::Int(as_exact_rational(&args[0])?.ceil()))
        }
        "Round" => {
            arity(head, args, 1)?;
            Ok(Value::Int(as_exact_rational(&args[0])?.round()))
        }
        "IntegerPart" => {
            arity(head, args, 1)?;
            Ok(Value::Int(as_exact_rational(&args[0])?.trunc()))
        }
        "FractionalPart" => {
            arity(head, args, 1)?;
            let r = as_exact_rational(&args[0])?;
            let frac = r.sub(&Rational::from_integer(r.trunc()));
            if matches!(&args[0], Value::Real(_) | Value::Sym { .. }) {
                value::real(Float::from_rational(&frac, WORK_BITS, NEAR))
            } else {
                Ok(value::from_rational(frac))
            }
        }
        "ContinuedFraction" => {
            arity(head, args, 1)?;
            let terms = value::to_rational(&args[0])?.continued_fraction();
            Ok(Value::List(terms.into_iter().map(Value::Int).collect()))
        }
        "FromContinuedFraction" => {
            arity(head, args, 1)?;
            let terms = int_vec(&args[0])?;
            if terms.is_empty() {
                return err("FromContinuedFraction: the list must be non-empty");
            }
            Ok(value::from_rational(Rational::from_continued_fraction(&terms)))
        }
        "Rationalize" => {
            arity(head, args, 2)?;
            let r = as_exact_rational(&args[0])?;
            Ok(value::from_rational(r.approximate(&value::as_int(&args[1])?)))
        }
        // --- matrices (exact, over rationals) ---
        "Det" => {
            arity(head, args, 1)?;
            let m = rat_matrix(&args[0])?;
            if !m.is_square() {
                return err("Det: the matrix must be square");
            }
            Ok(value::from_rational(m.determinant()))
        }
        "Inverse" => {
            arity(head, args, 1)?;
            let m = rat_matrix(&args[0])?;
            if !m.is_square() {
                return err("Inverse: the matrix must be square");
            }
            m.inverse()
                .map(|inv| matrix_to_value(&inv))
                .ok_or_else(|| EvalError("Inverse: the matrix is singular".into()))
        }
        "Transpose" => {
            arity(head, args, 1)?;
            Ok(matrix_to_value(&rat_matrix(&args[0])?.transpose()))
        }
        "MatrixRank" => {
            arity(head, args, 1)?;
            Ok(Value::Int(Int::from(rat_matrix(&args[0])?.rank() as i64)))
        }
        "Dot" => {
            arity(head, args, 2)?;
            let a = rat_matrix(&args[0])?;
            let b = rat_matrix(&args[1])?;
            if a.cols() != b.rows() {
                return err("Dot: inner dimensions must match");
            }
            Ok(matrix_to_value(&a.mul(&b)))
        }
        "LinearSolve" => {
            arity(head, args, 2)?;
            let m = rat_matrix(&args[0])?;
            let b = rat_vec(&args[1])?;
            m.solve(&b)
                .map(|x| Value::List(x.into_iter().map(value::from_rational).collect()))
                .ok_or_else(|| EvalError("LinearSolve: no unique solution".into()))
        }
        "IdentityMatrix" => {
            arity(head, args, 1)?;
            let n = value::to_u64(&value::as_int(&args[0])?)?.min(1024) as usize;
            Ok(matrix_to_value(&Matrix::<Rational>::identity(n)))
        }
        "Power" => {
            arity(head, args, 2)?;
            value::pow(&args[0], &args[1])
        }
        "Abs" => {
            arity(head, args, 1)?;
            match &args[0] {
                // |a + b i| = sqrt(a² + b²).
                Value::Cplx(c) => sqrt(&value::from_rational(c.norm_sqr())),
                Value::CplxReal(c) => value::real(c.abs()),
                _ => value::abs(&args[0]),
            }
        }
        "Re" => {
            arity(head, args, 1)?;
            match &args[0] {
                Value::Cplx(c) => Ok(value::from_rational(c.re.clone())),
                Value::CplxReal(c) => value::real(c.re.clone()),
                other => value::to_float(other).map(|_| other.clone()),
            }
        }
        "Im" => {
            arity(head, args, 1)?;
            match &args[0] {
                Value::Cplx(c) => Ok(value::from_rational(c.im.clone())),
                Value::CplxReal(c) => value::real(c.im.clone()),
                other => value::to_float(other).map(|_| Value::Int(Int::from(0))),
            }
        }
        "Conjugate" => {
            arity(head, args, 1)?;
            match &args[0] {
                Value::Cplx(c) => Ok(Value::Cplx(c.conj())),
                Value::CplxReal(c) => {
                    value::from_complex_float(Complex::new(c.re.clone(), c.im.neg()))
                }
                other => value::to_float(other).map(|_| other.clone()),
            }
        }
        "Arg" => {
            arity(head, args, 1)?;
            arg(&args[0])
        }
        "LatticeReduce" => lattice_reduce(head, args),
        "SMT" => smt(head, args),
        "Numerator" => {
            arity(head, args, 1)?;
            match &args[0] {
                Value::Int(n) => Ok(Value::Int(n.clone())),
                Value::Ratio(r) => Ok(Value::Int(r.numerator().clone())),
                _ => err("Numerator expects a number"),
            }
        }
        "Denominator" => {
            arity(head, args, 1)?;
            match &args[0] {
                Value::Int(_) => Ok(Value::Int(Int::from(1))),
                Value::Ratio(r) => Ok(Value::Int(r.denominator().clone())),
                _ => err("Denominator expects a number"),
            }
        }
        "N" => {
            if args.len() != 1 && args.len() != 2 {
                return err("N expects 1 or 2 arguments");
            }
            let digits = if args.len() == 2 {
                value::to_u64(&value::as_int(&args[1])?)?.min(4096) as u32
            } else {
                10
            };
            match &args[0] {
                Value::Int(n) => {
                    let r = puremp::Rational::from_integer(n.clone());
                    Ok(Value::Decimal(value::decimal_string(&r, digits)))
                }
                Value::Ratio(r) => Ok(Value::Decimal(value::decimal_string(r, digits))),
                // A real/symbolic value is only backed to ~300 digits (see
                // `WORK_BITS`); asking for more would print rounding noise.
                Value::Real(f) => Ok(Value::Decimal(value::real_decimal(f, digits.min(300)))),
                Value::Sym { val, .. } => {
                    Ok(Value::Decimal(value::real_decimal(val, digits.min(300))))
                }
                _ => err("N expects a number"),
            }
        }
        _ => err(format!("unknown function `{head}`")),
    }
}

fn fold_ints(head: &str, args: &[Value], f: impl Fn(&Int, &Int) -> Int) -> EResult<Value> {
    if args.is_empty() {
        return err(format!("{head} expects at least one argument"));
    }
    let mut acc = value::as_int(&args[0])?;
    for a in &args[1..] {
        acc = f(&acc, &value::as_int(a)?);
    }
    Ok(Value::Int(acc))
}

/// Apply an arbitrary-precision `Float` method (`Sin`, `Exp`, …) to one numeric
/// argument, returning a real.
fn real_unary(
    head: &str,
    args: &[Value],
    f: impl Fn(&Float, u64, RoundingMode) -> Float,
) -> EResult<Value> {
    arity(head, args, 1)?;
    let x = value::to_float(&args[0])?;
    value::real(f(&x, WORK_BITS, NEAR))
}

/// `BesselJ[n, x]` / `BesselI[n, x]` — an integer order `n` and a real argument.
fn bessel(
    head: &str,
    args: &[Value],
    f: impl Fn(&Float, i64, u64, RoundingMode) -> Float,
) -> EResult<Value> {
    arity(head, args, 2)?;
    let n = value::to_i64(&value::as_int(&args[0])?)?;
    let x = value::to_float(&args[1])?;
    value::real(f(&x, n, WORK_BITS, NEAR))
}

/// `Log[x]` is the natural logarithm; `Log[b, x]` is the logarithm base `b`.
fn log(head: &str, args: &[Value]) -> EResult<Value> {
    match args.len() {
        1 => {
            let z = &args[0];
            // Complex, or a negative real (Log[-1] = i·π) → the complex logarithm.
            if is_cplx(z) {
                return value::from_complex_float(value::complex_float(z)?.ln());
            }
            let x = value::to_float(z)?;
            if x.is_sign_negative() {
                return value::from_complex_float(value::complex_float(z)?.ln());
            }
            value::real(x.ln(WORK_BITS, NEAR))
        }
        2 => {
            let base = value::to_float(&args[0])?.ln(WORK_BITS, NEAR);
            if base.is_zero() {
                return err("Log base must not be 1");
            }
            let x = value::to_float(&args[1])?.ln(WORK_BITS, NEAR);
            value::real(x.div(&base, WORK_BITS, NEAR))
        }
        _ => err(format!("{head} expects 1 or 2 arguments, got {}", args.len())),
    }
}

fn is_cplx(v: &Value) -> bool {
    matches!(v, Value::Cplx(_) | Value::CplxReal(_))
}

/// A one-argument transcendental: the real `Float` method for real inputs, the
/// `Complex<Float>` method for complex ones.
fn transcendental(
    head: &str,
    args: &[Value],
    rf: impl Fn(&Float, u64, RoundingMode) -> Float,
    cf: impl Fn(&puremp::Complex<Float>) -> puremp::Complex<Float>,
) -> EResult<Value> {
    arity(head, args, 1)?;
    if is_cplx(&args[0]) {
        value::from_complex_float(cf(&value::complex_float(&args[0])?))
    } else {
        value::real(rf(&value::to_float(&args[0])?, WORK_BITS, NEAR))
    }
}

fn sqrt(v: &Value) -> EResult<Value> {
    if let Value::Int(n) = v {
        if n.is_negative() {
            // A negative perfect square is exactly imaginary: Sqrt[-4] = 2 I.
            if let Some(k) = n.abs().sqrt_exact() {
                return Ok(Value::Cplx(Complex::new(
                    Rational::from_integer(Int::from(0)),
                    Rational::from_integer(k),
                )));
            }
            // Otherwise an inexact complex root.
            return value::from_complex_float(value::complex_float(v)?.sqrt());
        }
        // A perfect square is exact.
        if let Some(root) = n.sqrt_exact() {
            return Ok(Value::Int(root));
        }
        let approx = Float::from_int(n, WORK_BITS, NEAR).sqrt(WORK_BITS, NEAR);
        // A small non-square is kept exact *and* symbolic (√n), with its decimal
        // alongside. A huge radicand can't render legibly under a radical, so it
        // falls back to the real (scientific) value.
        if n.bit_len() <= 64 {
            return Ok(Value::Sym {
                text: format!("Sqrt[{n}]"),
                tex: format!("\\sqrt{{{n}}}"),
                val: approx,
            });
        }
        return value::real(approx);
    }
    // Complex argument, or a negative real → a complex square root.
    if is_cplx(v) {
        return value::from_complex_float(value::complex_float(v)?.sqrt());
    }
    let x = value::to_float(v)?;
    if x.is_sign_negative() {
        return value::from_complex_float(value::complex_float(v)?.sqrt());
    }
    value::real(x.sqrt(WORK_BITS, NEAR))
}

/// `Arg[z]` — the argument (phase) of a number, in radians.
fn arg(v: &Value) -> EResult<Value> {
    match v {
        Value::Cplx(c) => {
            let im = Float::from_rational(&c.im, WORK_BITS, NEAR);
            let re = Float::from_rational(&c.re, WORK_BITS, NEAR);
            value::real(im.atan2(&re, WORK_BITS, NEAR))
        }
        Value::CplxReal(c) => value::real(c.arg()),
        _ => {
            let x = value::to_float(v)?;
            value::real(Float::zero(WORK_BITS).atan2(&x, WORK_BITS, NEAR))
        }
    }
}

fn int_from(x: i32) -> Value {
    Value::Int(Int::from(x))
}

/// `SMT["…"]` runs an SMT-LIB 2 script through the z3rs solver and returns the
/// verbatim solver output — one line per command (`sat`/`unsat`/`unknown`,
/// models, `get-value` results, …). The engine is sound and terminating: a work
/// budget yields `unknown` rather than a wrong answer or a hang.
fn smt(head: &str, args: &[Value]) -> EResult<Value> {
    arity(head, args, 1)?;
    let script = match &args[0] {
        Value::Text(s) => s.as_str(),
        _ => {
            return err(
                "SMT expects a string, e.g. SMT[\"(declare-const x Int)(assert (> x 5))(check-sat)\"]",
            );
        }
    };
    match z3rs::cmd_context::run_smt2(script) {
        Ok(lines) if lines.is_empty() => Ok(Value::Text("(no output)".to_string())),
        Ok(lines) => Ok(Value::Text(lines.join("\n"))),
        Err(e) => err(format!("SMT error: {e}")),
    }
}

/// `PowerMod[a, b, m]` = a^b mod m, using the modular inverse of `a` for b < 0.
fn power_mod(head: &str, args: &[Value]) -> EResult<Value> {
    arity(head, args, 3)?;
    let a = value::as_int(&args[0])?;
    let b = value::as_int(&args[1])?;
    let m = value::as_int(&args[2])?;
    if m.is_zero() {
        return err("PowerMod: the modulus must be nonzero");
    }
    if b.is_negative() {
        let inv = a
            .modinv(&m)
            .ok_or_else(|| EvalError("PowerMod: the base is not invertible modulo m".into()))?;
        Ok(Value::Int(inv.modpow(&b.abs(), &m)))
    } else {
        Ok(Value::Int(a.modpow(&b, &m)))
    }
}

fn sign(v: &Value) -> EResult<Value> {
    let r = as_exact_rational(v)?;
    Ok(int_from(if r.is_zero() {
        0
    } else if r.is_negative() {
        -1
    } else {
        1
    }))
}

/// Read a `{a, b, …}` list as a vector of integers.
fn int_vec(v: &Value) -> EResult<Vec<Int>> {
    match v {
        Value::List(xs) => xs.iter().map(value::as_int).collect(),
        _ => err("expected a list of integers, e.g. {1, 2, 3}"),
    }
}

/// Read a `{a, b, …}` list as a vector of exact rationals.
fn rat_vec(v: &Value) -> EResult<Vec<Rational>> {
    match v {
        Value::List(xs) => xs.iter().map(value::to_rational).collect(),
        _ => err("expected a vector (list of numbers), e.g. {1, 2, 3}"),
    }
}

fn numeric_cmp(a: &Value, b: &Value) -> EResult<Ordering> {
    Ok(as_exact_rational(a)?.cmp(&as_exact_rational(b)?))
}

fn as_bool(v: &Value) -> EResult<bool> {
    match v {
        Value::Bool(b) => Ok(*b),
        _ => err("expected a boolean (True or False)"),
    }
}

/// The exact rational value of any real number, used by `Floor`/`Round`/etc.
/// For a real/symbolic value this is the exact value of its float approximation.
pub(crate) fn as_exact_rational(v: &Value) -> EResult<Rational> {
    match v {
        Value::Int(n) => Ok(Rational::from_integer(n.clone())),
        Value::Ratio(r) => Ok(r.clone()),
        Value::Real(f) => f
            .to_rational()
            .ok_or_else(|| EvalError("value is not finite".into())),
        Value::Sym { val, .. } => val
            .to_rational()
            .ok_or_else(|| EvalError("value is not finite".into())),
        _ => err("expected a real number"),
    }
}

/// Read a `{{...}, ...}` list-of-lists as an exact rational matrix (integer or
/// rational entries), checking that it is non-empty and rectangular.
fn rat_matrix(v: &Value) -> EResult<Matrix<Rational>> {
    let rows = match v {
        Value::List(rows) => rows,
        _ => return err("expected a matrix, e.g. {{1, 2}, {3, 4}}"),
    };
    if rows.is_empty() {
        return err("the matrix has no rows");
    }
    let mut data: Vec<Vec<Rational>> = Vec::with_capacity(rows.len());
    for row in rows {
        let cells = match row {
            Value::List(cells) => cells,
            _ => return err("each matrix row must be a list, e.g. {1, 2, 3}"),
        };
        data.push(cells.iter().map(value::to_rational).collect::<EResult<Vec<_>>>()?);
    }
    let width = data[0].len();
    if width == 0 || data.iter().any(|r| r.len() != width) {
        return err("all matrix rows must have the same (nonzero) length");
    }
    Ok(Matrix::from_rows(data))
}

fn matrix_to_value(m: &Matrix<Rational>) -> Value {
    let rows = (0..m.rows())
        .map(|i| {
            Value::List(
                (0..m.cols())
                    .map(|j| value::from_rational(m.get(i, j).clone()))
                    .collect(),
            )
        })
        .collect();
    Value::List(rows)
}

/// `LatticeReduce[{{...}, ...}]` LLL-reduces an integer lattice basis (rows are
/// basis vectors), delegating to `puremp`. An optional second argument is the
/// reduction parameter δ ∈ (1/4, 1] (default 3/4).
fn lattice_reduce(head: &str, args: &[Value]) -> EResult<Value> {
    let basis = match args.first() {
        Some(v) => int_matrix(v)?,
        None => return err(format!("{head} expects a list of integer vectors")),
    };

    let reduced = match args.len() {
        1 => lll_reduce(&basis),
        2 => {
            let delta = value::to_rational(&args[1])
                .map_err(|_| EvalError("LatticeReduce: delta must be a rational".into()))?;
            let low = Rational::new(Int::from(1), Int::from(4));
            let high = Rational::from_integer(Int::from(1));
            if !(delta > low && delta <= high) {
                return err("LatticeReduce: delta must be in the range (1/4, 1]");
            }
            lll_reduce_delta(&basis, &delta)
        }
        _ => return err(format!("{head} expects 1 or 2 arguments, got {}", args.len())),
    };

    Ok(Value::List(
        reduced
            .into_iter()
            .map(|row| Value::List(row.into_iter().map(Value::Int).collect()))
            .collect(),
    ))
}

/// Read a `{{…}, …}` list-of-lists as an integer matrix, checking that it is
/// non-empty, rectangular, and integer-valued.
fn int_matrix(v: &Value) -> EResult<Vec<Vec<Int>>> {
    let rows = match v {
        Value::List(rows) => rows,
        _ => return err("expected a list of integer vectors, e.g. {{1, 0}, {0, 1}}"),
    };
    if rows.is_empty() {
        return err("the basis must have at least one vector");
    }
    let mut out: Vec<Vec<Int>> = Vec::with_capacity(rows.len());
    for row in rows {
        let entries = match row {
            Value::List(e) => e,
            _ => return err("each basis vector must be a list, e.g. {1, 2, 3}"),
        };
        if let Some(first) = out.first() {
            if entries.len() != first.len() {
                return err("all basis vectors must have the same length");
            }
        }
        out.push(entries.iter().map(value::as_int).collect::<EResult<Vec<_>>>()?);
    }
    Ok(out)
}

fn factor(n: &Int) -> EResult<Value> {
    if n.is_zero() {
        return Ok(Value::Int(Int::from(0)));
    }
    let negative = n.is_negative();
    let magnitude = n.abs();
    // `factorize` yields prime factors in ascending order, with repetition; a
    // magnitude of 1 yields none.
    let primes = magnitude.factorize();

    let mut factors: Vec<(Int, u32)> = Vec::new();
    for p in primes {
        match factors.last_mut() {
            Some((prev, count)) if *prev == p => *count += 1,
            _ => factors.push((p, 1)),
        }
    }
    Ok(Value::Factored { negative, factors })
}
