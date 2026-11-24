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

pub(super) fn relation(op: u8) -> Option<&'static str> {
    Some(match op {
        b'<' => "<",
        b'>' => ">",
        b'L' => "<=",
        b'G' => ">=",
        b'=' => "==",
        b'!' => "!=",
        _ => return None,
    })
}

struct Reader<'a> {
    s: &'a [u8],
    i: usize,
    nesting: u32,
    input: bool,
}

impl Reader<'_> {
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
        match self.peek() {
            Some(op @ (b'<' | b'>' | b'=' | b'!')) if self.input => {
                self.i += 1;
                let compare = self.eat(b'=');
                let right = Box::new(self.sum()?);
                let left = Box::new(left);
                if !compare && op == b'=' {
                    Some(Expr::Eq(left, right))
                } else {
                    let op = match (op, compare) {
                        (b'<', true) => b'L',
                        (b'>', true) => b'G',
                        (b'!', false) => return None,
                        (op, _) => op,
                    };
                    Some(Expr::Bin(op, left, right))
                }
            }
            Some(b'=') => {
                self.i += 1;
                Some(Expr::Eq(Box::new(left), Box::new(self.sum()?)))
            }
            _ => Some(left),
        }
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
                    left = Expr::Bin(b'/', Box::new(left), Box::new(self.unary()?));
                    continue;
                }
                Some(
                    b'(' | b'_' | b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z' | 0xCE | 0xCF | 0xE2,
                ) if self.input => {}
                _ => return Some(left),
            }
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
            Expr::Call("sqrt".into(), vec![self.atom()?])
        } else {
            self.atom()?
        };
        if self.eat(b'^') {
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
                self.i += symbol.len();
                return Some(Expr::Name(symbol.into()));
            }
        }
        if b == b'{' || b == b'[' {
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
                Expr::Tuple(items)
            });
        }
        if b.is_ascii_digit() || b == b'.' {
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
        i: 0,
        nesting: 0,
        input,
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
        if depth > MAX_DEPTH || count > 10_000 {
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
