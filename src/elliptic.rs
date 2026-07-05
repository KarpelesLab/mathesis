//! Elliptic curves `y² = x³ + a·x + b`, delegating the group law and point
//! counting to puremp. The finite-field forms work over `GF(p)` for a prime
//! modulus `p`; points are written `{x, y}`, and the point at infinity (the
//! group identity) is the empty list `{}`.

use puremp::{EllipticCurve, Int, ModInt, Point, Rational};

use crate::error::{EResult, EvalError, err};
use crate::value::{self, Value};

fn arity(head: &str, args: &[Value], want: usize) -> EResult<()> {
    if args.len() != want {
        return err(format!("{head} expects {want} argument(s), got {}", args.len()));
    }
    Ok(())
}

/// Build the curve `y² = x³ + a·x + b` over `GF(p)`, validating that `p` is
/// prime and the curve is non-singular.
fn curve_gfp(a: &Value, b: &Value, p: &Value) -> EResult<(EllipticCurve<ModInt>, Int)> {
    let p = value::as_int(p)?;
    if !p.is_prime_bpsw() {
        return err("the modulus must be a prime");
    }
    let a = ModInt::new(value::as_int(a)?, p.clone());
    let b = ModInt::new(value::as_int(b)?, p.clone());
    match EllipticCurve::new(a, b) {
        Some(c) => Ok((c, p)),
        None => err("singular curve: 4·a³ + 27·b² ≡ 0, so it is not an elliptic curve"),
    }
}

/// Parse a `{x, y}` point (or `{}` for the point at infinity) on the curve.
fn point_gfp(curve: &EllipticCurve<ModInt>, p: &Int, v: &Value) -> EResult<Point<ModInt>> {
    match v {
        Value::List(items) if items.is_empty() => Ok(curve.identity()),
        Value::List(items) if items.len() == 2 => {
            let x = ModInt::new(value::as_int(&items[0])?, p.clone());
            let y = ModInt::new(value::as_int(&items[1])?, p.clone());
            curve
                .point(x, y)
                .ok_or_else(|| EvalError("the point is not on the curve".into()))
        }
        _ => err("a point must be {x, y}, or {} for the point at infinity"),
    }
}

/// Render a point back as `{x, y}` (or `{}` at infinity).
fn point_value(pt: &Point<ModInt>) -> Value {
    match pt.coordinates() {
        None => Value::List(Vec::new()),
        Some((x, y)) => Value::List(vec![Value::Int(x.to_int()), Value::Int(y.to_int())]),
    }
}

/// `ECAdd[a, b, p, P, Q]` — the group sum `P + Q` on `y² = x³ + a·x + b` / `GF(p)`.
pub fn ec_add(args: &[Value]) -> EResult<Value> {
    arity("ECAdd", args, 5)?;
    let (curve, p) = curve_gfp(&args[0], &args[1], &args[2])?;
    let pp = point_gfp(&curve, &p, &args[3])?;
    let qq = point_gfp(&curve, &p, &args[4])?;
    Ok(point_value(&pp.add(&qq)))
}

/// `ECMultiply[a, b, p, k, P]` — the scalar multiple `k·P`.
pub fn ec_multiply(args: &[Value]) -> EResult<Value> {
    arity("ECMultiply", args, 5)?;
    let (curve, p) = curve_gfp(&args[0], &args[1], &args[2])?;
    let k = value::as_int(&args[3])?;
    let pt = point_gfp(&curve, &p, &args[4])?;
    Ok(point_value(&pt.scalar_mul(&k)))
}

/// `ECOrder[a, b, p]` — the number of points on the curve (including infinity).
pub fn ec_order(args: &[Value]) -> EResult<Value> {
    arity("ECOrder", args, 3)?;
    let (curve, _) = curve_gfp(&args[0], &args[1], &args[2])?;
    Ok(Value::Int(curve.curve_order()))
}

/// `ECPointOrder[a, b, p, P]` — the order of `P` in the group.
pub fn ec_point_order(args: &[Value]) -> EResult<Value> {
    arity("ECPointOrder", args, 4)?;
    let (curve, p) = curve_gfp(&args[0], &args[1], &args[2])?;
    let pt = point_gfp(&curve, &p, &args[3])?;
    Ok(Value::Int(curve.order_of_point(&pt)))
}

/// `ECPointQ[a, b, p, {x, y}]` — whether the point lies on the curve.
pub fn ec_point_q(args: &[Value]) -> EResult<Value> {
    arity("ECPointQ", args, 4)?;
    let (curve, p) = curve_gfp(&args[0], &args[1], &args[2])?;
    let on = match &args[3] {
        Value::List(items) if items.is_empty() => true,
        Value::List(items) if items.len() == 2 => {
            let x = ModInt::new(value::as_int(&items[0])?, p.clone());
            let y = ModInt::new(value::as_int(&items[1])?, p.clone());
            curve.point(x, y).is_some()
        }
        _ => return err("a point must be {x, y}, or {} for the point at infinity"),
    };
    Ok(Value::Bool(on))
}

/// `ECDiscriminant[a, b]` — `Δ = −16·(4·a³ + 27·b²)` (over the integers; `0`
/// means the curve is singular).
pub fn ec_discriminant(args: &[Value]) -> EResult<Value> {
    arity("ECDiscriminant", args, 2)?;
    let a = value::as_int(&args[0])?;
    let b = value::as_int(&args[1])?;
    let inner = a.pow(3).mul(&Int::from(4)).add(&b.mul(&b).mul(&Int::from(27)));
    Ok(Value::Int(inner.mul(&Int::from(-16))))
}

/// `ECjInvariant[a, b]` — `j = 1728·4a³ / (4a³ + 27b²)` (over ℚ); errors on a
/// singular curve.
pub fn ec_j_invariant(args: &[Value]) -> EResult<Value> {
    arity("ECjInvariant", args, 2)?;
    let a = Rational::from_integer(value::as_int(&args[0])?);
    let b = Rational::from_integer(value::as_int(&args[1])?);
    match EllipticCurve::new(a, b) {
        Some(c) => Ok(value::from_rational(c.j_invariant())),
        None => err("singular curve: the discriminant is zero"),
    }
}
