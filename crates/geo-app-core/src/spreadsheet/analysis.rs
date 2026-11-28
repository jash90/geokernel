//! Read-only descriptive statistics for spreadsheet selections.

use std::fmt::Write;
use std::sync::OnceLock;

use geo_kernel::spreadsheet::Address;
use geo_kernel::{script, Construction, Def, Kind, Value};

use super::Selection;

/// Bounds both the scan of the sheet and the list in the scratch evaluator.
const MAX_ANALYSIS_CELLS: usize = 256;
const ROW_COUNT: usize = 11;
const PAIRED_ROW_COUNT: usize = 9;
pub const SAMPLE_STATISTIC_INDICES: [usize; 9] = [0, 1, 2, 3, 6, 7, 8, 9, 10];

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

const PAIRED_STATISTICS: [&str; PAIRED_ROW_COUNT] = [
    "Mean(xs)",
    "Mean(ys)",
    "SampleSD(xs)",
    "SampleSD(ys)",
    "PMCC(xs, ys)",
    "Spearman(xs, ys)",
    "Sxx(xs)",
    "Sxx(ys)",
    "Sxy(xs, ys)",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnalysisError {
    Range,
    Cells,
    PairedRange,
    SampleRange,
    EmptySample,
    Pairs,
    Evaluation,
}

#[derive(Debug)]
pub struct Summary {
    pub values: Vec<f64>,
    pub rows: [f64; ROW_COUNT],
}

#[derive(Debug)]
pub struct PairedSummary {
    pub pairs: Vec<[f64; 2]>,
    pub rows: [f64; PAIRED_ROW_COUNT],
}

#[derive(Debug)]
pub struct SampleSummary {
    pub first: Address,
    pub last: Address,
    pub values: Vec<f64>,
    pub rows: [f64; SAMPLE_STATISTIC_INDICES.len()],
}

#[derive(Debug)]
enum Data {
    Single(Option<Summary>),
    Multiple(Vec<SampleSummary>),
    Paired {
        by_column: bool,
        swapped: bool,
        summary: Option<PairedSummary>,
    },
}

#[derive(Debug)]
pub struct Analysis {
    // The instance and its cache belong to this range until another launch.
    first: Address,
    last: Address,
    data: Data,
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
            data: Data::Single(None),
        })
    }

    pub fn new_paired(selection: Selection) -> Result<Self, AnalysisError> {
        let (first, last) = selection.bounds();
        let width = usize::from(last.column - first.column) + 1;
        let height = usize::from(last.row - first.row) + 1;
        if width.min(height) != 2
            || width * height > MAX_ANALYSIS_CELLS
            || Address::new(first.column, first.row).is_none()
            || Address::new(last.column, last.row).is_none()
        {
            return Err(AnalysisError::PairedRange);
        }
        Ok(Self {
            first,
            last,
            data: Data::Paired {
                by_column: width == 2,
                swapped: false,
                summary: None,
            },
        })
    }

    pub fn new_multiple(selection: Selection) -> Result<Self, AnalysisError> {
        let (first, last) = selection.bounds();
        let width = usize::from(last.column - first.column) + 1;
        let height = usize::from(last.row - first.row) + 1;
        if width < 2
            || height < 2
            || width * height > MAX_ANALYSIS_CELLS
            || Address::new(first.column, first.row).is_none()
            || Address::new(last.column, last.row).is_none()
        {
            return Err(AnalysisError::SampleRange);
        }
        let samples = (first.column..=last.column)
            .map(|column| SampleSummary {
                first: Address { column, ..first },
                last: Address { column, ..last },
                values: Vec::new(),
                rows: [f64::NAN; SAMPLE_STATISTIC_INDICES.len()],
            })
            .collect();
        Ok(Self {
            first,
            last,
            data: Data::Multiple(samples),
        })
    }

    pub fn is_multiple(&self) -> bool {
        matches!(self.data, Data::Multiple(_))
    }

    pub fn endpoints(&self) -> (Address, Address) {
        (self.first, self.last)
    }

    pub fn summary(&self) -> Option<&Summary> {
        match &self.data {
            Data::Single(summary) => summary.as_ref(),
            Data::Paired { .. } | Data::Multiple(_) => None,
        }
    }

    pub fn is_paired(&self) -> bool {
        matches!(self.data, Data::Paired { .. })
    }

    pub fn swap_axes(&mut self) {
        if let Data::Paired { swapped, .. } = &mut self.data {
            *swapped = !*swapped;
        }
    }

    pub fn cell_count(&self) -> usize {
        (usize::from(self.last.column - self.first.column) + 1)
            * (usize::from(self.last.row - self.first.row) + 1)
    }

    pub fn refresh(&mut self, construction: &Construction) -> Result<&Summary, AnalysisError> {
        let (first, last) = self.endpoints();
        let count = self.cell_count();
        let Data::Single(summary) = &mut self.data else {
            return Err(AnalysisError::Range);
        };
        let cells = (first.column..=last.column)
            .flat_map(move |column| (first.row..=last.row).map(move |row| Address { column, row }));
        let mut buffer = [0.0; MAX_ANALYSIS_CELLS];
        let values = &mut buffer[..count];
        for (cell, value) in cells.zip(values.iter_mut()) {
            match construction
                .spreadsheet_cell(cell)
                .map(|id| construction.value(id))
            {
                Some(Value::Number(current)) if current.is_finite() => *value = current,
                _ => {
                    *summary = None;
                    return Err(AnalysisError::Cells);
                }
            }
        }
        let unchanged = summary.as_ref().is_some_and(|summary| {
            summary
                .values
                .iter()
                .zip(values.iter())
                .all(|(cached, current)| cached.to_bits() == current.to_bits())
        });
        if !unchanged {
            *summary = evaluate(values).map(|rows| Summary {
                values: values.to_vec(),
                rows,
            });
        }
        summary.as_ref().ok_or(AnalysisError::Evaluation)
    }

    pub fn refresh_active(&mut self, construction: &Construction) -> Result<(), AnalysisError> {
        match self.data {
            Data::Single(_) => self.refresh(construction).map(|_| ()),
            Data::Paired { .. } => self.refresh_paired(construction).map(|_| ()),
            Data::Multiple(_) => self.refresh_multiple(construction).map(|_| ()),
        }
    }

    pub fn refresh_multiple(
        &mut self,
        construction: &Construction,
    ) -> Result<&[SampleSummary], AnalysisError> {
        let Data::Multiple(samples) = &mut self.data else {
            return Err(AnalysisError::SampleRange);
        };
        let mut buffer = [0.0; MAX_ANALYSIS_CELLS];
        for sample in samples.iter_mut() {
            let count = numeric_cells(construction, sample.first, sample.last)
                .zip(&mut buffer)
                .map(|(value, slot)| *slot = value)
                .count();
            if count == 0 {
                return Err(AnalysisError::EmptySample);
            }
            let values = &buffer[..count];
            if sample.values.len() != values.len()
                || sample
                    .values
                    .iter()
                    .zip(values)
                    .any(|(old, new)| old.to_bits() != new.to_bits())
            {
                sample.rows = evaluate_sample(values).ok_or(AnalysisError::Evaluation)?;
                sample.values.clear();
                sample.values.extend_from_slice(values);
            }
        }
        Ok(samples)
    }

    pub fn refresh_paired(
        &mut self,
        construction: &Construction,
    ) -> Result<&PairedSummary, AnalysisError> {
        let (first, last) = self.endpoints();
        let Data::Paired {
            by_column,
            swapped,
            summary,
        } = &mut self.data
        else {
            return Err(AnalysisError::PairedRange);
        };
        // The reference selects numeric cells by kind before pairing each axis.
        let by_column = *by_column;
        let axis = |offset| {
            let range = if by_column {
                let column = first.column + offset;
                (Address { column, ..first }, Address { column, ..last })
            } else {
                let row = first.row + offset;
                (Address { row, ..first }, Address { row, ..last })
            };
            numeric_cells(construction, range.0, range.1)
        };
        let start = u16::from(*swapped);
        let mut buffer = [[0.0; 2]; MAX_ANALYSIS_CELLS / 2];
        let mut len = 0;
        for (x, y) in axis(start).zip(axis(1 - start)) {
            buffer[len] = [x, y];
            len += 1;
        }
        let pairs = &buffer[..len];
        if pairs.is_empty() {
            *summary = None;
            return Err(AnalysisError::Pairs);
        }
        let unchanged = summary.as_ref().is_some_and(|summary| {
            summary.pairs.len() == pairs.len()
                && summary.pairs.iter().zip(pairs).all(|(old, new)| {
                    old[0].to_bits() == new[0].to_bits() && old[1].to_bits() == new[1].to_bits()
                })
        });
        if !unchanged {
            *summary = evaluate_pairs(pairs).map(|rows| PairedSummary {
                pairs: pairs.to_vec(),
                rows,
            });
        }
        summary.as_ref().ok_or(AnalysisError::Evaluation)
    }
}

