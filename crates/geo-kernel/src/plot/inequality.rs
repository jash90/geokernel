//! Bounded screen sampling of the area of a Boolean function.

use super::Frame;
use crate::{Construction, Id};

/// Screen rectangles covering the positive cells, with consecutive cells
/// in a row combined in one rectangle. Numeric functions define no region.
pub fn tiles(c: &Construction, id: Id, frame: &Frame) -> Vec<[f64; 4]> {
    let deadline = crate::expr::work_done().saturating_add(crate::plot::DRAW_DEADLINE);
    crate::expr::with_work_deadline(deadline, || {
        let mut tiles = Vec::new();
        if !frame.resolves() || !c.is_plane_inequality(id) {
            return tiles;
        }
        let columns = ((frame.width() / 10.0).ceil() as usize).clamp(8, 64);
        let rows = ((frame.height() / 10.0).ceil() as usize).clamp(8, 64);
        let (dx, dy) = (frame.width() / columns as f64, frame.height() / rows as f64);
        let work = crate::expr::IntersectionWork::new();
        for row in 0..rows {
            let y = row as f64 * dy;
            let mut start = None;
            for column in 0..=columns {
                let x = column as f64 * dx;
                let inside = column < columns && {
                    let (wx, wy) = frame.screen_to_world(x + dx / 2.0, y + dy / 2.0);
                    work.sample_system(c, id, wx, &[wy]) == 1.0
                };
                if inside {
                    start.get_or_insert(x);
                } else if let Some(start) = start.take() {
                    tiles.push([start, y, x, y + dy]);
                }
            }
            if work.exhausted() {
                return Vec::new();
            }
        }
        tiles
    })
}

#[cfg(test)]
#[test]
fn canvas_remaining_drawings_share_one_expression_budget_per_plot() {
    use std::fmt::Write as _;
    let mut script = String::from("f0(x) = x\n");
    for i in 1..100 {
        writeln!(script, "f{i}(x) = f{}(x) + 1", i - 1).unwrap();
    }
    script.push_str("region(x,y) = f99(x) > y\ncurve = Curve(t, f99(t), t, -3, 3)");
    let c = crate::script::load(&script).unwrap();
    let frame = Frame::fit(crate::svg::View::fit(&c), 1000.0, 800.0);
    let start = crate::expr::work_done();
    let _ = tiles(&c, c.id("region").unwrap(), &frame);
    assert!(crate::expr::work_done() - start <= crate::plot::MAX_DRAW_WORK);
    let start = crate::expr::work_done();
    let _ = crate::algo::curve::plot(&c, c.id("curve").unwrap(), &frame);
    assert!(crate::expr::work_done() - start <= crate::plot::MAX_DRAW_WORK);
}
