// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoStats1D.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoMedian.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoQ1.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoQ3.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdOneListFunction.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Sums and descriptive statistics.

use crate::expr::eval::{number_of, slot_val, Ctx, Lists, Val};
use crate::expr::parse::{list_form, Parser};
use crate::expr::types::of_numbers;
use crate::expr::{Bin, Elem, ListOp, Node, Ty, NONE};

/// The commands of this group (see `commands::is_command`).
pub(super) const NAMES: &[&str] = &[
    "Sum",
    "Mean",
    "mean",
    "Variance",
    "SD",
    "stdevp",
    "SampleVariance",
    "SampleSD",
    "stdev",
    "MAD",
    "mad",
    "Median",
    "Q1",
    "Quartile1",
    "Q3",
    "Quartile3",
];

/// These statistics accept a second list of frequencies or separate numbers.
pub(super) fn is_descriptive(op: ListOp) -> bool {
    matches!(
        op,
        ListOp::Mean
            | ListOp::Variance
            | ListOp::Sd
            | ListOp::SampleVariance
            | ListOp::SampleSd
            | ListOp::Mad
            | ListOp::Median
            | ListOp::Q1
            | ListOp::Q3
    )
}

/// List statistics, plus the fold and truncation forms of `Sum`.
pub(super) fn parse(p: &mut Parser, text: &str) -> Result<Option<u32>, String> {
    let op = match text {
        "Sum" => ListOp::Sum,
        "Mean" | "mean" => ListOp::Mean,
        "Variance" => ListOp::Variance,
        "SD" | "stdevp" => ListOp::Sd,
        "SampleVariance" => ListOp::SampleVariance,
        "SampleSD" | "stdev" => ListOp::SampleSd,
        "MAD" | "mad" => ListOp::Mad,
        "Median" => ListOp::Median,
        "Q1" | "Quartile1" => ListOp::Q1,
        "Q3" | "Quartile3" => ListOp::Q3,
        _ => return Ok(None),
    };
    // `Sum(expression, variable, from, to)`: the four-argument fold of
    // `CmdSum.processSymb`, with the variable bound locally, so it shadows
    // an object of the same name (`Sum(k^2, k, 1, 5)` with an existing
    // `k`). Over the variable `x` the expression stays a function of it.
    if op == ListOp::Sum {
        if let Some(node) = p.fold(text)? {
            return Ok(Some(node));
        }
    }
    p.typed_list_command(|p, arguments| {
        let args: Vec<_> = arguments.iter().map(|&(root, _)| root).collect();
        let lists: Vec<_> = arguments
            .iter()
            .map(|&(_, ty)| matches!(ty, Some(Ty::List(_))))
            .collect();
        if let (&[l, f], &[true, true]) = (&args[..], &lists[..]) {
            return Ok(Some(Node::ListOp(op, l, f)));
        }
        if op != ListOp::Sum {
            // wrapInList requires GeoClass.NUMERIC. Only dimension one
            // represents an angle object (MyDouble.isAngle).
            if args.len() >= 2
                && arguments
                    .iter()
                    .all(|&(_, ty)| matches!(ty, Some(Ty::Num(dimension)) if dimension != 1))
            {
                let list = p.wrap_list(&args);
                return Ok(Some(Node::ListOp(op, list, NONE)));
            }
            return list_form(text, op, false, &args, &lists);
        }
        // `Sum(a, b, ...)` of anything but a list folds the arguments,
        // which GeoGebra wraps in a list of the class of the first
        // (`CmdSum`). Functions splice their bodies into one sum
        // (`FunctionFold`): applying them at a shared variable does the
        // same. Of two arguments the fold never runs: `CmdSum` wants a
        // list first (`argErr`).
        if args.len() >= 3 && !lists[0] {
            if arguments.iter().any(|&(_, ty)| ty == Some(Ty::Num(1))) {
                return Ok(None);
            }
            // The variadic coordinate form wraps only point arguments.
            // Vectors are accepted when supplied as elements of a list.
            if arguments.iter().any(|&(a, ty)| {
                ty == Some(Ty::Vec) && p.coordinate_kind(a) == Some(crate::Kind::Vector)
            }) {
                return Err("Sum needs vectors to be in a list".into());
            }
            let functions: Option<Vec<_>> = args.iter().map(|&a| p.applied_function(a)).collect();
            if let Some(ids) = functions {
                let var = p.push_var();
                let applied: Vec<u32> =
                    ids.iter().map(|&id| p.push(Node::Apply(id, var))).collect();
                let (last, init) = applied.split_last().unwrap();
                let mut acc = init[0];
                for &a in &init[1..] {
                    acc = p.push(Node::Bin(Bin::Add, acc, a));
                }
                return Ok(Some(Node::Bin(Bin::Add, acc, *last)));
            }
            let list = p.wrap_list(&args);
            return Ok(Some(Node::ListOp(op, list, NONE)));
        }
        list_form(text, op, true, &args, &lists)
    })
}

