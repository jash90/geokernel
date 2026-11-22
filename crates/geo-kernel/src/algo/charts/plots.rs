// Parts translated from GeoGebra, commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
// source/shared/common/src/main/java/org/geogebra/common/kernel/statistics/AlgoDotPlot.java
// source/shared/common/src/main/java/org/geogebra/common/kernel/statistics/AlgoLineGraph.java
// source/shared/common/src/main/java/org/geogebra/common/kernel/statistics/AlgoNormalQuantilePlot.java
// source/shared/common/src/main/java/org/geogebra/common/kernel/statistics/AlgoPieChart.java
// source/shared/common/src/main/java/org/geogebra/common/kernel/statistics/AlgoResidualPlot.java
// source/shared/common/src/main/java/org/geogebra/common/euclidian/draw/DrawPointPlot.java
// Copyright (c) GeoGebra GmbH. Licensed under the EUPL 1.2.

use super::{frequencies, number, point, sort_work, ChartData, ChartMark, DotStack, Inputs, Pie};
use crate::expr::{Element, Slot, MAX_ELEMENTS};
use crate::kernel::{Id, Kind, Value};

impl Inputs<'_> {
    pub(super) fn line_graph(&self, ids: &[Id]) -> Option<ChartData> {
        let &[x, y] = ids else { return None };
        let (x, y) = (self.numbers(x)?, self.numbers(y)?);
        if x.len() < 2
            || x.len() != y.len()
            || !self.work.charge(3 * x.len() as u64)
            || x.windows(2).any(|p| p[0] >= p[1])
        {
            return None;
        }
        let vertices: Vec<_> = x.into_iter().zip(y).collect();
        let marks = vertices
            .windows(2)
            .map(|p| ChartMark::Segment([p[0], p[1]]))
            .collect();
        Some(ChartData {
            value: Value::Function,
            marks,
            vertices,
            ..ChartData::default()
        })
    }

    pub(super) fn dot_plot(&self, ids: &[Id]) -> Option<ChartData> {
        let (&input, rest) = ids.split_first()?;
        let list = self.list(input)?;
        if list.is_empty() || !self.work.charge(2 * list.len() as u64) {
            return None;
        }
        let scale = rest
            .last()
            .filter(|&&id| self.c.kind(id).is_number())
            .map_or(Some(1.0), |&id| self.number(id))?;
        let stack = rest.first().is_some_and(|&id| self.flag(id))
            && matches!(list.get(0), Some(Element::Value(Kind::Number, _)));
        let groups = frequencies(list, self.work)?;
        if stack {
            let stack = DotStack {
                groups,
                height_scale: scale,
            };
            let data = point_list(stack.dots(50.0, 50.0));
            return Some(ChartData {
                dot_stack: Some(stack),
                ..data
            });
        }
        Some(point_list(groups.into_iter().flat_map(|(x, count)| {
            (1..=count).map(move |i| (x, i as f64 * scale))
        })))
    }

    pub(super) fn residual_plot(&self, ids: &[Id]) -> Option<ChartData> {
        let &[list, f] = ids else { return None };
        let list = self.list(list)?;
        if list.is_empty() || !self.work.charge(2 * list.len() as u64) {
            return None;
        }
        let mut points = Vec::with_capacity(list.len());
        for item in list.iter() {
            let (x, y) = match item {
                Element::Value(Kind::Point, Value::Undefined) => (f64::NAN, f64::NAN),
                _ => point(item)?,
            };
            points.push((x, y - self.work.sample(self.c, f, x)));
            if self.work.exhausted() {
                return None;
            }
        }
        Some(point_list(points))
    }

    pub(super) fn normal_quantile(&self, ids: &[Id]) -> Option<ChartData> {
        let &[list] = ids else { return None };
        let mut values = self.numbers(list)?;
        let count = values.len();
        if count == 0
            || count as u64 >= MAX_ELEMENTS
            || !sort_work(self.work, count)
            || !self.work.charge(64 * count as u64)
        {
            return None;
        }
        values.sort_unstable_by(f64::total_cmp);
        let size = count as f64;
        let tail = 0.5f64.powf(1.0 / size);
        let mut data = point_list(values.iter().enumerate().map(|(i, &x)| {
            let p = if i + 1 == count {
                tail
            } else if i == 0 {
                1.0 - tail
            } else {
                (i as f64 + 1.0 - 0.3175) / (size + 0.365)
            };
            (x, normal_quantile(p))
        }));
        let mean = values.iter().sum::<f64>() / size;
        let sd = (values.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (size - 1.0)).sqrt();
        let (x1, x2) = (values[0], values[count - 1]);
        let (y1, y2) = (x1 / sd - mean / sd, x2 / sd - mean / sd);
        if sd > 0.0 && y1.is_finite() && y2.is_finite() {
            data.marks.push(ChartMark::Segment([(x1, y1), (x2, y2)]));
            data.slots.push(Slot::with_value(
                Kind::Segment,
                Value::Segment { x1, y1, x2, y2 },
            ));
        } else {
            data.slots.push(Slot::undefined(Kind::Segment));
        }
        data.value = Value::List {
            len: data.slots.len() as u32,
        };
        Some(data)
    }

    pub(super) fn pie_chart(&self, ids: &[Id]) -> Option<ChartData> {
        let (&list, geometry) = ids.split_first()?;
        let list = self.list(list)?;
        if list.is_empty() || !self.work.charge(2 * list.len() as u64) {
            return None;
        }
        let center = match geometry.first() {
            Some(&id) => self.c.point(id)?,
            None => (0.0, 0.0),
        };
        let radius = geometry.get(1).map_or(Some(3.0), |&id| self.number(id))?;
        let mut fractions: Vec<_> = list.iter().filter_map(number).collect();
        if fractions.is_empty() || fractions.iter().any(|x| *x < 0.0) {
            return None;
        }
        let sum: f64 = fractions.iter().sum();
        for value in &mut fractions {
            *value /= sum;
        }
        let pie = Some(Pie {
            center,
            radius,
            fractions,
        });
        Some(ChartData {
            value: Value::PieChart,
            pie,
            ..ChartData::default()
        })
    }
}

fn point_list(points: impl IntoIterator<Item = (f64, f64)>) -> ChartData {
    let mut result = ChartData::default();
    for (x, y) in points {
        result.slots.push(Slot::coords(Kind::Point, x, y));
        if x.is_finite() && y.is_finite() {
            result.marks.push(ChartMark::Point {
                at: (x, y),
                open: false,
            });
        }
    }
    result.value = Value::List {
        len: result.slots.len() as u32,
    };
    result
}

fn normal_quantile(p: f64) -> f64 {
    let (mut lo, mut hi) = (-12.0, 12.0);
    for _ in 0..60 {
        let mid = (lo + hi) / 2.0;
        if libm::erfc(-mid * std::f64::consts::FRAC_1_SQRT_2) * 0.5 < p {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (lo + hi) / 2.0
}
