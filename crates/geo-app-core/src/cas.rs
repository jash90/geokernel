//! The CAS table evaluates cells using the construction's single-row entry point.
//! Row references belong to the table, as in CASInputHandler and GeoCasCell.

use geo_import::SavedCasRow;
use geo_kernel::{CasRow, Construction};

pub use geo_import::{MAX_CAS_INPUT_BYTES as MAX_INPUT_BYTES, MAX_CAS_ROWS as MAX_ROWS};

pub const MAX_OUTPUT_BYTES: usize = 4 * 1024;
const MAX_CASCADE_BYTES: usize = 256 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub input: String,
    pub output: CasRow,
    pub(crate) saved: Option<SavedCasRow>,
}

#[derive(Default)]
pub(crate) struct Table(Vec<Entry>);

impl Table {
    pub(crate) fn rows(&self) -> &[Entry] {
        &self.0
    }

    pub(crate) fn submit(&mut self, c: &Construction, index: usize, source: &str) -> bool {
        if index > self.0.len()
            || index >= MAX_ROWS
            || source.len() > MAX_INPUT_BYTES
            || source.trim().is_empty()
        {
            return false;
        }
        let mut remaining = MAX_CASCADE_BYTES;
        let mut next = Vec::new();
        let previous = &self.0[..index];
        let input =
            substitute(source, index, b'#', |r| previous.get(r)).unwrap_or_else(|| source.into());
        for input in std::iter::once(input.as_str()).chain(
            self.0
                .iter()
                .skip(index + 1)
                .map(|entry| entry.input.as_str()),
        ) {
            let stack = index + next.len();
            let get = |r: usize| {
                if r < index {
                    previous.get(r)
                } else {
                    next.get(r - index)
                }
            };
            let resolved = substitute(input, stack, b'$', get);
            let cost = input.len() + resolved.as_ref().map_or(0, String::len);
            let Some(left) = remaining.checked_sub(cost) else {
                return false;
            };
            remaining = left;
            next.push(Entry {
                input: input.into(),
                output: evaluate(c, resolved),
                saved: None,
            });
        }
        for (old, new) in self
            .0
            .iter_mut()
            .skip(index + 1)
            .zip(next.iter_mut().skip(1))
        {
            if old
                .saved
                .as_ref()
                .is_some_and(|saved| saved.as_text || saved.output.is_none())
            {
                std::mem::swap(&mut new.output, &mut old.output);
                new.saved = old.saved.take();
            } else if new.output == old.output {
                new.saved = old.saved.take();
            }
        }
        self.0.truncate(index);
        self.0.extend(next);
        true
    }

    pub(crate) fn delete(&mut self, index: usize) -> bool {
        if index >= self.0.len() {
            return false;
        }
        let mut numbers: Vec<Option<usize>> = Vec::with_capacity(self.0.len());
        let mut surviving = 0;
        let mut row = 0;
        self.0.retain_mut(|entry| {
            let keep = if row < index {
                true
            } else if row == index {
                false
            } else {
                renumber(entry, row, surviving, &numbers)
            };
            numbers.push(keep.then_some(surviving));
            surviving += usize::from(keep);
            row += 1;
            keep
        });
        true
    }

    pub(crate) fn from_file(c: &Construction, rows: Vec<SavedCasRow>) -> Self {
        let mut table = Vec::with_capacity(rows.len());
        // Import bounds the file by row count and by the bytes of each
        // attribute. Opening a file restores every row within those limits.
        for row in rows {
            let output = if row.as_text || row.output.is_none() {
                CasRow {
                    text: String::new(),
                    latex: String::new(),
                    exact: None,
                }
            } else if row.output_options.error {
                CasRow {
                    text: row
                        .output
                        .as_deref()
                        .unwrap_or("?")
                        .chars()
                        .take(MAX_OUTPUT_BYTES / 4)
                        .collect(),
                    latex: String::new(),
                    exact: None,
                }
            } else {
                let resolved = substitute(&row.input, table.len(), b'$', |r| table.get(r));
                evaluate(c, resolved)
            };
            table.push(Entry {
                input: row.input.clone(),
                output,
                saved: Some(row),
            });
        }
        Self(table)
    }
}

