//! Symbolic differentiation: the `D[..]` form.
//!
//! `D` holds its arguments unevaluated (a free variable is not an error here —
//! it *is* the thing being differentiated with respect to), reads the
//! expression as a multivariate polynomial over ℚ, and differentiates that:
//!
//! - `D[f, x]` — the partial derivative ∂f/∂x;
//! - `D[f, x, y]` — successive derivatives ∂²f/∂y∂x;
//! - `D[f, {x, n}]` — the n-th derivative;
//! - `D[f, {{x, y, …}}]` — the gradient of a scalar `f`;
//! - `D[{f1, f2, …}, {{x, y, …}}]` — the Jacobian matrix (so
//!   `Det[D[{…}, {{x, y, z}}]]` is the Jacobian determinant).
//!
//! Only polynomial expressions are supported for now — `+ - *`, division by a
//! constant, and constant non-negative integer powers. Session variables bound
//! to exact numbers substitute as constants; everything else must be a free
//! symbol, which becomes a polynomial variable.

use puremp::{Int, Rational};

use crate::ast::{Expr, Op};
use crate::error::{EResult, EvalError, err};
use crate::mpoly::MPoly;
use crate::value::{self, Value};

/// A scalar or an arbitrarily nested list of polynomials — `D` threads over
/// lists, and a gradient turns a scalar leaf into a list.
enum Tree {
    Leaf(MPoly),
    List(Vec<Tree>),
}

pub fn d_form(args: &[Expr]) -> EResult<Value> {
    if args.len() < 2 {
        return err("D expects an expression and at least one variable, e.g. D[x^2, x] or D[{f, g}, {{x, y}}]");
    }
    let mut tree = tree_of(&args[0])?;
    for spec in &args[1..] {
        tree = apply_spec(tree, spec)?;
    }
    Ok(tree_value(tree))
}

fn tree_of(e: &Expr) -> EResult<Tree> {
    match e {
        Expr::List(items) => Ok(Tree::List(items.iter().map(tree_of).collect::<EResult<Vec<_>>>()?)),
        _ => Ok(Tree::Leaf(to_poly(e)?)),
    }
}

fn tree_value(t: Tree) -> Value {
    match t {
        Tree::Leaf(p) => value::from_mpoly(p),
        Tree::List(items) => Value::List(items.into_iter().map(tree_value).collect()),
    }
}

fn map_leaves(t: Tree, f: &impl Fn(&MPoly) -> Tree) -> Tree {
    match t {
        Tree::Leaf(p) => f(&p),
        Tree::List(items) => Tree::List(items.into_iter().map(|i| map_leaves(i, f)).collect()),
    }
}

/// One differentiation specification: a variable, `{x, n}`, or `{{x, y, …}}`.
fn apply_spec(tree: Tree, spec: &Expr) -> EResult<Tree> {
    match spec {
        Expr::Symbol(s) => {
            let x = diff_var(s)?;
            Ok(map_leaves(tree, &|p| Tree::Leaf(p.derivative(&x))))
        }
        Expr::List(items) => match &items[..] {
            // {{x, y, …}} — the gradient: each scalar becomes a list.
            [Expr::List(vars)] => {
                let xs = vars
                    .iter()
                    .map(|v| match v {
                        Expr::Symbol(s) => diff_var(s),
                        _ => err("each gradient variable must be a name, e.g. {{x, y, z}}"),
                    })
                    .collect::<EResult<Vec<_>>>()?;
                Ok(map_leaves(tree, &|p| {
                    Tree::List(xs.iter().map(|x| Tree::Leaf(p.derivative(x))).collect())
                }))
            }
            // {x, n} — the n-th derivative.
            [Expr::Symbol(s), n] => {
                let x = diff_var(s)?;
                let n = const_index(n)
                    .ok_or_else(|| EvalError("the derivative order in D[f, {x, n}] must be a small non-negative integer".into()))?;
                let mut t = tree;
                for _ in 0..n {
                    t = map_leaves(t, &|p| Tree::Leaf(p.derivative(&x)));
                }
                Ok(t)
            }
            _ => err("a D variable must be a name, {x, n}, or {{x, y, …}}"),
        },
        _ => err("a D variable must be a name, {x, n}, or {{x, y, …}}"),
    }
}

/// Validate a differentiation variable: it must be a *free* symbol — a bound
/// session variable or a built-in constant is a value, not a variable.
fn diff_var(s: &str) -> EResult<String> {
    if is_constant_name(s) {
        return err(format!("`{s}` is a built-in constant, not a variable to differentiate by"));
    }
    if crate::eval::binding(s).is_some() {
        return err(format!(
            "`{s}` already has a session value, so it cannot be a differentiation variable (reload to clear bindings)"
        ));
    }
    Ok(s.to_string())
}

fn is_constant_name(s: &str) -> bool {
    matches!(s, "Pi" | "E" | "I" | "True" | "False" | "EulerGamma" | "Catalan")
}

/// The value of a small constant non-negative integer literal.
fn const_index(e: &Expr) -> Option<u32> {
    if let Expr::Int(s) = e {
        let v = s.parse::<u32>().ok()?;
        if v <= 1024 {
            return Some(v);
        }
    }
    None
}

/// Read an expression as a multivariate polynomial. Bound session variables
/// substitute their exact values; free symbols become polynomial variables.
fn to_poly(e: &Expr) -> EResult<MPoly> {
    match e {
        Expr::Int(s) => Ok(MPoly::constant(Rational::from_integer(
            Int::from_str_radix(s, 10).map_err(|_| EvalError(format!("invalid integer literal `{s}`")))?,
        ))),
        Expr::Decimal { int, frac } => {
            let num = Int::from_str_radix(&format!("{int}{frac}"), 10)
                .map_err(|_| EvalError(format!("invalid decimal literal `{int}.{frac}`")))?;
            let den = Int::from(10).pow(frac.len() as u32);
            Ok(MPoly::constant(Rational::new(num, den)))
        }
        Expr::Symbol(s) => {
            if is_constant_name(s) {
                return err(format!("`{s}` is not rational, so it cannot appear in D (polynomials over ℚ only)"));
            }
            match crate::eval::binding(s) {
                None => Ok(MPoly::var(s)),
                Some(v) => match &v {
                    Value::Poly(p) => Ok(p.clone()),
                    _ => value::to_rational(&v).map(MPoly::constant).map_err(|_| {
                        EvalError(format!("`{s}` is bound to a non-rational value and cannot appear in D"))
                    }),
                },
            }
        }
        Expr::Neg(a) => Ok(to_poly(a)?.neg()),
        Expr::Bin(op, a, b) => match op {
            Op::Add => Ok(to_poly(a)?.add(&to_poly(b)?)),
            Op::Sub => Ok(to_poly(a)?.sub(&to_poly(b)?)),
            Op::Mul => Ok(to_poly(a)?.mul(&to_poly(b)?)),
            Op::Div => {
                let d = to_poly(b)?
                    .as_constant()
                    .ok_or_else(|| EvalError("D supports division by a constant only".into()))?;
                if d.is_zero() {
                    return err("division by zero");
                }
                Ok(to_poly(a)?.scalar_mul(&Rational::ONE.div(&d)))
            }
            Op::Pow => {
                let n = const_index(b).ok_or_else(|| {
                    EvalError("D supports constant non-negative integer exponents only".into())
                })?;
                Ok(to_poly(a)?.pow(n))
            }
            _ => err("D expects a polynomial expression, not a comparison or logical formula"),
        },
        _ => err(
            "D supports polynomial expressions only for now — built from + - * /, integer powers, numbers, and variables",
        ),
    }
}