/// The type of a list command of this group (see `commands::list_type`).
pub(super) fn list_type(
    op: ListOp,
    elem: Elem,
    numbers: bool,
    _nodes: &[Node],
    _tys: &[Ty],
    _l: u32,
    n: u32,
) -> Result<Option<Ty>, String> {
    Ok(Some(match op {
        // `AlgoStats1D` reads numbers; `Sum` adds points and vectors too
        ListOp::Sum if elem == Elem::Vec => Ty::Vec,
        ListOp::Sum => of_numbers(numbers)?,
        ListOp::Mean if n == NONE => of_numbers(numbers)?,
        op if is_descriptive(op) => Ty::Num(0),
        _ => return Ok(None),
    }))
}

/// The value of a list command of this group (see `commands::list_value`).
pub(super) fn list_value<const M: bool>(
    ctx: &Ctx<'_, M>,
    i: u32,
    op: ListOp,
    l: u32,
    n: u32,
) -> Option<Val> {
    Some(match op {
        ListOp::Sum if ctx.e.tys[i as usize] == Ty::Vec => ctx.sum_vectors(l, n),
        ListOp::Sum => Val::N(ctx.stats(op, l, n)),
        ListOp::Mean if n == NONE => Val::N(ctx.stats(op, l, n)),
        ListOp::Median | ListOp::Q1 | ListOp::Q3 => {
            Val::N(ctx.rank_statistic(op, l, n).unwrap_or(f64::NAN))
        }
        op if is_descriptive(op) => Val::N(ctx.moments(op, l, n).unwrap_or(f64::NAN)),
        _ => return None,
    })
}

