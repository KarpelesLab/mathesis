//! A Wolfram-style front end over the z3rs SMT solver.
//!
//! `SatisfiableQ[c]`, `FindInstance[c, vars]`, and `Solve[c, vars]` take a
//! constraint written in Mathesis syntax (using `== != < <= > >= && ||` and the
//! `And`/`Or`/`Not`/`Implies`/`Xor` heads), translate it to SMT-LIB 2, run it
//! through z3rs, and report the result. Only *linear* arithmetic over integers
//! or reals is decidable (z3rs decides QF_LIA / QF_LRA); a nonlinear constraint
//! (a product or power of two unknowns) may come back `unknown`.

use std::collections::{BTreeMap, BTreeSet};

use puremp::Int;

use crate::ast::{Expr, Op};
use crate::error::{EResult, EvalError, err};
use crate::value::{self, Value};

pub fn solve_form(head: &str, args: &[Expr]) -> EResult<Value> {
    match head {
        "SatisfiableQ" => {
            if args.len() != 1 {
                return err("SatisfiableQ expects one argument: a constraint");
            }
            satisfiable(&args[0])
        }
        "TautologyQ" => {
            if !(1..=2).contains(&args.len()) {
                return err("TautologyQ expects a formula and an optional domain");
            }
            let domain = if args.len() == 2 {
                domain_of(&args[1])?
            } else {
                Domain::Integers
            };
            tautology(&args[0], domain)
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
            if head == "Solve" {
                solve_all(&args[0], &wanted, domain)
            } else {
                find_instance(&args[0], &wanted, domain)
            }
        }
        "Maximize" | "Minimize" => {
            if !(3..=4).contains(&args.len()) {
                return err(format!(
                    "{head} expects Objective, Constraints, {{variables}}, and an optional domain"
                ));
            }
            let wanted = var_list(&args[2])?;
            let domain = if args.len() == 4 {
                domain_of(&args[3])?
            } else {
                Domain::Reals
            };
            optimize(head == "Maximize", &args[0], &args[1], &wanted, domain)
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
    let sorts = sorts_of(e);
    let lines = run(&build_script(&vars, &sorts, &term, Domain::Integers, None))?;
    match lines.first().map(String::as_str) {
        Some("sat") => Ok(Value::Bool(true)),
        Some("unsat") => Ok(Value::Bool(false)),
        _ => err(
            "SatisfiableQ: the solver returned `unknown` (the constraint may be nonlinear or use an unsupported theory) — try SMT[..] directly",
        ),
    }
}

/// `TautologyQ[φ]` — is `φ` valid (true for every assignment)? It is iff its
/// negation is unsatisfiable.
fn tautology(e: &Expr, domain: Domain) -> EResult<Value> {
    let mut vars = BTreeSet::new();
    let term = translate_top(e, &mut vars)?;
    let sorts = sorts_of(e);
    let neg = format!("(not {term})");
    let lines = run(&build_script(&vars, &sorts, &neg, domain, None))?;
    match lines.first().map(String::as_str) {
        Some("unsat") => Ok(Value::Bool(true)), // no counterexample → valid
        Some("sat") => Ok(Value::Bool(false)),  // a counterexample exists
        _ => err(
            "TautologyQ: the solver returned `unknown` (the formula may be nonlinear or use an unsupported theory)",
        ),
    }
}

fn find_instance(e: &Expr, wanted: &[String], domain: Domain) -> EResult<Value> {
    let mut vars: BTreeSet<String> = wanted.iter().cloned().collect();
    let term = translate_top(e, &mut vars)?;
    let sorts = sorts_of(e);

    // First decide satisfiability — `(get-value …)` is only legal after a *sat*
    // check-sat, so we can't ask for a model until we know there is one.
    let decision = run(&build_script(&vars, &sorts, &term, domain, None))?;
    match decision.first().map(String::as_str) {
        // Mathematica returns {} when there is no instance.
        Some("unsat") => Ok(Value::Rules(Vec::new())),
        Some("sat") => {
            let lines = run(&build_script(&vars, &sorts, &term, domain, Some(wanted)))?;
            let model = lines.get(1).map(String::as_str).unwrap_or("");
            Ok(Value::Rules(parse_model(model)))
        }
        _ => err(
            "FindInstance: the solver returned `unknown` (the constraint may be nonlinear or use an unsupported theory)",
        ),
    }
}

/// Enumerate every solution over a discrete domain, by repeatedly finding a
/// model and blocking it. Capped, and only over the integers — a real region is
/// dense, so there `Solve` falls back to a single instance.
const MAX_SOLUTIONS: usize = 256;

fn solve_all(e: &Expr, wanted: &[String], domain: Domain) -> EResult<Value> {
    if matches!(domain, Domain::Reals) {
        let rows = match find_instance(e, wanted, domain)? {
            Value::Rules(r) if r.is_empty() => Vec::new(),
            Value::Rules(r) => vec![r],
            _ => Vec::new(),
        };
        return Ok(Value::Solutions { rows, truncated: false });
    }

    let mut vars: BTreeSet<String> = wanted.iter().cloned().collect();
    let term = translate_top(e, &mut vars)?;
    let sorts = sorts_of(e);

    let mut session = z3rs::cmd_context::Session::new();
    let mut setup = format!("(set-logic {})\n", domain.logic());
    for v in &vars {
        let sort = match sorts.get(v) {
            Some(Kind::Bool) => "Bool",
            _ => domain.sort(),
        };
        setup.push_str(&format!("(declare-const {v} {sort})\n"));
    }
    setup.push_str(&format!("(assert {term})\n"));
    let ev = |sess: &mut z3rs::cmd_context::Session, s: &str| {
        sess.eval(s).map_err(|e| EvalError(format!("solver error: {e}")))
    };
    ev(&mut session, &setup)?;

    let getv = format!("(get-value ({}))", wanted.join(" "));
    let mut solutions: Vec<(Vec<f64>, Vec<(String, Value)>)> = Vec::new();
    let mut truncated = false;
    loop {
        if solutions.len() >= MAX_SOLUTIONS {
            truncated = true;
            break;
        }
        match ev(&mut session, "(check-sat)")?.first().map(String::as_str) {
            Some("sat") => {}
            Some("unsat") => break,
            _ => {
                if solutions.is_empty() {
                    return err(
                        "Solve: the solver returned `unknown` (the constraint may be nonlinear or use an unsupported theory)",
                    );
                }
                break;
            }
        }
        let vline = ev(&mut session, &getv)?;
        let pairs = model_sexp_pairs(vline.first().map(String::as_str).unwrap_or(""));
        if pairs.is_empty() {
            break;
        }
        let clause = pairs
            .iter()
            .map(|(n, s)| format!("(= {n} {})", render_str(s)))
            .collect::<Vec<_>>()
            .join(" ");
        ev(&mut session, &format!("(assert (not (and {clause})))"))?;

        let rules: Vec<(String, Value)> =
            pairs.iter().map(|(n, s)| (n.clone(), sexp_to_value(s))).collect();
        solutions.push((sort_key(&rules), rules));
    }

    // Present solutions in a natural (sorted) order — the solver's is internal.
    solutions.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(core::cmp::Ordering::Equal));
    let rows: Vec<Vec<(String, Value)>> = solutions.into_iter().map(|(_, r)| r).collect();
    Ok(Value::Solutions { rows, truncated })
}

/// A numeric sort key for a solution (first variable primary, then the rest).
fn sort_key(rules: &[(String, Value)]) -> Vec<f64> {
    rules
        .iter()
        .map(|(_, v)| match v {
            Value::Bool(b) => {
                if *b {
                    1.0
                } else {
                    0.0
                }
            }
            _ => value::to_f64(v).unwrap_or(f64::INFINITY),
        })
        .collect()
}

/// `Maximize[obj, constraints, {vars}]` / `Minimize[…]` — optimize a linear
/// objective subject to constraints, returning `{optimum, {x -> …, …}}`.
fn optimize(
    maximize: bool,
    obj: &Expr,
    constraints: &Expr,
    wanted: &[String],
    domain: Domain,
) -> EResult<Value> {
    let head = if maximize { "Maximize" } else { "Minimize" };
    let mut vars: BTreeSet<String> = wanted.iter().cloned().collect();
    let cterm = translate_top(constraints, &mut vars)?;
    let oterm = translate(obj, &mut vars)?;

    let mut sorts = BTreeMap::new();
    infer(constraints, Kind::Bool, &mut sorts);
    infer(obj, Kind::Num, &mut sorts);

    let mut s = format!("(set-logic {})\n", domain.logic());
    for v in &vars {
        let sort = match sorts.get(v) {
            Some(Kind::Bool) => "Bool",
            _ => domain.sort(),
        };
        s.push_str(&format!("(declare-const {v} {sort})\n"));
    }
    s.push_str(&format!("(assert {cterm})\n"));
    s.push_str(&format!(
        "({} {oterm})\n(check-sat)\n(get-objectives)\n(get-value ({}))\n",
        if maximize { "maximize" } else { "minimize" },
        wanted.join(" ")
    ));

    let lines = run(&s)?;
    match lines.first().map(String::as_str) {
        Some("unsat") => err(format!("{head}: the constraints have no feasible solution")),
        Some("sat") => {
            let obj_line = lines.get(1).map(String::as_str).unwrap_or("");
            if obj_line.contains("oo") {
                return err(format!("{head}: the objective is unbounded"));
            }
            if obj_line.contains("epsilon") {
                return err(format!(
                    "{head}: the optimum is a strict bound and is not attained"
                ));
            }
            let opt = objective_value(obj_line);
            let model = lines.get(2).map(String::as_str).unwrap_or("");
            Ok(Value::List(vec![opt, Value::Rules(parse_model(model))]))
        }
        _ => err(format!(
            "{head}: the solver returned `unknown` (the problem may be nonlinear)"
        )),
    }
}

// --- sort inference: which variables are Bool vs numeric --------------------

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Bool,
    Num,
}

