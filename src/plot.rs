//! `Plot` and `Plot3D`: sample an expression numerically over a range (with the
//! plot variable bound) and produce a compact JSON graphics payload that the
//! frontend draws. Per-point errors and complex/non-finite values become gaps
//! (`null`) rather than failing the whole plot.

use puremp::Float;

use crate::ast::Expr;
use crate::error::{EResult, EvalError, err};
use crate::value::{self, NEAR, Value};

const N2D: usize = 300;
const N3D: usize = 44;

pub fn plot_form(head: &str, args: &[Expr]) -> EResult<Value> {
    match head {
        "Plot" => plot2d(args),
        "Plot3D" => plot3d(args),
        _ => err(format!("unknown plot form `{head}`")),
    }
}

/// A `{var, lo, hi}` range: the variable name and its evaluated numeric bounds.
fn parse_range(e: &Expr) -> EResult<(String, f64, f64)> {
    if let Expr::List(items) = e {
        if items.len() == 3 {
            if let Expr::Symbol(v) = &items[0] {
                let lo = eval_f64(&items[1])?;
                let hi = eval_f64(&items[2])?;
                if !lo.is_finite() || !hi.is_finite() || lo == hi {
                    return err("the range bounds must be distinct finite numbers");
                }
                let (lo, hi) = if lo < hi { (lo, hi) } else { (hi, lo) };
                return Ok((v.clone(), lo, hi));
            }
        }
    }
    err("expected a range like {x, 0, 2*Pi}")
}

fn eval_f64(e: &Expr) -> EResult<f64> {
    let v = crate::eval::eval(e)?;
    value::to_f64(&v).ok_or_else(|| EvalError("expected a real number".into()))
}

/// Bind `bindings` and evaluate `expr` to a finite `f64`, or `None` on any error
/// / non-real / non-finite result (a gap in the plot).
fn sample(expr: &Expr, bindings: Vec<(String, Value)>) -> Option<f64> {
    let v = crate::eval::eval_bound(expr, bindings).ok()?;
    let y = value::to_f64(&v)?;
    if y.is_finite() { Some(y) } else { None }
}

fn real(x: f64) -> Value {
    Value::Real(Float::from_f64(x, 53, NEAR))
}

/// Format a finite `f64` as JSON, or `null`.
fn num(x: f64) -> String {
    if x.is_finite() {
        format!("{x}")
    } else {
        "null".to_string()
    }
}

fn plot2d(args: &[Expr]) -> EResult<Value> {
    if args.len() != 2 {
        return err("Plot expects Plot[expr, {x, a, b}]");
    }
    let (var, a, b) = parse_range(&args[1])?;

    // A list first argument plots several curves at once.
    let exprs: Vec<&Expr> = match &args[0] {
        Expr::List(items) => items.iter().collect(),
        e => vec![e],
    };

    let mut finite = 0usize;
    let mut series: Vec<String> = Vec::with_capacity(exprs.len());
    for e in &exprs {
        let mut pts = String::from("[");
        for i in 0..N2D {
            let x = a + (b - a) * (i as f64) / ((N2D - 1) as f64);
            if i > 0 {
                pts.push(',');
            }
            match sample(e, vec![(var.clone(), real(x))]) {
                Some(y) => {
                    finite += 1;
                    pts.push_str(&format!("[{},{}]", num(x), num(y)));
                }
                None => pts.push_str(&format!("[{},null]", num(x))),
            }
        }
        pts.push(']');
        series.push(format!("{{\"points\":{pts}}}"));
    }

    if finite == 0 {
        return err("Plot: the expression did not evaluate to a real number over this range");
    }

    Ok(Value::Graphics(format!(
        "{{\"kind\":\"plot2d\",\"var\":{},\"xmin\":{},\"xmax\":{},\"series\":[{}]}}",
        json_str(&var),
        num(a),
        num(b),
        series.join(",")
    )))
}

fn plot3d(args: &[Expr]) -> EResult<Value> {
    if args.len() != 3 {
        return err("Plot3D expects Plot3D[expr, {x, a, b}, {y, c, d}]");
    }
    let (vx, ax, bx) = parse_range(&args[1])?;
    let (vy, ay, by) = parse_range(&args[2])?;
    if vx == vy {
        return err("Plot3D needs two different variables");
    }

    let mut finite = 0usize;
    let mut rows: Vec<String> = Vec::with_capacity(N3D);
    for j in 0..N3D {
        let y = ay + (by - ay) * (j as f64) / ((N3D - 1) as f64);
        let mut row = String::from("[");
        for i in 0..N3D {
            let x = ax + (bx - ax) * (i as f64) / ((N3D - 1) as f64);
            if i > 0 {
                row.push(',');
            }
            match sample(
                &args[0],
                vec![(vx.clone(), real(x)), (vy.clone(), real(y))],
            ) {
                Some(z) => {
                    finite += 1;
                    row.push_str(&num(z));
                }
                None => row.push_str("null"),
            }
        }
        row.push(']');
        rows.push(row);
    }

    if finite == 0 {
        return err("Plot3D: the expression did not evaluate to a real number over this range");
    }

    Ok(Value::Graphics(format!(
        "{{\"kind\":\"plot3d\",\"xmin\":{},\"xmax\":{},\"ymin\":{},\"ymax\":{},\"z\":[{}]}}",
        num(ax),
        num(bx),
        num(ay),
        num(by),
        rows.join(",")
    )))
}

/// Minimal JSON string escaping for a variable name.
fn json_str(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
