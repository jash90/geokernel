//! The grid, the axes with arrow heads and the tick numbers.

use crate::interact::ViewTransform;
use crate::theme;
use crate::ticks;
use eframe::egui::{vec2, Align2, FontId, Painter, Pos2, Rect, Stroke};

const MIN_MAJOR_PX: f64 = 48.0;
const MIN_MINOR_PX: f64 = 9.0;
const TICK_FONT: f32 = 12.0;

/// Which parts of the plane decoration are shown.
#[derive(Clone, Copy, Debug)]
pub struct Decor {
    pub grid: bool,
    pub axes: bool,
}

fn crisp(v: f64) -> f32 {
    (v.round() + 0.5) as f32
}

pub fn paint(painter: &Painter, rect: Rect, t: &ViewTransform, decor: Decor) {
    let (w, h) = (rect.width() as f64, rect.height() as f64);
    let v = t.view();
    let steps = ticks::grid_steps(t.scale(), MIN_MAJOR_PX, MIN_MINOR_PX);
    let at = |sx: f64, sy: f64| -> Pos2 { rect.min + vec2(sx as f32, sy as f32) };
    let vline = |sx: f64, stroke: Stroke| {
        let x = crisp(sx);
        painter.line_segment(
            [at(0.0, 0.0) + vec2(x, 0.0), at(0.0, h) + vec2(x, 0.0)],
            stroke,
        );
    };
    let hline = |sy: f64, stroke: Stroke| {
        let y = crisp(sy);
        painter.line_segment(
            [at(0.0, 0.0) + vec2(0.0, y), at(w, 0.0) + vec2(0.0, y)],
            stroke,
        );
    };
    if decor.grid {
        if let Some(minor) = steps.minor {
            let stroke = Stroke::new(1.0, theme::GRID_MINOR);
            for i in ticks::multiples(v.xmin, v.xmax, minor) {
                vline(t.world_to_screen(i as f64 * minor, 0.0).0, stroke);
            }
            for i in ticks::multiples(v.ymin, v.ymax, minor) {
                hline(t.world_to_screen(0.0, i as f64 * minor).1, stroke);
            }
        }
        let stroke = Stroke::new(1.0, theme::GRID_MAJOR);
        for i in ticks::multiples(v.xmin, v.xmax, steps.major) {
            vline(t.world_to_screen(i as f64 * steps.major, 0.0).0, stroke);
        }
        for i in ticks::multiples(v.ymin, v.ymax, steps.major) {
            hline(t.world_to_screen(0.0, i as f64 * steps.major).1, stroke);
        }
    }
    if !decor.axes {
        return;
    }
    let (ax, ay) = t.world_to_screen(0.0, 0.0);
    let axis = Stroke::new(1.2, theme::AXIS);
    let x_axis_visible = (0.0..=h).contains(&ay);
    let y_axis_visible = (0.0..=w).contains(&ax);
    if x_axis_visible {
        hline(ay, axis);
        arrow(painter, at(w, crisp_y(ay)), vec2(-1.0, 0.0), axis);
    }
    if y_axis_visible {
        vline(ax, axis);
        arrow(painter, at(crisp_x(ax), 0.0), vec2(0.0, 1.0), axis);
    }
    let font = FontId::proportional(TICK_FONT);
    let galley_of = |s: String| painter.layout_no_wrap(s, font.clone(), theme::TICK_TEXT);
    let inside = |r: Rect| {
        r.min.x >= rect.min.x
            && r.max.x <= rect.max.x
            && r.min.y >= rect.min.y
            && r.max.y <= rect.max.y
    };

    // Numbers along the x axis, under it; at the edge when the axis is out of view.
    for i in ticks::multiples(v.xmin, v.xmax, steps.major) {
        if i == 0 {
            continue;
        }
        let g = galley_of(ticks::caption(i, steps.major));
        let sx = t.world_to_screen(i as f64 * steps.major, 0.0).0;
        let top = (ay + 4.0).clamp(2.0, (h - g.size().y as f64 - 2.0).max(2.0));
        let pos = at(sx, top) - vec2(g.size().x / 2.0, 0.0);
        if inside(Rect::from_min_size(pos, g.size())) {
            painter.galley(pos, g, theme::TICK_TEXT);
        }
    }
    // Numbers along the y axis, left of it.
    for i in ticks::multiples(v.ymin, v.ymax, steps.major) {
        if i == 0 {
            continue;
        }
        let g = galley_of(ticks::caption(i, steps.major));
        let sy = t.world_to_screen(0.0, i as f64 * steps.major).1;
        let right = (ax - 6.0).clamp(
            g.size().x as f64 + 4.0,
            (w - 4.0).max(g.size().x as f64 + 4.0),
        );
        let pos = at(right, sy) - vec2(g.size().x, g.size().y / 2.0);
        if inside(Rect::from_min_size(pos, g.size())) {
            painter.galley(pos, g, theme::TICK_TEXT);
        }
    }
    if x_axis_visible && y_axis_visible {
        painter.text(
            at(ax - 6.0, ay + 4.0),
            Align2::RIGHT_TOP,
            "0",
            font,
            theme::TICK_TEXT,
        );
    }
}

fn crisp_x(sx: f64) -> f64 {
    crisp(sx) as f64
}

fn crisp_y(sy: f64) -> f64 {
    crisp(sy) as f64
}

/// An open arrow head at `tip`, pointing opposite to `back`.
fn arrow(painter: &Painter, tip: Pos2, back: eframe::egui::Vec2, stroke: Stroke) {
    let n = vec2(-back.y, back.x);
    let base = tip + back * 9.0;
    painter.line_segment([tip, base + n * 4.0], stroke);
    painter.line_segment([tip, base - n * 4.0], stroke);
}