/// Record a variable's inferred kind; a numeric use always wins over a boolean
/// one (an ill-typed constraint then fails cleanly in the solver).
fn mark(sorts: &mut BTreeMap<String, Kind>, name: &str, k: Kind) {
    let new = match (sorts.get(name).copied(), k) {
        (Some(Kind::Num), _) | (_, Kind::Num) => Kind::Num,
        _ => Kind::Bool,
    };
    sorts.insert(name.to_string(), new);
}

/// Does this expression denote a boolean (a comparison, a logical connective, or
/// a boolean literal)? Used to type the operands of `==`/`!=`.
fn is_bool_expr(e: &Expr) -> bool {
    match e {
        Expr::Symbol(s) => s == "True" || s == "False",
        Expr::Bin(op, ..) => matches!(
            op,
            Op::Eq | Op::Ne | Op::Lt | Op::Le | Op::Gt | Op::Ge | Op::And | Op::Or
        ),
        Expr::Call(h, _) => matches!(h.as_str(), "And" | "Or" | "Not" | "Implies" | "Xor"),
        _ => false,
    }
}

/// Infer each variable's sort by walking the constraint with an expected kind.
fn infer(e: &Expr, exp: Kind, sorts: &mut BTreeMap<String, Kind>) {
    match e {
        Expr::Symbol(s) => {
            if !matches!(s.as_str(), "True" | "False" | "Pi" | "E" | "I" | "EulerGamma" | "Catalan") {
                mark(sorts, s, exp);
            }
        }
        Expr::Neg(x) => infer(x, Kind::Num, sorts),
        Expr::Bin(op, a, b) => match op {
            Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Pow | Op::Lt | Op::Le | Op::Gt | Op::Ge => {
                infer(a, Kind::Num, sorts);
                infer(b, Kind::Num, sorts);
            }
            Op::And | Op::Or => {
                infer(a, Kind::Bool, sorts);
                infer(b, Kind::Bool, sorts);
            }
            Op::Eq | Op::Ne => {
                let k = if is_bool_expr(a) || is_bool_expr(b) { Kind::Bool } else { Kind::Num };
                infer(a, k, sorts);
                infer(b, k, sorts);
            }
        },
        Expr::Call(head, args) => match head.as_str() {
            "And" | "Or" | "Xor" | "Not" | "Implies" => {
                for a in args {
                    infer(a, Kind::Bool, sorts);
                }
            }
            _ => {
                for a in args {
                    infer(a, Kind::Num, sorts);
                }
            }
        },
        Expr::List(items) => {
            for c in items {
                infer(c, exp, sorts);
            }
        }
        _ => {}
    }
}

