//! Bounded syntax reader shared by CAS rows and Giac results.

/// A value printed by Giac.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Expr {
    Num(f64, Option<String>),
    Name(String),
    Neg(Box<Expr>),
    Bin(u8, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
    List(Vec<Expr>),
    Tuple(Vec<Expr>),
    Eq(Box<Expr>, Box<Expr>),
}

/// How deeply the reader may recurse (nested brackets, signs, powers).
const MAX_NESTING: u32 = 64;

/// How deep a read expression may be, chains of `+` and `*` included, so
/// that every walk over it stays far from the end of the stack.
const MAX_DEPTH: usize = 256;
const MAX_NODES: usize = 10_000;

pub(super) fn relation(op: u8) -> Option<&'static str> {
    Some(match op {
        b'<' => "<",
        b'>' => ">",
        b'L' => "<=",
        b'G' => ">=",
        b'=' => "==",
        b'!' => "!=",
        b'&' => "&&",
        _ => return None,
    })
}

struct Reader<'a> {
    s: &'a [u8],
    source: &'a str,
    i: usize,
    nesting: u32,
    input: bool,
    nodes_left: usize,
}

impl Reader<'_> {
    fn charge(&mut self, nodes: usize) -> Option<()> {
        self.nodes_left = self.nodes_left.checked_sub(nodes)?;
        Some(())
    }

    fn peek(&mut self) -> Option<u8> {
        while self.s.get(self.i).is_some_and(u8::is_ascii_whitespace) {
            self.i += 1;
        }
        self.s.get(self.i).copied()
    }

    fn eat(&mut self, b: u8) -> bool {
        if self.peek() == Some(b) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn equation(&mut self) -> Option<Expr> {
        let left = self.sum()?;
        if self.input {
            if let Some(op) = self.inequality() {
                return self.inequality_chain(left, op);
            }
            if self.eat(b'!') {
                if !self.eat(b'=') {
                    return None;
                }
                self.charge(1)?;
                return Some(Expr::Bin(b'!', Box::new(left), Box::new(self.sum()?)));
            }
        }
        if self.eat(b'=') {
            self.charge(1)?;
            if self.input && self.eat(b'=') {
                return Some(Expr::Bin(b'=', Box::new(left), Box::new(self.sum()?)));
            }
            return Some(Expr::Eq(Box::new(left), Box::new(self.sum()?)));
        }
        Some(left)
    }

    fn inequality(&mut self) -> Option<u8> {
        let op @ (b'<' | b'>') = self.peek()? else {
            return None;
        };
        self.i += 1;
        Some(match (op, self.eat(b'=')) {
            (b'<', true) => b'L',
            (b'>', true) => b'G',
            _ => op,
        })
    }

    fn inequality_chain(&mut self, mut left: Expr, mut op: u8) -> Option<Expr> {
        let mut chain = None;
        for _ in 0..MAX_DEPTH {
            let right = self.sum()?;
            let next = self.inequality();
            self.charge(1 + usize::from(chain.is_some()))?;
            let next_left = if next.is_some() {
                self.charge(bounded_tree(&right)?)?;
                Some(right.clone())
            } else {
                None
            };
            let pair = Expr::Bin(op, Box::new(left), Box::new(right));
            let joined = match chain {
                Some(chain) => Expr::Bin(b'&', Box::new(chain), Box::new(pair)),
                None => pair,
            };
            if next.is_none() {
                return Some(joined);
            }
            chain = Some(joined);
            left = next_left?;
            op = next?;
        }
        None
    }

    fn sum(&mut self) -> Option<Expr> {
        let mut left = self.product()?;
        for _ in 0..MAX_DEPTH {
            let op = match self.peek() {
                Some(b'+') => b'+',
                Some(b'-') => b'-',
                _ => return Some(left),
            };
            self.i += 1;
            let right = self.product()?;
            self.charge(1)?;
            left = Expr::Bin(op, Box::new(left), Box::new(right));
        }
        None
    }

    fn product(&mut self) -> Option<Expr> {
        let mut left = self.quotient()?;
        for _ in 0..MAX_DEPTH {
            match self.peek() {
                Some(b'*') => self.i += 1,
                Some(b'/') if !self.input => {
                    self.i += 1;
                    self.charge(1)?;
                    left = Expr::Bin(b'/', Box::new(left), Box::new(self.unary()?));
                    continue;
                }
                Some(
                    b'(' | b'_' | b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z' | 0xCE | 0xCF | 0xE2,
                ) if self.input => {}
                _ => return Some(left),
            }
            self.charge(1)?;
            left = Expr::Bin(b'*', Box::new(left), Box::new(self.quotient()?));
        }
        None
    }

    fn quotient(&mut self) -> Option<Expr> {
        let mut left = self.unary()?;
        for _ in 0..MAX_DEPTH {
            if !self.input || !self.eat(b'/') {
                return Some(left);
            }
            self.charge(1)?;
            left = Expr::Bin(b'/', Box::new(left), Box::new(self.unary()?));
        }
        None
    }

    fn unary(&mut self) -> Option<Expr> {
        // Every nested bracket, sign or exponent passes through here.
        if self.nesting >= MAX_NESTING {
            return None;
        }
        self.nesting += 1;
        let e = self.signed();
        self.nesting -= 1;
        e
    }

    fn signed(&mut self) -> Option<Expr> {
        if self.eat(b'-') {
            self.charge(1)?;
            return Some(Expr::Neg(Box::new(self.unary()?)));
        }
        if self.eat(b'+') {
            return self.unary();
        }
        self.power()
    }

    fn power(&mut self) -> Option<Expr> {
        // `√a`, which the reference's parser reads as `sqrt(a)`.
        let base = if self.peek() == Some(0xE2) && self.s[self.i..].starts_with("√".as_bytes()) {
            self.i += "√".len();
            self.charge(1)?;
            Expr::Call("sqrt".into(), vec![self.atom()?])
        } else {
            self.atom()?
        };
        let base = if self.peek() == Some(b'!') && self.s.get(self.i + 1) != Some(&b'=') {
            self.i += 1;
            self.charge(1)?;
            Expr::Call("!".into(), vec![base])
        } else {
            base
        };
        if let Some(end) = crate::expr::lex::power_end(self.source, self.i) {
            let superscript = &self.source[self.i..end];
            self.i = end;
            self.charge(2 + usize::from(superscript.starts_with('⁻')))?;
            let text: String = superscript
                .chars()
                .filter_map(crate::expr::lex::superscript_digit)
                .map(|digit| char::from(b'0' + digit as u8))
                .collect();
            let exponent = Expr::Num(text.parse().ok()?, Some(text));
            let exponent = if superscript.starts_with('⁻') {
                Expr::Neg(Box::new(exponent))
            } else {
                exponent
            };
            return Some(Expr::Bin(b'^', Box::new(base), Box::new(exponent)));
        }
        if self.eat(b'^') {
            self.charge(1)?;
            let exponent = self.unary()?;
            return Some(Expr::Bin(b'^', Box::new(base), Box::new(exponent)));
        }
        Some(base)
    }

    fn items(&mut self, close: u8) -> Option<Vec<Expr>> {
        let mut items = Vec::new();
        if self.eat(close) {
            return Some(items);
        }
        loop {
            let item = self.equation()?;
            bounded_tree(&item)?;
            items.push(item);
            if self.eat(close) {
                return Some(items);
            }
            if !self.eat(b',') {
                return None;
            }
        }
    }

    fn atom(&mut self) -> Option<Expr> {
        let b = self.peek()?;
        for symbol in ["ί", "π", "ℯ"] {
            if self.s[self.i..].starts_with(symbol.as_bytes()) {
                self.charge(1)?;
                self.i += symbol.len();
                return Some(Expr::Name(symbol.into()));
            }
        }
        if b == b'{' || b == b'[' {
            self.charge(1)?;
            self.i += 1;
            let close = if b == b'{' { b'}' } else { b']' };
            return Some(Expr::List(self.items(close)?));
        }
        if b == b'(' {
            self.i += 1;
            let mut items = self.items(b')')?;
            return Some(if items.len() == 1 {
                items.pop()?
            } else {
                self.charge(1)?;
                Expr::Tuple(items)
            });
        }
        if b.is_ascii_digit() || b == b'.' {
            self.charge(1)?;
            let start = self.i;
            while self
                .s
                .get(self.i)
                .is_some_and(|c| c.is_ascii_digit() || *c == b'.')
            {
                self.i += 1;
            }
            if self.s.get(self.i).is_some_and(|c| *c == b'e' || *c == b'E')
                && self
                    .s
                    .get(self.i + 1)
                    .is_some_and(|c| c.is_ascii_digit() || *c == b'-' || *c == b'+')
            {
                self.i += 2;
                while self.s.get(self.i).is_some_and(u8::is_ascii_digit) {
                    self.i += 1;
                }
            }
            let text = std::str::from_utf8(&self.s[start..self.i]).ok()?;
            return Some(Expr::Num(text.parse().ok()?, Some(text.to_owned())));
        }
        if b.is_ascii_alphabetic() || b == b'_' || b == b'?' {
            self.charge(1)?;
            let start = self.i;
            self.i += 1;
            while self
                .s
                .get(self.i)
                .is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_')
            {
                self.i += 1;
            }
            let name = std::str::from_utf8(&self.s[start..self.i])
                .ok()?
                .to_string();
            if self.eat(b'(') {
                return Some(Expr::Call(name, self.items(b')')?));
            }
            if self.input && self.eat(b'[') {
                return Some(Expr::Call(name, self.items(b']')?));
            }
            return Some(Expr::Name(name));
        }
        None
    }
}

