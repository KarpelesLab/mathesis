//! Random-number generation.
//!
//! Every value here is drawn from the platform CSPRNG via `getrandom` — the Web
//! Crypto generator (`crypto.getRandomValues`) in the browser, the OS RNG when
//! run natively — so all output is cryptographically secure. There is no
//! seedable/insecure fast path: `Random…` is safe by construction.

use puremp::{Float, Int, Nat, RandomSource};

use crate::error::{EResult, err};
use crate::value::{self, NEAR, Value, WORK_BITS};

/// A `RandomSource` drawing directly from the platform CSPRNG.
struct Csprng;

impl RandomSource for Csprng {
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        getrandom::getrandom(dest).expect("platform CSPRNG unavailable");
    }
}

/// Upper bound on how many values a single call may produce, so a stray large
/// count can't exhaust memory (the worker's time budget is a further backstop).
const MAX_COUNT: usize = 100_000;
/// Attempts before `RandomPrime` gives up (a range may hold no primes).
const PRIME_ATTEMPTS: usize = 4096;

fn one() -> Int {
    Int::from(1)
}

/// A non-negative repeat count from a value.
fn count(v: &Value) -> EResult<usize> {
    let n = value::to_i64(&value::as_int(v)?)?;
    if n < 0 {
        return err("the count must be non-negative");
    }
    let n = n as usize;
    if n > MAX_COUNT {
        return err(format!("the count is too large (max {MAX_COUNT})"));
    }
    Ok(n)
}

/// Collect `k` samples of `f` into a list.
fn repeat(k: usize, mut f: impl FnMut() -> EResult<Value>) -> EResult<Value> {
    let mut out = Vec::with_capacity(k);
    for _ in 0..k {
        out.push(f()?);
    }
    Ok(Value::List(out))
}

// --- integers ---------------------------------------------------------------

/// Interpret an integer spec: `n` → `[0, n]`; `{min, max}` → `[min, max]`.
fn int_range(spec: &Value) -> EResult<(Int, Int)> {
    match spec {
        Value::List(items) => {
            if items.len() != 2 {
                return err("RandomInteger: a range must be {min, max}");
            }
            Ok((value::as_int(&items[0])?, value::as_int(&items[1])?))
        }
        _ => Ok((Int::from(0), value::as_int(spec)?)),
    }
}

/// One uniform integer in `[lo, hi]` (inclusive).
fn int_in(lo: &Int, hi: &Int) -> EResult<Value> {
    let range = hi.sub(lo).add(&one());
    match Int::random_below(&range, &mut Csprng) {
        Some(s) => Ok(Value::Int(lo.add(&s))),
        None => err("RandomInteger: the range is empty (need max >= min)"),
    }
}

pub fn random_integer(args: &[Value]) -> EResult<Value> {
    match args.len() {
        0 => int_in(&Int::from(0), &one()), // a coin flip: 0 or 1
        1 => {
            let (lo, hi) = int_range(&args[0])?;
            int_in(&lo, &hi)
        }
        2 => {
            let (lo, hi) = int_range(&args[0])?;
            repeat(count(&args[1])?, || int_in(&lo, &hi))
        }
        _ => err("RandomInteger expects a spec and an optional count"),
    }
}

// --- reals ------------------------------------------------------------------

/// A uniform real in `[0, 1)` at full working precision.
fn unit() -> Float {
    let mag = Int::from(Nat::random_bits(WORK_BITS, &mut Csprng));
    let denom = Int::from(2).pow(WORK_BITS as u32);
    Float::from_int(&mag, WORK_BITS, NEAR).div(&Float::from_int(&denom, WORK_BITS, NEAR), WORK_BITS, NEAR)
}

/// Interpret a real spec: `max` → `[0, max)`; `{min, max}` → `[min, max)`.
fn real_range(spec: &Value) -> EResult<(Float, Float)> {
    match spec {
        Value::List(items) => {
            if items.len() != 2 {
                return err("RandomReal: a range must be {min, max}");
            }
            Ok((value::to_float(&items[0])?, value::to_float(&items[1])?))
        }
        _ => Ok((Float::from_int(&Int::from(0), WORK_BITS, NEAR), value::to_float(spec)?)),
    }
}

fn real_in(lo: &Float, hi: &Float) -> EResult<Value> {
    let span = hi.sub(lo, WORK_BITS, NEAR);
    value::real(lo.add(&unit().mul(&span, WORK_BITS, NEAR), WORK_BITS, NEAR))
}

pub fn random_real(args: &[Value]) -> EResult<Value> {
    match args.len() {
        0 => value::real(unit()),
        1 => {
            let (lo, hi) = real_range(&args[0])?;
            real_in(&lo, &hi)
        }
        2 => {
            let (lo, hi) = real_range(&args[0])?;
            repeat(count(&args[1])?, || real_in(&lo, &hi))
        }
        _ => err("RandomReal expects a spec and an optional count"),
    }
}

// --- choice & primes --------------------------------------------------------

pub fn random_choice(args: &[Value]) -> EResult<Value> {
    let pool = match args.first() {
        Some(Value::List(items)) if !items.is_empty() => items,
        Some(Value::List(_)) => return err("RandomChoice: the list is empty"),
        _ => return err("RandomChoice expects a list"),
    };
    let pick = || {
        let i = Int::random_below(&Int::from(pool.len() as i64), &mut Csprng)
            .and_then(|n| value::to_i64(&n).ok())
            .unwrap_or(0) as usize;
        pool[i].clone()
    };
    match args.len() {
        1 => Ok(pick()),
        2 => repeat(count(&args[1])?, || Ok(pick())),
        _ => err("RandomChoice expects a list and an optional count"),
    }
}

pub fn random_prime(args: &[Value]) -> EResult<Value> {
    if args.len() != 1 {
        return err("RandomPrime expects a max or a {min, max} range");
    }
    let (lo, hi) = int_range(&args[0])?;
    // 2 is the smallest prime.
    let two = Int::from(2);
    let lo = if lo < two { two } else { lo };
    if hi < lo {
        return err("RandomPrime: no prime in the range");
    }
    let range = hi.sub(&lo).add(&one());
    for _ in 0..PRIME_ATTEMPTS {
        let cand = lo.add(&Int::random_below(&range, &mut Csprng).unwrap());
        if cand.is_prime_bpsw() {
            return Ok(Value::Int(cand));
        }
    }
    err("RandomPrime: no prime found in the range")
}

// --- raw entropy ------------------------------------------------------------

/// `RandomBytes[n]` — `n` cryptographically-secure bytes as a lowercase hex
/// string (for keys, nonces, salts, …).
pub fn random_bytes(args: &[Value]) -> EResult<Value> {
    if args.len() != 1 {
        return err("RandomBytes expects a byte count");
    }
    let n = count(&args[0])?;
    let mut buf = alloc_bytes(n);
    Csprng.fill_bytes(&mut buf);
    let mut hex = String::with_capacity(n * 2);
    for b in &buf {
        hex.push_str(&format!("{b:02x}"));
    }
    Ok(Value::Text(hex))
}

fn alloc_bytes(n: usize) -> Vec<u8> {
    vec![0u8; n]
}
