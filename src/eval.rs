//! The evaluator: walk an [`Expr`] and produce a [`Value`], delegating every
//! real computation to `puremp`. It also owns the session's `%` (last result)
//! via a thread-local — the wasm world is single-threaded, so one cell's output
//! is available to the next.

use core::cell::RefCell;

use puremp::{Float, Int, RoundingMode};

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

pub fn eval(e: &Expr) -> EResult<Value> {
    match e {
        Expr::Int(s) => Int::from_str_radix(s, 10)
            .map(Value::Int)
            .map_err(|_| EvalError(format!("invalid integer literal `{s}`"))),
        Expr::Decimal { int, frac } => decimal_literal(int, frac),
        Expr::Symbol(s) => symbol(s),
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
            }
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
    match name {
        "Pi" => value::real(Float::pi(WORK_BITS, NEAR)),
        "E" => value::real(Float::e(WORK_BITS, NEAR)),
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
        "Sin" => real_unary(head, args, Float::sin),
        "Cos" => real_unary(head, args, Float::cos),
        "Tan" => real_unary(head, args, Float::tan),
        "ArcTan" => real_unary(head, args, Float::atan),
        "Exp" => real_unary(head, args, Float::exp),
        "Log" => log(head, args),
        "Power" => {
            arity(head, args, 2)?;
            value::pow(&args[0], &args[1])
        }
        "Abs" => {
            arity(head, args, 1)?;
            value::abs(&args[0])
        }
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
                // A real is only backed to ~300 digits (see `WORK_BITS`); asking
                // for more would print rounding noise, so cap it there.
                Value::Real(f) => Ok(Value::Decimal(value::real_decimal(f, digits.min(300)))),
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

/// `Log[x]` is the natural logarithm; `Log[b, x]` is the logarithm base `b`.
fn log(head: &str, args: &[Value]) -> EResult<Value> {
    match args.len() {
        1 => value::real(value::to_float(&args[0])?.ln(WORK_BITS, NEAR)),
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

fn sqrt(v: &Value) -> EResult<Value> {
    // Stay exact when the argument is a perfect-square integer.
    if let Value::Int(n) = v {
        if n.is_negative() {
            return err("Sqrt of a negative number is not real (complex support is coming)");
        }
        if let Some(root) = n.sqrt_exact() {
            return Ok(Value::Int(root));
        }
    }
    // Otherwise, an arbitrary-precision real approximation.
    let x = value::to_float(v)?;
    if x.is_sign_negative() {
        return err("Sqrt of a negative number is not real (complex support is coming)");
    }
    value::real(x.sqrt(WORK_BITS, NEAR))
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
