//! Selection and editing for the spreadsheet view.

use crate::app::App;

use geo_kernel::spreadsheet::{self, Address, MAX_ROWS};

mod analysis;
pub use analysis::{
    Analysis, AnalysisError, PairedSummary, SampleSummary, Summary, SAMPLE_STATISTIC_INDICES,
    STATISTICS,
};

#[derive(Debug)]
pub struct Editor {
    pub selected: Address,
    pub selection: Selection,
    pub range_extension_armed: bool,
    pub draft: String,
    pub focus: bool,
    pub reveal: bool,
    pub error: Option<String>,
    original: String,
}

impl App {
    pub fn select_spreadsheet_cell(&mut self, selected: Address) {
        self.clear_spreadsheet_analysis_error();
        let input = self
            .wb
            .construction()
            .spreadsheet_input(selected)
            .map_or_else(String::new, std::borrow::Cow::into_owned);
        self.spreadsheet_editor = Some(Editor {
            selected,
            selection: Selection {
                anchor: selected,
                end: selected,
            },
            range_extension_armed: false,
            draft: input.clone(),
            original: input,
            focus: true,
            reveal: true,
            error: None,
        });
    }

    pub fn extend_spreadsheet_selection(&mut self, end: Address) {
        self.cancel_spreadsheet_range_extension();
        if self.commit_spreadsheet_input() {
            if let Some(editor) = &mut self.spreadsheet_editor {
                editor.selection.end = end;
                self.clear_spreadsheet_analysis_error();
            }
        }
    }

    fn clear_spreadsheet_analysis_error(&mut self) {
        if self.spreadsheet_analysis.is_err() {
            self.spreadsheet_analysis = Ok(None);
        }
    }

    pub fn launch_spreadsheet_analysis(&mut self) {
        self.launch_analysis(Analysis::new);
    }

    pub fn launch_paired_spreadsheet_analysis(&mut self) {
        self.launch_analysis(Analysis::new_paired);
    }

    pub fn launch_multiple_spreadsheet_analysis(&mut self) {
        self.launch_analysis(Analysis::new_multiple);
    }

    fn launch_analysis(&mut self, new: fn(Selection) -> Result<Analysis, AnalysisError>) {
        self.cancel_spreadsheet_range_extension();
        if !self.commit_spreadsheet_input() {
            return;
        }
        if let Some(editor) = &self.spreadsheet_editor {
            self.spreadsheet_analysis = new(editor.selection).and_then(|mut analysis| {
                analysis.refresh_active(self.wb.construction())?;
                Ok(Some(analysis))
            });
        }
    }

    pub fn sync_spreadsheet_document(&mut self) {
        if self.spreadsheet_document_generation != self.wb.document_generation {
            self.spreadsheet_document_generation = self.wb.document_generation;
            self.spreadsheet_editor = None;
            self.spreadsheet_analysis = Ok(None);
        }
    }

    pub fn submit_spreadsheet_input(&mut self) {
        self.cancel_spreadsheet_range_extension();
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
        let fill = editor
            .draft
            .strip_prefix('=')
            .map(str::trim)
            .filter(|line| spreadsheet::is_fill_line(line));
        let result = if let Some(command) = fill {
            self.wb.run_line(command)
        } else {
            self.wb
                .edit_spreadsheet_cell(editor.selected, &editor.draft)
        };
        match result {
            Ok(()) => {
                if fill.is_some() {
                    editor.draft = self
                        .wb
                        .construction()
                        .spreadsheet_input(editor.selected)
                        .map_or_else(String::new, std::borrow::Cow::into_owned);
                }
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
        self.cancel_spreadsheet_range_extension();
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
            let selection = editor.selection;
            self.select_spreadsheet_cell(editor.selected);
            self.spreadsheet_editor.as_mut().unwrap().selection = selection;
        }
    }

    pub fn cancel_spreadsheet_range_extension(&mut self) {
        if let Some(editor) = &mut self.spreadsheet_editor {
            editor.range_extension_armed = false;
        }
    }

    pub fn cancel_spreadsheet_edit(&mut self) {
        self.cancel_spreadsheet_range_extension();
        if let Some(editor) = self.spreadsheet_editor.as_mut() {
            editor.draft.clone_from(&editor.original);
            editor.error = None;
            editor.focus = false;
        }
    }
}

/// The anchor and far edge of a grid selection. The formula bar continues
/// to address its own active cell when the selection is extended.
#[derive(Clone, Copy, Debug)]
pub struct Selection {
    pub anchor: Address,
    pub end: Address,
}

impl Selection {
    pub fn bounds(self) -> (Address, Address) {
        (
            Address {
                column: self.anchor.column.min(self.end.column),
                row: self.anchor.row.min(self.end.row),
            },
            Address {
                column: self.anchor.column.max(self.end.column),
                row: self.anchor.row.max(self.end.row),
            },
        )
    }

