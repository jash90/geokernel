//! The colours from the cached polygon sides are those of `style::color`.

use super::session;
use crate::palette;
use crate::style::{self, Sides};
use crate::workbench::Workbench;

/// Every object's colour from `wb.sides()` against the scan of `color`,
/// and the cached sides against sides made anew.
fn assert_colours(wb: &Workbench, what: &str) {
    let c = wb.construction();
    assert_eq!(wb.sides(), &Sides::of(c), "{what}: stale sides");
    for id in c.ids() {
        assert_eq!(
            style::color_in(c, id, wb.sides()),
            style::color(c, id),
            "{what}: {}",
            c.label(id)
        );
    }
}

#[test]
fn cached_sides_give_the_colours_of_the_scan_through_random_sessions() {
    let mut sides_seen = 0;
    for seed in 1..=12u64 {
        let mut wb = Workbench::default();
        session(&mut wb, seed * 7919, 60, |wb, edit| {
            assert_colours(wb, &format!("seed {seed}, after {edit:?}"));
            let c = wb.construction();
            sides_seen += c
                .ids()
                .filter(|&id| {
                    c.kind(id) == geo_kernel::kernel::Kind::Segment
                        && style::color(c, id) == palette::POLYGON
                })
                .count();
        });
    }
    assert!(sides_seen > 0, "the sessions made no polygon side");
}

#[test]
fn a_segment_typed_before_or_after_its_polygon_is_a_side() {
    let mut wb =
        Workbench::from_script("A = (0, 0)\nB = (4, 0)\nC = (1, 3)\ns = Segment(A, B)\n").unwrap();
    let s = wb.construction().id("s").unwrap();
    assert_eq!(
        style::color_in(wb.construction(), s, wb.sides()),
        palette::CURVE
    );
    wb.run_line("q = Polygon(A, B, C)").unwrap();
    assert_colours(&wb, "polygon added");
    assert_eq!(
        style::color_in(wb.construction(), s, wb.sides()),
        palette::POLYGON
    );
    wb.run_line("t = Segment(B, C)").unwrap();
    assert_colours(&wb, "side typed after");
    assert!(wb.undo() && wb.undo());
    assert_colours(&wb, "undone");
    assert_eq!(
        style::color_in(wb.construction(), s, wb.sides()),
        palette::CURVE
    );
}
