use super::{typeset, Env, Layout, Node};
use crate::parser::{Align, Grid, Variant};
use crate::{Color, Style};

struct Cell {
    content: Layout,
    background: Option<Color>,
}

pub(super) fn table(table: &Grid<'_>, env: Env) -> Layout {
    let columns = table
        .rows
        .iter()
        .map(Vec::len)
        .max()
        .unwrap_or(0)
        .max(table.columns.len());
    if columns == 0 || table.rows.is_empty() {
        return Layout::default();
    }
    let em = env.em();
    let mut widths = vec![0.0_f32; columns];
    for (width, spec) in widths.iter_mut().zip(&table.columns) {
        // GeoGebra's uppercase table columns and bare row skips are in points.
        *width = spec.1.points(em).max(0.0);
    }
    let cell_env = if table.aligned {
        env
    } else if env.style == Style::Display {
        env.style(Style::Text)
    } else {
        env
    };
    let cell_env = if table.text {
        cell_env.variant(Variant::Text)
    } else {
        cell_env
    };
    let mut rows = Vec::with_capacity(table.rows.len());
    let mut height = 0.0;
    for (i, row) in table.rows.iter().enumerate() {
        let (mut asc, mut desc) = (0.72 * em, 0.22 * em);
        let cells: Vec<_> = row
            .iter()
            .zip(&mut widths)
            .map(|(cell, width)| {
                let (body, background) = if let Node::CellColor(color, body) = cell {
                    (body.as_ref(), Some(*color))
                } else {
                    (cell, None)
                };
                let content = typeset(body, cell_env);
                asc = asc.max(content.height);
                desc = desc.max(content.depth);
                *width = width.max(content.width);
                Cell {
                    content,
                    background,
                }
            })
            .collect();
        let gap = if i + 1 == table.rows.len() {
            0.0
        } else {
            0.18 * em + table.gaps.get(i).map_or(0.0, |gap| gap.points(em)).max(0.0)
        };
        rows.push((cells, height + asc, asc, desc, gap));
        height += asc + desc + gap;
    }

    let gap = |column: usize| {
        if table.aligned && column % 2 == 1 {
            0.09 * em
        } else {
            0.42 * em
        }
    };
    let mut x = Vec::with_capacity(columns + 1);
    let mut width = 0.0;
    for (i, &cell_width) in widths.iter().enumerate() {
        x.push(width);
        width += cell_width + gap(i) + gap(i + 1);
    }
    x.push(width);
    let top = -env.axis() - height / 2.0;
    let bottom = top + height;
    let mut result = Layout {
        width,
        height: -top,
        depth: bottom.max(0.0),
        ..Layout::default()
    };
    let mut boundaries = Vec::with_capacity(rows.len() + 1);
    boundaries.push(top);
    for (cells, baseline, asc, desc, skip) in rows {
        let baseline = top + baseline;
        boundaries.push(baseline + desc + skip * 0.5);
        for (i, cell) in cells.into_iter().enumerate() {
            if let Some(color) = cell.background {
                result.rect(
                    [x[i], baseline - asc],
                    [x[i + 1], baseline + desc],
                    Some(color),
                );
            }
            let alignment = table.columns.get(i).map_or_else(
                || {
                    if table.aligned {
                        if i % 2 == 0 {
                            Align::Right
                        } else {
                            Align::Left
                        }
                    } else {
                        Align::Center
                    }
                },
                |spec| spec.0,
            );
            let space = widths[i] - cell.content.width;
            let offset = match alignment {
                Align::Left => 0.0,
                Align::Center => space / 2.0,
                Align::Right => space,
            };
            result.stack(cell.content, x[i] + gap(i) + offset, baseline);
        }
    }
    let mut last = None;
    let mut repeated = 0_usize;
    for &i in &table.vertical {
        repeated = if last == Some(i) { repeated + 1 } else { 0 };
        last = Some(i);
        if let Some(&x) = x.get(i) {
            let x = x.clamp(
                env.rule() / 2.0,
                (width - env.rule() / 2.0).max(env.rule() / 2.0),
            );
            let x = x + repeated as f32 * env.rule() * if i == columns { -2.0 } else { 2.0 };
            result.stroke(vec![[x, top], [x, bottom]], env);
        }
    }
    last = None;
    repeated = 0;
    for &i in &table.horizontal {
        repeated = if last == Some(i) { repeated + 1 } else { 0 };
        last = Some(i);
        if let Some(&y) = boundaries.get(i) {
            let y = y + repeated as f32
                * env.rule()
                * if i == boundaries.len() - 1 { -2.0 } else { 2.0 };
            result.stroke(vec![[0.0, y], [width, y]], env);
        }
    }
    result
}
