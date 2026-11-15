//! Undo steps share the journal's storage, and restore exactly what full
//! copies of the journal restored.

use super::session;
use crate::workbench::Workbench;
use geo_kernel::kernel::Value;

/// What a state of the document is, as far as undo is concerned.
#[derive(Debug, PartialEq)]
struct State {
    text: String,
    undo: bool,
    redo: bool,
    notice: Option<String>,
    selected: Option<String>,
}

fn state(wb: &Workbench) -> State {
    let c = wb.construction();
    State {
        text: wb.snapshot_text(),
        undo: wb.can_undo(),
        redo: wb.can_redo(),
        notice: wb.notice.clone(),
        selected: wb.selected.map(|id| c.label(id).to_owned()),
    }
}

/// A random session (with now and then an edit whose rebuild fails, or
/// whose replay fails inside `rebuild`), and then undo to the start and
/// redo to the end: every state on the way.
fn walk(seed: u64, steps: usize, unshared: bool) -> Vec<State> {
    let mut wb = Workbench::default();
    wb.set_unshared(unshared);
    let mut out = Vec::new();
    session(&mut wb, seed, steps, |wb, _| out.push(state(wb)));
    while wb.undo() {
        out.push(state(&wb));
    }
    while wb.redo() {
        out.push(state(&wb));
    }
    out.push(state(&wb));
    out
}

#[test]
fn undo_and_redo_restore_what_full_copies_restore() {
    let mut failures = 0;
    for seed in 1..=10u64 {
        let shared = walk(seed * 6151, 90, false);
        let copies = walk(seed * 6151, 90, true);
        assert_eq!(shared.len(), copies.len(), "seed {seed}");
        for (i, (a, b)) in shared.iter().zip(&copies).enumerate() {
            assert_eq!(a, b, "seed {seed}, state {i}");
        }
        failures += shared.iter().filter(|s| s.notice.is_some()).count();
    }
    assert!(
        failures > 10,
        "only {failures} states after a failed rebuild"
    );
}

/// A grid of `cells` cells of seven objects.
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

/// `steps` edits of the kinds a user makes most: drags, new objects and
/// hiding, each its own undo step.
fn edit(wb: &mut Workbench, steps: usize) {
    for i in 0..steps {
        let c = wb.construction();
        let p = c.id(&format!("P_{{{}}}", i % 10)).unwrap();
        match i % 3 {
            0 => {
                wb.begin_gesture();
                for k in 0..4 {
                    wb.move_point(p, i as f64, k as f64 + 0.5);
                }
                wb.end_gesture();
            }
            1 => wb
                .run_line(&format!("Z_{{{i}}} = Midpoint(P_{{1}}, R_{{2}})"))
                .unwrap(),
            _ => wb.toggle_visible(p),
        }
    }
}

#[test]
fn the_history_takes_memory_for_what_the_steps_changed() {
    for cells in [20, 150] {
        let mut wb = grid(cells);
        let objects = wb.construction().len();
        edit(&mut wb, 600);
        let (stored, listed) = wb.history_entries();
        println!("{objects} objects, 600 steps: {stored} entries stored, {listed} in full copies");
        // The 500 kept steps share the journal: at most one entry per step
        // (a move or a new line) beyond it.
        assert!(
            stored <= objects + 600,
            "{objects} objects: {stored} stored"
        );
        assert!(
            listed >= 500 * objects,
            "{objects} objects: {listed} listed"
        );
        // Undo all the way back: 500 steps.
        let mut n = 0;
        while wb.undo() {
            n += 1;
        }
        assert_eq!(n, 500);
    }
}

#[test]
fn a_delete_copies_only_the_journal_after_the_deleted_line() {
    let mut wb = grid(100);
    let (before, _) = wb.history_entries();
    // The last cell's midpoint: nothing comes after it.
    let id = wb.construction().id("M_{99}").unwrap();
    assert!(wb.delete(id));
    let (after, _) = wb.history_entries();
    assert_eq!(after, before, "the state after shares the one before");
    assert!(wb.undo());
    assert!(matches!(
        wb.construction()
            .value(wb.construction().id("M_{99}").unwrap()),
        Value::Point { .. }
    ));
}

/// Past the cap of 500 steps the oldest steps are dropped: with shared
/// steps the walk back to the oldest kept step and forward again goes
/// through exactly the states full copies go through.
#[test]
fn past_the_step_cap_undo_and_redo_restore_what_full_copies_restore() {
    let walk = |unshared: bool| {
        let mut wb = grid(10);
        wb.set_unshared(unshared);
        edit(&mut wb, 640);
        let mut out = vec![state(&wb)];
        while wb.undo() {
            out.push(state(&wb));
        }
        while wb.redo() {
            out.push(state(&wb));
        }
        out
    };
    let (shared, copies) = (walk(false), walk(true));
    assert_eq!(shared.len(), 1 + 2 * 500, "500 steps back and forth");
    for (i, (a, b)) in shared.iter().zip(&copies).enumerate() {
        assert_eq!(a, b, "state {i}");
    }
    // The oldest kept state is not the start: 140 steps were dropped.
    assert_ne!(shared[500].text, grid(10).snapshot_text());
}
