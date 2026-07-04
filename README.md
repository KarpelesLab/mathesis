# Mathesis

**A Mathematica-style computational notebook that runs entirely in your browser.**

Mathesis is a free, client-side mathematics workbench: type an expression in
Wolfram-style syntax and get an *exact* answer, computed on your own machine with
no server round-trip. The math engine is written in Rust, compiled to
WebAssembly, and wrapped in a Vue 3 notebook UI.

> Live at **https://karpeleslab.github.io/mathesis/** (deployed from `master` via
> GitHub Actions).

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
| `N[22/7, 20]` | `3.14285714285714285714` |

Syntax supported: integer & exact-decimal literals, `+ - * / ^`, unary minus,
postfix `!`, parentheses, `{lists}`, function calls `Head[args]`, and `%` for the
previous result.

Builtins: `Factor`, `GCD`, `LCM`, `Factorial`, `Binomial`, `Fibonacci`,
`PrimeQ`, `Sqrt`, `Power`, `Abs`, `Numerator`, `Denominator`, `N`.

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

## Enabling GitHub Pages

The deploy job publishes with the modern Pages/OIDC flow. In the repository
settings, set **Settings → Pages → Build and deployment → Source** to
**GitHub Actions** once; every push to `master` then redeploys automatically.

## License

MIT — see [LICENSE](LICENSE).
