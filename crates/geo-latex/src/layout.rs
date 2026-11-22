use crate::font;
use crate::parser::{Accent, Class, Node, Variant};
use crate::{Color, Face, Ink, Layout, Point, Shape, Style, Transform, FALLBACK_COLOR};

mod decorations;
mod grid;
mod math;

#[derive(Clone, Copy)]
struct Env {
    base: f32,
    style: Style,
    face: Face,
    upright: bool,
    small_caps: bool,
    color: Option<Color>,
}

impl Env {
    fn em(self) -> f32 {
        self.base
            * match self.style {
                Style::Display | Style::Text => 1.0,
                Style::Script => 0.7,
                Style::ScriptScript => 0.5,
            }
    }

    fn style(self, style: Style) -> Self {
        Self { style, ..self }
    }

    fn script(self) -> Self {
        self.style(match self.style {
            Style::Display | Style::Text => Style::Script,
            Style::Script | Style::ScriptScript => Style::ScriptScript,
        })
    }

    fn fraction(self) -> Self {
        if self.style == Style::Display {
            self.style(Style::Text)
        } else {
            self.script()
        }
    }

    fn rule(self) -> f32 {
        (self.em() * 0.048).max(0.55)
    }

    fn axis(self) -> f32 {
        0.25 * self.em()
    }

    fn variant(self, variant: Variant) -> Self {
        let mut env = self;
        match variant {
            Variant::Text | Variant::Upright => {
                env.upright = true;
                env.face.italic = false;
            }
            Variant::Italic => env.face.italic = true,
            Variant::SmallCaps => {
                env.upright = true;
                env.small_caps = true;
            }
            Variant::Bold => {
                env.upright = true;
                env.face.bold = true;
            }
            Variant::Mono => {
                env.upright = true;
                env.face.monospace = true;
            }
        }
        env
    }

    fn glyph_face(self, ch: char) -> Face {
        let italic = self.face.italic
            || !self.upright
                && (ch.is_ascii_alphabetic() || ('\u{03b1}'..='\u{03ff}').contains(&ch));
        Face {
            italic,
            ..self.face
        }
    }
}

pub(super) fn layout(node: &Node<'_>, size: f32, style: Style, face: Face) -> Layout {
    let mut result = typeset(
        node,
        Env {
            base: size,
            style,
            face,
            upright: false,
            small_caps: false,
            color: None,
        },
    );
    result.width = result.width.max(0.0);
    result
}

impl Layout {
    fn item(&mut self, shape: Shape, origin: Point, color: Option<Color>) {
        self.ink.push(Ink {
            shape,
            transform: Transform {
                origin,
                ..Transform::default()
            },
            color,
        });
    }

    fn put(&mut self, mut other: Self, x: f32, y: f32) {
        self.width = self.width.max(x + other.width);
        self.height = self.height.max(other.height - y);
        self.depth = self.depth.max(other.depth + y);
        self.has_fallback |= other.has_fallback;
        for ink in &mut other.ink {
            ink.transform.origin[0] += x;
            ink.transform.origin[1] += y;
        }
        self.ink.append(&mut other.ink);
    }

    fn stack(&mut self, other: Self, x: f32, y: f32) {
        self.put(other, x, y);
        self.width = self.width.max(0.0);
    }

    fn push(&mut self, other: Self) {
        let end = self.width + other.width;
        self.put(other, self.width, 0.0);
        self.width = end;
    }

    fn stroke(&mut self, points: Vec<Point>, env: Env) {
        self.item(
            Shape::Stroke {
                points,
                width: env.rule(),
            },
            [0.0, 0.0],
            env.color,
        );
    }