impl<const M: bool> Ctx<'_, M> {
    /// `AlgoStats1D` takes fractional weights and uses midpoints when
    /// there is one more class boundary than frequency.
    fn moments(&self, op: ListOp, l: u32, f: u32) -> Option<f64> {
        let lists = self.lists?;
        let (start, len) = self.list(l)?;
        let (frequencies, grouped, size) = if f == NONE {
            (None, false, len)
        } else {
            let (fstart, flen) = self.list(f)?;
            if len != flen && len != flen + 1 {
                return None;
            }
            (Some(fstart), len > flen, flen)
        };
        let reads = 1 + u64::from(frequencies.is_some()) + u64::from(grouped);
        let passes = if op == ListOp::Mad { 2 } else { 1 };
        if size == 0 || !lists.charge(u64::from(size) * reads * passes) {
            return None;
        }
        let samples = (0..size).map(|k| {
            let first = number_of(lists.read(self.c, start + k))?;
            let value = if grouped {
                let next = number_of(lists.read(self.c, start + k + 1))?;
                (first + next) / 2.0
            } else {
                first
            };
            let weight = match frequencies {
                Some(fstart) => number_of(lists.read(self.c, fstart + k))?,
                None => 1.0,
            };
            if weight < 0.0 {
                return None;
            }
            Some((value, weight))
        });
        let (mut sum, mut squares, mut total) = (0.0, 0.0, 0.0);
        for sample in samples.clone() {
            let (value, weight) = sample?;
            let weighted = value * weight;
            sum += weighted;
            squares += value * weighted;
            total += weight;
        }
        let mean = sum / total;
        Some(match op {
            ListOp::Mean => mean,
            ListOp::Mad => {
                let mut deviation = 0.0;
                for sample in samples {
                    let (value, weight) = sample?;
                    deviation += (mean - value).abs() * weight;
                }
                deviation / total
            }
            ListOp::Variance => squares / total - mean * mean,
            ListOp::Sd => (squares / total - mean * mean).sqrt(),
            ListOp::SampleVariance => (squares - sum * sum / total) / (total - 1.0),
            ListOp::SampleSd => ((squares - sum * sum / total) / (total - 1.0)).sqrt(),
            _ => unreachable!(),
        })
    }

    /// Median, or the median of the lower/upper half (excluding the middle
    /// observation for odd sizes). Carry each value with its count so work
    /// depends on the supplied list length, not the sum of the frequencies.
    fn rank_statistic(&self, op: ListOp, l: u32, f: u32) -> Option<f64> {
        let lists = self.lists?;
        let (start, len) = self.list(l)?;
        let quartile = op != ListOp::Median;
        if len == 0 || (quartile && len < 2) {
            return None;
        }
        let frequencies = if f == NONE {
            None
        } else {
            let (fstart, flen) = self.list(f)?;
            if len != flen {
                return if quartile {
                    // The reference skips computation and a fresh output is zero.
                    Some(0.0)
                } else if len == flen + 1 {
                    self.grouped_median(lists, start, fstart, flen)
                } else {
                    None
                };
            }
            Some(fstart)
        };
        let work_per_row = 3 + len.ilog2() + u32::from(frequencies.is_some());
        if !lists.charge(u64::from(len) * u64::from(work_per_row)) {
            return None;
        }
        let mut rows = Vec::with_capacity(len as usize);
        let mut total = 0;
        for k in 0..len {
            let value = number_of(lists.read(self.c, start + k))?;
            let count = match frequencies {
                Some(fstart) => {
                    let weight = number_of(lists.read(self.c, fstart + k))?;
                    if weight < 0.0 {
                        return None;
                    }
                    // Java casts fractions toward zero and NaN to zero.
                    (weight as i32) as u64
                }
                None => 1,
            };
            total += count;
            rows.push((if value.is_nan() { f64::NAN } else { value }, count));
        }
        let size = if quartile { total / 2 } else { total };
        if size == 0 {
            return None;
        }
        let offset = if op == ListOp::Q3 { total - size } else { 0 };
        let lower = offset + (size - 1) / 2;
        let upper = offset + size / 2;
        rows.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
        let mut seen = 0;
        let mut first = None;
        for (value, count) in rows {
            seen += count;
            if seen > lower {
                let first = *first.get_or_insert(value);
                if seen > upper {
                    return Some(if lower == upper {
                        value
                    } else {
                        (first + value) / 2.0
                    });
                }
            }
        }
        None
    }

    /// `AlgoMedian` interpolates inside the class that contains half of the
    /// total weight. Its running count uses truncated frequencies, though
    /// the target uses their original fractional weights.
    fn grouped_median(&self, lists: &Lists, start: u32, fstart: u32, len: u32) -> Option<f64> {
        if !lists.charge(4 * u64::from(len)) {
            return None;
        }
        let read = |i| number_of(lists.read(self.c, i)).unwrap_or(f64::NAN);
        let target = (0..len).map(|k| read(fstart + k)).sum::<f64>() / 2.0;
        let mut cumulative = 0.0;
        for k in 0..len {
            let frequency = f64::from(read(fstart + k) as i32);
            if frequency < 0.0 {
                return None;
            }
            if cumulative + frequency >= target {
                let low = read(start + k);
                let high = read(start + k + 1);
                return Some(low + (high - low) * (target - cumulative) / frequency);
            }
            cumulative += frequency;
        }
        // With no qualifying class, the reference leaves a fresh output at zero.
        Some(0.0)
    }

    /// `AlgoStats1D`: the sum, product or mean of the first `n` (all by
    /// default) elements as doubles. `n` is cut to a whole number and
    /// anything outside the list is undefined; no elements sum to 0, and
    /// have no product (as recorded with the harness) and no mean.
    pub(crate) fn stats(&self, op: ListOp, l: u32, n: u32) -> f64 {
        let Some(lists) = self.lists else {
            return f64::NAN;
        };
        // `Sum(list, frequencies)`: a second list weights the elements,
        // and a list one longer than its frequencies holds class bounds
        // whose midpoints count instead (`AlgoStats1D`).
        if op == ListOp::Sum && n != NONE && matches!(self.e.tys[n as usize], Ty::List(_)) {
            return self.sum_frequencies(lists, l, n);
        }
        let Some((start, size)) = self.truncated(l, n) else {
            return f64::NAN;
        };
        if size == 0 {
            return if op == ListOp::Sum { 0.0 } else { f64::NAN };
        }
        let (mut sum, mut product) = (0.0, 1.0);
        if !lists.charge(u64::from(size)) {
            return f64::NAN;
        }
        for k in 0..size {
            let Some(v) = number_of(lists.read(self.c, start + k)) else {
                return f64::NAN;
            };
            sum += v;
            product *= v;
        }
        match op {
            ListOp::Sum => sum,
            ListOp::Product => product,
            _ => sum / f64::from(size),
        }
    }

    /// `Sum(list, frequencies)` (`AlgoStats1D` with `STATS_SIGMAX`): the
    /// sum of each element times its frequency, with the midpoint rule for
    /// class bounds; lists of any other lengths, a negative frequency or a
    /// missing element make the sum undefined, and no elements sum to 0.
    fn sum_frequencies(&self, lists: &crate::expr::eval::Lists, l: u32, f: u32) -> f64 {
        let (Some((start, len)), Some((fstart, flen))) = (self.list(l), self.list(f)) else {
            return f64::NAN;
        };
        let classes = len == flen + 1;
        if !classes && len != flen {
            return f64::NAN;
        }
        let size = if classes { len - 1 } else { len };
        if size == 0 {
            return 0.0;
        }
        if !lists.charge(u64::from(2 * size)) {
            return f64::NAN;
        }
        let mut sum = 0.0;
        for k in 0..size {
            let Some(value) = number_of(lists.read(self.c, start + k)) else {
                return f64::NAN;
            };
            let value = if classes {
                match number_of(lists.read(self.c, start + k + 1)) {
                    Some(next) => (value + next) / 2.0,
                    None => return f64::NAN,
                }
            } else {
                value
            };
            let Some(frequency) = number_of(lists.read(self.c, fstart + k)) else {
                return f64::NAN;
            };
            if frequency < 0.0 {
                return f64::NAN;
            }
            sum += value * frequency;
        }
        sum
    }

    /// The list `l` and how many of its elements a truncation `n` keeps
    /// (`AlgoStats1D`): all without `n`, `(int) n` otherwise, where 0 is
    /// none and a count below 0 or past the end is undefined.
    fn truncated(&self, l: u32, n: u32) -> Option<(u32, u32)> {
        let lists = self.lists?;
        let truncate = (n != NONE).then(|| {
            let mark = lists.len();
            let t = self.full_number(n);
            lists.truncate(mark);
            t
        });
        let (start, len) = self.list(l)?;
        match truncate {
            None => Some((start, len)),
            Some(t) if t.is_nan() => None,
            // Java's `(int)`: towards zero, saturating
            Some(t) => match u32::try_from(t as i32) {
                Ok(t) if t <= len => Some((start, t)),
                _ => None,
            },
        }
    }

    /// `Sum` of points and vectors: the sum of their coordinates.
    pub(crate) fn sum_vectors(&self, l: u32, n: u32) -> Val {
        let undefined = Val::V(f64::NAN, f64::NAN);
        let Some(lists) = self.lists else {
            return undefined;
        };
        let Some((start, size)) = self.truncated(l, n).filter(|&(_, size)| size > 0) else {
            return undefined;
        };
        let (mut x, mut y) = (0.0, 0.0);
        if !lists.charge(u64::from(size)) {
            return undefined;
        }
        for k in 0..size {
            match slot_val(&lists.read(self.c, start + k)) {
                Val::V(a, b) => {
                    x += a;
                    y += b;
                }
                _ => return undefined,
            }
        }
        Val::V(x, y)
    }
}

