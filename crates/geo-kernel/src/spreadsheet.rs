// Spreadsheet behavior follows GeoGebra's EUPL 1.2 GeoElementSpreadsheet,
// AlgoCellRange and CmdFillCells, CmdFillColumn and CmdFillRow.
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.

//! Cell coordinates and the scripting operations of the spreadsheet.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

use crate::{expr, script, Construction, Id, Kind, Value};

pub const MAX_ROWS: u16 = 9_999;
pub const MAX_COLUMNS: u16 = 9_999;
pub(crate) const MAX_CELLS_PER_OPERATION: usize = 1_024;
pub const MAX_INPUT_BYTES: usize = 16_384;
const MAX_SOURCE_BYTES: usize = MAX_INPUT_BYTES;
pub(crate) const LIMIT: &str = "spreadsheet limit exceeded";

/// One-based spreadsheet coordinates.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Address {
    pub column: u16,
    pub row: u16,
}

impl Address {
    pub fn new(column: u16, row: u16) -> Option<Self> {
        (column > 0 && column <= MAX_COLUMNS && row > 0 && row <= MAX_ROWS)
            .then_some(Self { column, row })
    }

    pub fn parse(label: &str) -> Option<Self> {
        Reference::parse(label).map(|reference| reference.address)
    }

    pub fn column_name(mut column: u16) -> String {
        let mut letters = Vec::new();
        while column > 0 {
            column -= 1;
            letters.push(char::from(b'A' + (column % 26) as u8));
            column /= 26;
        }
        letters.into_iter().rev().collect()
    }

    pub fn label(self) -> String {
        format!("{}{}", Self::column_name(self.column), self.row)
    }

    pub(crate) fn from_numbers(column: f64, row: f64) -> Option<Self> {
        Self::new(column as u16, row as u16)
    }

    fn offset(self, columns: u16, rows: u16) -> Option<Self> {
        Self::new(
            self.column.checked_add(columns)?,
            self.row.checked_add(rows)?,
        )
    }
}

#[derive(Clone, Copy)]
struct Reference {
    address: Address,
    fixed_column: bool,
    fixed_row: bool,
}

impl Reference {
    fn parse(label: &str) -> Option<Self> {
        let fixed_column = label.starts_with('$');
        let label = label.strip_prefix('$').unwrap_or(label);
        let at = label.bytes().position(|b| !b.is_ascii_uppercase())?;
        let (column, row) = label.split_at(at);
        let fixed_row = row.starts_with('$');
        let row = row.strip_prefix('$').unwrap_or(row);
        if column.is_empty()
            || !row.starts_with(|ch| ('1'..='9').contains(&ch))
            || !row.bytes().all(|b| b.is_ascii_digit())
        {
            return None;
        }
        let column = column.bytes().try_fold(0u16, |total, letter| {
            total
                .checked_mul(26)?
                .checked_add(u16::from(letter - b'A') + 1)
        })?;
        Some(Self {
            address: Address::new(column, row.parse().ok()?)?,
            fixed_column,
            fixed_row,
        })
    }

