//! # Mathesis
//!
//! A Mathematica-style computational notebook for the browser. This crate is the
//! **frontend**: it owns the surface language (lexer → parser → evaluator) and
//! the rendering of results, and delegates every actual computation to
//! dependency-free pure-Rust engines — today [`puremp`] for exact
//! arbitrary-precision arithmetic, with more (e.g. `z3rs` for logic/SMT) to slot
//! in as the language grows.
//!
//! The single wasm entry point is [`evaluate`]: hand it a line of input, get
//! back a small JSON object the Vue frontend renders (plain text + TeX, or an
//! error message).

mod ast;
mod error;
mod eval;
mod lexer;
mod parser;
mod elliptic;
mod plot;
mod random;
mod solve;
mod value;

use wasm_bindgen::prelude::*;

use error::{EResult, EvalError};
use value::Value;

/// Evaluate one line of Mathesis input.
///
/// Returns a JSON string, always one of:
/// - `{"ok":true,"text":"…","tex":"…"}`
/// - `{"ok":false,"error":"…"}`
#[wasm_bindgen]
pub fn evaluate(input: &str) -> String {
    match run(input) {
        Ok(v) => {
            if let Some(g) = v.graphics() {
                // A Plot/Plot3D payload — the frontend draws it. `g` is raw JSON.
                format!("{{\"ok\":true,\"graphics\":{g}}}")
            } else if let Some(t) = v.solutions() {
                // A Solve result — the frontend renders a table; `text` is a fallback.
                let vars = t.vars.iter().map(|s| json_string(s)).collect::<Vec<_>>().join(",");
                let rows = t
                    .rows
                    .iter()
                    .map(|r| format!("[{}]", r.iter().map(|c| json_string(c)).collect::<Vec<_>>().join(",")))
                    .collect::<Vec<_>>()
                    .join(",");
                format!(
                    "{{\"ok\":true,\"text\":{},\"solutions\":{{\"vars\":[{vars}],\"rows\":[{rows}],\"count\":{},\"truncated\":{}}}}}",
                    json_string(&v.to_text()),
                    t.rows.len(),
                    t.truncated
                )
            } else if let Some(text) = v.plain_text() {
                // Opaque text (string / SMT output) — rendered as monospace.
                format!("{{\"ok\":true,\"text\":{},\"plain\":true}}", json_string(text))
            } else {
                let approx = match v.approx() {
                    Some(a) => format!(",\"approx\":{}", json_string(&a)),
                    None => String::new(),
                };
                format!(
                    "{{\"ok\":true,\"text\":{},\"tex\":{}{}}}",
                    json_string(&v.to_text()),
                    json_string(&v.to_tex()),
                    approx,
                )
            }
        }
        Err(e) => format!("{{\"ok\":false,\"error\":{}}}", json_string(&e.0)),
    }
}

/// The crate version, surfaced in the UI.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

fn run(input: &str) -> EResult<Value> {
    if input.trim().is_empty() {
        return Err(EvalError("empty input".into()));
    }
    let expr = parser::parse(input).map_err(EvalError)?;
    let v = eval::eval(&expr)?;
    eval::set_last(v.clone());
    Ok(v)
}

