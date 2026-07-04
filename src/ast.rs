//! The abstract syntax tree produced by [`crate::parser`]. It is a faithful,
//! *unevaluated* picture of what the user typed in Wolfram-style surface syntax;
//! all meaning is assigned later in [`crate::eval`].

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    // Relational — evaluate to a boolean, or become SMT constraints in a solver.
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    // Logical.
    And,
    Or,
}

#[derive(Debug, Clone)]
pub enum Expr {
    /// An arbitrary-length integer literal, kept as its source digits until
    /// evaluation hands them to `puremp`.
    Int(String),
    /// A decimal literal `int.frac`; `frac` may be empty (e.g. `5.`). Evaluated
    /// to an *exact* rational, never a lossy float.
    Decimal { int: String, frac: String },
    /// A bare identifier (`Pi`, `x`, …).
    Symbol(String),
    /// A string literal (e.g. an SMT-LIB script passed to `SMT[..]`).
    Str(String),
    /// `%` — the previous result.
    Last,
    Neg(Box<Expr>),
    Factorial(Box<Expr>),
    Bin(Op, Box<Expr>, Box<Expr>),
    /// A Wolfram-style function application `Head[arg, …]`.
    Call(String, Vec<Expr>),
    /// A list `{a, b, …}`.
    List(Vec<Expr>),
}