fn sorts_of(e: &Expr) -> BTreeMap<String, Kind> {
    let mut sorts = BTreeMap::new();
    infer(e, Kind::Bool, &mut sorts);
    sorts
}

fn build_script(
    vars: &BTreeSet<String>,
    sorts: &BTreeMap<String, Kind>,
    term: &str,
    domain: Domain,
    get: Option<&[String]>,
) -> String {
    let mut s = format!("(set-logic {})\n", domain.logic());
    for v in vars {
        let sort = match sorts.get(v) {
            Some(Kind::Bool) => "Bool",
            _ => domain.sort(),
        };
        s.push_str(&format!("(declare-const {v} {sort})\n"));
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
        Expr::Symbol(s) => match s.as_str() {
            "True" => Ok("true".to_string()),
            "False" => Ok("false".to_string()),
            "Pi" | "E" | "I" | "EulerGamma" | "Catalan" => err(format!(
                "`{s}` cannot appear in an SMT constraint — only variables and rational numbers"
            )),
            _ => {
                vars.insert(s.clone());
                Ok(s.clone())
            }
        },
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

fn parse_str(line: &str) -> Option<Sexp> {
    let toks = tokenize(line);
    let mut pos = 0;
    parse_sexp(&toks, &mut pos)
}

/// Parse a `((x 6) (y (/ 1 2)))` model into `(name, value-s-expression)` pairs.
fn model_sexp_pairs(line: &str) -> Vec<(String, Sexp)> {
    let pairs = match parse_str(line) {
        Some(Sexp::List(p)) => p,
        _ => return Vec::new(),
    };
    let mut out = Vec::new();
    for pair in pairs {
        if let Sexp::List(mut kv) = pair {
            if kv.len() == 2 {
                let val = kv.pop().unwrap();
                if let Some(Sexp::Atom(name)) = kv.pop() {
                    out.push((name, val));
                }
            }
        }
    }
    out
}

/// Parse a model into exact `(name, Value)` pairs.
fn parse_model(line: &str) -> Vec<(String, Value)> {
    model_sexp_pairs(line)
        .iter()
        .map(|(n, s)| (n.clone(), sexp_to_value(s)))
        .collect()
}

/// The optimum from a `(objectives ((<objective> <value>)))` get-objectives line.
fn objective_value(line: &str) -> Value {
    if let Some(Sexp::List(items)) = parse_str(line) {
        // items[0] is the atom `objectives`; each following item is a pair.
        for item in items.iter().skip(1) {
            if let Sexp::List(kv) = item {
                if let Some(val) = kv.last() {
                    return sexp_to_value(val);
                }
            }
        }
    }
    Value::Text("?".to_string())
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

/// Convert an SMT value s-expression into an exact Mathesis value: integers,
/// `(/ a b)` fractions, `(- x)` negations, decimals, and booleans; anything else
/// falls back to its textual form.
fn sexp_to_value(s: &Sexp) -> Value {
    match s {
        Sexp::Atom(a) => atom_value(a),
        Sexp::List(items) => match &items[..] {
            [Sexp::Atom(op), x] if op == "-" => {
                value::neg(&sexp_to_value(x)).unwrap_or_else(|_| Value::Text(render_str(s)))
            }
            [Sexp::Atom(op), a, b] if op == "/" => {
                value::div(&sexp_to_value(a), &sexp_to_value(b))
                    .unwrap_or_else(|_| Value::Text(render_str(s)))
            }
            _ => Value::Text(render_str(s)),
        },
    }
}

fn atom_value(a: &str) -> Value {
    match a {
        "true" => return Value::Bool(true),
        "false" => return Value::Bool(false),
        _ => {}
    }
    if let Ok(n) = Int::from_str_radix(a, 10) {
        return Value::Int(n);
    }
    if let Some(v) = parse_decimal(a) {
        return v;
    }
    Value::Text(a.to_string())
}

/// A decimal literal like `1.0` or `-1.5` as an exact rational value.
fn parse_decimal(a: &str) -> Option<Value> {
    let (int, frac) = a.split_once('.')?;
    if frac.is_empty() || !frac.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let ip = int.strip_prefix('-').unwrap_or(int);
    if ip.is_empty() || !ip.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let num = Int::from_str_radix(&format!("{int}{frac}"), 10).ok()?;
    let den = Int::from(10).pow(frac.len() as u32);
    Some(value::from_rational(puremp::Rational::new(num, den)))
}

fn render_str(s: &Sexp) -> String {
    match s {
        Sexp::Atom(a) => a.clone(),
        Sexp::List(items) => {
            format!("({})", items.iter().map(render_str).collect::<Vec<_>>().join(" "))
        }
    }
}