fn numeric_cells(
    construction: &Construction,
    first: Address,
    last: Address,
) -> impl Iterator<Item = f64> + '_ {
    (first.column..=last.column)
        .flat_map(move |column| (first.row..=last.row).map(move |row| Address { column, row }))
        .filter_map(move |address| {
            let id = construction.spreadsheet_cell(address)?;
            match (construction.kind(id), construction.value(id)) {
                (Kind::Number, Value::Number(value)) => Some(value),
                (Kind::Number, Value::Undefined) => Some(f64::NAN),
                _ => None,
            }
        })
}

fn add_numeric_list(
    scratch: &mut Construction,
    label: &str,
    values: impl IntoIterator<Item = f64>,
) -> Option<()> {
    let mut list = format!("{label} = {{");
    for (index, value) in values.into_iter().enumerate() {
        if index != 0 {
            list.push(',');
        }
        // Bind finite values directly to avoid round-tripping through text.
        if value.is_nan() {
            list.push_str("0/0");
        } else if value.is_infinite() {
            list.push_str(if value.is_sign_positive() {
                "1/0"
            } else {
                "-1/0"
            });
        } else {
            let name = format!("{label}{index}");
            scratch.add(&name, Def::Number(value)).ok()?;
            write!(list, "{name}").ok()?;
        }
    }
    list.push('}');
    script::exec(scratch, &list).ok()??;
    Some(())
}

