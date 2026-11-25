//! Selection and editing for the spreadsheet view.

use crate::app::App;

use geo_kernel::spreadsheet::{self, Address, MAX_ROWS};

#[derive(Debug)]
pub struct Editor {
    pub selected: Address,
    pub draft: String,
    pub focus: bool,
    pub reveal: bool,
    pub error: Option<String>,
    original: String,
}

impl App {
    pub fn select_spreadsheet_cell(&mut self, selected: Address) {
        let input = self
            .wb
            .construction()
            .spreadsheet_input(selected)
            .map_or_else(String::new, std::borrow::Cow::into_owned);
        self.spreadsheet_editor = Some(Editor {
            selected,
            draft: input.clone(),
            original: input,
            focus: true,
            reveal: true,
            error: None,
        });
    }

    pub fn submit_spreadsheet_input(&mut self) {
        let Some(editor) = self.spreadsheet_editor.as_ref() else {
            return;
        };
        let next = Address {
            row: editor.selected.row.saturating_add(1).min(MAX_ROWS),
            ..editor.selected
        };
        if self.commit_spreadsheet_input() {
            self.select_spreadsheet_cell(next);
        }
    }

    pub fn commit_spreadsheet_input(&mut self) -> bool {
        let Some(editor) = self.spreadsheet_editor.as_mut() else {
            return true;
        };
        if editor.draft == editor.original {
            return true;
        }
        if editor.draft.len() > spreadsheet::MAX_INPUT_BYTES {
            editor.error = Some("Wpis jest za długi.".into());
            return false;
        }
        let result = match editor.draft.strip_prefix('=') {
            Some(command) if spreadsheet::is_fill_line(command.trim()) => {
                self.wb.run_line(command.trim())
            }
            _ => self
                .wb
                .edit_spreadsheet_cell(editor.selected, &editor.draft),
        };
        match result {
            Ok(()) => {
                editor.original.clone_from(&editor.draft);
                editor.error = None;
                true
            }
            Err(_) => {
                editor.error = Some("Sprawdź wpis i spróbuj ponownie.".into());
                false
            }
        }
    }

    pub fn drag_spreadsheet_cell(&mut self, from: Address, to: Address) {
        if !self.commit_spreadsheet_input() {
            return;
        }
        let result =
            spreadsheet::drag_line(self.wb.construction(), from, to).and_then(|line| match line {
                Some(line) => self.wb.run_line(&line),
                None => Ok(()),
            });
        if result.is_err() {
            if let Some(editor) = self.spreadsheet_editor.as_mut() {
                editor.error = Some("Nie można wypełnić tego zakresu.".into());
            }
        }
    }

    pub(crate) fn reload_spreadsheet_selection(&mut self) {
        if let Some(editor) = &self.spreadsheet_editor {
            self.select_spreadsheet_cell(editor.selected);
        }
    }

    pub fn cancel_spreadsheet_edit(&mut self) {
        if let Some(editor) = self.spreadsheet_editor.as_mut() {
            editor.draft.clone_from(&editor.original);
            editor.error = None;
            editor.focus = false;
        }
    }
}