#[cfg(test)]
mod tests {
    use crate::expr::{with_evaluation_budget, work_done};
    use crate::{script, Value};

    #[test]
    fn variadic_statistics_require_numbers_and_keep_list_and_numeric_expression_forms() {
        let mut c = script::load("a = 30°\nb = 60°\ns = Segment((0, 0), (3, 0))\nr = 42").unwrap();
        for (command, arguments) in [
            ("Median", "a, b"),
            ("Variance", "1, a, 2"),
            ("SD", "30°, 60°"),
            ("SampleVariance", "a + b, 2"),
            ("SampleSD", "a, 2"),
            ("MAD", "a, b"),
            ("Q1", "a, 2"),
            ("Q3", "1, a"),
            ("mean", "a, b"),
            ("Sum", "a, b, a"),
            ("Median", "s, 1"),
        ] {
            let line = format!("r = {command}({arguments})");
            assert!(script::exec(&mut c, &line).is_err(), "accepted {line}");
            assert_eq!(c.value(c.id("r").unwrap()), Value::Number(42.0));
        }
        for (call, expected) in [
            ("Median({a, b})", std::f64::consts::FRAC_PI_4),
            ("Mean({1, a, b})", (1.0 + std::f64::consts::FRAC_PI_2) / 3.0),
            ("Sum({1, a, b})", 1.0 + std::f64::consts::FRAC_PI_2),
            ("Median(a / b, 1, 2)", 1.0),
            ("Median(a * b, 1, 2)", 1.0),
            ("Mean(s + 1, s + 2)", 4.5),
            ("Sum(s + 1, s + 2, s + 3)", 15.0),
        ] {
            let id = script::exec(&mut c, &format!("out = {call}"))
                .unwrap()
                .unwrap();
            let Value::Number(actual) = c.value(id) else {
                panic!("{call} did not produce a number");
            };
            assert!((actual - expected).abs() < 1e-12, "{call} gave {actual}");
        }
    }

