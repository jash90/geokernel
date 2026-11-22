use crate::{Color, Style};
use std::collections::HashMap;

mod colors;
mod commands;
mod environments;
mod symbols;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Class {
    Ordinary,
    Operator,
    Binary,
    Relation,
    Open,
    Close,
    Punctuation,
    Inner,
}

#[derive(Clone, Copy)]
pub(crate) enum Variant {
    Text,
    Upright,
    Italic,
    Bold,
    Mono,
    SmallCaps,
}

#[derive(Clone, Copy)]
pub(crate) enum Accent {
    Bar,
    Under,
    Right(bool),
    Left(bool),
    Both(bool),
    Hat,
    Tilde,
    Dot,
    DoubleDot,
    Brace(bool),
}

#[derive(Clone, Copy)]
pub(crate) enum Align {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy)]
pub(crate) enum Length {
    Em(f32),
    Points(f32),
}

impl Length {
    pub(crate) fn points(self, em: f32) -> f32 {
        match self {
            Self::Em(n) => n * em,
            Self::Points(n) => n,
        }
    }
}

pub(crate) struct Grid<'a> {
    pub rows: Vec<Vec<Node<'a>>>,
    pub columns: Vec<(Align, Length)>,
    pub vertical: Vec<usize>,
    pub horizontal: Vec<usize>,
    pub gaps: Vec<Length>,
    pub aligned: bool,
    pub text: bool,
}

pub(crate) enum Node<'a> {
    Char(char, Class),
    Word(&'a str, Class),
    Row(Vec<Self>),
    Lines(Vec<Self>, bool),
    Space(Length),
    VerticalSpace(Length),
    Fraction(Box<[Self; 2]>, Option<Style>, bool),
    Root(Box<Self>, Option<Box<Self>>),
    Script {
        base: Box<Self>,
        sub: Option<Box<Self>>,
        sup: Option<Box<Self>>,
        limits: Option<bool>,
    },
    Operator(char, bool),
    Delimited(char, Box<Self>, char),
    SizedDelimiter(char, f32, Class),
    Grid(Grid<'a>),
    Variant(Variant, Box<Self>),
    Styled(Style, Option<Box<Self>>),
    SwitchVariant(Variant),
    SwitchColor(Color),
    Accent(Accent, Box<Self>),
    Arrow(Accent, Box<Self>, Option<Box<Self>>),
    Color(Color, Box<Self>, bool),
    Border(Box<Self>),
    Rotate(f32, Box<Self>),
    Scale(f32, f32, Box<Self>),
    CellColor(Color, Box<Self>),
    Phantom(Box<Self>, bool, bool),
    Classified(Class, Box<Self>),
    Fallback(&'a str),
    /// A missing or unusable argument: an empty box in the error colour.
    Error,
    /// An error with no ink of its own, where a closing token TeX would
    /// insert was missing. Flags the formula as partly broken.
    Broken,
}

pub(crate) const MAX_SOURCE: usize = 32 * 1024;
const MAX_DEPTH: usize = 96;
type Result<T> = std::result::Result<T, ()>;

pub(crate) fn parse(source: &str) -> Node<'_> {
    if source.len() > MAX_SOURCE {
        let end = source.floor_char_boundary(MAX_SOURCE);
        return Node::Fallback(&source[..end]);
    }
    let mut parser = Parser {
        rest: source,
        depth: 0,
        text: false,
        colors: HashMap::new(),
    };
    parser.document()
}

#[derive(Clone, Copy, PartialEq)]
enum Stop {
    Top,
    Brace,
    Bracket,
    Fence,
    Grid,
    Dollar,
    Inline,
    Display,
    Lines,
}

struct Parser<'a> {
    rest: &'a str,
    depth: usize,
    text: bool,
    colors: HashMap<&'a str, Color>,
}

impl<'a> Parser<'a> {
    fn nested<T>(&mut self, parse: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        if self.depth == MAX_DEPTH {
            return Err(());
        }
        self.depth += 1;
        let result = parse(self);
        self.depth -= 1;
        result
    }

    fn peek(&self) -> Option<char> {
        self.rest.chars().next()
    }

