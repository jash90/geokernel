use super::{glyphs, typeset, Accent, Env, Layout, Node};
use crate::{Face, Transform};

pub(super) fn delimiter_span(body: &Layout, env: Env) -> f32 {
    let axis = env.axis();
    let half_span = (body.height - axis).max(body.depth + axis);
    env.em().max(2.0 * half_span + 0.12 * env.em())
}

pub(super) fn delimiter(ch: char, span: f32, env: Env) -> Layout {
    if ch == '.' {
        return Layout {
            width: 0.08 * env.em(),
            ..Layout::default()
        };
    }
    let em = env.em();
    let face = Face {
        italic: false,
        ..env.face
    };
    let glyph = glyphs(ch.to_string(), face, env);
    let ink_height = glyph.height + glyph.depth;
    let scale_y = span / ink_height.max(0.25 * em);
    let scale_x = scale_y.sqrt().clamp(0.9, 1.65);
    let offset = (glyph.height - glyph.depth) * scale_y / 2.0 - env.axis();
    let mut result = glyph.transformed(Transform {
        origin: [0.0, offset],
        x: [scale_x, 0.0],
        y: [0.0, scale_y],
    });
    result.width += 0.05 * em;
    result
}

pub(super) fn fence(left: char, body: Layout, right: char, env: Env) -> Layout {
    let span = delimiter_span(&body, env);
    let mut result = delimiter(left, span, env);
    result.push(body);
    result.push(delimiter(right, span, env));
    result
}

pub(super) fn radical(body: &Node<'_>, index: Option<&Node<'_>>, env: Env) -> Layout {
    let content = typeset(body, env);
    let em = env.em();
    let rule = env.rule();
    let bar = -content.height.max(0.45 * em) - 0.10 * em;
    let bottom = content.depth.max(0.04 * em);
    let index = index.map(|index| typeset(index, env.script().script()));
    let index_width = index.as_ref().map_or(0.0, |b| b.width);
    let start = (index_width - 0.32 * em).max(0.0);
    let join = start + 0.64 * em;
    let end = join + content.width + 0.14 * em;
    let mut result = Layout {
        width: end,
        height: -bar + rule / 2.0,
        depth: bottom + rule / 2.0,
        ..Layout::default()
    };
    result.stroke(
        vec![
            [start + 0.02 * em, -0.15 * em],
            [start + 0.15 * em, -0.20 * em],
            [start + 0.30 * em, bottom],
            [join, bar],
            [end, bar],
        ],
        env,
    );
    result.stack(content, join + 0.07 * em, 0.0);
    if let Some(index) = index {
        let y = (-0.36 * em).min(bar + 0.45 * em) - index.depth;
        result.stack(index, 0.0, y);
    }
    result
}

pub(super) fn accent(accent: Accent, body: &Node<'_>, env: Env) -> Layout {
    let mut content = typeset(body, env);
    let em = env.em();
    let w = content.width.max(0.28 * em);
    let top = -content.height - 0.12 * em;
    let bottom = content.depth + 0.12 * em;
    let thickness = env.rule();
    match accent {
        Accent::Under => {
            content.stroke(vec![[0.0, bottom], [w, bottom]], env);
            content.depth = bottom + thickness / 2.0;
        }
        Accent::Bar => {
            content.stroke(vec![[0.0, top], [w, top]], env);
            content.height = -top + thickness / 2.0;
        }
        Accent::Right(below) | Accent::Left(below) | Accent::Both(below) => {
            let tip = arrow(
                &mut content,
                accent,
                w,
                if below { bottom } else { top },
                env,
            );
            if below {
                content.depth = bottom + tip + thickness / 2.0;
            } else {
                content.height = -top + tip + thickness / 2.0;
            }
        }
        Accent::Hat | Accent::Tilde => {
            let y = 0.16 * em;
            let points = if matches!(accent, Accent::Hat) {
                vec![[0.0, top], [w * 0.5, top - y], [w, top]]
            } else {
                vec![
                    [0.0, top],
                    [w * 0.22, top - y],
                    [w * 0.46, top - y],
                    [w * 0.68, top],
                    [w * 0.82, top],
                    [w, top - y],
                ]
            };
            content.stroke(points, env);
            content.height = -top + y + thickness / 2.0;
        }
        Accent::Dot | Accent::DoubleDot => {
            let mid = w / 2.0;
            let centers: &[_] = if matches!(accent, Accent::Dot) {
                &[mid]
            } else {
                &[mid - 0.10 * em, mid + 0.10 * em]
            };
            for &x in centers {
                content.rect(
                    [x - thickness, top - thickness],
                    [x + thickness, top + thickness],
                    env.color,
                );
            }
            content.height = -top + thickness;
        }
        Accent::Brace(over) => {
            let span = w.max(0.7 * em);
            let s = if over { -1.0 } else { 1.0 };
            let y = if over { top } else { bottom };
            let h = 0.22 * em;
            let points = vec![
                [0.0, y],
                [0.08 * span, y + s * h * 0.55],
                [0.42 * span, y + s * h * 0.55],
                [0.5 * span, y + s * h],
                [0.58 * span, y + s * h * 0.55],
                [0.92 * span, y + s * h * 0.55],
                [span, y],
            ];
            content.stroke(points, env);
            if over {
                content.height = -top + h;
            } else {
                content.depth = bottom + h;
            }
            content.width = span;
        }
    }
    content.width = content.width.max(w);
    content
}