pub(super) fn parse(s: &str, input: bool) -> Option<Expr> {
    let mut reader = Reader {
        s: s.as_bytes(),
        source: s,
        i: 0,
        nesting: 0,
        input,
        nodes_left: MAX_NODES,
    };
    let e = reader.equation()?;
    if reader.peek().is_some() {
        return None;
    }
    bounded_tree(&e)?;
    Some(e)
}

/// Whether the expression is at most [`MAX_DEPTH`] deep, measured without
/// recursion.
pub(super) fn bounded_tree(e: &Expr) -> Option<usize> {
    let mut count = 0;
    let mut stack = vec![(e, 1usize)];
    while let Some((e, depth)) = stack.pop() {
        count += 1;
        if depth > MAX_DEPTH || count > MAX_NODES {
            return None;
        }
        match e {
            Expr::Num(..) | Expr::Name(_) => {}
            Expr::Neg(a) => stack.push((a, depth + 1)),
            Expr::Bin(_, a, b) | Expr::Eq(a, b) => {
                stack.push((a, depth + 1));
                stack.push((b, depth + 1));
            }
            Expr::Call(_, items) | Expr::List(items) | Expr::Tuple(items) => {
                stack.extend(items.iter().map(|item| (item, depth + 1)));
            }
        }
    }
    Some(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_comparison_budget_is_checked_during_parsing() {
        let nested = |depth| "0<(".repeat(depth) + "x" + &")<1".repeat(depth);
        let fits = nested(10);
        assert_eq!(bounded_tree(&parse(&fits, true).unwrap()), Some(6_139));

        let source = nested(11);
        let mut reader = Reader {
            s: source.as_bytes(),
            source: &source,
            i: 0,
            nesting: 0,
            input: true,
            nodes_left: MAX_NODES,
        };
        assert!(
            reader.equation().is_none(),
            "reader built an oversized tree"
        );
    }
}