    fn shifted(self, columns: i16, rows: i16) -> Option<String> {
        let column = if self.fixed_column { 0 } else { columns };
        let row = if self.fixed_row { 0 } else { rows };
        let target = Address::new(
            self.address.column.checked_add_signed(column)?,
            self.address.row.checked_add_signed(row)?,
        )?;
        Some(format!(
            "{}{}{}{}",
            if self.fixed_column { "$" } else { "" },
            Address::column_name(target.column),
            if self.fixed_row { "$" } else { "" },
            target.row,
        ))
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Range {
    first: Address,
    last: Address,
}

impl Range {
    pub(crate) fn parse(first: &str, last: &str) -> Result<Self, String> {
        let (Some(first), Some(last)) = (Address::parse(first), Address::parse(last)) else {
            return Err("expected two spreadsheet addresses".into());
        };
        let range = Self {
            first: Address {
                column: first.column.min(last.column),
                row: first.row.min(last.row),
            },
            last: Address {
                column: first.column.max(last.column),
                row: first.row.max(last.row),
            },
        };
        if range.width() * range.height() > MAX_CELLS_PER_OPERATION {
            return Err(LIMIT.into());
        }
        Ok(range)
    }

    pub(crate) fn cell_count(self) -> usize {
        self.width() * self.height()
    }

    fn width(self) -> usize {
        usize::from(self.last.column - self.first.column) + 1
    }

    fn height(self) -> usize {
        usize::from(self.last.row - self.first.row) + 1
    }

    pub(crate) fn column_major(self) -> impl Iterator<Item = Address> {
        (self.first.column..=self.last.column).flat_map(move |column| {
            (self.first.row..=self.last.row).map(move |row| Address { column, row })
        })
    }

    fn row_major(self) -> impl Iterator<Item = Address> {
        (self.first.row..=self.last.row).flat_map(move |row| {
            (self.first.column..=self.last.column).map(move |column| Address { column, row })
        })
    }
}

impl Construction {
    pub fn spreadsheet_cells(&self) -> impl Iterator<Item = (Address, Id)> + '_ {
        self.spreadsheet_slots()
            .filter(|&(_, id)| !self.is_blank_cell(id))
    }

    pub fn spreadsheet_cell(&self, address: Address) -> Option<Id> {
        self.spreadsheet_slot(address)
            .filter(|&id| !self.is_blank_cell(id))
    }

    pub fn spreadsheet_display(&self, address: Address) -> String {
        self.spreadsheet_display_with_angle_unit(address, false)
    }

    pub fn spreadsheet_display_with_angle_unit(&self, address: Address, radians: bool) -> String {
        self.spreadsheet_cell(address)
            .map_or_else(String::new, |id| self.spreadsheet_value(id, radians))
    }

    fn spreadsheet_value(&self, id: Id, radians: bool) -> String {
        if let Some(list) = self.elements(id) {
            let mut budget = MAX_SOURCE_BYTES;
            let format = if radians {
                Format::Radians
            } else {
                Format::Degrees
            };
            return list_source(self, list, &mut budget, format).unwrap_or_else(|_| "?".into());
        }
        if let Some(text) = self.text(id) {
            return text.into();
        }
        match self.value(id) {
            Value::Number(value) if self.kind(id) == Kind::Angle => {
                if radians {
                    self.format_display_number(value)
                } else {
                    format!("{}°", self.format_display_number(value.to_degrees()))
                }
            }
            Value::Segment { x1, y1, x2, y2 } => {
                self.format_display_number(crate::algo::measure::segment_length(x1, y1, x2, y2))
            }
            Value::PolyLine { length } | Value::Arc { length } => {
                self.format_display_number(length)
            }
            Value::Polygon { area } | Value::Sector { area } => self.format_display_number(area),
            value => scalar_source(self, value, true).unwrap_or_else(|_| "?".into()),
        }
    }

    /// Apply one grid edit. A leading equals sign introduces a formula.
    pub fn edit_spreadsheet_cell(&mut self, address: Address, input: &str) -> Result<(), String> {
        Address::new(address.column, address.row).ok_or("invalid spreadsheet address")?;
        if input.len() > MAX_SOURCE_BYTES {
            return Err(LIMIT.into());
        }
        let label = address.label();
        if input.is_empty() {
            return self.clear_spreadsheet_cell(&label);
        }
        let source = if let Some(formula) = input.strip_prefix('=') {
            Cow::Borrowed(formula.trim())
        } else if (!input.contains('E') && expr::plain_number(input.trim()).is_some())
            || (input.starts_with('"') && input.ends_with('"'))
        {
            Cow::Borrowed(input)
        } else {
            Cow::Owned(quoted(input))
        };
        if script::split_call(&source).is_some_and(|(name, _)| name == "$cell" || is_fill(name)) {
            return Err("a cell formula must return a value".into());
        }
        if let Err(error) = script::exec(self, &format!("{label} = {source}")) {
            if !input.starts_with('=')
                || !(error.starts_with("unexpected ")
                    || error.ends_with(": a replacement cannot read itself")
                    || error.ends_with(": a replacement cannot introduce a dependency cycle")
                    || matches!(
                        error.as_str(),
                        "empty expression" | "expected `)`" | "expected `}`" | "expected `|`"
                    ))
            {
                return Err(error);
            }
            script::exec(self, &format!("{label} = ?"))?;
        }
        let id = self.id(&label).ok_or("the edit did not create a cell")?;
        self.remember_cell_input(id, input);
        self.request_hidden(id);
        Ok(())
    }

    /// Input shown when the user selects a cell to edit.
    pub fn spreadsheet_input(&self, address: Address) -> Option<Cow<'_, str>> {
        let id = self.spreadsheet_cell(address)?;
        if let Some(input) = self.spreadsheet_editor_input(id) {
            return Some(Cow::Borrowed(input));
        }
        if self.is_independent_text(id) {
            return self.text(id).map(Cow::Borrowed);
        }
        let source = self.construction_source(id);
        if let Some(source) = source.filter(|source| expr::plain_number(source).is_none()) {
            return Some(Cow::Owned(format!("={source}")));
        }
        Some(
            source
                .map(Cow::Borrowed)
                .unwrap_or_else(|| Cow::Owned(self.spreadsheet_value(id, false))),
        )
    }
}