    #[test]
    fn variadic_statistics_classify_arguments_with_linear_work() {
        use crate::expr::{parse, types::CHECKED_SLOTS};
        use crate::Construction;

        let c = Construction::new();
        let count = 512;
        for (command, argument, max_slots_per_argument) in [
            ("Median", "1 + 2", 8),
            ("Median", "Median(1, 3)", 16),
            ("Sum", "1 + 2", 16),
            ("Sum", "(1, 2)", 16),
            ("Sum", "\"x\"", 8),
        ] {
            let args = vec![argument; count].join(", ");
            let source = format!("{command}({args})");
            let before = CHECKED_SLOTS.get();
            parse::parse(&c, &source).unwrap();
            let slots = CHECKED_SLOTS.get() - before;
            let limit = max_slots_per_argument * count + 2;
            assert!(
                slots <= limit,
                "{command} of {argument}: {slots} type-check slots, limit {limit}"
            );
        }
    }

    #[test]
    fn statistics_charge_input_rows_without_expanding_frequencies() {
        let mut c =
            script::load("l = {9, 1, 9}\nf = {2, 2, 2}\ng = {20000000, 20000000, 20000000}")
                .unwrap();
        for (command, expected) in [
            ("Median", 9.0),
            ("Q1", 1.0),
            ("Q3", 9.0),
            ("MAD", 32.0 / 9.0),
        ] {
            let work = [("a", "f"), ("b", "g")].map(|(label, frequency)| {
                let line = format!("{label} = {command}(l, {frequency})");
                let before = work_done();
                let id = with_evaluation_budget(128, || script::exec(&mut c, &line))
                    .unwrap()
                    .unwrap();
                let Value::Number(value) = c.value(id) else {
                    panic!("{line} became undefined");
                };
                assert!((value - expected).abs() < 1e-12, "{line} gave {value}");
                work_done() - before
            });
            assert_eq!(work[0], work[1], "{command}");
        }

        let mut c = script::load("l = Sequence(1024)").unwrap();
        for (command, budget) in [("MAD", 1500), ("Median", 4096)] {
            let line = format!("a = {command}(l)");
            let id = with_evaluation_budget(budget, || script::exec(&mut c, &line))
                .unwrap()
                .unwrap();
            assert_eq!(c.value(id), Value::Undefined, "{command}");
            let id = script::exec(&mut c, &line).unwrap().unwrap();
            assert!(c.value(id).is_defined(), "{command}");
        }
    }
}
