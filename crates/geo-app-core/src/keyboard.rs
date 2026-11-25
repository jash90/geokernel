//! The on-screen keyboard's model: tabs with digits and letters that type
//! into the input row, and the text editing of a key press. The window
//! draws the panel (`geo-app`'s `keyboard::show`).
//!
//! The layout of the keys is our own, laid out by eye from a screenshot of
//! the reference keyboard; no code or artwork of the reference was used.

use crate::app::App;

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

/// The text on a key; empty for the keys drawn as icons.
pub fn cap(key: Key, shift: bool) -> String {
    match key {
        Key::Text { cap, .. } if shift => cap.to_uppercase(),
        Key::Text { cap, .. } => cap.to_owned(),
        // Drawn as icons.
        Key::Backspace | Key::Left | Key::Right | Key::Enter | Key::Shift | Key::Space => {
            String::new()
        }
    }
}

/// Types `key` into the input row and gives the row the keyboard focus back.
pub fn press(app: &mut App, key: Key) {
    if key == Key::Shift {
        app.keyboard.shift = !app.keyboard.shift;
        return;
    }
    if app.show_cas {
        if app.cas_editor.is_none() {
            app.edit_cas_row(app.wb.cas_rows().len().min(crate::cas::MAX_ROWS - 1));
        }
        let Some(editor) = app.cas_editor.as_mut() else {
            return;
        };
        let mut cursor = app.input_cursor;
        let mut draft = editor.draft.clone();
        let submit = apply(&mut draft, &mut cursor, key, app.keyboard.shift);
        if draft.len() <= crate::cas::MAX_INPUT_BYTES {
            editor.draft = draft;
            app.input_cursor = cursor;
            app.cursor_request = Some(cursor);
        }
        if matches!(key, Key::Text { .. }) {
            app.keyboard.shift = false;
        }
        editor.focus = true;
        if submit {
            app.submit_cas_input();
        }
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
