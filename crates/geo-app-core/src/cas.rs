//! The CAS table evaluates cells using the construction's single-row entry point.
//! Row references belong to the table, as in CASInputHandler and GeoCasCell.

use geo_import::SavedCasRow;
use geo_kernel::{CasRow, Construction};
use std::sync::Arc;

pub use geo_import::{MAX_CAS_INPUT_BYTES as MAX_INPUT_BYTES, MAX_CAS_ROWS as MAX_ROWS};

pub const MAX_OUTPUT_BYTES: usize = 4 * 1024;
const MAX_CASCADE_BYTES: usize = 256 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub input: String,
    pub output: CasRow,
    pub unevaluated: bool,
    pub(crate) saved: Option<SavedCasRow>,
}

impl Entry {
    fn is_comment(&self) -> bool {
        self.saved.as_ref().is_some_and(|saved| saved.as_text)
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Table(Arc<Vec<Arc<Entry>>>);

impl Table {
    pub(crate) fn rows(&self) -> &[Arc<Entry>] {
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
        let mut remaining = Some(MAX_CASCADE_BYTES);
        let input = if source.contains('#') {
            substitute(
                source,
                index,
                b'#',
                |r| self.0.get(r).map(Arc::as_ref),
                &mut remaining,
            )
            .unwrap_or_else(|| source.into())
        } else {
            source.into()
        };
        let end = self.0.len().min(MAX_ROWS).max(index + 1);
        let mut next: Vec<Option<Arc<Entry>>> = Vec::with_capacity(end - index);
        for row in index..end {
            let old = self.0.get(row).map(Arc::as_ref);
            let source = if row == index {
                input.as_str()
            } else {
                let old = old.expect("committed row");
                if old.unevaluated
                    || old.is_comment()
                    || !tokens(&old.input, row, b'$').any(|token| {
                        token.target.is_some_and(|r| {
                            r >= index && next.get(r - index).is_some_and(Option::is_some)
                        })
                    })
                {
                    next.push(None);
                    continue;
                }
                &old.input
            };
            let get = |r: usize| {
                if r >= index {
                    if let Some(Some(updated)) = next.get(r - index) {
                        return Some(updated.as_ref());
                    }
                }
                self.0.get(r).map(Arc::as_ref)
            };
            let (output, saved) = if let Some(comment) = old.filter(|old| old.is_comment()) {
                let mut saved = comment.saved.clone().expect("imported comment");
                saved.input = source.into();
                (comment_output(source), Some(saved))
            } else {
                let resolved = substitute(source, row, b'$', get, &mut remaining);
                if remaining.is_none() {
                    return false;
                }
                let output = evaluate(c, resolved.as_deref());
                let saved = old
                    .filter(|old| old.input == source && old.output == output && !old.unevaluated)
                    .and_then(|old| old.saved.clone());
                (output, saved)
            };
            next.push(Some(Arc::new(Entry {
                input: source.into(),
                output,
                saved,
                unevaluated: false,
            })));
        }
        if remaining.is_none() {
            return false;
        }
        let rows = Arc::make_mut(&mut self.0);
        for (position, entry) in (index..).zip(next) {
            if let Some(entry) = entry {
                if position == rows.len() {
                    rows.push(entry);
                } else {
                    rows[position] = entry;
                }
            }
        }
        true
    }

    pub(crate) fn delete(&mut self, index: usize) -> bool {
        if index >= self.0.len() {
            return false;
        }
        let rows = Arc::make_mut(&mut self.0);
        let mut numbers: Vec<Option<usize>> = Vec::with_capacity(rows.len().min(MAX_ROWS));
        let mut surviving = 0;
        let mut row = 0;
        rows.retain_mut(|entry| {
            let keep = if row == index {
                false
            } else if row < index
                || entry.unevaluated
                || entry.is_comment()
                || !entry.input.contains('$')
            {
                true
            } else {
                renumber(entry, row, surviving, &numbers)
            };
            if row < MAX_ROWS {
                numbers.push(keep.then_some(surviving));
            }
            surviving += usize::from(keep);
            row += 1;
            keep
        });
        true
    }

    pub(crate) fn from_file(c: &Construction, rows: Vec<SavedCasRow>) -> Self {
        let mut table: Vec<Arc<Entry>> = Vec::with_capacity(rows.len());
        for row in rows {
            let index = table.len();
            let options = &row.input_options;
            let unevaluated = index >= MAX_ROWS
                || row.input.len() > MAX_INPUT_BYTES
                || [
                    &options.prefix,
                    &options.eval,
                    &options.postfix,
                    &options.command,
                    &row.output,
                ]
                .into_iter()
                .flatten()
                .any(|value| value.len() > MAX_INPUT_BYTES);
            let output = if row.as_text {
                comment_output(&row.input)
            } else if unevaluated || row.output_options.error || row.output_options.native {
                saved_output(&row, unevaluated)
            } else if row.output.is_some() {
                let resolved = substitute(
                    &row.input,
                    index,
                    b'$',
                    |r| table.get(r).map(Arc::as_ref),
                    &mut Some(MAX_CASCADE_BYTES),
                );
                evaluate(c, resolved.as_deref())
            } else {
                blank_output()
            };
            table.push(Arc::new(Entry {
                input: row.input.clone(),
                output,
                saved: Some(row),
                unevaluated,
            }));
        }
        Self(Arc::new(table))
    }
}

fn blank_output() -> CasRow {
    CasRow {
        text: String::new(),
        latex: String::new(),
        exact: None,
    }
}

fn comment_output(source: &str) -> CasRow {
    let mut output = blank_output();
    if source.len() <= MAX_OUTPUT_BYTES.saturating_sub(7) {
        output.latex = format!(r"\text{{{source}}}");
    }
    output
}

fn saved_output(row: &SavedCasRow, unevaluated: bool) -> CasRow {
    let Some(saved) = row.output.as_deref() else {
        return blank_output();
    };
    let mut output = blank_output();
    output.text = saved.into();
    if saved.len() <= MAX_OUTPUT_BYTES.saturating_sub(9) && !row.output_options.error {
        // Plain integers need no CAS call or LaTeX escaping.
        let digits = saved.strip_prefix('-').unwrap_or(saved);
        if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) {
            output.latex = format!(r"\mathbf{{{saved}}}");
        }
        if !unevaluated {
            output.exact = Some(saved.into());
        }
    }
    output
}

fn renumber(
    entry: &mut Arc<Entry>,
    row: usize,
    surviving: usize,
    numbers: &[Option<usize>],
) -> bool {
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
        let entry = Arc::make_mut(entry);
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

fn evaluate(c: &Construction, source: Option<&str>) -> CasRow {
    let Some(source) = source else {
        return undefined();
    };
    let result = c.evaluate_cas_row(source);
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
    remaining: &mut Option<usize>,
) -> Option<String> {
    let mut result = String::new();
    let mut rest = 0;
    for token in tokens(source, row, delimiter) {
        let entry = get(token.target?)?;
        if entry.unevaluated || entry.is_comment() {
            return None;
        }
        let value = if token.use_input {
            &entry.input
        } else {
            entry.output.exact.as_ref()?
        };
        let parentheses = delimiter == b'$' || !value.trim().parse::<f64>().is_ok_and(|n| n >= 0.0);
        let literal = &source[rest..token.start];
        let bytes = literal.len() + value.len() + 2 * usize::from(parentheses);
        charge(remaining, result.len(), bytes)?;
        result.push_str(literal);
        if parentheses {
            result.push('(');
        }
        result.push_str(value);
        if parentheses {
            result.push(')');
        }
        rest = token.end;
    }
    charge(remaining, result.len(), source.len() - rest)?;
    result.push_str(&source[rest..]);
    Some(result)
}

fn charge(remaining: &mut Option<usize>, written: usize, bytes: usize) -> Option<()> {
    if written + bytes > MAX_INPUT_BYTES {
        return None;
    }
    *remaining = remaining.and_then(|left| left.checked_sub(bytes));
    remaining.map(|_| ())
}

struct Reference {
    start: usize,
    end: usize,
    target: Option<usize>,
    explicit: bool,
    use_input: bool,
}

fn tokens(source: &str, row: usize, delimiter: u8) -> impl Iterator<Item = Reference> + '_ {
    let bytes = source.as_bytes();
    let mut rest = 0;
    std::iter::from_fn(move || {
        let offset = bytes[rest..].iter().position(|&b| b == delimiter)?;
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
            .filter(|&i| if delimiter == b'#' { i != row } else { i < row });
        let use_input = bytes.get(end) == Some(&delimiter);
        rest = end + usize::from(use_input);
        Some(Reference {
            start,
            end: rest,
            target: reference,
            explicit,
            use_input,
        })
    })
}

fn rewrite(
    source: &str,
    row: usize,
    delimiter: u8,
    mut resolve: impl FnMut(Option<usize>, &str, bool, bool) -> Option<String>,
) -> Option<String> {
    let mut rest = 0;
    let mut result = String::new();
    for token in tokens(source, row, delimiter) {
        let literal = &source[rest..token.start];
        let expansion = resolve(
            token.target,
            &source[token.start..token.end],
            token.explicit,
            token.use_input,
        )?;
        if result.len() + literal.len() + expansion.len() > MAX_INPUT_BYTES {
            return None;
        }
        result.push_str(literal);
        result.push_str(&expansion);
        rest = token.end;
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
        let entry = self.wb.cas_rows().get(row);
        if row >= MAX_ROWS || entry.is_some_and(|entry| entry.input.len() > MAX_INPUT_BYTES) {
            return;
        }
        let draft = entry.map_or_else(String::new, |entry| entry.input.clone());
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
        let previous = if editor.draft.trim().is_empty() {
            let Some(index) = editor.row.checked_sub(1).filter(|&index| {
                self.wb
                    .cas_rows()
                    .get(index)
                    .is_some_and(|row| row.output.exact.is_some())
            }) else {
                editor.focus = true;
                return;
            };
            format!("${}", index + 1)
        } else {
            String::new()
        };
        let source = if previous.is_empty() {
            &editor.draft
        } else {
            &previous
        };
        if !self.wb.submit_cas(editor.row, source) {
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
                .is_none_or(|entry| entry.output.text.is_empty() && !entry.unevaluated);
        self.edit_cas_row(if advance { next } else { row });
    }

    pub fn cancel_cas_edit(&mut self) {
        self.cas_editor = None;
        self.cursor_request = None;
    }
}
