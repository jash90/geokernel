//! The cached algebra rows are those made from scratch, and a change makes
//! only the rows whose value changed.

use super::{session, Rng};
use crate::workbench::Workbench;
use geo_kernel::kernel::Value;

#[test]
fn cached_rows_equal_rows_made_from_scratch_through_random_sessions() {
    for seed in 1..=16u64 {
        let mut wb = Workbench::default();
        let mut rng = Rng(seed * 104_729);
        session(&mut wb, seed * 31, 70, |wb, edit| {
            assert_eq!(
                wb.rows().to_vec(),
                wb.rows_reference(),
                "seed {seed}, after {edit:?}"
            );
            // A few moves outside any gesture, each checked.
            let c = wb.construction();
            let free: Vec<_> = c
                .ids()
                .filter(|&id| matches!(c.value(id), Value::Point { .. }) && c.def(id).is_free())
                .collect();
            if free.is_empty() {
                return;
            }
            let id = free[rng.below(free.len())];
            for _ in 0..2 {
                wb.move_point(id, rng.coord(), rng.coord());
                assert_eq!(
                    wb.rows().to_vec(),
                    wb.rows_reference(),
                    "seed {seed}, a move"
                );
            }
        });
    }
}

/// A grid of `cells` independent cells: a free point with four dependents
/// (segment, circle, midpoint and, through its neighbours, a line).
fn grid(cells: usize) -> Workbench {
    let mut s = String::new();
    for i in 0..cells {
        let x = i as f64 * 5.0;
        s += &format!(
            "P_{{{i}}} = ({x}, 0)\nQ_{{{i}}} = ({}, 0)\nR_{{{i}}} = ({x}, 2)\n\
             s_{{{i}}} = Segment(P_{{{i}}}, R_{{{i}}})\nl_{{{i}}} = Line(Q_{{{i}}}, R_{{{i}}})\n\
             k_{{{i}}} = Circle(P_{{{i}}}, Q_{{{i}}})\nM_{{{i}}} = Midpoint(P_{{{i}}}, Q_{{{i}}})\n",
            x + 2.0
        );
    }
    Workbench::from_script(&s).unwrap()
}

#[test]
fn a_move_makes_only_the_rows_whose_value_changed() {
    for cells in [10, 100] {
        let mut wb = grid(cells);
        wb.rows();
        let (built, made) = (wb.rows_built(), wb.rows_formatted());
        assert_eq!(made, wb.construction().len() as u64, "every row made once");
        let p = wb.construction().id("P_{3}").unwrap();
        // P, its segment, its circle and its midpoint change.
        assert!(wb.move_point(p, 15.5, 0.25));
        wb.rows();
        assert_eq!(wb.rows_formatted() - made, 4, "{cells} cells");
        assert_eq!(wb.rows_built() - built, 1);
        // A move below the shown precision changes values, not texts: the
        // list does not change.
        let (built, made) = (wb.rows_built(), wb.rows_formatted());
        assert!(wb.move_point(p, 15.5 + 1e-9, 0.25));
        wb.rows();
        assert_eq!(wb.rows_built(), built, "{cells} cells");
        assert_eq!(wb.rows_formatted() - made, 4);
        // Nothing changed: nothing is made.
        let made = wb.rows_formatted();
        wb.rows();
        wb.rows();
        assert_eq!(wb.rows_formatted(), made);
    }
}

