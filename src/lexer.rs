//! A tiny hand-written lexer for the Wolfram-style surface syntax. It is
//! deliberately minimal — numbers, identifiers, and the handful of operators and
//! brackets the parser understands — and grows as the language does.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tok {
    /// A numeric literal. `frac` is `None` for integers, `Some(digits)` for
    /// decimals (the digits after the dot, possibly empty for `5.`).
    Num { int: String, frac: Option<String> },
    Ident(String),
    /// A double-quoted string literal, already unescaped.
    Str(String),
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    Bang,
    LParen,
    RParen,
    LBrack,
    RBrack,
    LBrace,
    RBrace,
    Comma,
    Percent,
    /// `=` — assignment (Set).
    Assign,
    // Relational / logical.
    EqEq,
    BangEq,
    Lt,
    Le,
    Gt,
    Ge,
    AmpAmp,
    PipePipe,
    Eof,
}

pub fn lex(src: &str) -> Result<Vec<Tok>, String> {
    let b = src.as_bytes();
    let mut i = 0;
    let mut out = Vec::new();

    while i < b.len() {
        let c = b[i];
        match c {
            b' ' | b'\t' | b'\r' | b'\n' => i += 1,
            b'"' => {
                i += 1; // opening quote
                let mut s = String::new();
                loop {
                    if i >= b.len() {
                        return Err("unterminated string literal".to_string());
                    }
                    match b[i] {
                        b'"' => {
                            i += 1; // closing quote
                            break;
                        }
                        b'\\' => {
                            // Escapes: \" \\ \n \t \r; anything else is literal.
                            i += 1;
                            if i >= b.len() {
                                return Err("unterminated string escape".to_string());
                            }
                            match b[i] {
                                b'n' => s.push('\n'),
                                b't' => s.push('\t'),
                                b'r' => s.push('\r'),
                                other => s.push(other as char),
                            }
                            i += 1;
                        }
                        _ => {
                            // Copy one UTF-8 char.
                            let ch = src[i..].chars().next().unwrap();
                            s.push(ch);
                            i += ch.len_utf8();
                        }
                    }
                }
                out.push(Tok::Str(s));
            }
            b'0'..=b'9' => {
                let start = i;
                while i < b.len() && b[i].is_ascii_digit() {
                    i += 1;
                }
                let int = src[start..i].to_string();
                let mut frac = None;
                // A dot is part of the number only when it isn't a stray `.` —
                // we accept both `3.14` and the trailing-dot form `5.`.
                if i < b.len() && b[i] == b'.' {
                    i += 1;
                    let fs = i;
                    while i < b.len() && b[i].is_ascii_digit() {
                        i += 1;
                    }
                    frac = Some(src[fs..i].to_string());
                }
                out.push(Tok::Num { int, frac });
            }
            b'A'..=b'Z' | b'a'..=b'z' => {
                let start = i;
                while i < b.len() && (b[i].is_ascii_alphanumeric()) {
                    i += 1;
                }
                out.push(Tok::Ident(src[start..i].to_string()));
            }
            b'+' => push(&mut out, &mut i, Tok::Plus),
            b'-' => push(&mut out, &mut i, Tok::Minus),
            b'*' => push(&mut out, &mut i, Tok::Star),
            b'/' => push(&mut out, &mut i, Tok::Slash),
            b'^' => push(&mut out, &mut i, Tok::Caret),
            b'!' => {
                if i + 1 < b.len() && b[i + 1] == b'=' {
                    out.push(Tok::BangEq);
                    i += 2;
                } else {
                    out.push(Tok::Bang);
                    i += 1;
                }
            }
            b'=' => {
                // `==` compares; a lone `=` assigns (Wolfram convention).
                if i + 1 < b.len() && b[i + 1] == b'=' {
                    out.push(Tok::EqEq);
                    i += 2;
                } else {
                    out.push(Tok::Assign);
                    i += 1;
                }
            }
            b'<' => {
                if i + 1 < b.len() && b[i + 1] == b'=' {
                    out.push(Tok::Le);
                    i += 2;
                } else {
                    out.push(Tok::Lt);
                    i += 1;
                }
            }
            b'>' => {
                if i + 1 < b.len() && b[i + 1] == b'=' {
                    out.push(Tok::Ge);
                    i += 2;
                } else {
                    out.push(Tok::Gt);
                    i += 1;
                }
            }
            b'&' => {
                if i + 1 < b.len() && b[i + 1] == b'&' {
                    out.push(Tok::AmpAmp);
                    i += 2;
                } else {
                    return Err("unexpected `&` (did you mean `&&`?)".to_string());
                }
            }
            b'|' => {
                if i + 1 < b.len() && b[i + 1] == b'|' {
                    out.push(Tok::PipePipe);
                    i += 2;
                } else {
                    return Err("unexpected `|` (did you mean `||`?)".to_string());
                }
            }
            b'(' => push(&mut out, &mut i, Tok::LParen),
            b')' => push(&mut out, &mut i, Tok::RParen),
            b'[' => push(&mut out, &mut i, Tok::LBrack),
            b']' => push(&mut out, &mut i, Tok::RBrack),
            b'{' => push(&mut out, &mut i, Tok::LBrace),
            b'}' => push(&mut out, &mut i, Tok::RBrace),
            b',' => push(&mut out, &mut i, Tok::Comma),
            b'%' => push(&mut out, &mut i, Tok::Percent),
            _ => {
                // Report the offending character as a real char, not a byte.
                let ch = src[i..].chars().next().unwrap_or('\u{fffd}');
                return Err(format!("unexpected character `{ch}`"));
            }
        }
    }

    out.push(Tok::Eof);
    Ok(out)
}

fn push(out: &mut Vec<Tok>, i: &mut usize, t: Tok) {
    out.push(t);
    *i += 1;
}