/// Builds one scripting operation for a single-cell drag fill.
pub fn drag_line(c: &Construction, from: Address, to: Address) -> Result<Option<String>, String> {
    let Some(id) = c.spreadsheet_cell(from).filter(|_| from != to) else {
        return Ok(None);
    };
    let (first, last) = if from.row < to.row {
        (
            Address {
                row: from.row + 1,
                ..from
            },
            Address {
                row: to.row,
                ..from
            },
        )
    } else if from.row > to.row {
        (
            Address {
                row: to.row,
                ..from
            },
            Address {
                row: from.row - 1,
                ..from
            },
        )
    } else if from.column < to.column {
        (
            Address {
                column: from.column + 1,
                ..from
            },
            to,
        )
    } else {
        (
            to,
            Address {
                column: from.column - 1,
                ..from
            },
        )
    };
    Range::parse(&first.label(), &last.label())?;
    let source = match c.construction_source(id) {
        Some(source) => Cow::Borrowed(source),
        None => Cow::Owned(if let Some(text) = c.text(id) {
            quoted(text)
        } else {
            scalar_source(c, c.value(id), false)?
        }),
    };
    let source = shift(
        &source,
        first.column as i16 - from.column as i16,
        first.row as i16 - from.row as i16,
    )?;
    Ok(Some(format!(
        "$spreadsheetDrag({}:{}, {source})",
        first.label(),
        last.label()
    )))
}

pub fn edit_line(address: Address, input: &str) -> String {
    format!("{} = $cell({})", address.label(), quoted(input))
}

pub(crate) fn stored_edit(
    c: &mut Construction,
    name: &str,
    source: &str,
) -> Option<Result<Option<Id>, String>> {
    let encoded = source.strip_prefix("$cell(")?.strip_suffix(')')?;
    let mut edit = || -> Result<Option<Id>, String> {
        let address = Address::parse(name)
            .filter(|a| a.label() == name)
            .ok_or("invalid cell address")?;
        if encoded.len() > 6 * MAX_INPUT_BYTES + 2 {
            return Err(LIMIT.into());
        }
        let tokens = expr::lex::tokens(encoded)?;
        let [expr::lex::Token {
            tok: expr::lex::Tok::Text(input),
            ..
        }] = &tokens[..]
        else {
            return Err("a saved cell edit needs a text input".into());
        };
        c.edit_spreadsheet_cell(address, input)?;
        Ok(c.id(name))
    };
    Some(edit())
}

pub fn is_fill_line(line: &str) -> bool {
    script::split_call(line).is_some_and(|(command, _)| is_fill(command))
        || line.split_once('=').is_some_and(|(_, source)| {
            script::split_call(source.trim()).is_some_and(|(command, _)| is_fill(command))
        })
}

pub(crate) fn is_fill(name: &str) -> bool {
    matches!(
        name,
        "FillRow" | "FillColumn" | "FillCells" | "$spreadsheetDrag"
    )
}

