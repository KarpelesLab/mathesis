//! A Wolfram-style front end over the z3rs SMT solver.
//!
//! `SatisfiableQ[c]`, `FindInstance[c, vars]`, and `Solve[c, vars]` take a
//! constraint written in Mathesis syntax (using `== != < <= > >= && ||` and the
//! `And`/`Or`/`Not`/`Implies`/`Xor` heads), translate it to SMT-LIB 2, run it
//! through z3rs, and report the result. Only *linear* arithmetic over integers
//! or reals is decidable (z3rs decides QF_LIA / QF_LRA); a nonlinear constraint
//! (a product or power of two unknowns) may come back `unknown`.

use std::collections::BTreeSet;

use crate::ast::{Expr, Op};
use crate::error::{EResult, EvalError, err};
use crate::value::Value;

pub fn solve_form(head: &str, args: &[Expr]) -> EResult<Value> {
    match head {
        "SatisfiableQ" => {
            if args.len() != 1 {
                return err("SatisfiableQ expects one argument: a constraint");
            }
            satisfiable(&args[0])
        }
        "FindInstance" | "Solve" => {
            if !(2..=3).contains(&args.len()) {
                return err(format!(
                    "{head} expects 2 or 3 arguments: a constraint, the variable(s), and an optional domain (Integers or Reals)"
                ));
            }
            let wanted = var_list(&args[1])?;
            let domain = if args.len() == 3 {
                domain_of(&args[2])?
            } else {
                Domain::Integers
            };
            find_instance(&args[0], &wanted, domain)
        }
        _ => err(format!("unknown solver form `{head}`")),
    }
}

#[derive(Clone, Copy)]
enum Domain {
    Integers,
    Reals,
}

impl Domain {
    fn sort(self) -> &'static str {
        match self {
            Domain::Integers => "Int",
            Domain::Reals => "Real",
        }
    }
    fn logic(self) -> &'static str {
        match self {
            Domain::Integers => "QF_LIA",
            Domain::Reals => "QF_LRA",
        }
    }
}

fn domain_of(e: &Expr) -> EResult<Domain> {
    match e {
        Expr::Symbol(s) if s == "Integers" => Ok(Domain::Integers),
        Expr::Symbol(s) if s == "Reals" => Ok(Domain::Reals),
        _ => err("the domain must be Integers or Reals"),
    }
}

fn var_list(e: &Expr) -> EResult<Vec<String>> {
    match e {
        Expr::Symbol(s) => Ok(vec![s.clone()]),
        Expr::List(items) => items
            .iter()
            .map(|it| match it {
                Expr::Symbol(s) => Ok(s.clone()),
                _ => err("each variable must be a name, e.g. {x, y}"),
            })
            .collect(),
        _ => err("the variables must be a name or a list of names, e.g. {x, y}"),
    }
}

// --- decisions --------------------------------------------------------------

fn satisfiable(e: &Expr) -> EResult<Value> {
    let mut vars = BTreeSet::new();
    let term = translate_top(e, &mut vars)?;
    let lines = run(&build_script(&vars, &term, Domain::Integers, None))?;
    match lines.first().map(String::as_str) {
        Some("sat") => Ok(Value::Bool(true)),
        Some("unsat") => Ok(Value::Bool(false)),
        _ => err(
            "SatisfiableQ: the solver returned `unknown` (the constraint may be nonlinear or use an unsupported theory) — try SMT[..] directly",
        ),
    }
}

fn find_instance(e: &Expr, wanted: &[String], domain: Domain) -> EResult<Value> {
    let mut vars: BTreeSet<String> = wanted.iter().cloned().collect();
    let term = translate_top(e, &mut vars)?;

    // First decide satisfiability — `(get-value …)` is only legal after a *sat*
    // check-sat, so we can't ask for a model until we know there is one.
    let decision = run(&build_script(&vars, &term, domain, None))?;
    match decision.first().map(String::as_str) {
        // Mathematica returns {} when there is no instance.
        Some("unsat") => Ok(Value::Text("{}".to_string())),
        Some("sat") => {
            let lines = run(&build_script(&vars, &term, domain, Some(wanted)))?;
            let model = lines.get(1).map(String::as_str).unwrap_or("");
            Ok(Value::Text(parse_model(model)?))
        }
        _ => err(
            "FindInstance: the solver returned `unknown` (the constraint may be nonlinear or use an unsupported theory)",
        ),
    }
}