fn arrow(out: &mut Layout, accent: Accent, w: f32, y: f32, env: Env) -> f32 {
    let tip = (0.14 * env.em()).min(w * 0.35);
    let h = tip * 0.65;
    out.stroke(vec![[0.0, y], [w, y]], env);
    if matches!(accent, Accent::Right(_) | Accent::Both(_)) {
        out.stroke(vec![[w - tip, y - h], [w, y], [w - tip, y + h]], env);
    }
    if matches!(accent, Accent::Left(_) | Accent::Both(_)) {
        out.stroke(vec![[tip, y - h], [0.0, y], [tip, y + h]], env);
    }
    h
}

pub(super) fn labeled_arrow(
    accent: Accent,
    top: &Node<'_>,
    bottom: Option<&Node<'_>>,
    env: Env,
) -> Layout {
    let top = typeset(top, env.script());
    let bottom = bottom.map(|node| typeset(node, env.script()));
    let width = top
        .width
        .max(bottom.as_ref().map_or(0.0, |b| b.width))
        .max(0.5 * env.em())
        + 0.3 * env.em();
    let y = -env.axis();
    let mut result = Layout {
        width,
        ..Layout::default()
    };
    let padding = arrow(&mut result, accent, width, y, env) + env.rule();
    let (left, rise) = ((width - top.width) / 2.0, top.depth + padding);
    result.stack(top, left, y - rise);
    if let Some(bottom) = bottom {
        let (left, drop) = ((width - bottom.width) / 2.0, bottom.height + padding);
        result.stack(bottom, left, y + drop);
    }
    result
}

pub(super) fn border(body: &Node<'_>, env: Env) -> Layout {
    let child = typeset(body, env);
    let pad = 0.16 * env.em();
    let width = child.width + 2.0 * pad;
    let top = -child.height - pad;
    let bottom = child.depth + pad;
    let mut result = Layout {
        width,
        height: -top,
        depth: bottom,
        ..Layout::default()
    };
    result.stack(child, pad, 0.0);
    let l = env.rule() / 2.0;
    result.stroke(
        vec![
            [l, top + l],
            [width - l, top + l],
            [width - l, bottom - l],
            [l, bottom - l],
            [l, top + l],
        ],
        env,
    );
    result
}

pub(super) fn symbol(ch: char, env: Env) -> Layout {
    let em = env.em();
    match ch {
        'ℏ' => {
            let mut base = glyphs(
                "h".into(),
                Face {
                    italic: true,
                    ..env.face
                },
                env,
            );
            base.stroke(vec![[0.03 * em, -0.51 * em], [0.31 * em, -0.56 * em]], env);
            base
        }
        '∮' => {
            let mut base = glyphs("∫".into(), env.face, env);
            let center = [base.width / 2.0, (base.depth - base.height) / 2.0];
            let ring = (0..=16)
                .map(|i| {
                    let (sin, cos) = (i as f32 * std::f32::consts::TAU / 16.0).sin_cos();
                    [center[0] + cos * em * 0.17, center[1] + sin * em * 0.13]
                })
                .collect();
            base.stroke(ring, env);
            base
        }
        '⊥' => {
            let mut result = Layout {
                width: 0.68 * em,
                height: 0.61 * em,
                ..Layout::default()
            };
            result.stroke(vec![[0.04 * em, -0.03 * em], [0.64 * em, -0.03 * em]], env);
            result.stroke(vec![[0.34 * em, -0.03 * em], [0.34 * em, -0.58 * em]], env);
            result
        }
        '⋮' | '⋱' => {
            let size = env.rule() * 1.2;
            let diagonal = ch == '⋱';
            let width = em * if diagonal { 0.82 } else { 0.26 };
            let mut result = Layout {
                width,
                height: 0.60 * em,
                ..Layout::default()
            };
            for i in 0..3 {
                let x = if diagonal {
                    em * (0.12 + i as f32 * 0.28)
                } else {
                    width / 2.0
                };
                let y = em * (-0.54 + i as f32 * 0.23);
                result.rect([x - size, y - size], [x + size, y + size], env.color);
            }
            result
        }
        _ => unreachable!("symbol must have a vector substitute"),
    }
}
