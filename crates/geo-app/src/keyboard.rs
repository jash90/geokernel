//! The on-screen keyboard: tabs with digits and letters that type into the
//! input row. The key model and text editing are plain functions; `show`
//! draws the panel.
//!
//! The layout of the keys is our own, laid out by eye from a screenshot of
//! the reference keyboard; no code or artwork of the reference was used.

use crate::app::App;
use crate::icons::{self, Icon};
use crate::theme;
use eframe::egui::{self, pos2, vec2, Align2, Color32, CursorIcon, FontId, Rect, Sense, Ui};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Numbers,
    Letters,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    /// Shows `cap`, types `text`.
    Text {
        cap: &'static str,
        text: &'static str,
    },
    Backspace,
    Left,
    Right,
    Enter,
    Shift,
    Space,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Keyboard {
    pub open: bool,
    pub tab: Tab,
    pub shift: bool,
}

const fn t(cap: &'static str, text: &'static str) -> Key {
    Key::Text { cap, text }
}

const fn c(s: &'static str) -> Key {
    Key::Text { cap: s, text: s }
}

/// The left block of the digits tab.
const NUM_LEFT: [[Key; 4]; 4] = [
    [c("x"), c("y"), c("z"), c("π")],
    [t("x²", "^2"), c("^"), t("√", "sqrt("), c("e")],
    [c("<"), c(">"), c("≤"), c("≥")],
    [c("("), c(")"), c("|"), c(",")],
];

/// The right block of the digits tab.
const NUM_RIGHT: [[Key; 5]; 4] = [
    [c("7"), c("8"), c("9"), t("×", "*"), t("÷", "/")],
    [c("4"), c("5"), c("6"), c("+"), t("−", "-")],
    [c("1"), c("2"), c("3"), c("="), Key::Backspace],
    [c("0"), c("."), Key::Left, Key::Right, Key::Enter],
];

const LETTER_ROWS: [&str; 3] = ["qwertyuiop", "asdfghjkl", "zxcvbnm"];

/// The rows of keys of `tab`, each row as a list of `(key, width in keys)`,
/// with a gap marker `None` between the two blocks of the digits tab.
pub fn rows(tab: Tab) -> Vec<Vec<Option<(Key, f32)>>> {
    match tab {
        Tab::Numbers => (0..4)
            .map(|r| {
                let mut row: Vec<Option<(Key, f32)>> =
                    NUM_LEFT[r].iter().map(|&k| Some((k, 1.0))).collect();
                row.push(None);
                row.extend(NUM_RIGHT[r].iter().map(|&k| Some((k, 1.0))));
                row
            })
            .collect(),
        Tab::Letters => {
            let letter = |ch: char| -> Key {
                let s: &'static str = LETTERS[(ch as u8 - b'a') as usize];
                Key::Text { cap: s, text: s }
            };
            let mut out: Vec<Vec<Option<(Key, f32)>>> = LETTER_ROWS
                .iter()
                .map(|r| r.chars().map(|ch| Some((letter(ch), 1.0))).collect())
                .collect();
            out[2].insert(0, Some((Key::Shift, 1.5)));
            out[2].push(Some((Key::Backspace, 1.5)));
            out.push(vec![
                Some((c(","), 1.0)),
                Some((c("'"), 1.0)),
                Some((Key::Space, 4.0)),
                Some((Key::Left, 1.0)),
                Some((Key::Right, 1.0)),
                Some((Key::Enter, 2.0)),
            ]);
            out
        }
    }
}

const LETTERS: [&str; 26] = [
    "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p", "q", "r", "s",
    "t", "u", "v", "w", "x", "y", "z",
];

/// Applies `key` to `text` with the cursor at character `cursor`. `shift`
/// turns letters to capitals. Returns `true` for Enter.
pub fn apply(text: &mut String, cursor: &mut usize, key: Key, shift: bool) -> bool {
    let len = text.chars().count();
    *cursor = (*cursor).min(len);
    let byte = |text: &str, i: usize| text.char_indices().nth(i).map_or(text.len(), |(b, _)| b);
    match key {
        Key::Text { text: ")", .. } if text.chars().nth(*cursor) == Some(')') => {
            // Typing the closing bracket steps over the one put in for it.
            *cursor += 1;
        }
        Key::Text { text: s, .. } => {
            let s = if shift {
                s.to_uppercase()
            } else {
                s.to_owned()
            };
            let at = byte(text, *cursor);
            text.insert_str(at, &s);
            *cursor += s.chars().count();
            // An opened bracket gets its partner, with the cursor inside.
            if s.ends_with('(') {
                let at = byte(text, *cursor);
                text.insert(at, ')');
            }
        }
        Key::Space => {
            let at = byte(text, *cursor);
            text.insert(at, ' ');
            *cursor += 1;
        }
        Key::Backspace if *cursor > 0 => {
            let at = byte(text, *cursor - 1);
            text.remove(at);
            *cursor -= 1;
        }
        Key::Backspace | Key::Shift => {}
        Key::Left => *cursor = cursor.saturating_sub(1),
        Key::Right => *cursor = (*cursor + 1).min(len),
        Key::Enter => return true,
    }
    false
}

