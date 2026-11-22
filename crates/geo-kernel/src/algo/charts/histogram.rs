// Parts translated from GeoGebra, commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
// source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoFunctionAreaSums.java
// source/shared/common/src/main/java/org/geogebra/common/kernel/statistics/AlgoFrequencyPolygon.java
// Copyright (c) GeoGebra GmbH. Licensed under the EUPL 1.2.

use super::{ChartData, ChartMark, Command, Inputs};
use crate::expr::IntersectionWork;
use crate::kernel::{Id, Kind, Value};
use crate::tol::STANDARD_PRECISION;

pub(super) struct Histogram {
    borders: Vec<f64>,
    heights: Vec<f64>,
    cumulative: bool,
}

impl Histogram {
    pub(super) fn compute(input: &Inputs<'_>, args: &[Id], right: bool) -> Option<Self> {
        let (cumulative, args) = match args {
            [id, tail @ ..] if input.c.kind(*id) == Kind::Boolean => (Some(*id), tail),
            _ => (None, args),
        };
        let [borders, data, tail @ ..] = args else {
            return None;
        };
        let (frequencies, tail) = match tail {
            [id, rest @ ..] if input.c.kind(*id) == Kind::List => (Some(*id), rest),
            _ => (None, tail),
        };
        let use_density = tail.first();
        let density = match use_density {
            Some(&flag) if !input.flag(flag) => -1.0,
            _ => tail.get(1).map_or(Some(1.0), |&id| input.number(id))?,
        };
        if density <= 0.0 && density != -1.0 {
            return None;
        }
        let borders = input.numbers(*borders)?;
        let count = borders.len().checked_sub(1).filter(|&n| n > 0)?;
        let data = input.numbers(*data)?;
        let cumulative_flag = cumulative.is_some_and(|id| input.flag(id));
        let raw = count != data.len() || use_density.is_some() || cumulative.is_some();
        let density = if raw { density } else { -1.0 };
        if !input.work.charge(3 * count as u64) {
            return None;
        }
        let mut heights = if raw {
            let weights = match frequencies {
                Some(id) => Some(input.numbers(id)?),
                None => None,
            };
            if weights.as_ref().is_some_and(|w| w.len() < data.len()) {
                return None;
            }
            let mut bins = vec![0.0; count];
            let last = borders[count];
            // Prefix maxima give the first matching class without sorting the
            // boundaries or changing the choice for unsorted classes.
            let mut maximum = f64::NEG_INFINITY;
            let cutoffs: Vec<_> = borders[1..]
                .iter()
                .enumerate()
                .map(|(i, &b)| {
                    let b = if !right && i + 1 == count {
                        b + (b / 100_000_000.0).abs()
                    } else {
                        b
                    };
                    maximum = maximum.max(b);
                    maximum
                })
                .collect();
            let lookups = u64::from(usize::BITS - count.leading_zeros()) + 1;
            if !input.work.charge(lookups * data.len() as u64) {
                return None;
            }
            for (i, x) in data.into_iter().enumerate() {
                let weight = weights.as_ref().map_or(1.0, |w| w[i]);
                if weight < 0.0 || x < borders[0] || x > last {
                    return None;
                }
                let bin = cutoffs.partition_point(|b| {
                    if right {
                        b + STANDARD_PRECISION <= x
                    } else {
                        *b <= x + STANDARD_PRECISION
                    }
                });
                if let Some(freq) = bins.get_mut(bin) {
                    *freq += weight;
                }
            }
            bins
        } else {
            data
        };
        if cumulative_flag {
            let mut sum = 0.0;
            for h in &mut heights {
                sum += *h;
                *h = sum;
            }
        }
        if density != -1.0 {
            for (b, h) in borders.windows(2).zip(&mut heights) {
                *h = density * *h / (b[1] - b[0]);
            }
        }
        Some(Self {
            borders,
            heights,
            cumulative: cumulative_flag,
        })
    }

    pub(super) fn output(self, command: Command, work: &IntersectionWork) -> Option<ChartData> {
        let count = self.heights.len();
        if !work.charge(2 * count as u64) {
            return None;
        }
        let bins = self.borders.windows(2).zip(self.heights);
        let mut area = 0.0;
        let mut vertices = Vec::new();
        let mut marks = Vec::new();
        if command == Command::FrequencyPolygon {
            let left = self.borders[0];
            vertices.push((
                if self.cumulative {
                    left
                } else {
                    left - (self.borders[1] - left) * 0.5
                },
                0.0,
            ));
        }
        for (b, height) in bins {
            let [left, right] = [b[0], b[1]];
            area += (right - left) * height;
            if command == Command::FrequencyPolygon {
                vertices.push((
                    if self.cumulative {
                        right
                    } else {
                        (left + right) * 0.5
                    },
                    height,
                ));
            } else {
                marks.push(ChartMark::Bar {
                    left,
                    right,
                    height,
                });
            }
        }
        if command == Command::FrequencyPolygon {
            if !area.is_finite() {
                return None;
            }
            if !self.cumulative {
                vertices.push((
                    1.5 * self.borders[count] - 0.5 * self.borders[count - 1],
                    0.0,
                ));
            }
            // The reference polyline stores the points without updating its length.
            return Some(ChartData {
                value: Value::PolyLine { length: 0.0 },
                marks,
                vertices,
                ..ChartData::default()
            });
        }
        ChartData::number(area, marks)
    }
}
