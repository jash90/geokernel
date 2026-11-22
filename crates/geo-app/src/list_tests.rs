//! The algebra list lays out only the rows in its viewport: the list must
//! look and scroll exactly as with every row laid out, and the harness
//! must name every row, laid out or not. A row with the keyboard focus
//! keeps it while scrolled out of view.

use crate::algebra::{self, ALL_ROWS};
use crate::geom::ToEgui;
use crate::headless::{Headless, Profile, Widget, WidgetKind};
use crate::workbench::Workbench;
use eframe::egui::{
    vec2, Event, Key, Modifiers, MouseWheelSource, MouseWheelUnit, PointerButton, Pos2, TouchPhase,
};

const DT: f64 = 1.0 / 60.0;

/// Free numbers, free points and segments mixed: rows of both heights,
/// and number rows among the rows scrolled past.
fn script(groups: usize) -> String {
    let mut s = String::new();
    for i in 0..groups {
        s += &format!(
            "a_{{{i}}} = {i}\nP_{{{i}}} = ({i}, 0)\nQ_{{{i}}} = ({i}, 1)\n\
             s_{{{i}}} = Segment(P_{{{i}}}, Q_{{{i}}})\n"
        );
    }
    s
}

fn window(script: &str) -> Headless {
    let wb = Workbench::from_script(script).unwrap();
    Headless::new(wb, None, Profile::Desktop, (1400.0, 900.0), 1.0)
}

/// One frame, with every row laid out or only the visible ones.
fn frame(h: &mut Headless, all: bool, events: Vec<Event>) {
    ALL_ROWS.with(|a| a.set(all));
    h.frame(DT, events);
    ALL_ROWS.with(|a| a.set(false));
}

fn list_middle(h: &Headless) -> Pos2 {
    h.app.regions.algebra.egui().center()
}

fn wheel(dy: f32) -> Event {
    Event::MouseWheel {
        unit: MouseWheelUnit::Point,
        delta: vec2(0.0, dy),
        phase: TouchPhase::Move,
        source: MouseWheelSource::Wheel,
        modifiers: Modifiers::NONE,
    }
}

fn click(h: &mut Headless, all: bool, p: Pos2) {
    for e in [
        Event::PointerMoved(p),
        press(p, true),
        press(p, false),
        Event::PointerMoved(list_middle(h)),
    ] {
        frame(h, all, vec![e]);
    }
}