const KEY: egui::Vec2 = vec2(66.0, 40.0);
const PITCH: egui::Vec2 = vec2(70.0, 44.0);
const BLOCK_GAP: f32 = 18.0;

fn cap(key: Key, shift: bool) -> String {
    match key {
        Key::Text { cap, .. } if shift => cap.to_uppercase(),
        Key::Text { cap, .. } => cap.to_owned(),
        // Drawn as icons.
        Key::Backspace | Key::Left | Key::Right | Key::Enter | Key::Shift | Key::Space => {
            String::new()
        }
    }
}

pub fn show(app: &mut App, ui: &mut Ui) {
    let panel = ui.max_rect();
    let painter = ui.painter().clone();
    painter.rect_filled(panel, 0.0, theme::KEYBOARD_BG);
    painter.hline(
        panel.x_range(),
        panel.top() + 0.5,
        egui::Stroke::new(1.0, theme::BORDER),
    );
    let rows = rows(app.keyboard.tab);
    let width = |row: &Vec<Option<(Key, f32)>>| -> f32 {
        row.iter()
            .map(|k| k.map_or(BLOCK_GAP, |(_, w)| PITCH.x * w))
            .sum::<f32>()
            - (PITCH.x - KEY.x)
    };
    let widest = rows.iter().map(width).fold(0.0, f32::max);
    let left = panel.center().x - widest / 2.0;

    // Tabs.
    let tab_y = panel.top() + 22.0;
    let mut x = left + 2.0;
    for (tab, caption) in [(Tab::Numbers, "123"), (Tab::Letters, "ABC")] {
        let galley =
            painter.layout_no_wrap(caption.into(), FontId::proportional(16.0), theme::TEXT);
        let rect = Rect::from_min_size(pos2(x, tab_y - 13.0), vec2(galley.size().x + 22.0, 26.0));
        let response = ui
            .interact(rect, ui.scope_id().with(("tab", caption)), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Tab, true, caption));
        let active = app.keyboard.tab == tab;
        if active {
            painter.rect_filled(rect, 13.0, theme::KEY_TAB_ACTIVE);
        }
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            caption,
            FontId::proportional(16.0),
            if active { theme::WHITE } else { theme::TEXT },
        );
        if response.clicked() {
            app.keyboard.tab = tab;
            app.focus_input = true;
        }
        x = rect.right() + 14.0;
    }
    let close = Rect::from_center_size(pos2(panel.right() - 20.0, tab_y), vec2(30.0, 30.0));
    let close_response = ui
        .interact(close, ui.scope_id().with("close"), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text("Zamknij klawiaturę");
    close_response
        .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Zamknij klawiaturę"));
    let k = close.center();
    for d in [vec2(7.0, 7.0), vec2(7.0, -7.0)] {
        painter.line_segment([k - d, k + d], egui::Stroke::new(1.6, theme::ICON));
    }
    if close_response.clicked() {
        app.keyboard.open = false;
    }

    // Keys.
    let top = panel.top() + 43.0;
    for (r, row) in rows.iter().enumerate() {
        let mut x = left;
        let y = top + r as f32 * PITCH.y;
        for entry in row {
            let Some((key, w)) = *entry else {
                x += BLOCK_GAP;
                continue;
            };
            let rect =
                Rect::from_min_size(pos2(x, y), vec2(PITCH.x * w - (PITCH.x - KEY.x), KEY.y));
            x += PITCH.x * w;
            let text = cap(key, app.keyboard.shift);
            let response = ui
                .interact(
                    rect,
                    ui.scope_id().with(("key", r, rect.left() as i32)),
                    Sense::click(),
                )
                .on_hover_cursor(CursorIcon::PointingHand);
            let name = match key {
                Key::Backspace => "Usuń znak".to_owned(),
                Key::Enter => "Zatwierdź".to_owned(),
                Key::Space => "Spacja".to_owned(),
                Key::Left => "W lewo".to_owned(),
                Key::Right => "W prawo".to_owned(),
                Key::Shift => "Wielkie litery".to_owned(),
                _ => text.clone(),
            };
            response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &name));
            let special = matches!(key, Key::Backspace | Key::Left | Key::Right | Key::Enter)
                || (key == Key::Shift && app.keyboard.shift);
            let fill = match (special, response.hovered()) {
                (true, _) => Color32::from_rgb(0xd0, 0xd0, 0xd6),
                (false, true) => Color32::from_rgb(0xf6, 0xf6, 0xf8),
                (false, false) => theme::WHITE,
            };
            painter.rect_filled(rect, 6.0, fill);
            let icon = match key {
                Key::Backspace => Some(Icon::Backspace),
                Key::Left => Some(Icon::Left),
                Key::Right => Some(Icon::Right),
                Key::Enter => Some(Icon::Enter),
                Key::Shift => Some(Icon::Shift),
                _ => None,
            };
            if let Some(icon) = icon {
                icons::paint(&painter, icon, rect.center(), 10.0, theme::TEXT);
            } else {
                painter.text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    &text,
                    FontId::proportional(19.0),
                    theme::TEXT,
                );
            }
            if response.clicked() {
                press(app, key);
            }
        }
    }
}