#[test]
fn an_added_object_makes_one_row_and_a_rebuild_only_the_rows_that_changed() {
    let mut wb = grid(20);
    wb.rows();
    let made = wb.rows_formatted();
    wb.run_line("Z = Midpoint(P_{1}, R_{1})").unwrap();
    wb.rows();
    assert_eq!(wb.rows_formatted() - made, 1);
    // Deleting renumbers the objects after the deleted ones; their rows
    // stay, with the new ids.
    let (made, built) = (wb.rows_formatted(), wb.rows_built());
    let z = wb.construction().id("Z").unwrap();
    assert!(wb.delete(z));
    assert_eq!(wb.rows().to_vec(), wb.rows_reference());
    assert_eq!(wb.rows_formatted() - made, 0);
    assert_eq!(wb.rows_built() - built, 1);
    let made = wb.rows_formatted();
    let p = wb.construction().id("P_{1}").unwrap();
    // P, its segment, its circle and its midpoint go.
    assert!(wb.delete(p));
    assert_eq!(wb.rows().to_vec(), wb.rows_reference());
    assert_eq!(wb.rows_formatted() - made, 0);
    // Undo brings them back: their four rows are made.
    let made = wb.rows_formatted();
    assert!(wb.undo());
    assert_eq!(wb.rows().to_vec(), wb.rows_reference());
    assert_eq!(wb.rows_formatted() - made, 4);
    let made = wb.rows_formatted();
    assert!(wb.redo());
    assert_eq!(wb.rows().to_vec(), wb.rows_reference());
    assert_eq!(wb.rows_formatted() - made, 0);
    // Undo of a move: the rows whose value changed back.
    assert!(wb.undo());
    wb.rows();
    let q = wb.construction().id("Q_{3}").unwrap();
    assert!(wb.move_point(q, 16.5, 0.5));
    wb.rows();
    let made = wb.rows_formatted();
    assert!(wb.undo());
    assert_eq!(wb.rows().to_vec(), wb.rows_reference());
    // Q, its line, its circle and its midpoint.
    assert_eq!(wb.rows_formatted() - made, 4);
}

#[test]
fn a_rebuild_makes_the_row_of_a_label_that_names_another_object() {
    // Each pair: the same label, kind and value, from another definition
    // line, another line's labels, or a definition shown another way. A
    // file opened over another replaces the construction in one step.
    let pairs = [
        (
            "A = (1, 2)\nB = (5, 3)\nM = Midpoint(B, A)\n",
            "A = (1, 2)\nB = (5, 3)\nM = Midpoint(A, B)\n",
        ),
        (
            "A = (1, 2)\nB = (5, 3)\nC = (3, 7)\nP = Polygon(A, B, C)\n",
            "a = 5\nA = (1, 2)\nB = (5, 3)\nC = (3, 7)\nP = Polygon(A, B, C)\n",
        ),
        (
            "A = (1, 0)\nB = (0, 0)\nC = (0, 1)\nw = Angle(A, B, C)\n",
            "A = (1, 0)\nB = (0, 0)\nC = (0, 1)\nw = pi / 2\n",
        ),
    ];
    for (one, other) in pairs {
        for (from, to) in [(one, other), (other, one)] {
            let mut wb = Workbench::from_script(from).unwrap();
            wb.rows();
            wb.load_script(to).unwrap();
            assert_eq!(
                wb.rows().to_vec(),
                wb.rows_reference(),
                "{from:?} to {to:?}"
            );
        }
    }
}

#[test]
fn conic_rows_show_the_equation_and_change_only_with_the_conic() {
    let mut wb = Workbench::from_script(
        "A = (-2, 0)\nB = (2, 0)\nm = 3\ne = Ellipse(A, B, m)\nP = (7, 7)\n\
         f(x) = x^2\nk = 1\nh = Hyperbola(A, B, k)\n",
    )
    .unwrap();
    let rows = wb.rows().to_vec();
    assert_eq!(rows, wb.rows_reference());
    // 5x² + 9y² = 45 is x²/9 + y²/5 = 1.
    assert_eq!(rows[3].value.as_deref(), Some("5x² + 9y² = 45"));
    // The sign of the matrix is the reference's.
    assert_eq!(rows[7].value.as_deref(), Some("-3x² + y² = -3"));
    assert_eq!(rows[5].value, None);
    // A point the conics do not read moves: only its own row is made.
    let made = wb.rows_formatted();
    let p = wb.construction().id("P").unwrap();
    assert!(wb.move_point(p, 8.0, 7.5));
    wb.rows();
    assert_eq!(wb.rows_formatted() - made, 1);
    // A focus moves: its row and both conics.
    let made = wb.rows_formatted();
    let a = wb.construction().id("A").unwrap();
    assert!(wb.move_point(a, -2.0, 1.0));
    assert_eq!(wb.rows().to_vec(), wb.rows_reference());
    assert_eq!(wb.rows_formatted() - made, 3);
}
