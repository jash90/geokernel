use super::{decorations, typeset, Accent, Env, Layout, Node};
use crate::Style;

pub(super) fn fraction(parts: &[Node<'_>; 2], env: Env, binomial: bool) -> Layout {
    let em = env.em();
    let num = typeset(&parts[0], env.fraction());
    let den = typeset(&parts[1], env.fraction());
    let width = num.width.max(den.width) + 0.24 * em;
    let clearance = if env.style == Style::Display {
        0.17
    } else {
        0.10
    } * em;
    let axis = -env.axis();
    let rule = if binomial { 0.0 } else { env.rule() };
    let above = axis - rule * 0.5 - clearance - num.depth;
    let below = axis + rule * 0.5 + clearance + den.height;
    let (nx, dx) = ((width - num.width) / 2.0, (width - den.width) / 2.0);
    let mut result = Layout {
        width,
        ..Layout::default()
    };
    result.stack(num, nx, above);
    result.stack(den, dx, below);
    if binomial {
        decorations::fence('(', result, ')', env)
    } else {
        result.rect(
            [0.0, axis - rule / 2.0],
            [width, axis + rule / 2.0],
            env.color,
        );
        result
    }
}

pub(super) fn scripts(
    base: &Node<'_>,
    sub: Option<&Node<'_>>,
    sup: Option<&Node<'_>>,
    limits: Option<bool>,
    env: Env,
) -> Layout {
    let mut base_box = typeset(base, env);
    let sub = sub.map(|n| typeset(n, env.script()));
    let sup = sup.map(|n| typeset(n, env.script()));
    let em = env.em();
    let stacked = limits.unwrap_or_else(|| {
        matches!(base, Node::Accent(Accent::Brace(_), _))
            || env.style == Style::Display
                && (matches!(base, Node::Operator(_, true))
                    || matches!(
                        base,
                        Node::Word(
                            "lim" | "liminf" | "limsup" | "max" | "min" | "sup" | "inf",
                            _
                        )
                    ))
    });
    if stacked {
        let width = base_box
            .width
            .max(sub.as_ref().map_or(0.0, |b| b.width))
            .max(sup.as_ref().map_or(0.0, |b| b.width));
        let mut result = Layout {
            width,
            ..Layout::default()
        };
        let top = -base_box.height - 0.12 * em;
        let bottom = base_box.depth + 0.12 * em;
        let x = (width - base_box.width) / 2.0;
        result.stack(base_box, x, 0.0);
        if let Some(sup) = sup {
            let (x, y) = ((width - sup.width) / 2.0, top - sup.depth);
            result.stack(sup, x, y);
        }
        if let Some(sub) = sub {
            let (x, y) = ((width - sub.width) / 2.0, bottom + sub.height);
            result.stack(sub, x, y);
        }
        return result;
    }
    let x = base_box.width;
    let mut raise = (0.46 * em).max(base_box.height - 0.24 * em);
    let lower = (0.25 * em).max(base_box.depth + 0.10 * em);
    if let Some(sup) = &sup {
        raise = raise.max(sup.depth + 0.27 * em);
        if let Some(sub) = &sub {
            raise = raise.max(sup.depth + sub.height + 0.15 * em - lower);
        }
    }
    if let Some(sup) = sup {
        base_box.stack(sup, x, -raise);
    }
    if let Some(sub) = sub {
        base_box.stack(sub, x - 0.03 * em, lower);
    }
    base_box.width += 0.05 * em;
    base_box
}