    fn next(&mut self) -> Result<char> {
        let c = self.peek().ok_or(())?;
        self.rest = &self.rest[c.len_utf8()..];
        Ok(c)
    }

    fn eat(&mut self, prefix: &str) -> bool {
        if let Some(rest) = self.rest.strip_prefix(prefix) {
            self.rest = rest;
            true
        } else {
            false
        }
    }

    fn space(&mut self) {
        self.rest = self.rest.trim_start_matches(char::is_whitespace);
    }

    fn control(&mut self) -> Result<&'a str> {
        let start = self.rest;
        self.next()?;
        if start.as_bytes()[0].is_ascii_alphabetic() {
            self.rest = self
                .rest
                .trim_start_matches(|c: char| c.is_ascii_alphabetic());
        }
        Ok(&start[..start.len() - self.rest.len()])
    }

    fn at_control(&self, name: &str) -> bool {
        self.rest
            .strip_prefix('\\')
            .and_then(|after| after.strip_prefix(name))
            .is_some_and(|tail| !tail.starts_with(|c: char| c.is_ascii_alphabetic()))
    }

    fn eat_control(&mut self, name: &str) -> bool {
        if self.at_control(name) {
            self.rest = &self.rest[1 + name.len()..];
            true
        } else {
            false
        }
    }

    /// The whole source, recovering as TeX does: a token a row refused is
    /// shown in the error colour and the text goes on after it, instead of
    /// the whole source standing in for one broken part.
    fn document(&mut self) -> Node<'a> {
        let mut lines = self.row_nodes(Stop::Top);
        loop {
            if self.rest.is_empty() {
                break;
            }
            let separator = self.eat(r"\\");
            if separator || self.eat("\n") {
                if separator {
                    self.separator_extra();
                }
                lines.push(Vec::new());
                lines.append(&mut self.row_nodes(Stop::Top));
                continue;
            }
            // The only token a top-level row stops before: a stray `}`.
            let start = self.rest;
            let _ = self.next();
            let line = lines.last_mut().unwrap();
            line.push(Node::Fallback(&start[..start.len() - self.rest.len()]));
            for mut more in self.row_nodes(Stop::Top) {
                line.append(&mut more);
            }
        }
        if lines.len() == 1 {
            Node::Row(lines.pop().unwrap())
        } else {
            Node::Lines(lines.into_iter().map(Node::Row).collect(), false)
        }
    }

    /// The star and bracketed length that may follow a `\\` separator. An
    /// unbalanced bracket stays visible instead of taking the rest of the
    /// source with it.
    fn separator_extra(&mut self) {
        self.eat("*");
        let before = self.rest;
        if self.raw_option().is_err() {
            self.rest = before;
        }
    }

    fn at_end(&self, stop: Stop) -> bool {
        self.rest.is_empty()
            || match stop {
                Stop::Top => self.rest.starts_with(['\n', '}']) || self.rest.starts_with(r"\\"),
                Stop::Brace => self.rest.starts_with('}'),
                Stop::Lines => self.rest.starts_with('}') || self.rest.starts_with(r"\\"),
                Stop::Bracket => self.rest.starts_with(']'),
                Stop::Fence => self.at_control("right"),
                Stop::Grid => {
                    self.rest.starts_with('&')
                        || self.rest.starts_with(r"\\")
                        || self.at_control("end")
                        || self.at_control("cr")
                }
                Stop::Dollar => self.rest.starts_with('$'),
                Stop::Inline => self.rest.starts_with(r"\)"),
                Stop::Display => self.rest.starts_with(r"\]"),
            }
    }

    fn row(&mut self, stop: Stop) -> Node<'a> {
        let mut lines = self.row_nodes(stop);
        if lines.len() == 1 {
            Node::Row(lines.pop().unwrap())
        } else {
            Node::Lines(lines.into_iter().map(Node::Row).collect(), false)
        }
    }

    /// The lines of one row. What could not be parsed is kept as a
    /// [`Node::Fallback`] of the tokens it consumed, and the row goes on
    /// after it, as TeX reports an error and reads on. In text mode a `\\`
    /// the enclosing stop does not claim (an array, `align` or the document
    /// level claim theirs) breaks the line, as it does in running text.
    fn row_nodes(&mut self, stop: Stop) -> Vec<Vec<Node<'a>>> {
        match self.nested(|p| {
            let mut lines: Vec<Vec<Node<'a>>> = vec![Vec::new()];
            while !p.at_end(stop) {
                let Some(c) = p.peek() else {
                    break;
                };
                let nodes = lines.last_mut().unwrap();
                if c.is_whitespace() {
                    p.next()?;
                    if p.text {
                        nodes.push(Node::Char(' ', Class::Ordinary));
                    }
                } else if p.text && p.eat(r"\\") {
                    lines.push(Vec::new());
                } else if !p.text && matches!(c, '_' | '^' | '\'') {
                    let base = nodes.pop().unwrap_or_else(|| Node::Row(Vec::new()));
                    nodes.push(p.scripts(base)?);
                } else if !p.text
                    && (p.at_control("limits")
                        || p.at_control("nolimits")
                        || p.at_control("displaylimits"))
                {
                    let base = nodes.pop().unwrap_or_else(|| Node::Row(Vec::new()));
                    nodes.push(p.scripts(base)?);
                } else {
                    let before = p.rest;
                    match p.atom() {
                        Ok(node) => nodes.push(node),
                        Err(()) => {
                            nodes.push(Node::Fallback(&before[..before.len() - p.rest.len()]))
                        }
                    }
                }
            }
            Ok(lines)
        }) {
            Ok(lines) => lines,
            // Only the nesting limit lands here: the rest of the row is
            // shown as it stands.
            Err(()) => {
                let rest = std::mem::take(&mut self.rest);
                vec![vec![Node::Fallback(rest)]]
            }
        }
    }

    fn display_lines(&mut self) -> Result<Node<'a>> {
        self.space();
        if !self.eat("{") {
            return Err(());
        }
        let mut lines = vec![self.row(Stop::Lines)];
        while self.eat(r"\\") {
            self.separator_extra();
            lines.push(self.row(Stop::Lines));
        }
        // A missing closing brace is supplied, as TeX does.
        if self.eat("}") {
            Ok(Node::Lines(lines, true))
        } else {
            Ok(Node::Row(vec![Node::Lines(lines, true), Node::Broken]))
        }
    }

    fn group(&mut self, stop: Stop, close: &str) -> Node<'a> {
        let node = self.row(stop);
        // A missing closing token is supplied, as TeX does; the formula is
        // flagged as partly broken.
        if self.eat(close) {
            node
        } else {
            Node::Row(vec![node, Node::Broken])
        }
    }

    fn arg(&mut self) -> Node<'a> {
        self.space();
        match self.nested(Self::atom) {
            Ok(node) => node,
            // A missing or unusable argument: its empty place, in the error
            // colour.
            Err(()) => Node::Error,
        }
    }

    fn text_arg(&mut self) -> Node<'a> {
        let old = std::mem::replace(&mut self.text, true);
        let result = self.arg();
        self.text = old;
        result
    }

    fn option(&mut self) -> Result<Option<Box<Node<'a>>>> {
        self.space();
        if self.eat("[") {
            Ok(Some(Box::new(self.group(Stop::Bracket, "]"))))
        } else {
            Ok(None)
        }
    }

    fn raw_group(&mut self) -> Result<&'a str> {
        self.space();
        if !self.eat("{") {
            return Err(());
        }
        self.raw_until('{', '}')
    }

    fn raw_until(&mut self, open: char, close: char) -> Result<&'a str> {
        let start = self.rest;
        let mut level = 1;
        while let Ok(c) = self.next() {
            if c == close {
                level -= 1;
                if level == 0 {
                    return Ok(&start[..start.len() - self.rest.len() - c.len_utf8()]);
                }
            } else if c == open {
                level += 1;
            } else if c == '\\' {
                self.next()?;
            }
        }
        Err(())
    }

    fn atom(&mut self) -> Result<Node<'a>> {
        let start = self.rest;
        Ok(match self.next()? {
            '{' => self.group(Stop::Brace, "}"),
            '}' => return Err(()),
            '\\' => {
                let name = self.control()?;
                self.command(name, start)?
            }
            '$' => {
                let double = self.eat("$");
                let old = std::mem::replace(&mut self.text, false);
                let node = self.group(Stop::Dollar, if double { "$$" } else { "$" });
                self.text = old;
                Node::Variant(Variant::Upright, Box::new(node))
            }
            '~' if !self.text => Node::Space(Length::Em(0.33)),
            '-' if !self.text => Node::Char('−', Class::Binary),
            c => Node::Char(c, if self.text { Class::Ordinary } else { class(c) }),
        })
    }

    fn scripts(&mut self, base: Node<'a>) -> Result<Node<'a>> {
        let (base, mut sub, mut sup, mut limits) = match base {
            Node::Script {
                base,
                sub,
                sup,
                limits,
            } => (base, sub, sup, limits),
            other => (Box::new(other), None, None, None),
        };
        if self.eat_control("limits") {
            limits = Some(true);
        } else if self.eat_control("nolimits") {
            limits = Some(false);
        } else if self.eat_control("displaylimits") {
            limits = None;
        } else if self.eat("_") {
            if sub.is_some() {
                return Err(());
            }
            sub = Some(Box::new(self.arg()));
        } else if self.eat("^") {
            if sup.is_some() {
                return Err(());
            }
            sup = Some(Box::new(self.arg()));
        } else if self.eat("'") {
            let mut primes = vec![Node::Char('′', Class::Ordinary)];
            while self.eat("'") {
                primes.push(Node::Char('′', Class::Ordinary));
            }
            if let Some(old) = sup.take() {
                primes.insert(0, *old);
            }
            self.space();
            if self.eat("^") {
                primes.push(self.arg());
            }
            sup = Some(Box::new(Node::Row(primes)));
        }
        Ok(Node::Script {
            base,
            sub,
            sup,
            limits,
        })
    }

    fn delimiter(&mut self) -> Result<char> {
        self.space();
        let ch = self.next()?;
        if ch != '\\' {
            return ".,()[]{}|/⟨⟩⌈⌉⌊⌋‖".contains(ch).then_some(ch).ok_or(());
        }
        Ok(match self.control()? {
            "{" | "lbrace" => '{',
            "}" | "rbrace" => '}',
            "[" | "lbrack" => '[',
            "]" | "rbrack" => ']',
            "langle" => '⟨',
            "rangle" => '⟩',
            "lfloor" => '⌊',
            "rfloor" => '⌋',
            "lceil" => '⌈',
            "rceil" => '⌉',
            "vert" | "mid" | "lvert" | "rvert" => '|',
            "Vert" | "|" | "lVert" | "rVert" => '‖',
            "backslash" => '\\',
            "uparrow" => '↑',
            "downarrow" => '↓',
            "updownarrow" => '↕',
            "Uparrow" => '⇑',
            "Downarrow" => '⇓',
            "Updownarrow" => '⇕',
            _ => return Err(()),
        })
    }

    fn unsupported(&mut self, start: &'a str) -> Result<Node<'a>> {
        let mut end = self.rest;
        loop {
            self.space();
            if self.eat("{") {
                self.raw_until('{', '}')?;
            } else if self.eat("[") {
                self.raw_until('[', ']')?;
            } else {
                self.rest = end;
                break;
            }
            end = self.rest;
        }
        Ok(Node::Fallback(&start[..start.len() - self.rest.len()]))
    }
}

pub(crate) fn class(c: char) -> Class {
    use Class::*;
    match c {
        '+' | '-' | '−' | '±' | '∓' | '·' | '×' | '÷' | '∪' | '∩' | '∧' | '∨' | '∖' | '⊕' | '⊗'
        | '⊙' | '∗' | '∘' | '⋅' => Binary,
        '=' | '<' | '>' | '≤' | '≥' | '≠' | '≡' | '≈' | '≃' | '∼' | '∝' | '∈' | '∉' | '∋' | '⊂'
        | '⊃' | '⊆' | '⊇' | '⊥' | '∥' | '∣' | '≟' | '→' | '←' | '↔' | '⇒' | '⇐' | '⇔' => {
            Relation
        }
        '(' | '[' | '{' | '⟨' | '⌊' | '⌈' => Open,
        ')' | ']' | '}' | '⟩' | '⌋' | '⌉' => Close,
        ',' | ';' => Punctuation,
        _ => Ordinary,
    }
}
