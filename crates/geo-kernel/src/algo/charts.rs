// Parts translated from GeoGebra, commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
// source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoBarChart.java
// source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoBoxPlot.java
// source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoMedian.java
// source/shared/common/src/main/java/org/geogebra/common/euclidian/draw/DrawBarGraph.java
// source/shared/common/src/main/java/org/geogebra/common/euclidian/draw/DrawBoxPlot.java
// Copyright (c) GeoGebra GmbH. Licensed under the EUPL 1.2.

use crate::expr::{self, Element, IntersectionWork, List};
use crate::kernel::{Construction, Def, Error, Id, IdList, Kind, Value};
use crate::script::split_call;

mod histogram;
mod plots;

use self::histogram::Histogram;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Command {
    Bar,
    Box,
    Histogram,
    HistogramRight,
    FrequencyPolygon,
    Stick,
    Step,
    Line,
    Dot,
    Residual,
    NormalQuantile,
    Pie,
}

impl Command {
    fn of(name: &str) -> Option<Self> {
        Some(match name {
            "BarChart" => Self::Bar,
            "BoxPlot" => Self::Box,
            "Histogram" => Self::Histogram,
            "HistogramRight" => Self::HistogramRight,
            "FrequencyPolygon" => Self::FrequencyPolygon,
            "StickGraph" => Self::Stick,
            "StepGraph" => Self::Step,
            "LineGraph" => Self::Line,
            "DotPlot" => Self::Dot,
            "ResidualPlot" => Self::Residual,
            "NormalQuantilePlot" => Self::NormalQuantile,
            "PieChart" => Self::Pie,
            _ => return None,
        })
    }