pub(crate) fn fill(
    c: &mut Construction,
    command: &str,
    args: &[&str],
    output: Option<&str>,
) -> Result<Option<Id>, String> {
    let [target, source] = *args else {
        return Err(format!("{command} takes two arguments"));
    };
    if source.len() > MAX_SOURCE_BYTES || target.len() > MAX_SOURCE_BYTES {
        return Err(LIMIT.into());
    }
    let mut visited = BTreeSet::new();
    let checkpoint = c.checkpoint();
    let plan = plan_fill(c, command, target, source, &mut visited);
    c.discard_since(checkpoint);
    let plan = plan?;
    visit_cells(&mut visited, plan.iter().map(|&(address, _)| address))?;
    if let Some(label) = output {
        if c.id(label).is_some() || Address::parse(label).is_some() {
            return Err("a fill result needs a fresh non-cell label".into());
        }
    }
    let checkpoint = c.checkpoint();
    // Define the labelled result before preflighting replacements, so the
    // stage also checks that those replacements preserve its inputs.
    let result = (|| {
        let result = if matches!(command, "FillRow" | "FillColumn") {
            output
                .map(|label| {
                    prepare_cells(c, source, Some(label), &mut visited, &mut BTreeSet::new())?;
                    let id = expr::define(c, label, source, false)?;
                    c.remember_construction_source(id, source);
                    Ok::<_, String>(id)
                })
                .transpose()?
        } else {
            None
        };
        let staged = c.checkpoint();
        let mut inputs = BTreeSet::new();
        let order = stage_fill(c, &plan, &mut visited, &mut inputs);
        c.discard_since(staged);
        let order = order?;

        // Keep the addresses discovered during staging. A changing coordinate
        // must not start a fresh reservation pass halfway through the writes.
        for (destination, _) in &plan {
            inputs.remove(destination);
        }
        reserve_cells(c, inputs, None)?;
        Ok::<_, String>((order, result))
    })();
    let (order, result) = match result {
        Ok(staged) => staged,
        Err(error) => {
            c.discard_since(checkpoint);
            return Err(error);
        }
    };
    for index in order {
        let (address, formula) = &plan[index];
        let label = address.label();
        if let Some(id) = script::define_prepared_cell(c, &label, formula)? {
            c.request_hidden(id);
        }
    }
    Ok(result)
}