fn build_script(vars: &BTreeSet<String>, term: &str, domain: Domain, get: Option<&[String]>) -> String {
    let mut s = format!("(set-logic {})\n", domain.logic());
    for v in vars {
        s.push_str(&format!("(declare-const {} {})\n", v, domain.sort()));
    }
    s.push_str(&format!("(assert {term})\n(check-sat)\n"));
    if let Some(gs) = get {
        if !gs.is_empty() {
            s.push_str(&format!("(get-value ({}))\n", gs.join(" ")));
        }
    }
    s
}

fn run(script: &str) -> EResult<Vec<String>> {
    z3rs::cmd_context::run_smt2(script).map_err(|e| EvalError(format!("solver error: {e}")))
}

// --- translation: Mathesis AST → SMT-LIB term -------------------------------

/// A `{c1, c2, …}` list of constraints is their conjunction; anything else is a
/// single constraint.
fn translate_top(e: &Expr, vars: &mut BTreeSet<String>) -> EResult<String> {
    match e {
        Expr::List(items) => {
            if items.is_empty() {
                return err("no constraints were given");
            }
            let parts = items
                .iter()
                .map(|c| translate(c, vars))
                .collect::<EResult<Vec<_>>>()?;
            if parts.len() == 1 {
                Ok(parts.into_iter().next().unwrap())
            } else {
                Ok(format!("(and {})", parts.join(" ")))
            }
        }
        _ => translate(e, vars),
    }
}

fn translate(e: &Expr, vars: &mut BTreeSet<String>) -> EResult<String> {
    match e {
        Expr::Int(s) => Ok(s.clone()),
        Expr::Decimal { int, frac } => {
            let digits = format!("{int}{frac}");
            let trimmed = digits.trim_start_matches('0');
            let num = if trimmed.is_empty() { "0" } else { trimmed };
            if frac.is_empty() {
                Ok(num.to_string())
            } else {
                Ok(format!("(/ {num} 1{})", "0".repeat(frac.len())))
            }
        }
        Expr::Symbol(s) => {
            if matches!(s.as_str(), "Pi" | "E" | "I") {
                return err(format!(
                    "`{s}` cannot appear in an SMT constraint — only variables and rational numbers"
                ));
            }
            vars.insert(s.clone());
            Ok(s.clone())
        }
        Expr::Neg(x) => Ok(format!("(- {})", translate(x, vars)?)),
        Expr::Bin(op, a, b) => {
            let sa = translate(a, vars)?;
            if let Op::Pow = op {
                return pow_expand(&sa, b);
            }
            let sb = translate(b, vars)?;
            Ok(match op {
                Op::Add => format!("(+ {sa} {sb})"),
                Op::Sub => format!("(- {sa} {sb})"),
                Op::Mul => format!("(* {sa} {sb})"),
                Op::Div => format!("(/ {sa} {sb})"),
                Op::Eq => format!("(= {sa} {sb})"),
                Op::Ne => format!("(not (= {sa} {sb}))"),
                Op::Lt => format!("(< {sa} {sb})"),
                Op::Le => format!("(<= {sa} {sb})"),
                Op::Gt => format!("(> {sa} {sb})"),
                Op::Ge => format!("(>= {sa} {sb})"),
                Op::And => format!("(and {sa} {sb})"),
                Op::Or => format!("(or {sa} {sb})"),
                Op::Pow => unreachable!(),
            })
        }
        Expr::Call(head, cargs) => translate_call(head, cargs, vars),
        Expr::List(_) => err("a nested list is not a valid constraint"),
        Expr::Factorial(_) | Expr::Last | Expr::Str(_) => {
            err("this expression is not supported inside an SMT constraint")
        }
    }
}