    fn forms(self) -> &'static [&'static [Kind]] {
        use Kind::{Boolean as B, Function as F, List as L, Number as N, Point as P};
        const HISTOGRAM: &[&[Kind]] = &[
            &[L, L],
            &[L, L, B],
            &[L, L, B, N],
            &[B, L, L, B],
            &[B, L, L, B, N],
            &[B, L, L, L, B],
            &[B, L, L, L, B, N],
        ];
        match self {
            Self::Bar => &[&[L, N], &[L, L], &[N, N, L], &[L, L, N], &[L, N, N]],
            Self::Box => &[
                &[N, N, L],
                &[N, N, L, B],
                &[N, N, L, L, B],
                &[N, N, N, N, N, N, N],
            ],
            Self::Histogram | Self::HistogramRight | Self::FrequencyPolygon => HISTOGRAM,
            Self::Line => &[&[L, L]],
            Self::Dot => &[&[L], &[L, B], &[L, N], &[L, B, N]],
            Self::Residual => &[&[L, F]],
            Self::NormalQuantile => &[&[L]],
            Self::Pie => &[&[L], &[L, P], &[L, P, N]],
            Self::Stick => &[&[L], &[L, B], &[L, L], &[L, L, B]],
            Self::Step => &[
                &[L],
                &[L, B],
                &[L, L],
                &[L, B, N],
                &[L, L, B],
                &[L, L, B, N],
            ],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChartDef {
    command: Command,
    pub(crate) args: IdList,
}

impl ChartDef {
    pub(crate) fn kind(self) -> Kind {
        match self.command {
            Command::FrequencyPolygon => Kind::PolyLine,
            Command::Line => Kind::Function,
            Command::Dot | Command::Residual | Command::NormalQuantile => Kind::List,
            Command::Pie => Kind::PieChart,
            _ => Kind::Number,
        }
    }

    pub(crate) fn check(self, c: &Construction) -> Result<(), Error> {
        let args = c.list(self.args);
        if self.command.forms().iter().any(|form| {
            form.len() == args.len()
                && form
                    .iter()
                    .zip(args)
                    .all(|(&kind, &id)| c.exists(id) && same_kind(kind, c.kind(id)))
        }) {
            Ok(())
        } else {
            Err(Error::UnknownObject)
        }
    }

    /// Each scan, sort and produced mark consumes work before it runs.
    pub(crate) fn compute(self, c: &Construction) -> ChartData {
        let work = IntersectionWork::new();
        let inputs = Inputs { c, work: &work };
        let args = c.list(self.args);
        let data = match self.command {
            Command::Bar => inputs.bar_chart(args),
            Command::Box => inputs.box_plot(args),
            Command::Histogram | Command::HistogramRight | Command::FrequencyPolygon => {
                Histogram::compute(&inputs, args, self.command == Command::HistogramRight)
                    .and_then(|hist| hist.output(self.command, &work))
            }
            Command::Stick | Command::Step => inputs.stick_or_step(args, self.command),
            Command::Line => inputs.line_graph(args),
            Command::Dot => inputs.dot_plot(args),
            Command::Residual => inputs.residual_plot(args),
            Command::NormalQuantile => inputs.normal_quantile(args),
            Command::Pie => inputs.pie_chart(args),
        };
        data.unwrap_or_default()
    }
}

fn same_kind(expected: Kind, actual: Kind) -> bool {
    expected == actual || (expected == Kind::Number && actual.is_number())
}

pub(crate) fn define(
    c: &mut Construction,
    label: &str,
    name: &str,
    args: &[&str],
) -> Option<Result<Id, String>> {
    let command = Command::of(name)?;
    Some((|| {
        if command == Command::NormalQuantile
            && args.first().is_some_and(|arg| {
                !arg.starts_with('{') && c.id(arg).is_none_or(|id| c.kind(id) != Kind::List)
            })
        {
            let source = format!("{{{}}}", args.join(", "));
            return define(c, label, name, &[&source]).expect("known command");
        }
        if command == Command::Bar {
            if let [a, b, expression, variable, from, to, step @ ..] = args {
                if step.len() <= 1 {
                    let step = step.first().map_or(String::new(), |s| format!(", {s}"));
                    let sequence =
                        format!("Sequence({expression}, {variable}, {from}, {to}{step})");
                    return define(c, label, name, &[a, b, &sequence]).expect("known command");
                }
            }
        }
        for form in command.forms() {
            let prefix = matches!(command, Command::Histogram | Command::HistogramRight)
                && args.len() == 6
                && **form
                    == [
                        Kind::Boolean,
                        Kind::List,
                        Kind::List,
                        Kind::Boolean,
                        Kind::Number,
                    ];
            if (form.len() != args.len() && !prefix)
                || (command == Command::FrequencyPolygon && form.len() == 6)
            {
                continue;
            }
            let checkpoint = c.checkpoint();
            let mut ids = [Id(0); 7];
            let mut resolved = true;
            for (i, (&arg, &kind)) in args.iter().zip(*form).enumerate() {
                let id = c
                    .id(arg)
                    .filter(|&id| same_kind(kind, c.kind(id)))
                    .or_else(|| {
                        let label = format!("\0chart_arg{}", c.len());
                        if kind == Kind::Function {
                            expr::define_function(c, &label, arg, false).ok()
                        } else {
                            expr::argument_object(c, arg, &label, kind)
                                .then(|| c.id(&label))
                                .flatten()
                        }
                    });
                if let Some(id) = id {
                    ids[i] = id;
                } else {
                    resolved = false;
                    break;
                }
            }
            if resolved {
                return c
                    .add_with_list(label, &ids[..form.len()], |args| {
                        Def::Chart(ChartDef { command, args })
                    })
                    .map_err(|e| e.to_string());
            }
            c.discard_since(checkpoint);
        }
        Err(format!("unsupported {name} arguments"))
    })())
}

pub(crate) fn exec(c: &mut Construction, label: &str, rhs: &str) -> Option<Result<Id, String>> {
    let name = rhs.split_once('(')?.0.trim();
    Command::of(name)?;
    let (name, args) = split_call(rhs)?;
    define(c, label, name, &args)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ChartMark {
    Bar { left: f64, right: f64, height: f64 },
    Segment([(f64, f64); 2]),
    Point { at: (f64, f64), open: bool },
    Cross((f64, f64)),
    Box { x: [f64; 2], y: [f64; 2] },
}

#[derive(Debug)]
pub struct ChartData {
    pub(crate) value: Value,
    pub marks: Vec<ChartMark>,
    pub(crate) vertices: Vec<(f64, f64)>,
    pub(crate) slots: Vec<expr::Slot>,
    pub dot_stack: Option<DotStack>,
    pub pie: Option<Pie>,
}

impl Default for ChartData {
    fn default() -> Self {
        Self {
            value: Value::Undefined,
            marks: Vec::new(),
            vertices: Vec::new(),
            slots: Vec::new(),
            dot_stack: None,
            pie: None,
        }
    }
}

impl ChartData {
    pub(crate) fn same(&self, other: &Self) -> bool {
        self.value == other.value
            && self.marks == other.marks
            && self.vertices == other.vertices
            && self.dot_stack == other.dot_stack
            && match (&self.pie, &other.pie) {
                (Some(a), Some(b)) => a.same(b),
                (None, None) => true,
                _ => false,
            }
            && expr::same_slots(&self.slots, &other.slots)
    }

    /// The world extent used to fit a chart to the initial view.
    pub fn bounds(&self) -> Option<[f64; 4]> {
        let mut bounds = [
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ];
        let mut include = |(x, y): (f64, f64)| {
            if x.is_finite() && y.is_finite() {
                bounds[0] = bounds[0].min(x);
                bounds[1] = bounds[1].min(y);
                bounds[2] = bounds[2].max(x);
                bounds[3] = bounds[3].max(y);
            }
        };
        for &point in &self.vertices {
            include(point);
        }
        for &mark in &self.marks {
            match mark {
                ChartMark::Bar {
                    left,
                    right,
                    height,
                } => {
                    include((left, 0.0));
                    include((right, height));
                }
                ChartMark::Box {
                    x: [left, right],
                    y: [bottom, top],
                } => {
                    include((left, bottom));
                    include((right, top));
                }
                ChartMark::Segment([a, b]) => {
                    include(a);
                    include(b);
                }
                ChartMark::Cross(at) | ChartMark::Point { at, .. } => include(at),
            }
        }
        if let Some(pie) = &self.pie {
            let (x, y) = pie.center;
            include((x - pie.radius, y - pie.radius));
            include((x + pie.radius, y + pie.radius));
        }
        bounds[0].is_finite().then_some(bounds)
    }

    pub(crate) fn function_domain(&self) -> Option<(f64, f64)> {
        let left = self.vertices.first()?.0;
        let right = self.vertices.last()?.0;
        (self.value == Value::Function).then_some((left, right))
    }

    pub(crate) fn sample(&self, x: f64) -> f64 {
        if !x.is_finite() {
            return f64::NAN;
        }
        match self
            .vertices
            .binary_search_by(|p| p.0.partial_cmp(&x).expect("finite knots"))
        {
            Ok(i) => self.vertices[i].1,
            Err(i) if i > 0 && i < self.vertices.len() => {
                let ((a, y), (b, z)) = (self.vertices[i - 1], self.vertices[i]);
                y + (x - a) / (b - a) * (z - y)
            }
            Err(_) => f64::NAN,
        }
    }

    fn number(value: f64, marks: Vec<ChartMark>) -> Option<Self> {
        value.is_finite().then(|| Self {
            value: Value::Number(value),
            marks,
            ..Self::default()
        })
    }
}

struct Inputs<'a> {
    c: &'a Construction,
    work: &'a IntersectionWork,
}

impl Inputs<'_> {
    fn list(&self, id: Id) -> Option<List<'_>> {
        let list = self.c.elements(id)?;
        self.work.charge(list.len() as u64).then_some(list)
    }

    fn number(&self, id: Id) -> Option<f64> {
        match self.c.value(id) {
            Value::Number(v) if v.is_finite() => Some(v),
            _ => None,
        }
    }

    fn flag(&self, id: Id) -> bool {
        self.c.value(id) == Value::Bool(true)
    }

    fn numbers(&self, id: Id) -> Option<Vec<f64>> {
        self.list(id)?.iter().map(number).collect()
    }

    fn bar_chart(&self, args: &[Id]) -> Option<ChartData> {
        match args {
            &[a, b, values] if self.c.kind(a).is_number() => {
                let (a, b) = (self.number(a)?, self.number(b)?);
                let list = self.list(values)?;
                let count = list.len().min(10_000);
                if count == 0 || !self.work.charge(count as u64) {
                    return None;
                }
                let width = (b - a) / count as f64;
                let mut sum = 0.0;
                let mut bars = Vec::with_capacity(count);
                for (i, item) in list.iter().take(count).enumerate() {
                    let h = match item {
                        Element::Value(Kind::Number | Kind::Angle, _) => number(item)?,
                        _ => 0.0,
                    };
                    sum += h;
                    bars.push(bar(a + i as f64 * width, width, h));
                }
                ChartData::number(sum * width, bars)
            }
            &[data, width] | &[data, width, _] if self.c.kind(width).is_number() => {
                let width = self.number(width).filter(|w| *w >= 0.0)?;
                let scale = args.get(2).map_or(Some(1.0), |&id| self.number(id))?;
                let rows = frequencies(self.list(data)?, self.work)?;
                self.bars(
                    rows.into_iter().map(|(x, count)| (x, count as f64 * scale)),
                    width,
                )
            }
            &[x, y] | &[x, y, _] => {
                let rows = self.xy(x, y, true)?;
                let width = if let Some(&id) = args.get(2) {
                    self.number(id).filter(|v| *v >= 0.0)?
                } else {
                    match rows.as_slice() {
                        [_, (x1, _), ..]
                            if matches!(
                                self.c.elements(x)?.get(1)?,
                                Element::Value(Kind::Number | Kind::Angle, _)
                            ) =>
                        {
                            *x1 - rows[0].0
                        }
                        [_, _, ..] => 1.0,
                        [_] => 0.5,
                        [] => return None,
                    }
                };
                self.bars(rows.into_iter(), width)
            }
            _ => None,
        }
    }

    fn xy(&self, x: Id, y: Id, equal_length: bool) -> Option<Vec<(f64, f64)>> {
        let (x, y) = (self.list(x)?, self.list(y)?);
        if y.len() < x.len() || (equal_length && x.len() != y.len()) {
            return None;
        }
        x.iter()
            .zip(y.iter())
            .enumerate()
            .map(|(i, (x, y))| {
                let x = match x {
                    Element::Value(Kind::Number | Kind::Angle, _) => number(x)?,
                    _ => (i + 1) as f64,
                };
                Some((x, number(y)?))
            })
            .collect()
    }

    fn bars(
        &self,
        rows: impl ExactSizeIterator<Item = (f64, f64)>,
        width: f64,
    ) -> Option<ChartData> {
        if !self.work.charge(rows.len() as u64) {
            return None;
        }
        let mut sum = 0.0;
        let marks = rows
            .map(|(x, height)| {
                sum += height;
                bar(x - width * 0.5, width, height)
            })
            .collect();
        ChartData::number(sum.abs() * width, marks)
    }

    fn box_plot(&self, args: &[Id]) -> Option<ChartData> {
        let &[offset, height, data, ..] = args else {
            return None;
        };
        let (offset, height) = (self.number(offset)?, self.number(height)?);
        let y = [offset - height, offset + height];
        let mut marks = Vec::new();
        let [min, q1, median, q3, max] = match args {
            &[_, _, min, q1, med, q3, max] => [
                self.number(min)?,
                self.number(q1)?,
                self.number(med)?,
                self.number(q3)?,
                self.number(max)?,
            ],
            _ => {
                let values = self.numbers(data)?;
                if values.len() < 2 || !sort_work(self.work, values.len()) {
                    return None;
                }
                let weighted = args.len() == 5;
                let weights = if weighted {
                    self.numbers(args[3])?
                } else {
                    vec![1.0; values.len()]
                };
                if weights.len() != values.len() || weights.iter().any(|&n| n < 0.0) {
                    return None;
                }
                let mut samples: Vec<_> = values.into_iter().zip(weights).collect();
                samples.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
                let mut total = 0usize;
                let mut ends = Vec::with_capacity(samples.len());
                for &(_, weight) in &samples {
                    total = total.checked_add(weight as i32 as usize)?;
                    ends.push(total);
                }
                if total < 2 || total > i32::MAX as usize {
                    return None;
                }
                let at = |i| samples[ends.partition_point(|&end| end <= i)].0;
                let median_of = |start: usize, size: usize| {
                    let middle = start + size / 2;
                    if size.is_multiple_of(2) {
                        (at(middle - 1) + at(middle)) * 0.5
                    } else {
                        at(middle)
                    }
                };
                let q1 = median_of(0, total / 2);
                let median = median_of(0, total);
                let q3 = median_of(total.div_ceil(2), total / 2);
                let (mut min, mut max) = (f64::MAX, -f64::MAX);
                let outliers = args
                    .get(if weighted { 4 } else { 3 })
                    .is_some_and(|&id| self.flag(id));
                let cutoff = 1.5 * (q3 - q1);
                for (x, weight) in samples {
                    if weight <= 0.0 {
                        continue;
                    }
                    if outliers && (x < q1 - cutoff || x > q3 + cutoff) {
                        marks.push(ChartMark::Cross((x, offset)));
                    } else {
                        min = min.min(x);
                        max = max.max(x);
                    }
                }
                [min, q1, median, q3, max]
            }
        };
        marks.extend([
            ChartMark::Box { x: [q1, q3], y },
            ChartMark::Segment([(min, offset), (q1, offset)]),
            ChartMark::Segment([(q3, offset), (max, offset)]),
            ChartMark::Segment([(min, y[0]), (min, y[1])]),
            ChartMark::Segment([(median, y[0]), (median, y[1])]),
            ChartMark::Segment([(max, y[0]), (max, y[1])]),
        ]);
        ChartData::number(median, marks)
    }

    fn stick_or_step(&self, args: &[Id], command: Command) -> Option<ChartData> {
        let &[first, ..] = args else { return None };
        let list = self.list(first)?;
        let points = matches!(list.get(0), Some(Element::Value(Kind::Point, _)));
        let (rows, style) = if points {
            let rows = list.iter().map(point).collect::<Option<Vec<_>>>()?;
            (rows, &args[1..])
        } else {
            let &second = args.get(1).filter(|&&id| self.c.kind(id) == Kind::List)?;
            (self.xy(first, second, false)?, &args[2..])
        };
        if !self.work.charge(5 * rows.len() as u64) {
            return None;
        }
        let value = rows.iter().map(|&(_, y)| y).sum::<f64>();
        let flag = style.first().is_some_and(|&id| self.flag(id));
        let mut marks = Vec::new();
        if command == Command::Stick {
            for &(x, y) in &rows {
                marks.push(ChartMark::Segment([
                    if flag { (0.0, y) } else { (x, 0.0) },
                    (x, y),
                ]));
                marks.push(ChartMark::Point {
                    at: (x, y),
                    open: false,
                });
            }
        } else {
            let style = style.get(1).and_then(|&id| self.number(id)).unwrap_or(0.0) as i32;
            for window in rows.windows(2) {
                let [(x, y), (next_x, next_y)] = [window[0], window[1]];
                marks.push(ChartMark::Segment([(x, y), (next_x, y)]));
                if flag {
                    marks.push(ChartMark::Segment([(next_x, y), (next_x, next_y)]));
                }
                if matches!(style, -2 | 1 | 2) {
                    marks.push(ChartMark::Point {
                        at: (next_x, y),
                        open: style == -2,
                    });
                }
            }
            if matches!(style, -2 | -1 | 2) {
                for at in rows {
                    marks.push(ChartMark::Point {
                        at,
                        open: style == 2,
                    });
                }
            }
        }
        ChartData::number(if points { value } else { value.abs() }, marks)
    }
}