    fn rect(&mut self, [x0, y0]: Point, [x1, y1]: Point, color: Option<Color>) {
        self.item(
            Shape::Fill {
                points: vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]],
            },
            [0.0, 0.0],
            color,
        );
    }

    fn background(&mut self, width: f32, height: f32, depth: f32, color: Color) {
        self.rect([0.0, -height], [width, depth], Some(color));
        self.ink.rotate_right(1);
    }

    fn transformed(mut self, mut transform: Transform) -> Self {
        let extent = self.width.max(self.height + self.depth).max(1.0);
        let spread = transform
            .x
            .into_iter()
            .chain(transform.y)
            .map(f32::abs)
            .fold(1.0, f32::max);
        let shrink = (1_000_000.0 / (extent * spread)).min(1.0);
        transform.x = transform.x.map(|n| n * shrink);
        transform.y = transform.y.map(|n| n * shrink);
        let corners = [
            [0.0, -self.height],
            [self.width, -self.height],
            [0.0, self.depth],
            [self.width, self.depth],
        ];
        let (mut left, mut right, mut top, mut bottom) =
            (f32::INFINITY, f32::NEG_INFINITY, 0.0_f32, 0.0_f32);
        for corner in corners {
            let [x, y] = transform.point(corner);
            left = left.min(x);
            right = right.max(x);
            top = top.min(y);
            bottom = bottom.max(y);
        }
        transform.origin[0] -= left;
        for ink in &mut self.ink {
            ink.transform = transform.compose(ink.transform);
        }
        self.width = right - left;
        self.height = -top;
        self.depth = bottom;
        self
    }
}

fn glyphs(text: String, face: Face, env: Env) -> Layout {
    let size = env.em();
    let (width, height, depth) = font::measure(&text, size, face);
    let mut result = Layout {
        width,
        height,
        depth,
        ..Layout::default()
    };
    let text = if text.chars().any(|ch| font::bundled_symbol(ch) != ch) {
        text.chars().map(font::bundled_symbol).collect()
    } else {
        text
    };
    result.item(Shape::Glyphs { text, size, face }, [0.0, 0.0], env.color);
    result
}