/// Types `key` into the input row and gives the row the keyboard focus back.
pub fn press(app: &mut App, key: Key) {
    if key == Key::Shift {
        app.keyboard.shift = !app.keyboard.shift;
        return;
    }
    let mut cursor = app.input_cursor;
    let submit = apply(&mut app.input, &mut cursor, key, app.keyboard.shift);
    if matches!(key, Key::Text { .. }) {
        app.keyboard.shift = false;
    }
    app.input_cursor = cursor;
    app.cursor_request = Some(cursor);
    app.focus_input = true;
    if submit {
        app.submit_input();
    } else {
        app.wb.history.stop_browsing();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(keys: &[Key]) -> (String, usize) {
        let mut s = String::new();
        let mut cur = 0;
        for &k in keys {
            apply(&mut s, &mut cur, k, false);
        }
        (s, cur)
    }

    #[test]
    fn keys_insert_at_the_cursor_and_move_it() {
        let (s, cur) = typed(&[c("A"), c("="), c("("), c("1"), c(","), c("2"), c(")")]);
        assert_eq!((s.as_str(), cur), ("A=(1,2)", 7));
        let (s, cur) = typed(&[c("1"), c("3"), Key::Left, c("2")]);
        assert_eq!((s.as_str(), cur), ("123", 2));
        let (s, cur) = typed(&[c("x"), t("x²", "^2"), Key::Backspace]);
        assert_eq!((s.as_str(), cur), ("x^", 2));
    }

    #[test]
    fn a_function_key_opens_and_closes_its_bracket() {
        let (s, cur) = typed(&[t("√", "sqrt("), c("2")]);
        assert_eq!((s.as_str(), cur), ("sqrt(2)", 6));
    }

    #[test]
    fn edits_stay_inside_the_text_and_work_on_characters() {
        let mut s = "ąb".to_owned();
        let mut cur = 9;
        assert!(!apply(&mut s, &mut cur, Key::Backspace, false));
        assert_eq!((s.as_str(), cur), ("ą", 1));
        apply(&mut s, &mut cur, Key::Left, false);
        apply(&mut s, &mut cur, Key::Left, false);
        assert_eq!(cur, 0);
        apply(&mut s, &mut cur, c("q"), true);
        assert_eq!(s, "Qą");
        assert!(apply(&mut s, &mut cur, Key::Enter, false));
    }

    #[test]
    fn both_tabs_have_four_rows_and_the_digits() {
        let caps = |tab| -> Vec<String> {
            rows(tab)
                .into_iter()
                .flatten()
                .flatten()
                .map(|(k, _)| cap(k, false))
                .collect()
        };
        assert_eq!(rows(Tab::Numbers).len(), 4);
        assert_eq!(rows(Tab::Letters).len(), 4);
        for d in 0..10 {
            assert!(caps(Tab::Numbers).contains(&d.to_string()));
        }
        assert_eq!(
            caps(Tab::Letters).iter().filter(|s| s.len() == 1).count(),
            28
        );
    }
}