fn bar(left: f64, width: f64, height: f64) -> ChartMark {
    if width <= 0.0 {
        ChartMark::Segment([(left, 0.0), (left, height)])
    } else {
        ChartMark::Bar {
            left,
            right: left + width,
            height,
        }
    }
}

fn number(item: Element<'_>) -> Option<f64> {
    match item {
        Element::Value(_, Value::Number(v)) if v.is_finite() => Some(v),
        _ => None,
    }
}

fn point(item: Element<'_>) -> Option<(f64, f64)> {
    match item {
        Element::Value(Kind::Point, Value::Point { x, y }) => Some((x, y)),
        _ => None,
    }
}

/// Pay for copying, sorting and scanning a vector before allocating it.
fn sort_work(work: &IntersectionWork, count: usize) -> bool {
    work.charge((count as u64) * (8 + u64::from(usize::BITS - count.leading_zeros())))
}

fn frequencies(list: List<'_>, work: &IntersectionWork) -> Option<Vec<(f64, usize)>> {
    if !sort_work(work, list.len()) {
        return None;
    }
    let mut result: Vec<(f64, usize)> = Vec::new();
    if matches!(list.get(0), Some(Element::Text(_))) {
        let mut values = list
            .iter()
            .map(|element| match element {
                Element::Text(text) => Some(text),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()?;
        values.sort_unstable();
        for group in values.chunk_by(|a, b| a == b) {
            result.push(((result.len() + 1) as f64, group.len()));
        }
    } else {
        let mut values = list.iter().map(number).collect::<Option<Vec<_>>>()?;
        values.sort_unstable_by(f64::total_cmp);
        for group in values.chunk_by(|a, b| a == b) {
            result.push((group[0], group.len()));
        }
    }
    Some(result)
}

/// Values of a pie in clockwise order starting at twelve o'clock.
#[derive(Debug)]
pub struct Pie {
    pub center: (f64, f64),
    pub radius: f64,
    pub fractions: Vec<f64>,
}

impl Pie {
    fn same(&self, other: &Self) -> bool {
        self.center == other.center
            && self.radius == other.radius
            && self.fractions.len() == other.fractions.len()
            && self
                .fractions
                .iter()
                .zip(&other.fractions)
                .all(|(a, b)| a.to_bits() == b.to_bits())
    }
}

/// Dot density uses distances measured in screen pixels.
#[derive(Debug, PartialEq)]
pub struct DotStack {
    groups: Vec<(f64, usize)>,
    height_scale: f64,
}

impl DotStack {
    pub fn dots(&self, view_scale: f64) -> impl Iterator<Item = (f64, f64)> + '_ {
        let radius = 5.0;
        let spacing = 2.0 * radius / view_scale;
        self.groups
            .iter()
            .flat_map(|&(x, count)| std::iter::repeat_n(x, count))
            .scan((f64::NEG_INFINITY, 0usize), move |(stack, count), x| {
                if x > *stack + spacing {
                    *stack = x;
                    *count = 0;
                }
                let y = (radius + 2.0 * *count as f64 * radius * self.height_scale) / view_scale;
                *count += 1;
                Some((*stack, y))
            })
    }
}