fn renumber(entry: &mut Entry, row: usize, surviving: usize, numbers: &[Option<usize>]) -> bool {
    if !entry.input.contains('$') || entry.saved.as_ref().is_some_and(|saved| saved.as_text) {
        return true;
    }
    let rewrite_input = |source: &str| {
        rewrite(
            source,
            row,
            b'$',
            |reference, token, explicit, use_input| {
                let Some(reference) = reference else {
                    // An already invalid reference had no dependency to delete.
                    return Some(token.into());
                };
                let target = numbers[reference]?;
                let suffix = if use_input { "$" } else { "" };
                Some(if explicit || target + 1 != surviving {
                    format!("${}{suffix}", target + 1)
                } else {
                    format!("${suffix}")
                })
            },
        )
    };
    let Some(input) = rewrite_input(&entry.input) else {
        return false;
    };
    if entry.input != input {
        if let Some(saved) = &mut entry.saved {
            saved.input.clone_from(&input);
            for part in [
                &mut saved.input_options.prefix,
                &mut saved.input_options.eval,
                &mut saved.input_options.postfix,
            ] {
                *part = part.as_deref().and_then(rewrite_input);
            }
        }
        entry.input = input;
    }
    true
}

fn undefined() -> CasRow {
    CasRow {
        text: "?".into(),
        latex: r"\mathbf{?}".into(),
        exact: None,
    }
}

fn evaluate(c: &Construction, source: Option<String>) -> CasRow {
    let Some(source) = source else {
        return undefined();
    };
    let result = c.evaluate_cas_row(&source);
    if result.latex.len() > MAX_OUTPUT_BYTES
        || result.text.len() > MAX_OUTPUT_BYTES
        || result
            .exact
            .as_ref()
            .is_some_and(|exact| exact.len() > MAX_OUTPUT_BYTES)
    {
        return undefined();
    }
    result
}

fn substitute<'a>(
    source: &str,
    row: usize,
    delimiter: u8,
    get: impl Fn(usize) -> Option<&'a Entry>,
) -> Option<String> {
    rewrite(source, row, delimiter, |reference, _, _, use_input| {
        let entry = get(reference?)?;
        let value = if use_input {
            &entry.input
        } else {
            entry.output.exact.as_ref()?
        };
        Some(format!("({value})"))
    })
}

/// A bare marker reads the previous row. A repeated marker after the row
/// number selects its input. Only committed predecessors can be referenced.
fn rewrite(
    source: &str,
    row: usize,
    delimiter: u8,
    mut resolve: impl FnMut(Option<usize>, &str, bool, bool) -> Option<String>,
) -> Option<String> {
    let bytes = source.as_bytes();
    let mut rest = 0;
    let mut result = String::with_capacity(source.len());
    while let Some(offset) = bytes[rest..].iter().position(|&b| b == delimiter) {
        let start = rest + offset;
        let digits = start + 1;
        let end = digits
            + bytes[digits..]
                .iter()
                .take_while(|b| b.is_ascii_digit())
                .count();
        let explicit = end != digits;
        let target = if explicit {
            source[digits..end].parse::<usize>().ok()
        } else {
            Some(row)
        };
        let reference = target
            .and_then(|number| number.checked_sub(1))
            .filter(|&i| i < row);
        let use_input = bytes.get(end) == Some(&delimiter);
        let token_end = end + usize::from(use_input);
        let expansion = resolve(reference, &source[start..token_end], explicit, use_input)?;
        if result.len() + offset + expansion.len() > MAX_INPUT_BYTES {
            return None;
        }
        result.push_str(&source[rest..start]);
        result.push_str(&expansion);
        rest = token_end;
    }
    if result.len() + source.len() - rest > MAX_INPUT_BYTES {
        return None;
    }
    result.push_str(&source[rest..]);
    Some(result)
}

/// The one CAS row accepting edits from either keyboard.
pub struct Editor {
    pub row: usize,
    pub draft: String,
    pub focus: bool,
    pub refused: bool,
}

impl crate::app::App {
    pub fn edit_cas_row(&mut self, row: usize) {
        let draft = self
            .wb
            .cas_rows()
            .get(row)
            .map_or(String::new(), |row| row.input.clone());
        self.input_cursor = draft.chars().count();
        self.cursor_request = Some(self.input_cursor);
        self.cas_editor = Some(Editor {
            row,
            draft,
            focus: true,
            refused: false,
        });
    }

    pub fn submit_cas_input(&mut self) {
        let Some(editor) = &mut self.cas_editor else {
            return;
        };
        if editor.draft.trim().is_empty() {
            editor.focus = true;
            return;
        }
        if !self.wb.submit_cas(editor.row, &editor.draft) {
            editor.refused = true;
            editor.focus = true;
            return;
        }
        let row = editor.row;
        let rows = self.wb.cas_rows();
        let next = row + 1;
        let advance = next < MAX_ROWS
            && rows
                .get(next)
                .is_none_or(|entry| entry.output.text.is_empty());
        self.edit_cas_row(if advance { next } else { row });
    }

    pub fn cancel_cas_edit(&mut self) {
        self.cas_editor = None;
        self.cursor_request = None;
    }
}
