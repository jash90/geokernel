// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/javacc/org/geogebra/common/kernel/parser/Parser.jj
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Tokens of the expression language, a subset of the tokens of GeoGebra's
//! input-bar grammar.

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Tok {
    Num(f64),
    /// A name; `call` when `(` follows without a space (GeoGebra's
    /// `FUNCTION_LABEL`).
    Name {
        text: String,
        call: bool,
    },
    LParen,
    RParen,
    Comma,
    Semicolon,
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    /// `²` or `³`, a power written as a superscript.
    Square(u8),
    Bang,
    Bar,
    Degree,
    Sqrt,
    Infinity,
    Undefined,
}

/// A token and the byte range it covers in the source.
#[derive(Clone, Debug)]
pub(crate) struct Token {
    pub(crate) tok: Tok,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

pub(crate) fn tokens(src: &str) -> Result<Vec<Token>, String> {
    let bytes = src.as_bytes();
    let mut out = Vec::new();
    let mut chars = src.char_indices().peekable();
    while let Some(&(start, ch)) = chars.peek() {
        if ch.is_whitespace() {
            chars.next();
            continue;
        }
        let tok = if ch.is_ascii_digit() || ch == '.' {
            let end = number_end(bytes, start);
            if end == start {
                return Err(format!("unexpected `{ch}`"));
            }
            while chars.peek().is_some_and(|&(i, _)| i < end) {
                chars.next();
            }
            let text = &src[start..end];
            let v = text
                .parse::<f64>()
                .map_err(|_| format!("`{text}` is not a number"))?;
            out.push(Token {
                tok: Tok::Num(v),
                start,
                end,
            });
            continue;
        } else if ch.is_alphabetic() || ch == '_' {
            let mut end = start;
            while let Some(&(i, c)) = chars.peek() {
                if c.is_alphanumeric() || c == '_' || c == '\'' {
                    chars.next();
                    end = i + c.len_utf8();
                    // a subscript in braces: `a_{12}`
                    if c == '_' && chars.peek().is_some_and(|&(_, n)| n == '{') {
                        let close = src[end..].find('}').ok_or("missing `}` in a subscript")?;
                        end += close + 1;
                        while chars.peek().is_some_and(|&(j, _)| j < end) {
                            chars.next();
                        }
                    }
                } else {
                    break;
                }
            }
            let call = bytes.get(end) == Some(&b'(');
            out.push(Token {
                tok: Tok::Name {
                    text: src[start..end].to_string(),
                    call,
                },
                start,
                end,
            });
            continue;
        } else {
            match ch {
                '(' => Tok::LParen,
                ')' => Tok::RParen,
                ',' => Tok::Comma,
                ';' => Tok::Semicolon,
                '+' => Tok::Plus,
                '-' => Tok::Minus,
                '*' => Tok::Star,
                '/' => Tok::Slash,
                '^' => Tok::Caret,
                '!' => Tok::Bang,
                '|' => Tok::Bar,
                '\u{b2}' => Tok::Square(2),
                '\u{b3}' => Tok::Square(3),
                '\u{b0}' | '\u{2da}' | '\u{ba}' => Tok::Degree,
                '\u{221a}' => Tok::Sqrt,
                '\u{221e}' => Tok::Infinity,
                '?' => Tok::Undefined,
                _ => return Err(format!("unexpected `{ch}`")),
            }
        };
        chars.next();
        out.push(Token {
            tok,
            start,
            end: start + ch.len_utf8(),
        });
    }
    Ok(out)
}

/// End of a number literal starting at `start`: digits with an optional
/// fraction, then an optional exponent `E` or `e` that has digits after it.
fn number_end(b: &[u8], start: usize) -> usize {
    let digits = |mut i: usize| {
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        i
    };
    let mut i = digits(start);
    let int_digits = i > start;
    if b.get(i) == Some(&b'.') {
        let j = digits(i + 1);
        if !int_digits && j == i + 1 {
            return start; // a lone `.`
        }
        i = j;
    }
    if matches!(b.get(i), Some(b'e' | b'E')) {
        let mut j = i + 1;
        if matches!(b.get(j), Some(b'+' | b'-')) {
            j += 1;
        }
        let k = digits(j);
        if k > j {
            i = k;
        }
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<Tok> {
        tokens(src).unwrap().into_iter().map(|t| t.tok).collect()
    }

    #[test]
    fn numbers_names_and_calls() {
        let name = |t: &str, call| Tok::Name {
            text: t.into(),
            call,
        };
        assert_eq!(
            kinds("2a + 1E3*sin(b_{1})"),
            vec![
                Tok::Num(2.0),
                name("a", false),
                Tok::Plus,
                Tok::Num(1000.0),
                Tok::Star,
                name("sin", true),
                Tok::LParen,
                name("b_{1}", false),
                Tok::RParen
            ]
        );
        assert_eq!(kinds("2e"), vec![Tok::Num(2.0), name("e", false)]);
        assert_eq!(kinds(".5°"), vec![Tok::Num(0.5), Tok::Degree]);
        assert!(tokens("a # b").is_err());
        assert!(tokens(". 5").is_err());
        assert!(tokens("a_{1").is_err());
    }
}
