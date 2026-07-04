//! A single flat error type carried through the whole pipeline (lex → parse →
//! eval). Mathesis is a frontend, so an error is just a human-readable message
//! that the notebook renders next to the offending input.

use core::fmt;

#[derive(Debug, Clone)]
pub struct EvalError(pub String);

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

pub type EResult<T> = Result<T, EvalError>;

/// Shorthand for building an `Err(EvalError(..))` from anything string-like.
pub fn err<T>(msg: impl Into<String>) -> EResult<T> {
    Err(EvalError(msg.into()))
}
