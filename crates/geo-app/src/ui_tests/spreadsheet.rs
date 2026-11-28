use crate::headless::{self, Headless, Profile, Widget, DT};
use crate::workbench::Workbench;
use eframe::egui::{Event, Key, Modifiers, PointerButton, TouchPhase};
use geo_app_core::spreadsheet::Analysis;
use geo_kernel::spreadsheet::Address;

const ANALYZE: &str = "Analiza jednej zmiennej";
const EXTEND: &str = "Zaznacz zakres";
const CLOSE: &str = "Zamknij podsumowanie";

fn sheet(profile: Profile, size: (f32, f32)) -> Headless {
    let wb = Workbench::from_script("A1 = 1\nA2 = 3\nA3 = 5").unwrap();
    let mut window = Headless::new(wb, None, profile, size, 1.0);
    window.app.show_spreadsheet = true;
    window.frame(DT, Vec::new());
    window.frame(DT, Vec::new());
    window
}

fn widget(window: &Headless, label: &str) -> Widget {
    window
        .widgets()
        .into_iter()
        .find(|w| w.visible && w.enabled && w.label == label)
        .unwrap_or_else(|| panic!("no visible and enabled widget named {label:?}"))
}

fn activate(window: &mut Headless, label: &str) {
    let position = widget(window, label).rect.center();
    if window.profile() == Profile::Touch {
        for phase in [TouchPhase::Start, TouchPhase::End] {
            let events = window.touch(1, phase, position);
            window.frame(DT, events);
        }
    } else {
        window.frame(
            DT,
            vec![
                Event::PointerMoved(position),
                headless::press(position, PointerButton::Primary, true),
            ],
        );
        window.frame(
            DT,
            vec![headless::press(position, PointerButton::Primary, false)],
        );
    }
    window.frame(DT, Vec::new());
}

fn press_key(window: &mut Headless, key: Key) {
    for pressed in [true, false] {
        window.frame(DT, vec![headless::key(key, pressed, Modifiers::NONE)]);
    }
}

fn analysis(window: &Headless) -> &Analysis {
    window
        .app
        .spreadsheet_analysis
        .as_ref()
        .unwrap()
        .as_ref()
        .unwrap()
}

#[test]
fn touch_extends_a_spreadsheet_range_once_and_analyzes_it() {
    let mut window = sheet(Profile::Touch, (450.0, 800.0));
    let a1 = Address::parse("A1").unwrap();
    let a2 = Address::parse("A2").unwrap();
    let a3 = Address::parse("A3").unwrap();

    activate(&mut window, "A1");
    window.frame(DT, vec![Event::Text("2".into())]);
    activate(&mut window, EXTEND);
    assert_eq!(widget(&window, EXTEND).checked, Some(true));
    activate(&mut window, "A3");
    assert_eq!(widget(&window, EXTEND).checked, Some(false));
    let editor = window.app.spreadsheet_editor.as_ref().unwrap();
    assert_eq!(editor.selected, a1);
    assert_eq!((editor.selection.anchor, editor.selection.end), (a1, a3));

    // The next ordinary tap selects a new anchor, even before analysis opens.
    activate(&mut window, "A2");
    let editor = window.app.spreadsheet_editor.as_ref().unwrap();
    assert_eq!((editor.selection.anchor, editor.selection.end), (a2, a2));
    activate(&mut window, EXTEND);
    press_key(&mut window, Key::Escape);
    activate(&mut window, "A1");
    let editor = window.app.spreadsheet_editor.as_ref().unwrap();
    assert_eq!((editor.selection.anchor, editor.selection.end), (a1, a1));

    activate(&mut window, EXTEND);
    activate(&mut window, "A3");
    activate(&mut window, ANALYZE);
    assert_eq!(analysis(&window).endpoints(), (a1, a3));
    let summary = analysis(&window).summary().unwrap();
    assert_eq!(summary.values, [2.0, 3.0, 5.0]);
    assert_eq!(summary.rows[0], 3.0);
}

#[test]
fn short_spreadsheet_keeps_cells_editable_with_analysis_open() {
    let mut window = sheet(Profile::Desktop, (1000.0, 218.0));
    let a2_height = widget(&window, "A2").rect.height();

    activate(&mut window, "A2");
    activate(&mut window, ANALYZE);
    assert_eq!(analysis(&window).summary().unwrap().values, [3.0]);
    assert_eq!(widget(&window, "A2").rect.height(), a2_height);

    activate(&mut window, "A2");
    window.frame(DT, vec![Event::Text("7".into())]);
    press_key(&mut window, Key::Enter);
    assert_eq!(
        window
            .app
            .wb
            .spreadsheet_display(Address::parse("A2").unwrap()),
        "7"
    );
    assert_eq!(analysis(&window).summary().unwrap().values, [7.0]);

    // The close action stays reachable even when the grid itself has no room.
    window.set_size((1000.0, 110.0));
    window.frame(DT, Vec::new());
    assert!(window.screen().contains_rect(widget(&window, CLOSE).rect));
    activate(&mut window, CLOSE);
    assert!(window.app.spreadsheet_analysis.as_ref().unwrap().is_none());
}