fn typeset(node: &Node<'_>, env: Env) -> Layout {
    match node {
        Node::Char(ch, _) if font::drawn_symbol(*ch) => decorations::symbol(*ch, env),
        Node::Char(ch, _) if env.small_caps && ch.is_lowercase() => glyphs(
            ch.to_uppercase().collect(),
            env.glyph_face(*ch),
            Env {
                base: env.base * 0.8,
                ..env
            },
        ),
        Node::Char(ch, _) => glyphs(ch.to_string(), env.glyph_face(*ch), env),
        Node::Word(word, _) => glyphs((*word).to_owned(), env.face, env),
        Node::Row(nodes) => row(nodes, env),
        Node::Lines(nodes, centered) => {
            let lines: Vec<_> = nodes.iter().map(|node| typeset(node, env)).collect();
            let width = lines.iter().map(|b| b.width).fold(0.0_f32, f32::max);
            let mut result = Layout {
                width,
                ..Layout::default()
            };
            for (i, line) in lines.into_iter().enumerate() {
                let y = if i == 0 {
                    0.0
                } else {
                    result.depth + 0.25 * env.em() + line.height
                };
                let x = if *centered {
                    (width - line.width) / 2.0
                } else {
                    0.0
                };
                result.stack(line, x, y);
                result.depth = result.depth.max(y + 0.2 * env.em());
            }
            result
        }
        Node::Space(amount) => Layout {
            width: amount.points(env.em()),
            ..Layout::default()
        },
        Node::VerticalSpace(amount) => Layout {
            height: amount.points(env.em()).max(0.0),
            ..Layout::default()
        },
        Node::Fraction(parts, style, binomial) => {
            math::fraction(parts, env.style(style.unwrap_or(env.style)), *binomial)
        }
        Node::Root(body, index) => decorations::radical(body, index.as_deref(), env),
        Node::Script {
            base,
            sub,
            sup,
            limits,
        } => math::scripts(base, sub.as_deref(), sup.as_deref(), *limits, env),
        Node::Operator(ch, _) => {
            let em = env.em();
            let large = if env.style == Style::Display {
                1.45
            } else {
                1.0
            };
            let op_env = Env {
                base: env.base * large,
                ..env
            };
            let mut op = if *ch == '∮' && !font::has_symbol(*ch) {
                decorations::symbol(*ch, op_env)
            } else {
                glyphs(ch.to_string(), env.face, op_env)
            };
            let y = (op.height - op.depth) / 2.0 - env.axis();
            for ink in &mut op.ink {
                ink.transform.origin[1] += y;
            }
            op.height = (op.height - y).max(0.0);
            op.depth = (op.depth + y).max(0.0);
            op.width += em * 0.04;
            op
        }
        Node::Delimited(left, body, right) => {
            let content = match body.as_ref() {
                Node::Row(nodes) => fenced_row(nodes, env),
                _ => typeset(body, env),
            };
            decorations::fence(*left, content, *right, env)
        }
        Node::SizedDelimiter(ch, size, _) => decorations::delimiter(*ch, env.em() * size, env),
        Node::Grid(table) => grid::table(table, env),
        Node::Variant(variant, body) => typeset(body, env.variant(*variant)),
        Node::Styled(style, body) => body
            .as_deref()
            .map_or_else(Layout::default, |body| typeset(body, env.style(*style))),
        Node::SwitchColor(_) | Node::SwitchVariant(_) => Layout::default(),
        Node::Accent(accent, body) => decorations::accent(*accent, body, env),
        Node::Arrow(accent, above, below) => {
            decorations::labeled_arrow(*accent, above, below.as_deref(), env)
        }
        Node::Color(color, body, background) => {
            if *background {
                let mut content = typeset(body, env);
                content.background(content.width, content.height, content.depth, *color);
                content
            } else {
                typeset(
                    body,
                    Env {
                        color: Some(*color),
                        ..env
                    },
                )
            }
        }
        Node::CellColor(_, body) | Node::Classified(_, body) => typeset(body, env),
        Node::Border(body) => decorations::border(body, env),
        Node::Rotate(angle, body) => {
            let (sin, cos) = angle.sin_cos();
            typeset(body, env).transformed(Transform {
                x: [cos, sin],
                y: [-sin, cos],
                ..Transform::default()
            })
        }
        Node::Scale(x, y, body) => typeset(body, env).transformed(Transform {
            x: [*x, 0.0],
            y: [0.0, *y],
            ..Transform::default()
        }),
        Node::Phantom(body, horizontal, vertical) => {
            let mut content = typeset(body, env);
            content.ink.clear();
            if *horizontal {
                content.height = 0.0;
                content.depth = 0.0;
            }
            if *vertical {
                content.width = 0.0;
            }
            content
        }
        Node::Fallback(source) => {
            let mut out = Layout::default();
            for (i, line) in source.lines().enumerate() {
                let text = glyphs(
                    line.to_owned(),
                    Face {
                        monospace: true,
                        ..Face::default()
                    },
                    Env {
                        color: Some(FALLBACK_COLOR),
                        ..env
                    },
                );
                out.stack(text, 0.0, i as f32 * env.em() * 1.2);
            }
            out.has_fallback = true;
            out
        }
    }
}

fn atom_class(node: &Node<'_>) -> Option<Class> {
    use Class::{Inner, Operator, Ordinary};
    Some(match node {
        Node::Char(_, class)
        | Node::Word(_, class)
        | Node::SizedDelimiter(_, _, class)
        | Node::Classified(class, _) => *class,
        Node::Operator(..) => Operator,
        Node::Fraction(..) | Node::Delimited(..) | Node::Grid(..) => Inner,
        Node::Script { base, .. } => return atom_class(base),
        Node::Color(_, body, _) | Node::Styled(_, Some(body)) | Node::Variant(_, body) => {
            return atom_class(body);
        }
        Node::Arrow(..) => Class::Relation,
        Node::Space(_)
        | Node::VerticalSpace(_)
        | Node::Styled(_, None)
        | Node::SwitchColor(_)
        | Node::SwitchVariant(_) => return None,
        _ => Ordinary,
    })
}

