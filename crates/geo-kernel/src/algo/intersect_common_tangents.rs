// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoCommonTangents.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Common tangents via the points of contact on the first conic.

use super::conics::Intersection;
use crate::algo::{conic_nd::ConicData, lines::LineDef, CONIC_OBJECTS};
use crate::kernel::{Construction, Def, Id, Value};

type Matrix = [[f64; 3]; 3];

pub(super) fn parse_multi(c: &Construction, name: &str, args: &[&str]) -> Option<Vec<Def>> {
    let ("Tangent", [first, second]) = (name, args) else {
        return None;
    };
    let (first, second) = (c.id(first)?, c.id(second)?);
    if ![first, second]
        .into_iter()
        .all(|id| CONIC_OBJECTS.contains(&c.kind(id)))
    {
        return None;
    }
    Some(
        (0..4)
            .map(|which| {
                Def::Lines(LineDef::CommonTangent {
                    first,
                    second,
                    which,
                })
            })
            .collect(),
    )
}

pub(crate) fn tangent(c: &Construction, first: Id, second: Id, which: u8) -> Option<Value> {
    let (a, b, c) = contacts(c, first, second)?
        .get(usize::from(which))?
        .as_ref()?
        .0;
    Some(Value::Line { a, b, c })
}

pub(crate) fn contact(c: &Construction, first: Id, second: Id, which: u8) -> Option<(f64, f64)> {
    Some(
        contacts(c, first, second)?
            .get(usize::from(which))?
            .as_ref()?
            .1,
    )
}

type Contact = ((f64, f64, f64), (f64, f64));

fn contacts(c: &Construction, first: Id, second: Id) -> Option<[Option<Contact>; 4]> {
    let a = conic(c, first)?;
    let b = conic(c, second)?;
    let ma = symmetric(a.matrix);
    // The polar of p on A is tangent to B if p^T A B^-1 A p = 0.
    let m = multiply(&multiply(&ma, &inverse(b.matrix)?), &ma);
    let mut auxiliary = ConicData::new();
    auxiliary.set_matrix([m[0][0], m[1][1], m[2][2], m[0][1], m[0][2], m[1][2]]);

    let mut intersection = Intersection::default();
    let points = intersection.update(&auxiliary, &a, false, |_| true);
    let mut result = [None; 4];
    let mut outer = 0;
    let mut inner = 2;
    for (x, y) in points.into_iter().flatten() {
        let [xx, yy, constant, xy, lx, ly] = a.matrix;
        let l = (
            xx * x + xy * y + lx,
            xy * x + yy * y + ly,
            lx * x + ly * y + constant,
        );
        if l.0 == 0.0 && l.1 == 0.0 {
            continue;
        }
        let side = |[px, py]: [f64; 2]| l.0 * px + l.1 * py + l.2;
        let slot = if side(a.midpoint) * side(b.midpoint) < 0.0 && inner < 4 {
            let slot = inner;
            inner += 1;
            slot
        } else if outer < 2 {
            let slot = outer;
            outer += 1;
            slot
        } else {
            continue;
        };
        result[slot] = Some((l, (x, y)));
    }
    Some(result)
}

fn conic(c: &Construction, id: Id) -> Option<ConicData> {
    c.arc(id)
        .filter(|part| part.defined)
        .map(|part| part.conic)
        .or_else(|| c.conic(id))
        .filter(|conic| conic.defined)
}

fn symmetric([xx, yy, c, xy, x, y]: [f64; 6]) -> Matrix {
    [[xx, xy, x], [xy, yy, y], [x, y, c]]
}

fn multiply(a: &Matrix, b: &Matrix) -> Matrix {
    std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| a[i][k] * b[k][j]).sum()))
}

fn inverse([a, b, c, d, e, f]: [f64; 6]) -> Option<Matrix> {
    let adj = [
        b * c - f * f,
        a * c - e * e,
        a * b - d * d,
        e * f - d * c,
        d * f - b * e,
        d * e - a * f,
    ];
    let det = a * adj[0] + d * adj[3] + e * adj[4];
    (det.is_finite() && det != 0.0).then(|| symmetric(adj.map(|v| v / det)))
}