#[test]
fn paired_analysis_swaps_axes_and_tracks_sheet_edits_and_undo() {
    let mut window = sheet(Profile::Desktop, (1000.0, 800.0));
    for (cell, value) in [("B1", "10"), ("B2", "30"), ("B3", "50")] {
        window
            .app
            .wb
            .edit_spreadsheet_cell(Address::parse(cell).unwrap(), value)
            .unwrap();
    }
    window.frame(DT, Vec::new());
    activate(&mut window, "A1");
    activate(&mut window, EXTEND);
    activate(&mut window, "B3");
    activate(&mut window, "Analiza dwóch zmiennych");
    assert!(analysis(&window).is_paired());
    let rows = |window: &mut Headless| {
        let app = &mut window.app;
        app.spreadsheet_analysis
            .as_mut()
            .unwrap()
            .as_mut()
            .unwrap()
            .refresh_paired(app.wb.construction())
            .unwrap()
            .rows
    };
    assert_eq!(rows(&mut window)[..2], [3.0, 30.0]);
    activate(&mut window, "Zamień X i Y");
    assert_eq!(rows(&mut window)[..2], [30.0, 3.0]);

    activate(&mut window, "B2");
    window.frame(DT, vec![Event::Text("60".into())]);
    press_key(&mut window, Key::Enter);
    assert_eq!(rows(&mut window)[..2], [40.0, 3.0]);
    assert!(window.app.wb.undo());
    window.frame(DT, Vec::new());
    assert_eq!(rows(&mut window)[..2], [30.0, 3.0]);
    activate(&mut window, CLOSE);
    assert!(window.app.spreadsheet_analysis.as_ref().unwrap().is_none());
}

#[test]
fn multiple_sample_analysis_keeps_separate_rows_through_edits_and_undo() {
    let mut window = sheet(Profile::Desktop, (1000.0, 800.0));
    for (cell, value) in [("B1", "10"), ("B3", "=A3 + 15"), ("C2", "=90°")] {
        window
            .app
            .wb
            .edit_spreadsheet_cell(Address::parse(cell).unwrap(), value)
            .unwrap();
    }
    window.frame(DT, Vec::new());
    activate(&mut window, "A1");
    activate(&mut window, EXTEND);
    activate(&mut window, "C3");
    activate(&mut window, "Analiza wielu prób");
    assert!(analysis(&window).is_multiple());
    widget(&window, "B1:B3");
    widget(&window, "C1:C3");
    let means = |window: &mut Headless| {
        let app = &mut window.app;
        app.spreadsheet_analysis
            .as_mut()
            .unwrap()
            .as_mut()
            .unwrap()
            .refresh_multiple(app.wb.construction())
            .unwrap()
            .iter()
            .map(|sample| sample.rows[1])
            .collect::<Vec<_>>()
    };
    let original = means(&mut window);
    assert_eq!(original[..2], [3.0, 15.0]);
    assert!(original[2].is_nan());

    activate(&mut window, "A3");
    window.frame(DT, vec![Event::Text("11".into())]);
    press_key(&mut window, Key::Enter);
    assert_eq!(means(&mut window)[..2], [5.0, 18.0]);
    assert!(window.app.wb.undo());
    window.frame(DT, Vec::new());
    assert_eq!(means(&mut window)[..2], [3.0, 15.0]);
    assert!(window.app.wb.redo());
    window.frame(DT, Vec::new());
    assert_eq!(means(&mut window)[..2], [5.0, 18.0]);
    activate(&mut window, CLOSE);
    assert!(window.app.spreadsheet_analysis.as_ref().unwrap().is_none());
}

#[test]
fn selected_spreadsheet_cells_keep_their_gridlines() {
    use eframe::egui::Shape;

    let mut window = sheet(Profile::Desktop, (1000.0, 500.0));
    activate(&mut window, EXTEND);
    activate(&mut window, "A3");
    let cell = widget(&window, "A2").rect;
    let shapes = &window.last_output().unwrap().shapes;
    let fill = shapes
        .iter()
        .rposition(|s| match &s.shape {
            Shape::Rect(rect) => {
                rect.fill == crate::theme::TOOLBAR_BG && rect.rect.contains(cell.center())
            }
            _ => false,
        })
        .expect("selection highlights A2");
    let line = shapes
        .iter()
        .position(|s| match &s.shape {
            Shape::LineSegment { points, stroke } => {
                stroke.color == crate::theme::SEPARATOR
                    && (points[0].y - (cell.bottom() - 0.5)).abs() < 0.1
                    && points[0].y == points[1].y
                    && points[0].x <= cell.center().x
                    && cell.center().x <= points[1].x
            }
            _ => false,
        })
        .expect("grid draws the line below A2");
    assert!(fill < line, "selection must be painted behind the gridline");
}

#[test]
fn opening_analysis_does_not_add_a_header_row_in_a_narrow_spreadsheet() {
    let mut window = sheet(Profile::Desktop, (300.0, 225.0));
    let before = widget(&window, "A1").rect;
    activate(&mut window, ANALYZE);
    assert_eq!(widget(&window, "A1").rect, before);
    widget(&window, CLOSE);
}

#[test]
fn an_open_analysis_can_be_replaced_without_closing_it_first() {
    let mut window = sheet(Profile::Desktop, (1000.0, 218.0));
    activate(&mut window, ANALYZE);
    assert_eq!(analysis(&window).summary().unwrap().values, [1.0]);
    activate(&mut window, "A2");
    activate(&mut window, ANALYZE);
    assert_eq!(analysis(&window).summary().unwrap().values, [3.0]);
    widget(&window, CLOSE);
}

#[test]
fn a_hidden_grid_keeps_the_selected_cell_reveal_until_resize() {
    let mut window = sheet(Profile::Desktop, (1000.0, 110.0));
    window
        .app
        .select_spreadsheet_cell(Address::parse("A100").unwrap());
    window.frame(DT, Vec::new());
    assert!(window.app.spreadsheet_editor.as_ref().unwrap().reveal);

    window.set_size((1000.0, 500.0));
    for _ in 0..3 {
        window.frame(DT, Vec::new());
    }
    widget(&window, "A100");
    assert!(!window.app.spreadsheet_editor.as_ref().unwrap().reveal);
}