fn glue(left: Option<Class>, right: Class, env: Env) -> f32 {
    // TeX's Ord, Op, Bin, Rel, Open, Close, Punct, Inner spacing table.
    const TABLE: [[u8; 8]; 8] = [
        [0, 1, 2, 3, 0, 0, 0, 4],
        [1, 1, 0, 3, 0, 0, 0, 4],
        [2, 2, 0, 0, 2, 0, 0, 2],
        [3, 3, 0, 0, 3, 0, 0, 3],
        [0, 0, 0, 0, 0, 0, 0, 0],
        [0, 1, 2, 3, 0, 0, 0, 4],
        [4, 4, 0, 4, 4, 4, 4, 4],
        [4, 1, 2, 3, 4, 0, 4, 4],
    ];
    match left.map_or(0, |lhs| TABLE[lhs as usize][right as usize]) {
        1 => env.em() * (3.0 / 18.0),
        n if matches!(env.style, Style::Script | Style::ScriptScript) || n == 0 => 0.0,
        2 => env.em() * (4.0 / 18.0),
        3 => env.em() * (5.0 / 18.0),
        _ => env.em() * (3.0 / 18.0),
    }
}

fn row(nodes: &[Node<'_>], env: Env) -> Layout {
    row_pieces(nodes, env, false).0
}

type Middle = (usize, f32, char, Env);

fn fenced_row(nodes: &[Node<'_>], env: Env) -> Layout {
    let (mut result, middles) = row_pieces(nodes, env, true);
    if middles.is_empty() {
        return result;
    }
    let span = decorations::delimiter_span(&result, env);
    let content_width = result.width;
    let mut ink = std::mem::take(&mut result.ink).into_iter();
    let (mut shift, mut used) = (0.0, 0);
    for (index, x, ch, env) in middles {
        for mut item in ink.by_ref().take(index - used) {
            item.transform.origin[0] += shift;
            result.ink.push(item);
        }
        used = index;
        let delimiter = decorations::delimiter(ch, span, env);
        let width = delimiter.width;
        result.stack(delimiter, x + shift, 0.0);
        shift += width;
    }
    result.ink.extend(ink.map(|mut item| {
        item.transform.origin[0] += shift;
        item
    }));
    result.width = content_width + shift;
    result
}

fn row_pieces(nodes: &[Node<'_>], mut env: Env, fenced: bool) -> (Layout, Vec<Middle>) {
    use Class::*;
    let mut result = Layout::default();
    let mut middles = Vec::new();
    let mut it = nodes.iter().peekable();
    let mut left = None;
    while let Some(node) = it.next() {
        match node {
            Node::Styled(style, None) => {
                env.style = *style;
                continue;
            }
            Node::SwitchColor(color) => {
                env.color = Some(*color);
                continue;
            }
            Node::SwitchVariant(variant) => {
                env = env.variant(*variant);
                continue;
            }
            _ => {}
        }
        let Some(mut class) = atom_class(node) else {
            result.push(typeset(node, env));
            continue;
        };
        if class == Binary
            && (matches!(
                left,
                None | Some(Binary | Operator | Relation | Open | Punctuation)
            ) || matches!(
                it.clone().find_map(atom_class),
                None | Some(Relation | Close | Punctuation)
            ))
        {
            class = Ordinary;
        }
        result.width += glue(left, class, env);
        if let (Node::Char(ch, Ordinary), false) = (node, env.small_caps) {
            if font::drawn_symbol(*ch) {
                result.push(decorations::symbol(*ch, env));
            } else {
                let face = env.glyph_face(*ch);
                let mut run = String::from(*ch);
                while let Some(Node::Char(ch, Ordinary)) = it.peek() {
                    if env.glyph_face(*ch) != face || font::drawn_symbol(*ch) {
                        break;
                    }
                    run.push(*ch);
                    it.next();
                }
                result.push(glyphs(run, face, env));
            }
        } else if let (true, Node::SizedDelimiter(ch, 1.0, _)) = (fenced, node) {
            middles.push((result.ink.len(), result.width, *ch, env));
        } else {
            result.push(typeset(node, env));
        }
        left = Some(class);
    }
    (result, middles)
}
