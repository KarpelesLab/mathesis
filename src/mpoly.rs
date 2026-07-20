//! Multivariate polynomials over the rationals — the symbolic core behind
//! `D[..]` (derivatives, gradients, Jacobians) and `Det` over polynomial
//! entries.
//!
//! Like everything mathematical in Mathesis this is a *temporary stand-in*: it
//! lives here only until `puremp` (which today has univariate `Poly`) grows a
//! multivariate polynomial type to delegate to. The representation is the
//! simplest correct one — a sparse map from monomials to nonzero rational
//! coefficients — which is plenty for notebook-sized expressions.

use std::collections::BTreeMap;

use puremp::{Int, Rational};

/// A monomial: variable name → exponent, with every exponent ≥ 1 (a variable
/// with exponent 0 is simply absent; the empty map is the constant monomial).
pub type Monomial = BTreeMap<String, u32>;

/// A multivariate polynomial with rational coefficients, kept canonical: no
/// zero coefficients are stored, so the zero polynomial has no terms.
#[derive(Clone)]
pub struct MPoly {
    terms: BTreeMap<Monomial, Rational>,
}

impl MPoly {
    pub fn zero() -> Self {
        MPoly { terms: BTreeMap::new() }
    }

    pub fn constant(c: Rational) -> Self {
        let mut p = MPoly::zero();
        p.accumulate(Monomial::new(), c);
        p
    }

    pub fn var(name: &str) -> Self {
        let mut m = Monomial::new();
        m.insert(name.to_string(), 1);
        let mut p = MPoly::zero();
        p.accumulate(m, Rational::ONE);
        p
    }

    pub fn is_zero(&self) -> bool {
        self.terms.is_empty()
    }

    /// The value of a constant polynomial (0 for the zero polynomial), or
    /// `None` when any variable is present.
    pub fn as_constant(&self) -> Option<Rational> {
        match self.terms.len() {
            0 => Some(Rational::ZERO),
            1 => {
                let (m, c) = self.terms.iter().next().unwrap();
                m.is_empty().then(|| c.clone())
            }
            _ => None,
        }
    }

    /// Add `c` to the coefficient of `m`, dropping the term if it cancels.
    fn accumulate(&mut self, m: Monomial, c: Rational) {
        if c.is_zero() {
            return;
        }
        match self.terms.get(&m) {
            Some(prev) => {
                let sum = prev.add(&c);
                if sum.is_zero() {
                    self.terms.remove(&m);
                } else {
                    self.terms.insert(m, sum);
                }
            }
            None => {
                self.terms.insert(m, c);
            }
        }
    }

    pub fn add(&self, other: &Self) -> Self {
        let mut r = self.clone();
        for (m, c) in &other.terms {
            r.accumulate(m.clone(), c.clone());
        }
        r
    }

    pub fn neg(&self) -> Self {
        MPoly {
            terms: self.terms.iter().map(|(m, c)| (m.clone(), c.neg())).collect(),
        }
    }

    pub fn sub(&self, other: &Self) -> Self {
        self.add(&other.neg())
    }

    pub fn scalar_mul(&self, k: &Rational) -> Self {
        if k.is_zero() {
            return MPoly::zero();
        }
        MPoly {
            terms: self.terms.iter().map(|(m, c)| (m.clone(), c.mul(k))).collect(),
        }
    }

    pub fn mul(&self, other: &Self) -> Self {
        let mut r = MPoly::zero();
        for (ma, ca) in &self.terms {
            for (mb, cb) in &other.terms {
                let mut m = ma.clone();
                for (v, e) in mb {
                    *m.entry(v.clone()).or_insert(0) += e;
                }
                r.accumulate(m, ca.mul(cb));
            }
        }
        r
    }

    /// `self^n` by exponentiation by squaring.
    pub fn pow(&self, n: u32) -> Self {
        let mut acc = MPoly::constant(Rational::ONE);
        let mut base = self.clone();
        let mut n = n;
        while n > 0 {
            if n & 1 == 1 {
                acc = acc.mul(&base);
            }
            n >>= 1;
            if n > 0 {
                base = base.mul(&base);
            }
        }
        acc
    }

    /// The partial derivative ∂/∂`x`.
    pub fn derivative(&self, x: &str) -> Self {
        let mut r = MPoly::zero();
        for (m, c) in &self.terms {
            let Some(&e) = m.get(x) else { continue };
            let mut dm = m.clone();
            if e == 1 {
                dm.remove(x);
            } else {
                dm.insert(x.to_string(), e - 1);
            }
            r.accumulate(dm, c.mul(&Rational::from_integer(Int::from(e as i64))));
        }
        r
    }

    /// Terms in display order — ascending total degree, then lexicographic —
    /// the familiar `2 x - 3 x^2 y - x^3 z` shape Mathematica uses.
    fn ordered_terms(&self) -> Vec<(&Monomial, &Rational)> {
        let mut ts: Vec<_> = self.terms.iter().collect();
        ts.sort_by(|(ma, _), (mb, _)| {
            let da: u32 = ma.values().sum();
            let db: u32 = mb.values().sum();
            da.cmp(&db).then_with(|| ma.cmp(mb))
        });
        ts
    }

    pub fn to_text(&self) -> String {
        self.render(false)
    }

    pub fn to_tex(&self) -> String {
        self.render(true)
    }

    fn render(&self, tex: bool) -> String {
        if self.terms.is_empty() {
            return "0".to_string();
        }
        let mut out = String::new();
        for (i, (m, c)) in self.ordered_terms().into_iter().enumerate() {
            let neg = c.is_negative();
            if i == 0 {
                if neg {
                    out.push('-');
                }
            } else {
                out.push_str(if neg { " - " } else { " + " });
            }
            out.push_str(&term_string(m, &c.abs(), tex));
        }
        out
    }
}

/// One term `|c|·monomial` (the sign is emitted by the caller as `+`/`-`).
fn term_string(m: &Monomial, mag: &Rational, tex: bool) -> String {
    let mut parts: Vec<String> = Vec::new();
    if m.is_empty() || !mag.is_one() {
        parts.push(rational_string(mag, tex));
    }
    for (v, e) in m {
        parts.push(match e {
            1 => v.clone(),
            _ if tex => format!("{v}^{{{e}}}"),
            _ => format!("{v}^{e}"),
        });
    }
    parts.join(" ")
}

fn rational_string(r: &Rational, tex: bool) -> String {
    match r.to_integer() {
        Some(n) => n.to_string(),
        None if tex => format!("\\frac{{{}}}{{{}}}", r.numerator(), r.denominator()),
        None => format!("{}/{}", r.numerator(), r.denominator()),
    }
}
