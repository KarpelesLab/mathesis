# Mathesis

**A Mathematica-style computational notebook that runs entirely in your browser.**

Mathesis is a free, client-side mathematics workbench: type an expression in
Wolfram-style syntax and get an *exact* answer, computed on your own machine with
no server round-trip. The math engine is written in Rust, compiled to
WebAssembly, and wrapped in a Vue 3 notebook UI.

> Live at **https://karpeleslab.github.io/mathesis/** (deployed from `master` via
> GitHub Actions).

## The name

*Mathesis* (μάθησις) derives from the ancient Greek word for "learning" or "that
which is learned". It typically refers to the rigorous, active pursuit of
knowledge — particularly mathematical and scientific discipline.

## Philosophy

Mathesis is a **frontend**. It owns the *language* — a lexer, parser, and
evaluator for a Wolfram-style surface syntax — and the *presentation* (KaTeX
rendering, the notebook). It deliberately owns as little *mathematics* as
possible: every real computation is delegated to dependency-free, pure-Rust
engines, and anything Mathesis implements itself is a temporary stand-in until an
adequate crate exists to hold it.

Engines:

- [`puremp`](https://github.com/KarpelesLab/puremp) — exact arbitrary-precision
  integers, rationals, and more. Powers today's numeric tower.
- [`z3rs`](https://github.com/KarpelesLab/z3rs) — a pure-Rust Z3 port
  (SMT / logic). To be wired in as the language grows to solving and constraints.

## What works today

Exact evaluation of a numeric core:

| You type | You get |
| --- | --- |
| `2^128` | `340282366920938463463374607431768211456` |
| `1/3 + 1/3 + 1/3` | `1` |
| `1/2 + 1/3` | `5/6` (rendered as a fraction) |
| `(1 + 1/2)^10` | `59049/1024` |
| `20!` | `2432902008176640000` |
| `Factor[360]` | `2³ · 3² · 5` |
| `GCD[462, 1071]` | `21` |
| `Fibonacci[100]` | `354224848179261915075` |
| `PrimeQ[2^61 - 1]` | `True` |
| `Sqrt[152399025]` | `12345` |
| `Pi` | `π` &nbsp; (≈ 3.141592653589793) |
| `Sqrt[2]` | `√2` &nbsp; (≈ 1.4142135623730951) |
| `N[Pi, 40]` | `3.1415926535897932384626433832795028841972` |
| `Sin[Pi/4]` | `0.7071067811865476` |

**Exact is preferred, with the decimal shown alongside.** Results are kept exact
whenever possible, and anything that isn't a plain integer also shows a decimal
approximation (`≈ …`):

- **exact** — integers, rationals (in lowest terms), and irrational leaves kept
  in symbolic form: `Pi` → π, `Sqrt[2]` → √2, each with its decimal beneath.
- **real** — an arbitrary-precision decimal, used when a result can't be kept
  exact. It is *contagious*: `1/3 + 1/3` stays exact `1`, but `1/3 + Pi` (and
  `Sin[1]`, `2·Pi`, …) becomes a real. Reals show ~16 digits; `N[x, d]` shows `d`.

There is no symbolic simplifier yet, so exact irrationals survive only as leaves:
`Sqrt[2]` stays √2, but `2·Pi` collapses to a decimal.

Syntax supported: integer & exact-decimal literals, `+ - * / ^`, unary minus,
postfix `!`, parentheses, `{lists}`, function calls `Head[args]`, and `%` for the
previous result.

Builtins (all delegating to `puremp`):

- **Arithmetic & general** — `Power`, `Sqrt`, `Abs`, `Sign`, `N`, `Floor`,
  `Ceiling`, `Round`, `IntegerPart`, `FractionalPart`.
- **Number theory** — `Factor`, `GCD`, `LCM`, `PrimeQ`, `NextPrime`,
  `Factorial`, `Binomial`, `Multinomial`, `Fibonacci`, `LucasL`, `Mod`,
  `Quotient`, `PowerMod`, `ModularInverse`, `ExtendedGCD`, `JacobiSymbol`,
  `ChineseRemainder`, `SqrtMod`, `EvenQ`, `OddQ`, `IntegerQ`.
- **Rationals** — `Numerator`, `Denominator`, `ContinuedFraction`,
  `FromContinuedFraction`, `Rationalize`.
- **Constants & transcendentals** — `Pi`, `E`; `Exp`, `Log` (`Log[x]` natural,
  `Log[b, x]` base `b`), `Log2`, `Log10`; `Sin`, `Cos`, `Tan`, `ArcSin`,
  `ArcCos`, `ArcTan` (`ArcTan[x, y]` = atan2); `Sinh`, `Cosh`, `Tanh`,
  `ArcSinh`, `ArcCosh`, `ArcTanh`.
- **Complex** (exact, Gaussian rationals) — the imaginary unit `I`, plus `Re`,
  `Im`, `Conjugate`, `Abs`, `Arg`; exact `+ - * /` and integer powers, e.g.
  `(1 + I)^2` → `2 I`, `Sqrt[-4]` → `2 I`, `1/(1 + I)` → `1/2 - 1/2 I`. Inexact
  complex (e.g. `Pi*I`, `Sqrt[-2]`) is not supported yet — it reports a clear
  error rather than a wrong answer, because `puremp`'s `Float` has no operator
  impls for `Complex<Float>`.
- **Linear algebra** (exact, over rationals) — `Det`, `Inverse`, `Transpose`,
  `Dot`, `MatrixRank`, `LinearSolve`, `IdentityMatrix`; `LatticeReduce` (LLL
  reduction of an integer basis, optional second argument δ ∈ (1/4, 1]).
  Rectangular list-of-lists render as bracketed matrices.

## Sharing

Every computation and the whole notebook are shareable as a self-contained link.
The inputs are encoded into the URL hash (`#c=…` for one computation, `#n=…` for
a notebook) — no server is involved — and opening the link replays them locally.
Use the **Share** button in the header for the whole notebook, or hover a cell
for its own share button. On supported devices this opens the native share sheet
(`navigator.share`); otherwise the link is copied to the clipboard.

## Long-running computations

The engine runs in a **Web Worker**, so the UI never freezes — even on an
accidental `3000000!`. A computation that exceeds a wall-clock budget (a few
seconds) is force-stopped by terminating the worker (pure wasm can't be
interrupted cooperatively), and there's a **Stop** button to abort sooner.
Results large enough to choke the renderer are shown as truncated text instead
of typeset math.

## Repository layout

```
Cargo.toml            # the wasm engine crate (Rust)
src/                  # lexer → parser → evaluator, delegating math to puremp
  lexer.rs  parser.rs  ast.rs  value.rs  eval.rs  lib.rs
frontend/             # Vue 3 + Vite notebook UI
  src/App.vue         # the notebook
  src/components/     # Editor (CodeMirror) + MathOutput (KaTeX)
  src/engine.ts       # loads the wasm module
.github/workflows/    # build wasm + frontend, deploy to Pages
```

## Building

Everything is built in CI (see `.github/workflows/pages.yml`); no build tooling
needs to be installed to hack on the sources. To build locally you need Rust with
the `wasm32-unknown-unknown` target, [`wasm-pack`], and Node:

```sh
# 1. Compile the Rust engine to wasm (outputs to frontend/src/pkg/)
wasm-pack build --target web --out-dir frontend/src/pkg --release

# 2. Run the notebook UI
cd frontend
npm install
npm run dev
```

Rust tests for the engine run without any wasm tooling:

```sh
cargo test
```

[`wasm-pack`]: https://rustwasm.github.io/wasm-pack/

## License

MIT — see [LICENSE](LICENSE).
