use super::colors::{color, named_color};
use super::symbols::symbol;
use super::{class, Accent, Class, Length, Node, Parser, Result, Stop, Variant};
use crate::{Color, Style};

impl<'a> Parser<'a> {
    pub(super) fn command(&mut self, name: &'a str, start: &'a str) -> Result<Node<'a>> {
        use Class::{Close, Open, Operator, Punctuation, Relation};
        Ok(match name {
            "frac" | "dfrac" | "tfrac" | "binom" | "dbinom" | "tbinom" => {
                let style = match name.as_bytes()[0] {
                    b'd' => Some(Style::Display),
                    b't' => Some(Style::Text),
                    _ => None,
                };
                Node::Fraction(
                    Box::new([self.arg()?, self.arg()?]),
                    style,
                    name.ends_with("binom"),
                )
            }
            "sqrt" => {
                let index = self.option()?;
                Node::Root(Box::new(self.arg()?), index)
            }
            "left" => {
                let left = self.delimiter()?;
                let child = self.row(Stop::Fence)?;
                if !self.eat_control("right") {
                    return Err(());
                }
                Node::Delimited(left, Box::new(child), self.delimiter()?)
            }
            "middle" => Node::SizedDelimiter(self.delimiter()?, 1.0, Relation),
            "big" | "Big" | "bigg" | "Bigg" | "bigl" | "Bigl" | "biggl" | "Biggl" | "bigr"
            | "Bigr" | "biggr" | "Biggr" | "bigm" | "Bigm" | "biggm" | "Biggm" => {
                let size = 1.2
                    + if name.starts_with('B') { 0.35 } else { 0.0 }
                    + if name.contains("gg") { 0.7 } else { 0.0 };
                Node::SizedDelimiter(
                    self.delimiter()?,
                    size,
                    match name.chars().last() {
                        Some('l') => Open,
                        Some('r') => Close,
                        _ => Relation,
                    },
                )
            }
            "begin" => {
                let environment = self.raw_group()?;
                if let Some(grid) = self.environment(environment)? {
                    grid
                } else {
                    self.unsupported_environment(environment, start)?
                }
            }
            "text" | "textrm" | "textnormal" | "textsf" | "mbox" => {
                self.variant(Variant::Text, true)?
            }
            "textit" | "emph" => self.variant(Variant::Italic, true)?,
            "textbf" => self.variant(Variant::Bold, true)?,
            "textsc" => self.variant(Variant::SmallCaps, true)?,
            "texttt" => self.variant(Variant::Mono, true)?,
            "mathrm" | "mathsf" => self.variant(Variant::Upright, false)?,
            "mathit" => self.variant(Variant::Italic, false)?,
            "mathbf" | "boldsymbol" | "bm" => self.variant(Variant::Bold, false)?,
            "mathtt" => self.variant(Variant::Mono, false)?,
            "rm" | "sf" => Node::SwitchVariant(Variant::Upright),
            "it" => Node::SwitchVariant(Variant::Italic),
            "bf" => Node::SwitchVariant(Variant::Bold),
            "tt" => Node::SwitchVariant(Variant::Mono),
            "sc" => Node::SwitchVariant(Variant::SmallCaps),
            "displaylines" => self.display_lines()?,
            "operatorname" => {
                self.eat("*");
                Node::Classified(Operator, Box::new(self.variant(Variant::Text, true)?))
            }
            "sin" | "cos" | "tan" | "sec" | "csc" | "cot" | "arcsin" | "arccos" | "arctan"
            | "arccot" | "sinh" | "cosh" | "tanh" | "coth" | "sech" | "csch" | "log" | "ln"
            | "lg" | "exp" | "lim" | "liminf" | "limsup" | "min" | "max" | "inf" | "sup"
            | "arg" | "gcd" | "det" | "deg" | "dim" | "ker" | "hom" | "Pr" | "mod" | "bmod" => {
                Node::Word(name, Operator)
            }
            "sum" | "prod" | "coprod" | "bigcup" | "bigcap" | "bigvee" | "bigwedge"
            | "bigoplus" | "bigotimes" | "int" | "iint" | "iiint" | "oint" => {
                let glyph = match name {
                    "sum" => '∑',
                    "prod" => '∏',
                    "coprod" => '∐',
                    "bigcup" => '⋃',
                    "bigcap" => '⋂',
                    "bigvee" => '∨',
                    "bigwedge" => '∧',
                    "bigoplus" => '⊕',
                    "bigotimes" => '⊗',
                    "int" => '∫',
                    "iint" => '∬',
                    "iiint" => '∭',
                    _ => '∮',
                };
                Node::Operator(glyph, !matches!(name, "int" | "iint" | "iiint" | "oint"))
            }
            "overline" | "bar" => self.accent(Accent::Bar)?,
            "underline" => self.accent(Accent::Under)?,
            "vec" | "overrightarrow" | "underrightarrow" => {
                self.accent(Accent::Right(name.starts_with("under")))?
            }
            "overleftarrow" | "underleftarrow" => {
                self.accent(Accent::Left(name.starts_with("under")))?
            }
            "overleftrightarrow" | "underleftrightarrow" => {
                self.accent(Accent::Both(name.starts_with("under")))?
            }
            "xrightarrow" | "xleftarrow" | "xleftrightarrow" => {
                let below = self.option()?;
                let accent = match name {
                    "xrightarrow" => Accent::Right(false),
                    "xleftarrow" => Accent::Left(false),
                    _ => Accent::Both(false),
                };
                Node::Arrow(accent, Box::new(self.arg()?), below)
            }
            "overset" | "underset" | "stackrel" => {
                let mark = Some(Box::new(self.arg()?));
                let base = Box::new(self.arg()?);
                let (sub, sup) = if name == "underset" {
                    (mark, None)
                } else {
                    (None, mark)
                };
                let script = Node::Script {
                    base,
                    sub,
                    sup,
                    limits: Some(true),
                };
                if name == "stackrel" {
                    Node::Classified(Relation, Box::new(script))
                } else {
                    script
                }
            }
            "hat" | "widehat" => self.accent(Accent::Hat)?,
            "tilde" | "widetilde" => self.accent(Accent::Tilde)?,
            "dot" => self.accent(Accent::Dot)?,
            "ddot" => self.accent(Accent::DoubleDot)?,
            "overbrace" | "underbrace" => self.accent(Accent::Brace(name == "overbrace"))?,
            "mathord" | "mathop" | "mathbin" | "mathrel" | "mathopen" | "mathclose"
            | "mathpunct" | "mathinner" => {
                let class = match name {
                    "mathop" => Operator,
                    "mathbin" => Class::Binary,
                    "mathrel" => Relation,
                    "mathopen" => Open,
                    "mathclose" => Close,
                    "mathpunct" => Punctuation,
                    "mathinner" => Class::Inner,
                    _ => Class::Ordinary,
                };
                Node::Classified(class, Box::new(self.arg()?))
            }
            "displaystyle" | "textstyle" | "scriptstyle" | "scriptscriptstyle" => {
                let style = match name {
                    "displaystyle" => Style::Display,
                    "textstyle" => Style::Text,
                    "scriptstyle" => Style::Script,
                    _ => Style::ScriptScript,
                };
                Node::Styled(style, None)
            }
            "[" | "(" => {
                let display = name == "[";
                let (stop, close) = if display {
                    (Stop::Display, r"\]")
                } else {
                    (Stop::Inline, r"\)")
                };
                let style = if display { Style::Display } else { Style::Text };
                Node::Styled(style, Some(Box::new(self.group(stop, close)?)))
            }
            "," | "thinspace" => Node::Space(Length::Em(3.0 / 18.0)),
            ":" | ">" | "medspace" => Node::Space(Length::Em(4.0 / 18.0)),
            ";" | "thickspace" => Node::Space(Length::Em(5.0 / 18.0)),
            "!" | "negthinspace" => Node::Space(Length::Em(-3.0 / 18.0)),
            " " | "nbsp" | "~" | "enspace" | "enskip" => Node::Space(Length::Em(0.33)),
            "quad" => Node::Space(Length::Em(1.0)),
            "qquad" => Node::Space(Length::Em(2.0)),
            "hspace" | "kern" | "mkern" | "vspace" => {
                self.eat("*");
                let raw = if self.rest.trim_start().starts_with('{') {
                    self.raw_group()?
                } else {
                    self.dimension_token()
                };
                if let Some(value) = dimension(raw) {
                    if name == "vspace" {
                        Node::VerticalSpace(value)
                    } else {
                        Node::Space(value)
                    }
                } else {
                    Node::Fallback(&start[..start.len() - self.rest.len()])
                }
            }
            "definecolor" => {
                let name = self.raw_group()?.trim();
                let model = self.raw_group()?.trim();
                let raw = self.raw_group()?;
                if let Some(color) = color(Some(model), raw) {
                    self.colors.insert(name, color);
                    Node::Space(Length::Em(0.0))
                } else {
                    Node::Fallback(&start[..start.len() - self.rest.len()])
                }
            }
            "color" => {
                let model = self.raw_option()?;
                let raw = self.raw_group()?;
                if let Some(color) = self.resolve_color(model, raw) {
                    Node::SwitchColor(color)
                } else {
                    Node::Fallback(&start[..start.len() - self.rest.len()])
                }
            }
            "textcolor" | "fgcolor" | "bgcolor" | "colorbox" => {
                let model = self.raw_option()?;
                let raw = self.raw_group()?;
                let child = Box::new(self.arg()?);
                if let Some(color) = self.resolve_color(model, raw) {
                    Node::Color(color, child, matches!(name, "bgcolor" | "colorbox"))
                } else {
                    Node::Fallback(&start[..start.len() - self.rest.len()])
                }
            }
            "rotatebox" => {
                let angle = number(self.raw_group()?).map(|n| n.rem_euclid(360.0));
                let child = Box::new(self.arg()?);
                if let Some(angle) = angle {
                    Node::Rotate(-angle.to_radians(), child)
                } else {
                    Node::Fallback(&start[..start.len() - self.rest.len()])
                }
            }
            "scalebox" => {
                let x = number(self.raw_group()?);
                let y = self.raw_option()?.map(number).unwrap_or(x);
                let child = Box::new(self.arg()?);
                if let (Some(x), Some(y)) = (x, y) {
                    Node::Scale(x.clamp(-16.0, 16.0), y.clamp(-16.0, 16.0), child)
                } else {
                    Node::Fallback(&start[..start.len() - self.rest.len()])
                }
            }
            "fbox" | "boxed" => Node::Border(Box::new(if name == "fbox" {
                self.text_arg()?
            } else {
                self.arg()?
            })),
            "phantom" | "hphantom" | "vphantom" => Node::Phantom(
                Box::new(self.arg()?),
                name == "hphantom",
                name == "vphantom",
            ),
            "over" | "atop" | "choose" | "end" | "right" => return Err(()),
            "nonumber" | "notag" => Node::Space(Length::Em(0.0)),
            _ => {
                if let Some(glyph) = symbol(name) {
                    if crate::font::has_symbol(glyph) || crate::font::drawn_symbol(glyph) {
                        Node::Char(
                            glyph,
                            if self.text {
                                Class::Ordinary
                            } else {
                                class(glyph)
                            },
                        )
                    } else {
                        Node::Fallback(&start[..start.len() - self.rest.len()])
                    }
                } else if let Some(color) = named_color(name) {
                    Node::Color(color, Box::new(self.arg()?), false)
                } else {
                    self.unsupported(start)?
                }
            }
        })
    }

    pub(super) fn resolve_color(&self, model: Option<&str>, raw: &str) -> Option<Color> {
        if model.is_none() {
            if let Some(color) = self.colors.get(raw.trim()) {
                return Some(*color);
            }
        }
        color(model, raw)
    }

    pub(super) fn raw_option(&mut self) -> Result<Option<&'a str>> {
        self.space();
        if self.eat("[") {
            Ok(Some(self.raw_until('[', ']')?))
        } else {
            Ok(None)
        }
    }

    fn dimension_token(&mut self) -> &'a str {
        self.space();
        let start = self.rest;
        self.rest = self
            .rest
            .trim_start_matches(|c: char| c.is_ascii_alphanumeric() || "+-.".contains(c));
        &start[..start.len() - self.rest.len()]
    }

    fn variant(&mut self, variant: Variant, text: bool) -> Result<Node<'a>> {
        let child = if text { self.text_arg()? } else { self.arg()? };
        Ok(Node::Variant(variant, Box::new(child)))
    }

    fn accent(&mut self, accent: Accent) -> Result<Node<'a>> {
        Ok(Node::Accent(accent, Box::new(self.arg()?)))
    }
}

pub(super) fn number(raw: &str) -> Option<f32> {
    raw.trim().parse::<f32>().ok().filter(|n| n.is_finite())
}

pub(super) fn dimension(raw: &str) -> Option<Length> {
    let raw = raw.trim();
    for (suffix, ems) in [("em", 1.0), ("ex", 0.48), ("mu", 1.0 / 18.0)] {
        if let Some(value) = raw.strip_suffix(suffix) {
            return Some(Length::Em((number(value)? * ems).clamp(-64.0, 64.0)));
        }
    }
    if let Some(value) = raw.strip_suffix("pt").or_else(|| raw.strip_suffix("px")) {
        return Some(Length::Points(number(value)?.clamp(-2048.0, 2048.0)));
    }
    Some(Length::Em(number(raw)?.clamp(-64.0, 64.0)))
}