/// Stage the new dependencies together so a multi-cell cycle is refused
/// before changing any of the destinations. Parent cells are committed first.
fn stage_fill(
    c: &mut Construction,
    plan: &[(Address, String)],
    visited: &mut BTreeSet<Address>,
    inputs: &mut BTreeSet<Address>,
) -> Result<Vec<usize>, String> {
    let mut targets = BTreeMap::new();
    let mut slots = Vec::with_capacity(plan.len());
    for (address, _) in plan {
        let label = address.label();
        let existing = c.id(&label);
        if existing.is_none() {
            c.add_blank_cell(&label)?;
        }
        slots.push((c.id(&label).expect("registered cell"), existing.is_some()));
    }
    let start = c.len();
    let mut budget = MAX_CELLS_PER_OPERATION * 4;
    for (index, ((address, formula), &(slot, existing))) in plan.iter().zip(&slots).enumerate() {
        let label = address.label();
        prepare_cells(c, formula, Some(&label), visited, inputs)?;
        let temp = format!("\0spreadsheet{}", c.len());
        let before = c.len();
        let id = script::define_after_label(c, &temp, formula)?
            .ok_or("a fill formula must return a value")?;
        if existing {
            c.check_spreadsheet_replacement(slot, id)?;
            if c.ids_from(before).any(|id| !c.label(id).starts_with('\0')) {
                return Err(format!(
                    "object `{label}` already exists: a replacement must have one visible output"
                ));
            }
        }
        if c.len() - start > MAX_CELLS_PER_OPERATION * 4 {
            return Err(LIMIT.into());
        }
        let reads = c.reads(id);
        charge(&mut budget, reads.len())?;
        visit_cells(
            visited,
            reads
                .iter()
                .filter_map(|&parent| Address::parse(c.label(parent))),
        )?;
        targets.insert(slot, (id, index));
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Visit {
        Open,
        Closed,
    }
    let mut state = BTreeMap::new();
    let mut next = Vec::with_capacity(slots.len());
    for (slot, _) in slots {
        let mut pending = vec![(slot, false)];
        while let Some((id, visited)) = pending.pop() {
            charge(&mut budget, 1)?;
            if visited {
                state.insert(id, Visit::Closed);
                if let Some(&(_, index)) = targets.get(&id) {
                    next.push(index);
                }
            } else {
                match state.get(&id) {
                    Some(Visit::Closed) => continue,
                    Some(Visit::Open) => {
                        return Err("a fill would introduce a dependency cycle".into())
                    }
                    None => {}
                }
                state.insert(id, Visit::Open);
                pending.push((id, true));
                let definition = targets.get(&id).map_or(id, |&(definition, _)| definition);
                let parents = c.reads(definition);
                if parents.len() + pending.len() > budget {
                    return Err(LIMIT.into());
                }
                pending.extend(parents.into_iter().rev().map(|parent| (parent, false)));
            }
        }
    }
    Ok(next)
}

fn plan_fill(
    c: &mut Construction,
    command: &str,
    target: &str,
    source: &str,
    visited: &mut BTreeSet<Address>,
) -> Result<Vec<(Address, String)>, String> {
    if command == "$spreadsheetDrag" {
        let (first, last) = target.split_once(':').ok_or("expected a drag range")?;
        return shifted_fill(Range::parse(first, last)?, source);
    }
    let range = if command == "FillCells" && Address::parse(target).is_none() {
        let target = c
            .id(target)
            .filter(|&id| c.kind(id) == Kind::List)
            .and_then(|id| c.construction_source(id))
            .unwrap_or(target);
        let bounds = target.split_once(':').or_else(|| {
            let ("CellRange", args) = script::split_call(target)? else {
                return None;
            };
            match args.as_slice() {
                [first, last] | [first, last, _] => Some((*first, *last)),
                _ => None,
            }
        });
        bounds
            .map(|(a, b)| Range::parse(a.trim(), b.trim()))
            .transpose()?
    } else {
        None
    };
    let temp = format!("\0spreadsheet{}", c.len());
    // The locus and other directly named objects are already in the graph.
    let value = match c.id(source) {
        Some(id) => {
            visit_cells(visited, Address::parse(source))?;
            id
        }
        None => {
            let destination = range.map(|range| range.first.label());
            prepare_cells(
                c,
                source,
                destination.as_deref(),
                visited,
                &mut BTreeSet::new(),
            )?;
            script::define_after_label(c, &temp, source)?
                .ok_or("a fill source must return a value")?
        }
    };
    let mut budget = MAX_SOURCE_BYTES * 4;
    let mut plan = Vec::new();
    if command != "FillCells" {
        let index = match c.id(target).map(|id| c.value(id)) {
            Some(Value::Number(n)) => n,
            Some(_) => return Err("expected a row or column number".into()),
            None => {
                let id = expr::define(c, &format!("{temp}index"), target, false)?;
                match c.value(id) {
                    Value::Number(n) => n,
                    _ => return Err("expected a row or column number".into()),
                }
            }
        };
        let start = if command == "FillRow" {
            Address::from_numbers(1.0, index)
        } else {
            Address::from_numbers(index, 1.0)
        }
        .ok_or("invalid spreadsheet row or column")?;
        let list = c.elements(value).ok_or("expected a list to fill")?;
        if list.len() > MAX_CELLS_PER_OPERATION {
            return Err(LIMIT.into());
        }
        for (index, item) in list.iter().enumerate() {
            let offset = index as u16;
            let (columns, rows) = if command == "FillRow" {
                (offset, 0)
            } else {
                (0, offset)
            };
            let address = start.offset(columns, rows).ok_or(LIMIT)?;
            plan.push((
                address,
                element_source(c, item, &mut budget, Format::Source)?,
            ));
        }
        return Ok(plan);
    }

    if let Some(range) = range {
        if c.kind(value) == Kind::Locus {
            if range.width() != 2 {
                return Err("a locus fill needs two columns".into());
            }
            let points = c
                .locus(value)
                .filter(|points| points.defined)
                .ok_or("expected a defined locus")?;
            for (row, point) in points.points.iter().take(range.height() - 1).enumerate() {
                let row = range.first.row + row as u16;
                let column = range.first.column;
                plan.extend([
                    (Address { row, column }, literal_number(point.x)),
                    (
                        Address {
                            row,
                            column: column + 1,
                        },
                        literal_number(point.y),
                    ),
                ]);
            }
        } else if let Some(list) = c.elements(value) {
            if list.is_empty() {
                return Err("cannot fill a range with an empty list".into());
            }
            let columns = matrix_columns(list, &mut budget)?;
            let source = expr::literal_list_source(c, value).unwrap_or(source);
            let literals = script::split_list_literal(source);
            let rows = columns.and_then(|_| {
                literals.as_ref().map(|items| {
                    items
                        .iter()
                        .map(|item| script::split_list_literal(item))
                        .collect::<Vec<_>>()
                })
            });
            for (index, address) in range.row_major().enumerate() {
                let (items, offset, sources) = if columns.is_some() {
                    let row = usize::from(address.row - range.first.row) % list.len();
                    let Some(expr::Element::List(items)) = list.get(row) else {
                        unreachable!("matrix rows checked")
                    };
                    let offset = usize::from(address.column - range.first.column) % items.len();
                    let sources = rows
                        .as_ref()
                        .and_then(|rows| rows.get(row))
                        .and_then(Option::as_deref);
                    (items, offset, sources)
                } else {
                    (list, index % list.len(), literals.as_deref())
                };
                let literal = sources.and_then(|sources| sources.get(offset)).copied();
                let formula = literal.or_else(|| {
                    items
                        .object(offset)
                        .map(|id| c.label(id))
                        .filter(|label| !label.starts_with('\0'))
                });
                let formula = if let Some(formula) = formula {
                    charge(&mut budget, formula.len())?;
                    formula.to_owned()
                } else {
                    element_source(
                        c,
                        items.get(offset).expect("fill element"),
                        &mut budget,
                        Format::Source,
                    )?
                };
                plan.push((address, formula));
            }
        } else {
            let formula = if c.id(source).is_some() {
                // A named input is copied from its definition. An unlabelled
                // expression is shifted from the first destination.
                match c.construction_source(value) {
                    Some(source) => source.to_owned(),
                    None => match c.text(value) {
                        Some(text) => quoted(text),
                        None => scalar_source(c, c.value(value), false)?,
                    },
                }
            } else {
                source.to_owned()
            };
            return shifted_fill(range, &formula);
        }
    } else {
        let start = Address::parse(target).ok_or("expected a cell or range to fill")?;
        let list = c.elements(value).ok_or("expected a list to fill a cell")?;
        if let Some(columns) = matrix_columns(list, &mut budget)? {
            if list.len().saturating_mul(columns) > MAX_CELLS_PER_OPERATION {
                return Err(LIMIT.into());
            }
            for (row, item) in list.iter().enumerate() {
                let expr::Element::List(items) = item else {
                    unreachable!("matrix rows checked")
                };
                for (column, item) in items.iter().enumerate() {
                    let address = start.offset(column as u16, row as u16).ok_or(LIMIT)?;
                    plan.push((
                        address,
                        element_source(c, item, &mut budget, Format::Source)?,
                    ));
                }
            }
        } else {
            if list.len() > MAX_CELLS_PER_OPERATION {
                return Err(LIMIT.into());
            }
            for (column, item) in list.iter().enumerate() {
                let address = start.offset(column as u16, 0).ok_or(LIMIT)?;
                plan.push((
                    address,
                    element_source(c, item, &mut budget, Format::Source)?,
                ));
            }
        }
    }
    Ok(plan)
}

fn matrix_columns(list: expr::List<'_>, budget: &mut usize) -> Result<Option<usize>, String> {
    let Some(expr::Element::List(row)) = list.get(0) else {
        return Ok(None);
    };
    if row.is_empty() {
        return Ok(None);
    }
    charge(budget, list.len())?;
    let columns = row.len();
    Ok(list
        .iter()
        .all(|item| matches!(item, expr::Element::List(row) if row.len() == columns))
        .then_some(columns))
}

pub(crate) fn charge(budget: &mut usize, amount: usize) -> Result<(), String> {
    *budget = budget.checked_sub(amount).ok_or(LIMIT)?;
    Ok(())
}

fn shifted_fill(range: Range, formula: &str) -> Result<Vec<(Address, String)>, String> {
    let mut plan = Vec::with_capacity(range.cell_count());
    let mut budget = MAX_SOURCE_BYTES * 4;
    for address in range.row_major() {
        let shifted = shift(
            formula,
            address.column as i16 - range.first.column as i16,
            address.row as i16 - range.first.row as i16,
        )?;
        charge(&mut budget, shifted.len())?;
        plan.push((address, shifted));
    }
    Ok(plan)
}

#[derive(Clone, Copy)]
enum Format {
    Source,
    Degrees,
    Radians,
}

fn list_source(
    c: &Construction,
    list: expr::List<'_>,
    budget: &mut usize,
    format: Format,
) -> Result<String, String> {
    charge(budget, 2 + 2 * list.len())?;
    let mut output = String::from("{");
    for item in list.iter() {
        if output.len() > 1 {
            output.push_str(", ");
        }
        output.push_str(&element_source(c, item, budget, format)?);
    }
    output.push('}');
    Ok(output)
}

fn element_source(
    c: &Construction,
    item: expr::Element<'_>,
    budget: &mut usize,
    format: Format,
) -> Result<String, String> {
    match item {
        expr::Element::List(list) => list_source(c, list, budget, format),
        expr::Element::Text(text) => {
            charge(budget, text.len().saturating_mul(2) + 2)?;
            Ok(quoted(text))
        }
        expr::Element::Value(kind, value) => {
            let source = match value {
                Value::Number(n) if kind == Kind::Angle && n.is_finite() => match format {
                    Format::Radians => format!("{} rad", c.format_display_number(n)),
                    Format::Degrees => format!("{}°", c.format_display_number(n.to_degrees())),
                    Format::Source => format!("{}°", literal_number(n.to_degrees())),
                },
                _ => scalar_source(c, value, !matches!(format, Format::Source))?,
            };
            charge(budget, source.len())?;
            Ok(source)
        }
    }
}

fn literal_number(number: f64) -> String {
    if number.is_nan() {
        "?".into()
    } else if number.is_infinite() {
        if number.is_sign_negative() {
            "-Infinity"
        } else {
            "Infinity"
        }
        .into()
    } else {
        number.to_string()
    }
}

fn scalar_source(c: &Construction, value: Value, display: bool) -> Result<String, String> {
    let number = |n| {
        if display {
            c.format_display_number(n)
        } else {
            literal_number(n)
        }
    };
    Ok(match value {
        Value::Number(n) => number(n),
        Value::Bool(value) => value.to_string(),
        Value::Point { x, y } | Value::Vector { x, y } => format!("({}, {})", number(x), number(y)),
        Value::Undefined => "?".into(),
        _ => return Err("unsupported spreadsheet cell value".into()),
    })
}

fn quoted(text: &str) -> String {
    format!(
        "\"{}\"",
        text.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\u{a}")
            .replace('\r', "\\u{d}")
    )
}

fn shift_reference(label: &str, columns: i16, rows: i16) -> Result<Option<String>, String> {
    Reference::parse(label)
        .map(|reference| reference.shifted(columns, rows).ok_or_else(|| LIMIT.into()))
        .transpose()
}

/// Make the cells read by an expression addressable before building its tree.
pub(crate) fn prepare(
    c: &mut Construction,
    source: &str,
    target: Option<&str>,
) -> Result<(), String> {
    prepare_cells(
        c,
        source,
        target,
        &mut BTreeSet::new(),
        &mut BTreeSet::new(),
    )
}

fn prepare_cells(
    c: &mut Construction,
    source: &str,
    target: Option<&str>,
    visited: &mut BTreeSet<Address>,
    inputs: &mut BTreeSet<Address>,
) -> Result<(), String> {
    use expr::lex::{tokens, Tok, Token};
    let grid_edit = target.and_then(Address::parse).is_some();
    if !needs_preparation(target.unwrap_or_default(), source) {
        return Ok(());
    }
    if source.len() > MAX_SOURCE_BYTES {
        return Err(LIMIT.into());
    }
    let Ok(tokens) = tokens(source) else {
        return Ok(());
    };
    let name = |token: &Token| match &token.tok {
        Tok::Name { text, call: false } => Address::parse(text),
        _ => None,
    };
    let mut addresses = BTreeSet::new();
    let mut coordinate_work = MAX_SOURCE_BYTES;
    let mut remaining = MAX_CELLS_PER_OPERATION;
    let add_range =
        |addresses: &mut BTreeSet<_>, remaining: &mut usize, first, last| -> Result<(), String> {
            let range = Range::parse(first, last)?;
            charge(remaining, range.width() * range.height())?;
            addresses.extend(range.column_major());
            Ok(())
        };
    for (i, token) in tokens.iter().enumerate() {
        if let Tok::Name { text, call: false } = &token.tok {
            if grid_edit || text.contains('$') {
                addresses.extend(name(token));
            }
            if let Some(
                [Token {
                    tok: Tok::Colon, ..
                }, Token {
                    tok:
                        Tok::Name {
                            text: end,
                            call: false,
                        },
                    ..
                }],
            ) = tokens.get(i + 1..i + 3)
            {
                if name(token).is_some() {
                    add_range(&mut addresses, &mut remaining, text, end)?;
                }
            }
        }
        if let Tok::Name { text, call: true } = &token.tok {
            match text.as_str() {
                "Row" | "Column" | "ColumnName" => {
                    addresses.extend(tokens.get(i + 2).and_then(name));
                }
                "CellRange" => {
                    if let Some(
                        [Token {
                            tok: Tok::Name { text: a, .. },
                            ..
                        }, Token {
                            tok: Tok::Comma, ..
                        }, Token {
                            tok: Tok::Name { text: b, .. },
                            ..
                        }],
                    ) = tokens.get(i + 2..i + 5)
                    {
                        add_range(&mut addresses, &mut remaining, a, b)?;
                    }
                }
                "Cell" => {
                    let mut depth = 0;
                    for end in tokens.iter().skip(i + 1) {
                        charge(&mut coordinate_work, 1)?;
                        match end.tok {
                            Tok::LParen => depth += 1,
                            Tok::RParen => {
                                depth -= 1;
                                if depth == 0 {
                                    let call = &source[token.start..end.end];
                                    if let Some((_, args)) = script::split_call(call) {
                                        if let [column, row] = args.as_slice() {
                                            if let (Some(column), Some(row)) = (
                                                coordinate_value(c, column, &mut coordinate_work)?,
                                                coordinate_value(c, row, &mut coordinate_work)?,
                                            ) {
                                                addresses
                                                    .extend(Address::from_numbers(column, row));
                                            }
                                        }
                                    }
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => (),
            }
        }
        if addresses.len() > MAX_CELLS_PER_OPERATION {
            return Err(LIMIT.into());
        }
    }
    visit_cells(visited, addresses.iter().copied())?;
    inputs.extend(addresses.iter().copied());
    reserve_cells(c, addresses, target)
}

fn visit_cells(
    visited: &mut BTreeSet<Address>,
    cells: impl IntoIterator<Item = Address>,
) -> Result<(), String> {
    for address in cells {
        visited.insert(address);
        if visited.len() > MAX_CELLS_PER_OPERATION {
            return Err(LIMIT.into());
        }
    }
    Ok(())
}

fn reserve_cells(
    c: &mut Construction,
    cells: impl IntoIterator<Item = Address>,
    target: Option<&str>,
) -> Result<(), String> {
    for address in cells {
        let label = address.label();
        if target == Some(label.as_str()) {
            // A range may include its result cell. The parser will reject
            // that dependency when it cannot find the cell during definition.
            continue;
        }
        if c.id(&label).is_none() {
            c.add_blank_cell(&label)?;
        }
    }
    Ok(())
}

fn coordinate_value(
    c: &Construction,
    source: &str,
    budget: &mut usize,
) -> Result<Option<f64>, String> {
    charge(budget, source.len())?;
    expr::spreadsheet_coordinate(c, source, budget)
}

fn shift(source: &str, columns: i16, rows: i16) -> Result<String, String> {
    let mut shifted = String::new();
    let mut at = 0;
    for token in expr::lex::tokens(source)? {
        if let expr::lex::Tok::Name { text, call: false } = token.tok {
            if let Some(replacement) = shift_reference(&text, columns, rows)? {
                shifted.push_str(&source[at..token.start]);
                shifted.push_str(&replacement);
                at = token.end;
            }
        }
    }
    shifted.push_str(&source[at..]);
    Ok(shifted)
}

pub(crate) fn needs_preparation(target: &str, source: &str) -> bool {
    use expr::lex::{tokens, Tok, Token};

    if Address::parse(target).is_some() {
        return true;
    }
    if !source.contains([':', '$'])
        && !["Cell(", "CellRange(", "Row(", "Column(", "ColumnName("]
            .iter()
            .any(|command| source.contains(command))
    {
        return false;
    }
    let Ok(tokens) = tokens(source) else {
        return false;
    };
    tokens.iter().any(|token| match &token.tok {
        Tok::Name { text, call: true } => matches!(
            text.as_str(),
            "Cell" | "CellRange" | "Row" | "Column" | "ColumnName"
        ),
        Tok::Name { text, call: false } => text.contains('$') && Address::parse(text).is_some(),
        _ => false,
    }) || tokens.windows(3).any(|window| {
        matches!(window, [Token { tok: Tok::Name { text: first, call: false }, .. },
            Token { tok: Tok::Colon, .. },
            Token { tok: Tok::Name { text: last, call: false }, .. }]
            if Address::parse(first).is_some() && Address::parse(last).is_some())
    })
}