/// `base^k` for a constant non-negative integer `k`, expanded to a product. A
/// power of a variable is nonlinear and the solver may then report `unknown`.
fn pow_expand(base: &str, exp: &Expr) -> EResult<String> {
    let k = match exp {
        Expr::Int(s) => s
            .parse::<u32>()
            .map_err(|_| EvalError("exponent too large in an SMT constraint".into()))?,
        _ => return err("an SMT constraint allows only a constant non-negative integer exponent"),
    };
    match k {
        0 => Ok("1".to_string()),
        1 => Ok(base.to_string()),
        _ => {
            let factors = std::iter::repeat(base).take(k as usize).collect::<Vec<_>>();
            Ok(format!("(* {})", factors.join(" ")))
        }
    }
}

fn translate_call(head: &str, args: &[Expr], vars: &mut BTreeSet<String>) -> EResult<String> {
    let all = |vars: &mut BTreeSet<String>| {
        args.iter().map(|a| translate(a, vars)).collect::<EResult<Vec<_>>>()
    };
    match head {
        "And" => Ok(format!("(and {})", all(vars)?.join(" "))),
        "Or" => Ok(format!("(or {})", all(vars)?.join(" "))),
        "Xor" => Ok(format!("(xor {})", all(vars)?.join(" "))),
        "Not" => {
            if args.len() != 1 {
                return err("Not expects one argument");
            }
            Ok(format!("(not {})", translate(&args[0], vars)?))
        }
        "Implies" => {
            if args.len() != 2 {
                return err("Implies expects two arguments");
            }
            Ok(format!(
                "(=> {} {})",
                translate(&args[0], vars)?,
                translate(&args[1], vars)?
            ))
        }
        _ => err(format!("`{head}` is not supported inside an SMT constraint")),
    }
}

// --- model parsing: SMT-LIB get-value output → `{x -> 6, y -> 4}` -----------

enum Sexp {
    Atom(String),
    List(Vec<Sexp>),
}

fn parse_model(line: &str) -> EResult<String> {
    let toks = tokenize(line);
    let mut pos = 0;
    let sexp = parse_sexp(&toks, &mut pos)
        .ok_or_else(|| EvalError("could not parse the solver's model".into()))?;
    let pairs = match sexp {
        Sexp::List(p) => p,
        Sexp::Atom(_) => return err("unexpected model shape from the solver"),
    };
    let mut out = Vec::new();
    for pair in &pairs {
        if let Sexp::List(kv) = pair {
            if let [Sexp::Atom(name), value] = &kv[..] {
                out.push(format!("{name} -> {}", render_value(value)));
            }
        }
    }
    Ok(format!("{{{}}}", out.join(", ")))
}

fn tokenize(s: &str) -> Vec<String> {
    let mut toks = Vec::new();
    let mut cur = String::new();
    for c in s.chars() {
        match c {
            '(' | ')' => {
                if !cur.is_empty() {
                    toks.push(std::mem::take(&mut cur));
                }
                toks.push(c.to_string());
            }
            c if c.is_whitespace() => {
                if !cur.is_empty() {
                    toks.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        toks.push(cur);
    }
    toks
}

fn parse_sexp(toks: &[String], pos: &mut usize) -> Option<Sexp> {
    let t = toks.get(*pos)?.clone();
    *pos += 1;
    match t.as_str() {
        "(" => {
            let mut items = Vec::new();
            while toks.get(*pos).map(String::as_str) != Some(")") {
                items.push(parse_sexp(toks, pos)?);
            }
            *pos += 1; // consume ")"
            Some(Sexp::List(items))
        }
        ")" => None,
        _ => Some(Sexp::Atom(t)),
    }
}

fn render_value(s: &Sexp) -> String {
    match s {
        Sexp::Atom(a) => a.clone(),
        Sexp::List(items) => match &items[..] {
            // (- x) → negation; (/ a b) → fraction.
            [Sexp::Atom(op), x] if op == "-" => format!("-{}", render_value(x)),
            [Sexp::Atom(op), a, b] if op == "/" => {
                format!("{}/{}", render_value(a), render_value(b))
            }
            _ => {
                let parts: Vec<String> = items.iter().map(render_value).collect();
                format!("({})", parts.join(" "))
            }
        },
    }
}