    pub fn contains(self, address: Address) -> bool {
        let (first, last) = self.bounds();
        (first.column..=last.column).contains(&address.column)
            && (first.row..=last.row).contains(&address.row)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workbench::Workbench;

    #[test]
    fn fill_command_refreshes_the_editor_when_extending_selection() {
        let mut app = App::new(Workbench::from_script("A1 = 1").unwrap(), None);
        let a1 = Address::parse("A1").unwrap();
        let a2 = Address::parse("A2").unwrap();
        app.select_spreadsheet_cell(a1);
        app.spreadsheet_editor.as_mut().unwrap().draft = "=FillCells(A1:A2, 4)".into();
        app.extend_spreadsheet_selection(a2);
        let editor = app.spreadsheet_editor.as_ref().unwrap();
        let stored = app.wb.construction().spreadsheet_input(a1).unwrap();
        assert_eq!(app.wb.spreadsheet_display(a1), "4");
        assert_eq!(editor.draft, stored);
        assert_eq!(editor.original, stored);
        assert_eq!(editor.selected, a1);
        assert_eq!(editor.selection.bounds(), (a1, a2));
        assert!(editor.focus);
    }

    #[test]
    fn analysis_preserves_its_range_and_discards_stale_results() {
        let mut app = App::new(
            Workbench::from_script(
                "A1 = 1
A2 = 3",
            )
            .unwrap(),
            None,
        );
        let a1 = Address::parse("A1").unwrap();
        let a2 = Address::parse("A2").unwrap();
        let b2 = Address::parse("B2").unwrap();

        app.select_spreadsheet_cell(a1);
        app.spreadsheet_editor.as_mut().unwrap().draft = "2".into();
        app.extend_spreadsheet_selection(a2);
        assert_eq!(app.wb.spreadsheet_display(a1), "2");
        assert_eq!(app.spreadsheet_editor.as_ref().unwrap().selected, a1);
        let depth = app.wb.undo_depth();
        app.launch_spreadsheet_analysis();
        assert_eq!(app.wb.undo_depth(), depth);

        app.select_spreadsheet_cell(b2);
        let analysis = app.spreadsheet_analysis.as_mut().unwrap().as_mut().unwrap();
        assert_eq!(analysis.endpoints(), (a1, a2));
        assert_eq!(analysis.summary().unwrap().values, [2.0, 3.0]);
        app.wb.edit_spreadsheet_cell(a1, "2.000001").unwrap();
        assert_eq!(app.wb.spreadsheet_display(a1), "2");
        assert_eq!(
            analysis.refresh(app.wb.construction()).unwrap().values,
            [2.000001, 3.0]
        );
        app.wb.edit_spreadsheet_cell(a1, "").unwrap();
        assert_eq!(
            analysis.refresh(app.wb.construction()).unwrap_err(),
            AnalysisError::Cells
        );
        assert!(analysis.summary().is_none());
        assert!(app.wb.undo());
        assert!(analysis.refresh(app.wb.construction()).is_ok());

        app.select_spreadsheet_cell(a1);
        app.extend_spreadsheet_selection(b2);
        app.launch_spreadsheet_analysis();
        assert!(matches!(
            app.spreadsheet_analysis,
            Err(AnalysisError::Range)
        ));
        app.extend_spreadsheet_selection(Address::parse("A257").unwrap());
        assert!(matches!(app.spreadsheet_analysis, Ok(None)));
        app.launch_spreadsheet_analysis();
        assert!(matches!(
            app.spreadsheet_analysis,
            Err(AnalysisError::Range)
        ));
        app.select_spreadsheet_cell(a1);
        assert!(matches!(app.spreadsheet_analysis, Ok(None)));
        app.launch_spreadsheet_analysis();
        let relaunched = app.spreadsheet_analysis.as_ref().unwrap().as_ref().unwrap();
        assert_eq!(relaunched.endpoints(), (a1, a1));

        app.spreadsheet_editor.as_mut().unwrap().draft =
            "x".repeat(spreadsheet::MAX_INPUT_BYTES + 1);
        app.launch_spreadsheet_analysis();
        let retained = app.spreadsheet_analysis.as_ref().unwrap().as_ref().unwrap();
        assert_eq!(retained.endpoints(), (a1, a1));
        assert!(app.spreadsheet_editor.as_ref().unwrap().error.is_some());

        app.wb.load_script("A1 = 5").unwrap();
        app.sync_spreadsheet_document();
        assert!(matches!(app.spreadsheet_analysis, Ok(None)));
        assert!(app.spreadsheet_editor.is_none());
    }
}
