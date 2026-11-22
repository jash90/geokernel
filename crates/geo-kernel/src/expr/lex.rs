// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/javacc/org/geogebra/common/kernel/parser/Parser.jj
//   source/shared/common/src/main/java/org/geogebra/common/util/ParserPreprocessor.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Tokens of the expression language, a subset of the tokens of GeoGebra's
//! input-bar grammar.

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Tok {
    Num(f64),
    Text(String),
    /// A name; `call` when `(` follows without a space (GeoGebra's
    /// `FUNCTION_LABEL`).
    Name {
        text: String,
        call: bool,
    },
    LParen,
    RParen,
    /// `{` and `}` of a list.
    LBrace,
    RBrace,
    Comma,
    Semicolon,
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    /// A power written in superscript digits, `²` or `⁻¹²` (`POWERN`).
    Power(f64),
    /// `%` after a number literal: a percentage.
    Percent,
    Bang,
    Bar,
    Degree,
    Sqrt,
    Infinity,
    Undefined,
    /// `¬`; a `!` before an operand is a negation too, decided by the
    /// parser.
    Not,
    /// `||`, `∨`.
    Or,
    /// `&&`, `∧`.
    And,
    /// `⊻`, `⊕`.
    Xor,
    /// `->`, `→`.
    Implies,
    /// `==`, `≟`.
    Equal,
    /// `!=`, `≠`, `<>`.
    NotEqual,
    Less,
    Greater,
    /// `<=`, `≤`.
    LessEqual,
    /// `>=`, `≥`.
    GreaterEqual,
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
        let tok = if ch == '"' {
            chars.next();
            let mut value = String::new();
            let mut end = None;
            while let Some((i, next)) = chars.next() {
                match next {
                    '"' => {
                        end = Some(i + 1);
                        break;
                    }
                    '\\' if chars
                        .peek()
                        .is_some_and(|&(_, next)| matches!(next, '"' | '\\')) =>
                    {
                        value.push(chars.next().unwrap().1);
                    }
                    _ => value.push(next),
                }
            }
            out.push(Token {
                tok: Tok::Text(value),
                start,
                end: end.ok_or("unterminated text")?,
            });
            continue;
        } else if ch.is_ascii_digit() || ch == '.' {
            let end = number_end(src, start)?;
            while chars.peek().is_some_and(|&(i, _)| i < end) {
                chars.next();
            }
            let v = literal_value(&src[start..end])?;
            out.push(Token {
                tok: Tok::Num(v),
                start,
                end,
            });
            continue;
        } else if is_name_char(ch) && !ch.is_numeric() || ch == '_' {
            let mut end = start;
            while let Some(&(i, c)) = chars.peek() {
                if is_name_char(c) || c == '_' || c == '\'' {
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
        } else if let Some(end) = power_end(src, start) {
            while chars.peek().is_some_and(|&(i, _)| i < end) {
                chars.next();
            }
            out.push(Token {
                tok: Tok::Power(superscript_value(&src[start..end])),
                start,
                end,
            });
            continue;
        } else if let Some(tok) = two_char_operator(&src[start..]) {
            // JavaCC takes the longest token: `a!=b` is `a != b`, never a
            // factorial, and `<=` is one token
            chars.next();
            chars.next();
            out.push(Token {
                tok,
                start,
                end: start + 2,
            });
            continue;
        } else {
            match ch {
                '(' => Tok::LParen,
                ')' => Tok::RParen,
                '{' => Tok::LBrace,
                '}' => Tok::RBrace,
                ',' => Tok::Comma,
                ';' => Tok::Semicolon,
                '+' => Tok::Plus,
                // `MINUS`: hyphen, en dash, minus sign
                '-' | '\u{2013}' | '\u{2212}' => Tok::Minus,
                // `MULTIPLY`: asterisk, dot operator, bullet operator, times
                '*' | '\u{22c5}' | '\u{2219}' | '\u{d7}' => Tok::Star,
                // `DIVIDE`: slash, division sign
                '/' | '\u{f7}' => Tok::Slash,
                // `POWER`: caret, modifier circumflex (`**` above)
                '^' | '\u{2c6}' => Tok::Caret,
                '%' => Tok::Percent,
                '!' => Tok::Bang,
                '|' => Tok::Bar,
                '\u{b0}' | '\u{2da}' | '\u{ba}' => Tok::Degree,
                '\u{221a}' => Tok::Sqrt,
                '\u{221e}' => Tok::Infinity,
                '?' => Tok::Undefined,
                '\u{ac}' => Tok::Not,
                '\u{2228}' => Tok::Or,
                '\u{2227}' => Tok::And,
                '\u{22bb}' | '\u{2295}' => Tok::Xor,
                '\u{2192}' => Tok::Implies,
                '\u{225f}' => Tok::Equal,
                '\u{2260}' => Tok::NotEqual,
                '<' => Tok::Less,
                '>' => Tok::Greater,
                '\u{2264}' => Tok::LessEqual,
                '\u{2265}' => Tok::GreaterEqual,
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

/// The operators spelled with two ASCII characters.
fn two_char_operator(rest: &str) -> Option<Tok> {
    Some(match rest.get(..2)? {
        "**" => Tok::Caret,
        "||" => Tok::Or,
        "&&" => Tok::And,
        "->" => Tok::Implies,
        "==" => Tok::Equal,
        "!=" | "<>" => Tok::NotEqual,
        "<=" => Tok::LessEqual,
        ">=" => Tok::GreaterEqual,
        _ => return None,
    })
}

/// `ParserPreprocessor`'s treatment of `||`, which is the `OR` token unless
/// a space goes between the bars: scanning forwards with the splitters
/// `√ + - * / ^ =`, then backwards with `* / ^ =` and the superscripts, a
/// bar gets a space after it when it is the first character, when an odd
/// number of bars came before and two more follow, or when a bar follows
/// and the last character that is not a space is a splitter. So
/// `||a| - |b||` is a nesting of absolute values and `a || b` is an `OR`.
pub(crate) fn split_bars(src: &str) -> std::borrow::Cow<'_, str> {
    if !src.contains("||") {
        return std::borrow::Cow::Borrowed(src);
    }
    let forward = scan_bars(src.chars(), &['\u{221a}', '+', '-', '*', '/', '^', '=']);
    let backward = scan_bars(
        forward.chars().rev(),
        &[
            '*', '/', '^', '=', '\u{2070}', '\u{b9}', '\u{b2}', '\u{b3}', '\u{2074}', '\u{2075}',
            '\u{2076}', '\u{2077}', '\u{2078}', '\u{2079}', '\u{207b}',
        ],
    );
    std::borrow::Cow::Owned(backward.chars().rev().collect())
}

fn scan_bars(chars: impl Iterator<Item = char>, splitters: &[char]) -> String {
    let chars: Vec<char> = chars.collect();
    let n = chars.len();
    let mut out = String::with_capacity(n + 4);
    let (mut bars, mut last) = (0usize, ' ');
    let mut quoted = false;
    for (i, &ch) in chars.iter().enumerate() {
        out.push(ch);
        if ch == '"' {
            quoted = !quoted;
        }
        if quoted || ch.is_whitespace() {
            continue;
        }
        if ch == '|' {
            let bar = |k: usize| chars.get(k) == Some(&'|');
            if i == 0
                || bars % 2 == 1 && bar(i + 1) && bar(i + 2)
                || bar(i + 1) && splitters.contains(&last)
            {
                out.push(' ');
            }
            bars += 1;
        }
        last = ch;
    }
    out
}

/// End of a number literal starting at `start`: GeoGebra's `FLOAT`, a run
/// of digits and decimal points with exactly one point at most, then
/// optionally `EFLOAT`'s exponent: an upper-case `E`, a sign and digits.
/// A lower-case `e` never starts an exponent: `2e3` is `2` and the name
/// `e3`. A run like `1.2.3` or `1..2` is an error, as upstream
/// (`MyDouble.parseDouble`), and so is a lone `.`.
fn number_end(src: &str, start: usize) -> Result<usize, String> {
    let b = src.as_bytes();
    let mut i = start;
    let (mut digits, mut points) = (0, 0);
    while let Some(&c) = b.get(i) {
        match c {
            b'0'..=b'9' => digits += 1,
            b'.' => points += 1,
            _ => break,
        }
        i += 1;
    }
    if digits == 0 || points > 1 {
        return Err(format!("`{}` is not a number", &src[start..i]));
    }
    if b.get(i) == Some(&b'E') {
        let rest = &src[i + 1..];
        let sign = rest
            .chars()
            .next()
            .filter(|c| matches!(c, '+' | '-' | '\u{2013}' | '\u{2212}'))
            .map_or(0, char::len_utf8);
        let k = rest[sign..].bytes().take_while(u8::is_ascii_digit).count();
        if k > 0 {
            i += 1 + sign + k;
        }
    }
    Ok(i)
}

/// A letter or digit of a name. The masculine ordinal `º` (a degree sign
/// on some keyboards), the modifier circumflex `ˆ` (a power) and the
/// superscript digits are letters or digits to Unicode but operators here.
pub(crate) fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() && !matches!(c, '\u{ba}' | '\u{2c6}') && superscript_digit(c).is_none()
}

/// The value of a literal that [`number_end`] delimited. GeoGebra's lexer
/// takes `–` and `−` as the sign of an exponent, but converting such a
/// literal fails ("Not a digit"), so the line is an error there too.
fn literal_value(text: &str) -> Result<f64, String> {
    if !text.contains(['\u{2013}', '\u{2212}']) {
        if let Ok(v) = text.parse::<f64>() {
            return Ok(v);
        }
    }
    Err(format!("`{text}` is not a number"))
}

/// A whole string that is one number literal, optionally after a `-`:
/// the plain numbers of the script forms (`r = 3`, `A = (1, -2.5)`).
/// Anything else (`1e3`, `+5`, `inf`) is left to the expression parser.
pub(crate) fn plain_number(s: &str) -> Option<f64> {
    let (negative, body) = match s.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, s),
    };
    if !body.starts_with(|c: char| c.is_ascii_digit() || c == '.') {
        return None;
    }
    match number_end(body, 0) {
        Ok(end) if end == body.len() => {
            let v = literal_value(body).ok()?;
            Some(if negative { -v } else { v })
        }
        _ => None,
    }
}

/// The value of a superscript digit (`INDEXDIGIT`).
fn superscript_digit(c: char) -> Option<u32> {
    match c {
        '\u{2070}' => Some(0),
        '\u{b9}' => Some(1),
        '\u{b2}' => Some(2),
        '\u{b3}' => Some(3),
        '\u{2074}'..='\u{2079}' => Some(c as u32 - 0x2070),
        _ => None,
    }
}

/// End of `POWERN` at `start`: an optional superscript minus, then
/// superscript digits.
fn power_end(src: &str, start: usize) -> Option<usize> {
    let rest = &src[start..];
    let sign = if rest.starts_with('\u{207b}') {
        '\u{207b}'.len_utf8()
    } else {
        0
    };
    let digits: usize = rest[sign..]
        .chars()
        .take_while(|&c| superscript_digit(c).is_some())
        .map(char::len_utf8)
        .sum();
    (digits > 0).then_some(start + sign + digits)
}

/// `StringUtil.indexToNumber`: the integer a run of superscripts spells.
fn superscript_value(text: &str) -> f64 {
    let mut v = 0.0;
    for d in text.chars().filter_map(superscript_digit) {
        v = v * 10.0 + f64::from(d);
    }
    if text.starts_with('\u{207b}') {
        -v
    } else {
        v
    }
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
        // only an upper-case `E` starts an exponent
        assert_eq!(kinds("2e3"), vec![Tok::Num(2.0), name("e3", false)]);
        assert_eq!(kinds("1.E-3"), vec![Tok::Num(0.001)]);
        assert!(tokens("1.E\u{2212}3").is_err() && tokens("1E\u{2013}3 + 1").is_err());
        assert_eq!(kinds("2E"), vec![Tok::Num(2.0), name("E", false)]);
        assert_eq!(kinds(".5°"), vec![Tok::Num(0.5), Tok::Degree]);
        assert_eq!(kinds("5."), vec![Tok::Num(5.0)]);
        assert!(tokens("a # b").is_err());
        assert!(tokens(". 5").is_err());
        assert!(tokens("a_{1").is_err());
        assert!(tokens("1.2.3").is_err());
        assert!(tokens("1..2").is_err());
        assert!(tokens("2 ** ..").is_err());
    }

    #[test]
    fn operator_spellings_and_superscripts() {
        assert_eq!(
            kinds("2**3 \u{22c5} 4 \u{2219} 5 \u{d7} 6 \u{f7} 7 \u{2212} 8 \u{2013} 9 \u{2c6} 1"),
            vec![
                Tok::Num(2.0),
                Tok::Caret,
                Tok::Num(3.0),
                Tok::Star,
                Tok::Num(4.0),
                Tok::Star,
                Tok::Num(5.0),
                Tok::Star,
                Tok::Num(6.0),
                Tok::Slash,
                Tok::Num(7.0),
                Tok::Minus,
                Tok::Num(8.0),
                Tok::Minus,
                Tok::Num(9.0),
                Tok::Caret,
                Tok::Num(1.0)
            ]
        );
        assert_eq!(
            kinds("2\u{b3}\u{2074} 2\u{207b}\u{b9}\u{b2} 5%"),
            vec![
                Tok::Num(2.0),
                Tok::Power(34.0),
                Tok::Num(2.0),
                Tok::Power(-12.0),
                Tok::Num(5.0),
                Tok::Percent
            ]
        );
        assert!(tokens("2\u{207b}").is_err());
    }

    #[test]
    fn plain_numbers_are_whole_literals() {
        assert_eq!(plain_number("3"), Some(3.0));
        assert_eq!(plain_number("-2.5"), Some(-2.5));
        assert_eq!(plain_number("1E300"), Some(1e300));
        assert_eq!(plain_number("-0.0").map(f64::is_sign_negative), Some(true));
        for not_plain in [
            "1e3", "+5", "inf", "nan", "infinity", "1.2.3", "1 + 1", "", "-", "--1",
        ] {
            assert_eq!(plain_number(not_plain), None, "{not_plain}");
        }
    }
}