/// Encode a string as a JSON string literal (quotes included). Hand-rolled to
/// avoid pulling `serde` into the wasm bundle for such a small need.
fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn out(input: &str) -> String {
        // Reset last-result between assertions is unnecessary for these cases.
        evaluate(input)
    }

    #[test]
    fn exact_integer_power() {
        assert!(out("2^128").contains("340282366920938463463374607431768211456"));
    }

    #[test]
    fn rationals_stay_exact() {
        let r = out("1/3 + 1/3 + 1/3");
        assert!(r.contains("\"text\":\"1\""), "{r}");
    }

    #[test]
    fn fraction_renders() {
        let r = out("1/2 + 1/3");
        assert!(r.contains("5/6"), "{r}");
        assert!(r.contains("\\\\frac{5}{6}") || r.contains("\\frac{5}{6}"), "{r}");
    }

    #[test]
    fn factorial_postfix() {
        assert!(out("10!").contains("3628800"));
    }

    #[test]
    fn builtins() {
        assert!(out("GCD[462, 1071]").contains("21"));
        assert!(out("Fibonacci[10]").contains("55"));
        assert!(out("PrimeQ[97]").contains("True"));
        assert!(out("Sqrt[144]").contains("12"));
        assert!(out("Factor[360]").contains("2^3"));
    }

    #[test]
    fn exact_preferred_with_approx() {
        // Constants stay symbolic (π) and carry a decimal approximation.
        let pi = out("Pi");
        assert!(pi.contains("\\\\pi"), "{pi}");
        assert!(pi.contains("\"approx\""), "{pi}");
        assert!(pi.contains("3.141592653589793"), "{pi}");

        // Sqrt of a non-square stays exact & symbolic: √2 with its decimal.
        let s2 = out("Sqrt[2]");
        assert!(s2.contains("\\\\sqrt{2}"), "{s2}");
        assert!(s2.contains("1.4142135623730951"), "{s2}");

        // Rationals: exact fraction primary + decimal approximation.
        let third = out("1/3");
        assert!(third.contains("\\\\frac{1}{3}"), "{third}");
        assert!(third.contains("0.3333333333333333"), "{third}");

        // N[..] gives as many digits as asked.
        assert!(out("N[Pi, 20]").contains("3.14159265358979"), "{}", out("N[Pi, 20]"));

        // Mixed/irrational arithmetic degrades to a real (no exact form).
        assert!(out("2 * Pi").contains("6.283185307179586"), "{}", out("2 * Pi"));
        assert!(out("Sin[0]").contains("\"text\":\"0\""), "{}", out("Sin[0]"));

        // Exact stays exact.
        assert!(out("Sqrt[16]").contains("\"text\":\"4\""), "{}", out("Sqrt[16]"));
        assert!(out("1/3 + 1/3 + 1/3").contains("\"text\":\"1\""));
    }

    #[test]
    fn lattice_reduce() {
        // Reduces an integer basis; result is a list of vectors.
        let r = out("LatticeReduce[{{1, 1, 1}, {-1, 0, 2}, {3, 5, 6}}]");
        assert!(r.contains("\"ok\":true"), "{r}");
        assert!(r.contains("{"), "{r}");
        // A custom reduction parameter is accepted.
        assert!(out("LatticeReduce[{{12, 2}, {13, 4}}, 99/100]").contains("\"ok\":true"));
        // Shape/type/range errors are reported clearly.
        assert!(out("LatticeReduce[{{1, 2}, {3}}]").contains("same length"));
        assert!(out("LatticeReduce[{1, 2, 3}]").contains("\"ok\":false"));
        assert!(out("LatticeReduce[{{1, 0}, {0, 1}}, 2]").contains("(1/4, 1]"));
    }

    #[test]
    fn matrix_rendering() {
        // A rectangular list-of-lists renders as a bracketed matrix…
        assert!(out("{{1, 2}, {3, 4}}").contains("bmatrix"), "{}", out("{{1, 2}, {3, 4}}"));
        assert!(
            out("LatticeReduce[{{1, 1, 1}, {-1, 0, 2}, {3, 5, 6}}]").contains("bmatrix"),
            "lattice result should render as a matrix"
        );
        // …but a plain vector stays a list, and a ragged one is not a matrix.
        let v = out("{1, 2, 3}");
        assert!(!v.contains("bmatrix") && v.contains("left"), "{v}");
        assert!(!out("{{1, 2}, {3}}").contains("bmatrix"));
    }

    #[test]
    fn number_theory() {
        assert!(out("PowerMod[2, 10, 1000]").contains("\"text\":\"24\""));
        assert!(out("PowerMod[3, -1, 7]").contains("\"text\":\"5\""));
        assert!(out("ModularInverse[3, 7]").contains("\"text\":\"5\""));
        assert!(out("ChineseRemainder[{2, 3}, {3, 5}]").contains("\"text\":\"8\""));
        assert!(out("Mod[-3, 5]").contains("\"text\":\"2\""));
        assert!(out("Quotient[17, 5]").contains("\"text\":\"3\""));
        assert!(out("NextPrime[10]").contains("\"text\":\"11\""));
        assert!(out("LucasL[10]").contains("\"text\":\"123\""));
        assert!(out("EvenQ[4]").contains("True"));
        assert!(out("Sign[-5]").contains("\"text\":\"-1\""));
        assert!(out("ExtendedGCD[12, 18]").contains("6"), "{}", out("ExtendedGCD[12, 18]"));
    }

    #[test]
    fn rounding_and_continued_fractions() {
        assert!(out("Floor[7/2]").contains("\"text\":\"3\""));
        assert!(out("Ceiling[7/2]").contains("\"text\":\"4\""));
        assert!(out("Round[7/2]").contains("\"text\":\"4\""));
        assert!(out("Floor[Pi]").contains("\"text\":\"3\""));
        assert!(out("IntegerPart[7/2]").contains("\"text\":\"3\""));
        assert!(out("FractionalPart[7/2]").contains("1/2"));
        assert!(out("ContinuedFraction[7/3]").contains("{2, 3}"), "{}", out("ContinuedFraction[7/3]"));
        assert!(out("FromContinuedFraction[{2, 3}]").contains("7/3"));
    }

    #[test]
    fn number_theory_extras() {
        assert!(out("EulerPhi[10]").contains("\"text\":\"4\""));
        assert!(out("Divisors[12]").contains("{1, 2, 3, 4, 6, 12}"), "{}", out("Divisors[12]"));
        assert!(out("DivisorSigma[1, 12]").contains("\"text\":\"28\""));
        assert!(out("DivisorSigma[0, 12]").contains("\"text\":\"6\""));
        assert!(out("MoebiusMu[30]").contains("\"text\":\"-1\""));
        assert!(out("MoebiusMu[4]").contains("\"text\":\"0\""));
        assert!(out("Radical[12]").contains("\"text\":\"6\""));
        assert!(out("NextPrime[10]").contains("\"text\":\"11\""));
        assert!(out("PreviousPrime[10]").contains("\"text\":\"7\""));
        assert!(out("EulerGamma").contains("0.5772156649"), "{}", out("EulerGamma"));
        assert!(out("Catalan").contains("0.915965594"), "{}", out("Catalan"));
    }

    #[test]
    fn matrices() {
        assert!(out("Det[{{1, 2}, {3, 4}}]").contains("\"text\":\"-2\""));
        assert!(out("Transpose[{{1, 2}, {3, 4}}]").contains("{1, 3}"));
        assert!(out("MatrixRank[{{1, 2}, {2, 4}}]").contains("\"text\":\"1\""));
        assert!(out("Inverse[{{1, 2}, {3, 4}}]").contains("-2"), "{}", out("Inverse[{{1, 2}, {3, 4}}]"));
        assert!(
            out("LinearSolve[{{1, 1}, {1, -1}}, {3, 1}]").contains("{2, 1}"),
            "{}",
            out("LinearSolve[{{1, 1}, {1, -1}}, {3, 1}]")
        );
        assert!(out("IdentityMatrix[2]").contains("{{1, 0}, {0, 1}}"));
    }

    #[test]
    fn more_transcendentals() {
        assert!(out("ArcSin[1]").contains("1.5707963267948966"), "{}", out("ArcSin[1]"));
        assert!(out("Log10[1000]").contains("\"text\":\"3\"") || out("Log10[1000]").contains("2.9999"), "{}", out("Log10[1000]"));
        assert!(out("Cosh[0]").contains("\"text\":\"1\""), "{}", out("Cosh[0]"));
    }

    #[test]
    fn complex_numbers() {
        assert!(out("I").contains("\"text\":\"I\""), "{}", out("I"));
        assert!(out("I^2").contains("\"text\":\"-1\""));
        assert!(out("(1 + I)^2").contains("2 I"), "{}", out("(1 + I)^2"));
        assert!(out("(3 + 4*I)*(3 - 4*I)").contains("\"text\":\"25\""));
        assert!(out("Re[3 + 4*I]").contains("\"text\":\"3\""));
        assert!(out("Im[3 + 4*I]").contains("\"text\":\"4\""));
        assert!(out("Conjugate[3 + 4*I]").contains("3 - 4 I"), "{}", out("Conjugate[3 + 4*I]"));
        assert!(out("Abs[3 + 4*I]").contains("\"text\":\"5\""));
        assert!(out("Sqrt[-4]").contains("2 I"), "{}", out("Sqrt[-4]"));
        assert!(out("Sqrt[-1]").contains("\"text\":\"I\""));
        assert!(out("1/(1 + I)").contains("1/2 - 1/2 I"), "{}", out("1/(1 + I)"));
    }

    #[test]
    fn smt_solver() {
        // A satisfiable linear-integer problem, with a model.
        let sat = out(r#"SMT["(declare-const x Int)(assert (> x 5))(assert (< x 7))(check-sat)(get-value (x))"]"#);
        assert!(sat.contains("sat"), "{sat}");
        assert!(sat.contains("\"plain\":true"), "{sat}");

        // Unsatisfiable.
        let unsat = out(r#"SMT["(declare-const x Int)(assert (> x 5))(assert (< x 5))(check-sat)"]"#);
        assert!(unsat.contains("unsat"), "{unsat}");

        // Solver/parse errors surface as errors.
        assert!(out(r#"SMT["(check-sat"]"#).contains("\"ok\":false"));
        // Non-string argument is rejected.
        assert!(out("SMT[5]").contains("expects a string"));
        // A bare string literal renders as plain text.
        assert!(out(r#""hello""#).contains("\"plain\":true"));
    }

    #[test]
    fn relational_and_logical() {
        assert!(out("2 < 3").contains("\"text\":\"True\""));
        assert!(out("2 == 2").contains("\"text\":\"True\""));
        assert!(out("5 >= 6").contains("\"text\":\"False\""));
        assert!(out("1/2 < 2/3").contains("\"text\":\"True\""));
        assert!(out("True && False").contains("\"text\":\"False\""));
        assert!(out("True || False").contains("\"text\":\"True\""));
    }

    #[test]
    fn solving() {
        assert!(out("SatisfiableQ[x > 5 && x < 8]").contains("\"text\":\"True\""));
        assert!(out("SatisfiableQ[x > 5 && x < 3]").contains("\"text\":\"False\""));

        let fi = out("FindInstance[x + y == 10 && x - y == 2, {x, y}]");
        assert!(fi.contains("x -> 6") && fi.contains("y -> 4"), "{fi}");

        assert!(out("Solve[2*x == 6, x]").contains("x -> 3"), "{}", out("Solve[2*x == 6, x]"));
        // Unsatisfiable → no instance.
        assert!(out("FindInstance[x > 5 && x < 5, {x}]").contains("{}"), "{}", out("FindInstance[x > 5 && x < 5, {x}]"));
        // Reals domain works too.
        assert!(out("FindInstance[2*x == 3, {x}, Reals]").contains("x ->"), "{}", out("FindInstance[2*x == 3, {x}, Reals]"));
    }

    #[test]
    fn inexact_complex() {
        // Irrational/transcendental times I → an inexact complex.
        let pii = out("Pi*I");
        assert!(pii.contains("3.141592653589793") && pii.contains("I"), "{pii}");
        assert!(out("Sqrt[-2]").contains("1.4142135623730951"), "{}", out("Sqrt[-2]"));
        assert!(out("Im[Pi*I]").contains("3.141592653589793"), "{}", out("Im[Pi*I]"));
        assert!(out("Re[Pi*I]").contains("\"text\":\"0\""), "{}", out("Re[Pi*I]"));
        // |√2 i| = √2, a real.
        assert!(out("Abs[Sqrt[-2]]").contains("1.41421"), "{}", out("Abs[Sqrt[-2]]"));
        // Exact complex is still exact.
        assert!(out("I^2").contains("\"text\":\"-1\""));
        assert!(out("Sqrt[-4]").contains("2 I"), "{}", out("Sqrt[-4]"));
    }

    #[test]
    fn special_functions_and_dlog() {
        assert!(out("Erf[0]").contains("\"text\":\"0\""));
        assert!(out("Erf[1]").contains("0.8427007"), "{}", out("Erf[1]"));
        assert!(out("Erfc[0]").contains("\"text\":\"1\""));
        assert!(out("Zeta[2]").contains("1.6449340668"), "{}", out("Zeta[2]"));
        // 3^4 = 81 ≡ 4 (mod 7).
        assert!(out("DiscreteLog[3, 4, 7]").contains("\"text\":\"4\""), "{}", out("DiscreteLog[3, 4, 7]"));
        // 3 is not in the subgroup generated by 2 mod 7.
        assert!(out("DiscreteLog[2, 3, 7]").contains("\"ok\":false"), "{}", out("DiscreteLog[2, 3, 7]"));
    }

    #[test]
    fn large_reals_use_scientific() {
        // Sqrt of a huge non-square falls back to a real in scientific notation
        // (no unreadable √ over a 158-digit radicand, no wall of zeros).
        let r = out("Sqrt[100!]");
        assert!(r.contains("9.66054943799493e78") && !r.contains("sqrt"), "{r}");
        // Small radicands stay exact and symbolic.
        let s2 = out("Sqrt[2]");
        assert!(s2.contains("\\\\sqrt{2}") && s2.contains("1.4142135623730951"), "{s2}");
    }

    #[test]
    fn plotting() {
        let p = out("Plot[Sin[x], {x, 0, 2*Pi}]");
        assert!(p.contains("\"graphics\"") && p.contains("plot2d"), "{p}");
        assert!(out("Plot3D[x + y, {x, 0, 1}, {y, 0, 1}]").contains("plot3d"));
        // The plot variable is only bound inside the plot.
        assert!(out("x + 1").contains("\"ok\":false"));
        // An expression that never yields a real over the range errors.
        assert!(out("Plot[q, {x, 0, 1}]").contains("\"ok\":false"));
    }

    #[test]
    fn optimization_and_typeset_solutions() {
        // FindInstance renders as typeset rules (tex uses \to and \frac).
        let fi = out("FindInstance[2*x == 3, {x}, Reals]");
        assert!(fi.contains("\\\\to") && fi.contains("\\\\frac{3}{2}"), "{fi}");
        // Linear optimization returns {optimum, {assignment}}.
        let mx = out("Maximize[x + y, x + 2*y <= 14 && 3*x - y >= 0 && x - y <= 2, {x, y}, Reals]");
        assert!(mx.contains("\"text\":\"{10, {x -> 6, y -> 4}}\""), "{mx}");
        let mn = out("Minimize[y, y >= x && x >= 3, {x, y}]");
        assert!(mn.contains("{3, {x -> 3, y -> 3}}"), "{mn}");
        // Unbounded / infeasible objectives error clearly.
        assert!(out("Maximize[x, x >= 0, {x}]").contains("unbounded"), "{}", out("Maximize[x, x >= 0, {x}]"));
        assert!(out("Maximize[x, x >= 5 && x <= 1, {x}]").contains("\"ok\":false"));
    }

    #[test]
    fn solve_all_solutions() {
        // Every integer solution, sorted.
        assert!(
            out("Solve[x > 0 && x < 5, x]").contains("{{x -> 1}, {x -> 2}, {x -> 3}, {x -> 4}}"),
            "{}",
            out("Solve[x > 0 && x < 5, x]")
        );
        // A two-variable system → all points.
        let s = out("Solve[x + y == 4 && x >= 0 && y >= 0, {x, y}]");
        assert!(s.contains("{x -> 0, y -> 4}") && s.contains("{x -> 4, y -> 0}"), "{s}");
        // Boolean enumeration.
        assert!(out("Solve[p && q, {p, q}]").contains("{p -> True, q -> True}"));
        // No solution → {}.
        assert!(out("Solve[x == 1 && x == 2, x]").contains("\"text\":\"{}\""));
        // Reals fall back to a single instance.
        assert!(out("Solve[2*x == 3, x, Reals]").contains("x -> 3/2"));
    }

    #[test]
    fn tautology() {
        // Propositional validity, incl. modus ponens.
        assert!(out("TautologyQ[Implies[p, p || q]]").contains("\"text\":\"True\""));
        assert!(out("TautologyQ[Implies[p && Implies[p, q], q]]").contains("\"text\":\"True\""));
        assert!(out("TautologyQ[p || q]").contains("\"text\":\"False\""));
        // Arithmetic validity over the reals.
        assert!(out("TautologyQ[x + 1 > x, Reals]").contains("\"text\":\"True\""));
        assert!(out("TautologyQ[x > 0, Reals]").contains("\"text\":\"False\""));
    }

    #[test]
    fn boolean_logic() {
        // Propositional variables are inferred as Bool.
        assert!(out("SatisfiableQ[p || q]").contains("\"text\":\"True\""));
        assert!(out("SatisfiableQ[p && Not[p]]").contains("\"text\":\"False\""));
        assert!(out("SatisfiableQ[Implies[p, q] && p && Not[q]]").contains("\"text\":\"False\""));
        // A boolean model is returned as typeset rules.
        assert!(out("FindInstance[p || q, {p, q}]").contains("p -> True"));
        // Mixed numeric/boolean.
        assert!(out("SatisfiableQ[(x > 0) && p && Not[p]]").contains("\"text\":\"False\""));
        // Nonlinear that z3rs can refute → unsatisfiable.
        assert!(out("SatisfiableQ[x^2 == -1]").contains("\"text\":\"False\""));
    }

    #[test]
    fn comments() {
        // `(* … *)` comments are skipped anywhere whitespace is allowed.
        assert!(out("1 + 2 (* inline *) + 3").contains("\"text\":\"6\""));
        assert!(out("(* leading *) 40 + 2").contains("\"text\":\"42\""));
        assert!(out("40 + 2 (* trailing *)").contains("\"text\":\"42\""));
        // Comments nest.
        assert!(out("(* a (* b *) c *) 5 * 5").contains("\"text\":\"25\""));
        // Unterminated is a clear error; a comment-only line is blank.
        assert!(out("1 + (* oops").contains("unterminated comment"));
        assert!(out("(* just a note *)").contains("empty input"));
    }

    #[test]
    fn session_variables() {
        // Assignment returns the value and persists to later evaluations.
        assert!(out("myx = 42").contains("\"text\":\"42\""));
        assert!(out("myx + 8").contains("\"text\":\"50\""));
        // Reassignment.
        out("myx = 100");
        assert!(out("myx").contains("\"text\":\"100\""));
        // Chained assignment is right-associative.
        out("mya = myb = 7");
        assert!(out("mya + myb").contains("\"text\":\"14\""));
        // An expression on the right-hand side is evaluated.
        out("myw = 3^2");
        assert!(out("myw").contains("\"text\":\"9\""));
        // `==` stays equality; `=` is assignment.
        assert!(out("myx == 100").contains("\"text\":\"True\""));
        // Built-in constants are protected.
        assert!(out("Pi = 3").contains("\"ok\":false"));
    }

    #[test]
    fn elliptic_curves() {
        // y² = x³ + 2x + 2 over GF(17); (5,1) generates the whole group (order 19).
        assert!(out("ECPointQ[2, 2, 17, {5, 1}]").contains("\"text\":\"True\""));
        assert!(out("ECPointQ[2, 2, 17, {5, 2}]").contains("\"text\":\"False\""));
        // Doubling via ECAdd matches scalar multiplication.
        assert!(out("ECAdd[2, 2, 17, {5, 1}, {5, 1}]").contains("{6, 3}"));
        assert!(out("ECMultiply[2, 2, 17, 2, {5, 1}]").contains("{6, 3}"));
        // Group order, point order, and identity.
        assert!(out("ECOrder[2, 2, 17]").contains("\"text\":\"19\""));
        assert!(out("ECPointOrder[2, 2, 17, {5, 1}]").contains("\"text\":\"19\""));
        assert!(out("ECMultiply[2, 2, 17, 19, {5, 1}]").contains("\"text\":\"{}\"")); // = O
        assert!(out("ECAdd[2, 2, 17, {5, 1}, {}]").contains("{5, 1}")); // P + O = P
        // Invariants and validation.
        assert!(out("ECDiscriminant[2, 2]").contains("-2240"));
        assert!(out("ECjInvariant[2, 2]").contains("13824/35"));
        assert!(out("ECOrder[2, 3, 4]").contains("\"ok\":false")); // modulus not prime
        assert!(out("ECOrder[0, 0, 17]").contains("\"ok\":false")); // singular
    }

    #[test]
    fn random_numbers() {
        // Bounds hold across many draws (values are CSPRNG-sourced, so we can't
        // assert exact outputs — only their ranges/shapes).
        for _ in 0..100 {
            let n: i64 = out("RandomInteger[{3, 7}]")
                .split("\"text\":\"")
                .nth(1)
                .and_then(|s| s.split('"').next())
                .and_then(|s| s.parse().ok())
                .unwrap();
            assert!((3..=7).contains(&n), "RandomInteger out of range: {n}");
        }
        // A coin flip is 0 or 1.
        for _ in 0..50 {
            let b = out("RandomInteger[]");
            assert!(b.contains("\"text\":\"0\"") || b.contains("\"text\":\"1\""), "{b}");
        }
        // Counts produce lists; RandomPrime yields a prime; RandomBytes is hex.
        let list = out("RandomInteger[9, 5]");
        let text = list.split("\"text\":\"").nth(1).and_then(|s| s.split('"').next()).unwrap();
        assert!(text.starts_with('{') && text.matches(',').count() == 4, "{text}");
        assert!(out("PrimeQ[RandomPrime[500]]").contains("\"text\":\"True\""));
        assert!(out("RandomBytes[8]").contains("\"plain\":true"));
        // Empty ranges are rejected.
        assert!(out("RandomInteger[{9, 2}]").contains("\"ok\":false"));
    }

    #[test]
    fn gamma_bessel_identify() {
        assert!(out("Gamma[5]").contains("\"text\":\"24\""));
        assert!(out("N[Gamma[1/2], 15]").contains("1.7724538509055"), "{}", out("N[Gamma[1/2], 15]"));
        assert!(out("N[BesselJ[0, 1], 12]").contains("0.7651976865"), "{}", out("N[BesselJ[0, 1], 12]"));
        // The inverse symbolic calculator recognises closed forms.
        assert!(out("Identify[Pi^2/6]").contains("π"), "{}", out("Identify[Pi^2/6]"));
        assert!(out("Identify[Zeta[3]]").contains("ζ(3)"), "{}", out("Identify[Zeta[3]]"));
        // 0.2.3 additions: Beta, PolyGamma (digamma / nth), second-kind Bessel.
        assert!(out("N[Beta[2, 3], 12]").contains("0.0833333333"), "{}", out("N[Beta[2, 3], 12]"));
        assert!(out("N[PolyGamma[1], 10]").contains("-0.5772156649")); // digamma(1) = -γ
        assert!(out("N[PolyGamma[1, 1], 10]").contains("1.644934066")); // trigamma(1) = π²/6
        assert!(out("N[BesselY[0, 1], 10]").contains("0.0882569642"));
    }

    #[test]
    fn errors_are_reported() {
        assert!(out("1/0").contains("\"ok\":false"));
        assert!(out("Foo[1]").contains("unknown function"));
    }
}