fn evaluate(values: &[f64]) -> Option<[f64; ROW_COUNT]> {
    let mut scratch = Construction::new();
    add_numeric_list(&mut scratch, "data", values.iter().copied())?;
    evaluate_rows(&mut scratch, STATISTICS.map(|(_, expression)| expression))
}

fn evaluate_sample(values: &[f64]) -> Option<[f64; SAMPLE_STATISTIC_INDICES.len()]> {
    let mut scratch = Construction::new();
    add_numeric_list(&mut scratch, "data", values.iter().copied())?;
    evaluate_rows(
        &mut scratch,
        SAMPLE_STATISTIC_INDICES.map(|index| STATISTICS[index].1),
    )
}

fn evaluate_pairs(pairs: &[[f64; 2]]) -> Option<[f64; PAIRED_ROW_COUNT]> {
    if pairs.iter().flatten().any(|value| value.is_nan()) {
        // An undefined singleton leaves every row undefined. With more
        // pairs the reference errors while ranking undefined coordinates.
        return (pairs.len() == 1).then_some([f64::NAN; PAIRED_ROW_COUNT]);
    }
    let mut scratch = Construction::new();
    // Keep both coordinates of a pair even when one is infinite.
    for (label, axis) in [("xs", 0), ("ys", 1)] {
        add_numeric_list(&mut scratch, label, pairs.iter().map(|pair| pair[axis]))?;
    }
    evaluate_rows(&mut scratch, PAIRED_STATISTICS)
}

