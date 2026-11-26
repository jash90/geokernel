//! Read-only descriptive statistics for a single spreadsheet row or column.

use std::fmt::Write;
use std::sync::OnceLock;

use geo_kernel::spreadsheet::Address;
use geo_kernel::{script, Construction, Def, Value};

use super::Selection;

/// Bounds both the scan of the sheet and the list in the scratch evaluator.
const MAX_ANALYSIS_CELLS: usize = 256;
const ROW_COUNT: usize = 11;

/// Shared model row identifiers and their equivalent kernel expressions.
pub const STATISTICS: [(&str, &str); ROW_COUNT] = [
    ("LENGTH", "Length(data)"),
    ("MEAN", "Mean(data)"),
    ("SD", "SD(data)"),
    ("SAMPLE_SD", "SampleSD(data)"),
    ("SUM", "Sum(data)"),
    // AlgoStats1D at 112919f accumulates val * val in list order for SigmaXX.
    ("SIGMAXX", "Dot(data, data)"),
    ("MIN", "Min(data)"),
    ("Q1", "Q1(data)"),
    ("MEDIAN", "Median(data)"),
    ("Q3", "Q3(data)"),
    ("MAX", "Max(data)"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnalysisError {
    Range,
    Cells,
    Evaluation,
}

#[derive(Debug)]
pub struct Summary {
    pub values: Vec<f64>,
    pub rows: [f64; ROW_COUNT],
}

#[derive(Debug)]
pub struct Analysis {
    // The instance and its cache belong to this range until another launch.
    first: Address,
    last: Address,
    summary: Option<Summary>,
}

impl Analysis {
    /// Keeps the launch control hidden until all summary commands are usable.
    pub fn available() -> bool {
        static SUPPORTED: OnceLock<bool> = OnceLock::new();
        *SUPPORTED.get_or_init(|| {
            evaluate(&[1.0, 2.0, 3.0, 4.0]).is_some_and(|rows| rows.iter().all(|v| v.is_finite()))
        })
    }

    pub fn new(selection: Selection) -> Result<Self, AnalysisError> {
        let (first, last) = selection.bounds();
        if (first.column != last.column && first.row != last.row)
            || Address::new(first.column, first.row).is_none()
            || Address::new(last.column, last.row).is_none()
            || usize::from(last.column - first.column) + usize::from(last.row - first.row) + 1
                > MAX_ANALYSIS_CELLS
        {
            return Err(AnalysisError::Range);
        }
        Ok(Self {
            first,
            last,
            summary: None,
        })
    }

    pub fn endpoints(&self) -> (Address, Address) {
        (self.first, self.last)
    }

    pub fn summary(&self) -> Option<&Summary> {
        self.summary.as_ref()
    }

    pub fn cell_count(&self) -> usize {
        usize::from(self.last.column - self.first.column)
            + usize::from(self.last.row - self.first.row)
            + 1
    }

    pub fn refresh(&mut self, construction: &Construction) -> Result<&Summary, AnalysisError> {
        let (first, last) = self.endpoints();
        let cells = (first.column..=last.column)
            .flat_map(move |column| (first.row..=last.row).map(move |row| Address { column, row }));
        let mut buffer = [0.0; MAX_ANALYSIS_CELLS];
        let values = &mut buffer[..self.cell_count()];
        for (cell, value) in cells.zip(values.iter_mut()) {
            match construction
                .spreadsheet_cell(cell)
                .map(|id| construction.value(id))
            {
                Some(Value::Number(current)) if current.is_finite() => *value = current,
                _ => {
                    self.summary = None;
                    return Err(AnalysisError::Cells);
                }
            }
        }
        let unchanged = self.summary.as_ref().is_some_and(|summary| {
            summary
                .values
                .iter()
                .zip(values.iter())
                .all(|(cached, current)| cached.to_bits() == current.to_bits())
        });
        if !unchanged {
            self.summary = evaluate(values).map(|rows| Summary {
                values: values.to_vec(),
                rows,
            });
        }
        self.summary.as_ref().ok_or(AnalysisError::Evaluation)
    }
}

fn evaluate(values: &[f64]) -> Option<[f64; ROW_COUNT]> {
    let mut scratch = Construction::new();
    let mut list = String::from("data = {");
    // Transfer the exact binary values without a display or decimal round trip.
    for (index, &value) in values.iter().enumerate() {
        let name = format!("v{index}");
        scratch.add(&name, Def::Number(value)).ok()?;
        if index != 0 {
            list.push(',');
        }
        write!(list, "{name}").ok()?;
    }
    list.push('}');
    script::exec(&mut scratch, &list).ok()??;
    let mut rows = [f64::NAN; ROW_COUNT];
    for (index, ((_, expression), row)) in STATISTICS.iter().zip(&mut rows).enumerate() {
        let line = format!("s{index} = {expression}");
        let id = script::exec(&mut scratch, &line).ok()??;
        match scratch.value(id) {
            Value::Number(value) => *row = value,
            Value::Undefined => (),
            _ => return None,
        }
    }
    Some(rows)
}