fn press(p: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos: p,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

/// Scrolls the list by `dy` points (negative: down) and lets the scroll
/// animation finish.
fn scroll(h: &mut Headless, all: bool, dy: f32) {
    let at = list_middle(h);
    frame(h, all, vec![Event::PointerMoved(at), wheel(dy)]);
    for _ in 0..40 {
        frame(h, all, vec![]);
    }
}

/// The rows' click areas and the value fields, as the harness names them.
fn rows(h: &Headless) -> Vec<Widget> {
    h.widgets()
        .into_iter()
        .filter(|w| {
            matches!(
                w.kind,
                WidgetKind::Row | WidgetKind::RowMarker | WidgetKind::RowMenu
            )
        })
        .collect()
}

/// The value fields in the list's viewport.
fn fields(h: &Headless) -> Vec<Widget> {
    let view = algebra::geometry(h.ctx()).unwrap().viewport;
    h.widgets()
        .into_iter()
        .filter(|w| w.kind == WidgetKind::RowValue && w.rect.intersects(view))
        .collect()
}

#[test]
fn the_visible_rows_are_laid_out_as_with_all_rows() {
    let s = script(30);
    let mut fast = window(&s);
    let mut full = window(&s);
    for _ in 0..4 {
        frame(&mut fast, false, vec![]);
        frame(&mut full, true, vec![]);
    }
    let n = fast.app.wb.construction().len();
    let mut offsets = Vec::new();
    for dy in [0.0, -700.0, -1530.0, -2600.0, -100_000.0, 1900.0, -333.0] {
        scroll(&mut fast, false, dy);
        scroll(&mut full, true, dy);
        let (g, gf) = (
            algebra::geometry(fast.ctx()).unwrap(),
            algebra::geometry(full.ctx()).unwrap(),
        );
        // The content (so the scroll bar), the viewport and the offset.
        assert_eq!(g, gf, "after scrolling by {dy}");
        offsets.push(g.offset);
        // Every row where it is with all rows laid out, whether laid out
        // here or placed from the model, and in view the same.
        let (r, rf) = (rows(&fast), rows(&full));
        assert_eq!(r.len(), 3 * n, "after scrolling by {dy}: rows named");
        assert_eq!(r, rf, "after scrolling by {dy}");
        assert!(r
            .iter()
            .all(|w| w.object.as_deref() == Some(w.label.as_str())));
        assert_eq!(fields(&fast), fields(&full), "after scrolling by {dy}");
        // Only the rows in view (and none left out) are laid out.
        let shown = r.iter().filter(|w| w.visible).count();
        assert!(shown > 0 && shown < 3 * n / 2, "{shown} widgets in view");
    }
    // The offsets were different, and reached the end of the list.
    let max = offsets.iter().copied().fold(0.0, f32::max);
    assert!(offsets.windows(2).all(|w| w[0] != w[1]), "{offsets:?}");
    let g = algebra::geometry(fast.ctx()).unwrap();
    assert!(
        (max - (g.content.y - g.viewport.height())).abs() < 1.0,
        "{offsets:?}"
    );
}

/// The rows of a list taller than its panel, from the model: every object
/// once, in order, each a row height below the previous one.
#[test]
fn rows_outside_the_viewport_are_named_and_placed() {
    let mut h = window(&script(12));
    for _ in 0..4 {
        frame(&mut h, false, vec![]);
    }
    scroll(&mut h, false, -900.0);
    let c = h.app.wb.construction();
    let labels: Vec<String> = c.ids().map(|id| c.label(id).to_owned()).collect();
    let bodies: Vec<Widget> = rows(&h)
        .into_iter()
        .filter(|w| w.kind == WidgetKind::Row)
        .collect();
    let names: Vec<&str> = bodies.iter().map(|w| w.label.as_str()).collect();
    assert_eq!(names, labels);
    for pair in bodies.windows(2) {
        assert!(pair[1].rect.top() >= pair[0].rect.bottom() - 0.5);
        assert!(pair[1].rect.top() <= pair[0].rect.bottom() + 0.5);
    }
    let hidden = bodies.iter().filter(|w| !w.visible).count();
    assert!(
        hidden > 0 && hidden < bodies.len(),
        "{hidden} of {} hidden",
        bodies.len()
    );
}

/// Types into the value field of `n`, scrolls the list by `away` and back
/// if asked, and presses Enter: the value `n` then has.
fn type_into_number(away: Option<f32>) -> f64 {
    let mut s = "n = 5\n".to_owned();
    s += &script(15);
    let mut h = window(&s);
    for _ in 0..4 {
        frame(&mut h, false, vec![]);
    }
    let field = h
        .widgets()
        .into_iter()
        .find(|w| w.kind == WidgetKind::RowValue && w.object.as_deref() == Some("n"))
        .expect("the value field of n");
    click(&mut h, false, field.rect.center());
    let focused = h.ctx().memory(|m| m.focused());
    assert!(focused.is_some(), "the field has no focus");
    // The field selected its text when it took the focus: typing
    // replaces it.
    frame(&mut h, false, vec![]);
    frame(&mut h, false, vec![Event::Text("12.25".into())]);
    if let Some(dy) = away {
        scroll(&mut h, false, dy);
        let n_row = rows(&h)
            .into_iter()
            .find(|w| w.kind == WidgetKind::Row && w.label == "n")
            .unwrap();
        assert!(!n_row.visible, "the row of n is still in view");
        assert_eq!(h.ctx().memory(|m| m.focused()), focused, "focus lost");
        scroll(&mut h, false, -dy);
        assert_eq!(h.ctx().memory(|m| m.focused()), focused, "focus lost");
    }
    frame(
        &mut h,
        false,
        vec![key(Key::Enter, true), key(Key::Enter, false)],
    );
    frame(&mut h, false, vec![]);
    let c = h.app.wb.construction();
    match c.value(c.id("n").unwrap()) {
        geo_kernel::kernel::Value::Number(v) => v,
        v => panic!("n is {v:?}"),
    }
}

fn key(k: Key, pressed: bool) -> Event {
    crate::headless::key(k, pressed, Modifiers::NONE)
}

#[test]
fn a_number_being_typed_keeps_its_text_and_focus_while_scrolled_out() {
    let typed = type_into_number(None);
    assert_eq!(typed, 12.25, "typing into the field");
    assert_eq!(type_into_number(Some(-2500.0)), typed);
}

/// A settled frame lays out only the rows in its viewport: the heights of
/// the others were measured when their content last changed, so they ask
/// for no layout (their anchors sit past the canvas's right edge, so the
/// drawing asks for none either).
#[test]
fn off_screen_rows_ask_for_no_layout() {
    use geo_kernel::script::text::literal;
    use std::fmt::Write;

    let mut s = String::new();
    for i in 0..60 {
        let formula = literal(&format!(r"\frac{{{i}}}{{x^2}}"));
        writeln!(s, "t_{{{i}}} = {formula}").unwrap();
        writeln!(
            s,
            "$textProperties(t_{{{i}}}, true, -, -, -, -, screen, 30000, 50)"
        )
        .unwrap();
    }
    let mut h = window(&s);
    for _ in 0..4 {
        frame(&mut h, false, vec![]);
    }
    scroll(&mut h, false, -4000.0);
    let before = crate::latex::cache_lookups();
    frame(&mut h, false, vec![]);
    let asked = crate::latex::cache_lookups() - before;
    let shown = rows(&h)
        .iter()
        .filter(|w| w.kind == WidgetKind::Row && w.visible)
        .count();
    assert!(shown > 0 && shown < 30, "{shown} rows in view");
    assert!(
        asked < shown as u64 + 2,
        "{asked} layouts asked for {shown} rows in view"
    );
}