fn evaluate_rows<const N: usize>(
    scratch: &mut Construction,
    expressions: [&str; N],
) -> Option<[f64; N]> {
    let mut rows = [f64::NAN; N];
    for (index, (expression, row)) in expressions.into_iter().zip(&mut rows).enumerate() {
        let line = format!("s{index} = {expression}");
        let id = script::exec(scratch, &line).ok()??;
        match scratch.value(id) {
            Value::Number(value) => *row = value,
            Value::Undefined => (),
            _ => return None,
        }
    }
    Some(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair_table(source: &str, end: &str) -> PairedSummary {
        let construction = script::load(source).unwrap();
        let mut analysis = Analysis::new_paired(Selection {
            anchor: Address::parse("B2").unwrap(),
            end: Address::parse(end).unwrap(),
        })
        .unwrap();
        let summary = analysis.refresh_paired(&construction).unwrap();
        PairedSummary {
            pairs: summary.pairs.clone(),
            rows: summary.rows,
        }
    }

    #[test]
    fn multiple_samples_keep_nonfinite_numeric_cells() {
        let construction = script::load(
            "B2 = 2\nB3 = 4\nB4 = 6\nB5 = 8\nC2 = 10\nC3 = \"label\"\nC5 = 14\nD2 = 90°\nD4 = 5\nE2 = 9\nF2 = 0/0\nG2 = 1/0\nG3 = 4\nH2 = -1/0\nH3 = 5",
        )
        .unwrap();
        let mut analysis = Analysis::new_multiple(Selection {
            anchor: Address::parse("H5").unwrap(),
            end: Address::parse("B2").unwrap(),
        })
        .unwrap();
        let rows = analysis.refresh_multiple(&construction).unwrap();
        assert_eq!(rows.len(), 7);
        assert_eq!(rows[0].values, [2.0, 4.0, 6.0, 8.0]);
        assert_eq!(rows[1].values, [10.0, 14.0]);
        assert_eq!(rows[2].values, [5.0]);
        assert_eq!(rows[2].rows[4], 5.0);
        assert!(rows[2].rows[3].is_nan());
        assert_eq!(rows[3].values, [9.0]);
        assert_eq!(rows[4].rows[0], 1.0);
        assert!(rows[4].values[0].is_nan());
        assert!(rows[4].rows[1..].iter().all(|value| value.is_nan()));
        assert_eq!(rows[5].rows[1], f64::INFINITY);
        assert_eq!(rows[6].rows[1], f64::NEG_INFINITY);
        assert_eq!(rows[6].rows[8], 5.0);
    }

    #[test]
    fn multiple_samples_require_at_least_two_nonempty_samples() {
        let first = Address::parse("A1").unwrap();
        for end in ["B1", "A2", "D100"] {
            assert!(matches!(
                Analysis::new_multiple(Selection {
                    anchor: first,
                    end: Address::parse(end).unwrap(),
                }),
                Err(AnalysisError::SampleRange),
            ));
        }

        let mut analysis = Analysis::new_multiple(Selection {
            anchor: first,
            end: Address::parse("B2").unwrap(),
        })
        .unwrap();
        let only_one = script::load("A1 = 2\nA2 = 4").unwrap();
        let both = script::load("A1 = 2\nA2 = 4\nB1 = 9").unwrap();
        assert!(matches!(
            analysis.refresh_multiple(&only_one),
            Err(AnalysisError::EmptySample)
        ));
        assert_eq!(analysis.refresh_multiple(&both).unwrap()[1].values, [9.0]);
        assert!(matches!(
            analysis.refresh_multiple(&only_one),
            Err(AnalysisError::EmptySample)
        ));
        assert_eq!(analysis.refresh_multiple(&both).unwrap()[1].values, [9.0]);
    }

    #[test]
    fn paired_columns_and_rows_agree_with_recorded_statistics() {
        let vertical = pair_table("B2 = 2\nB3 = 3\nB4 = 4\nC2 = 20\nC3 = 30\nC4 = 40", "C4");
        let horizontal = pair_table("B2 = 2\nC2 = 3\nD2 = 4\nB3 = 20\nC3 = 30\nD3 = 40", "D3");
        assert_eq!(vertical.pairs, [[2.0, 20.0], [3.0, 30.0], [4.0, 40.0]]);
        assert_eq!(vertical.pairs, horizontal.pairs);
        let expected = [3.0, 30.0, 1.0, 10.0, 1.0, 1.0, 2.0, 200.0, 20.0];
        for summary in [vertical, horizontal] {
            for (actual, expected) in summary.rows.into_iter().zip(expected) {
                assert!((actual - expected).abs() < 1e-9, "{actual} vs {expected}");
            }
        }
    }

    #[test]
    fn paired_columns_drop_missing_or_text_cells_before_joining_axes() {
        let mut construction = script::load("B2 = 2\nB3 = 3\nB4 = 4\nC2 = 20\nC4 = 40").unwrap();
        let mut analysis = Analysis::new_paired(Selection {
            anchor: Address::parse("C4").unwrap(),
            end: Address::parse("B2").unwrap(),
        })
        .unwrap();
        let first = analysis.refresh_paired(&construction).unwrap();
        assert_eq!(first.pairs, [[2.0, 20.0], [3.0, 40.0]]);
        assert_eq!(first.rows[0], 2.5);
        script::exec(&mut construction, "C3 = 30").unwrap();
        let filled = analysis.refresh_paired(&construction).unwrap();
        assert_eq!(filled.pairs, [[2.0, 20.0], [3.0, 30.0], [4.0, 40.0]]);
        assert_eq!(filled.rows[0], 3.0);
        script::exec(&mut construction, "C3 = \"missing\"").unwrap();
        analysis.swap_axes();
        let swapped = analysis.refresh_paired(&construction).unwrap();
        assert_eq!(swapped.pairs, [[20.0, 2.0], [40.0, 3.0]]);
        assert_eq!(swapped.rows[0], 30.0);
        assert_eq!(swapped.rows[1], 2.5);
    }

    #[test]
    fn paired_statistics_skip_angles_and_keep_infinite_numbers() {
        let angle = pair_table("B2 = 2\nB3 = 90°\nB4 = 4\nC2 = 20\nC3 = 30\nC4 = 40", "C4");
        assert_eq!(angle.pairs, [[2.0, 20.0], [4.0, 30.0]]);
        assert_eq!(angle.rows[1], 25.0);

        let infinite = pair_table(
            "B2 = 2\nB3 = 1 / 0\nB4 = 4\nC2 = 20\nC3 = 30\nC4 = 40",
            "C4",
        );
        assert_eq!(
            infinite.pairs,
            [[2.0, 20.0], [f64::INFINITY, 30.0], [4.0, 40.0]]
        );
        for (actual, expected) in infinite.rows.into_iter().zip([
            f64::INFINITY,
            30.0,
            f64::NAN,
            10.0,
            f64::NAN,
            0.5,
            f64::NAN,
            200.0,
            f64::NAN,
        ]) {
            assert!(actual == expected || (actual.is_nan() && expected.is_nan()));
        }
    }

    #[test]
    fn undefined_numeric_pair_clears_the_summary_without_shifting_the_axes() {
        let mut construction = script::load("B2 = 2\nB3 = 3\nC2 = 20\nC3 = 30").unwrap();
        let mut analysis = Analysis::new_paired(Selection {
            anchor: Address::parse("B2").unwrap(),
            end: Address::parse("C3").unwrap(),
        })
        .unwrap();
        assert_eq!(
            analysis.refresh_paired(&construction).unwrap().pairs.len(),
            2
        );
        script::exec(&mut construction, "B2 = 0 / 0").unwrap();
        assert!(matches!(
            analysis.refresh_paired(&construction),
            Err(AnalysisError::Evaluation)
        ));
        script::exec(&mut construction, "B2 = 4").unwrap();
        let restored = analysis.refresh_paired(&construction).unwrap();
        assert_eq!(restored.pairs, [[4.0, 20.0], [3.0, 30.0]]);
    }

    #[test]
    fn undefined_singleton_keeps_the_statistics_table() {
        let construction = script::load("B2 = 0 / 0\nC2 = 20").unwrap();
        let mut analysis = Analysis::new_paired(Selection {
            anchor: Address::parse("B2").unwrap(),
            end: Address::parse("C3").unwrap(),
        })
        .unwrap();
        for axis in 0..2 {
            let summary = analysis.refresh_paired(&construction).unwrap();
            let [pair] = summary.pairs.as_slice() else {
                panic!("expected one pair");
            };
            assert!(pair[axis].is_nan());
            assert_eq!(pair[1 - axis], 20.0);
            assert!(summary.rows.iter().all(|value| value.is_nan()));
            analysis.swap_axes();
        }
    }

    #[test]
    fn pairing_stops_at_the_selection_budget() {
        let anchor = Address::parse("A1").unwrap();
        let select = |end| Selection {
            anchor,
            end: Address::parse(end).unwrap(),
        };
        assert_eq!(
            Analysis::new_paired(select("B128")).unwrap().cell_count(),
            256
        );
        assert!(matches!(
            Analysis::new_paired(select("B129")),
            Err(AnalysisError::PairedRange)
        ));
    }
}
