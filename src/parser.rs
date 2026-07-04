//! A Pratt (precedence-climbing) parser turning a token stream into an [`Expr`].
//!
//! Precedence, lowest to highest, matching Wolfram Language conventions:
//!
//! | operator            | binding | assoc  |
//! |---------------------|---------|--------|
//! | `+` `-` (binary)    | 10      | left   |
//! | `*` `/`             | 20      | left   |
//! | unary `-` / `+`     | 30      | prefix |
//! | `^`                 | 40      | right  |
//! | `!` (factorial)     | 50      | postfix|
//!
//! So `-2^2` is `-(2^2) = -4` and `2^-2` is `2^(-2)`, exactly as Mathematica
//! parses them.

use crate::ast::{Expr, Op};
use crate::lexer::{Tok, lex};

pub fn parse(src: &str) -> Result<Expr, String> {
    let toks = lex(src)?;
    let mut p = Parser { toks, pos: 0 };
    let e = p.expr(0)?;
    match p.peek() {
        Tok::Eof => Ok(e),
        other => Err(format!("unexpected trailing token `{}`", describe(other))),
    }
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> &Tok {
        &self.toks[self.pos]
    }

    fn bump(&mut self) -> Tok {
        let t = self.toks[self.pos].clone();
        if self.pos + 1 < self.toks.len() {
            self.pos += 1;
        }
        t
    }

    fn expect(&mut self, want: &Tok) -> Result<(), String> {
        if self.peek() == want {
            self.bump();
            Ok(())
        } else {
            Err(format!(
                "expected `{}`, found `{}`",
                describe(want),
                describe(self.peek())
            ))
        }
    }

    fn expr(&mut self, min_bp: u8) -> Result<Expr, String> {
        let mut lhs = self.prefix()?;

        loop {
            // Postfix `!` binds tightest of all.
            if *self.peek() == Tok::Bang {
                self.bump();
                lhs = Expr::Factorial(Box::new(lhs));
                continue;
            }

            let (op, lbp, rbp) = match self.peek() {
                Tok::PipePipe => (Op::Or, 3, 4),
                Tok::AmpAmp => (Op::And, 5, 6),
                Tok::EqEq => (Op::Eq, 7, 8),
                Tok::BangEq => (Op::Ne, 7, 8),
                Tok::Lt => (Op::Lt, 7, 8),
                Tok::Le => (Op::Le, 7, 8),
                Tok::Gt => (Op::Gt, 7, 8),
                Tok::Ge => (Op::Ge, 7, 8),
                Tok::Plus => (Op::Add, 10, 11),
                Tok::Minus => (Op::Sub, 10, 11),
                Tok::Star => (Op::Mul, 20, 21),
                Tok::Slash => (Op::Div, 20, 21),
                Tok::Caret => (Op::Pow, 41, 40), // right-associative
                _ => break,
            };
            if lbp < min_bp {
                break;
            }
            self.bump();
            let rhs = self.expr(rbp)?;
            lhs = Expr::Bin(op, Box::new(lhs), Box::new(rhs));
        }

        Ok(lhs)
    }

    fn prefix(&mut self) -> Result<Expr, String> {
        match self.bump() {
            Tok::Minus => Ok(Expr::Neg(Box::new(self.expr(30)?))),
            Tok::Plus => self.expr(30), // unary plus is a no-op
            Tok::Num { int, frac } => Ok(match frac {
                None => Expr::Int(int),
                Some(frac) => Expr::Decimal { int, frac },
            }),
            Tok::Str(s) => Ok(Expr::Str(s)),
            Tok::Percent => Ok(Expr::Last),
            Tok::Ident(name) => {
                if *self.peek() == Tok::LBrack {
                    self.bump();
                    let args = self.args(&Tok::RBrack)?;
                    Ok(Expr::Call(name, args))
                } else {
                    Ok(Expr::Symbol(name))
                }
            }
            Tok::LParen => {
                let e = self.expr(0)?;
                self.expect(&Tok::RParen)?;
                Ok(e)
            }
            Tok::LBrace => {
                let items = self.args(&Tok::RBrace)?;
                Ok(Expr::List(items))
            }
            Tok::Eof => Err("unexpected end of input".to_string()),
            other => Err(format!("unexpected token `{}`", describe(&other))),
        }
    }

    /// Parse a comma-separated argument/element list, consuming the closing
    /// bracket. Assumes the opening bracket was already consumed.
    fn args(&mut self, close: &Tok) -> Result<Vec<Expr>, String> {
        let mut v = Vec::new();
        if self.peek() == close {
            self.bump();
            return Ok(v);
        }
        loop {
            v.push(self.expr(0)?);
            match self.bump() {
                Tok::Comma => continue,
                ref t if t == close => break,
                other => {
                    return Err(format!(
                        "expected `,` or `{}`, found `{}`",
                        describe(close),
                        describe(&other)
                    ));
                }
            }
        }
        Ok(v)
    }
}

fn describe(t: &Tok) -> String {
    match t {
        Tok::Num { int, frac } => match frac {
            None => int.clone(),
            Some(f) => format!("{int}.{f}"),
        },
        Tok::Ident(s) => s.clone(),
        Tok::Str(s) => format!("\"{s}\""),
        Tok::Plus => "+".into(),
        Tok::Minus => "-".into(),
        Tok::Star => "*".into(),
        Tok::Slash => "/".into(),
        Tok::Caret => "^".into(),
        Tok::Bang => "!".into(),
        Tok::LParen => "(".into(),
        Tok::RParen => ")".into(),
        Tok::LBrack => "[".into(),
        Tok::RBrack => "]".into(),
        Tok::LBrace => "{".into(),
        Tok::RBrace => "}".into(),
        Tok::Comma => ",".into(),
        Tok::Percent => "%".into(),
        Tok::EqEq => "==".into(),
        Tok::BangEq => "!=".into(),
        Tok::Lt => "<".into(),
        Tok::Le => "<=".into(),
        Tok::Gt => ">".into(),
        Tok::Ge => ">=".into(),
        Tok::AmpAmp => "&&".into(),
        Tok::PipePipe => "||".into(),
        Tok::Eof => "end of input".into(),
    }
}
