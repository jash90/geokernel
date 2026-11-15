// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdArea.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoAreaPoints.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoPolygon.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoPoint.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Definitions that read any number of points.
//!
//! These definitions keep their points in a list stored by the construction
//! (see [`Construction::add_with_list`]). A list cannot be stored from
//! [`Group::parse`], which only sees the construction, so the script lines of
//! this group are run by [`exec`], which the script layer asks first.

use super::{Group, Parents, POINT};
use crate::kernel::{Construction, Def, Error, Id, IdList, Kind, Value};
use crate::script::{object, split_call};

/// Definitions that read a list of points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PolygonDef {
    /// `Area(P1, ..., Pn)` for five or more points: the area of the polygon
    /// through the points in order (`AlgoAreaPoints`). Three and four points
    /// are `MeasureDef::Area3` and `MeasureDef::Area4`.
    AreaPoints(IdList),
}

/// `AlgoPolygon.calcAreaWithSign` for points of the plane: half the sum of
/// `GeoPoint.det` over the closed sequence; positive when the points run
/// counter-clockwise. `None` when a point is undefined or there are fewer
/// than two.
fn signed_area(c: &Construction, ids: &[Id]) -> Option<f64> {
    let (&first, rest) = ids.split_first()?;
    if rest.is_empty() {
        return None;
    }
    let start = c.point(first)?;
    let mut prev = start;
    let mut sum = 0.0;
    for &id in rest {
        let p = c.point(id)?;
        sum += prev.0 * p.1 - p.0 * prev.1;
        prev = p;
    }
    sum += prev.0 * start.1 - start.0 * prev.1;
    Some(sum / 2.0)
}

/// Objects a definition of this group reads besides its parents and its
/// list. None so far.
pub(crate) fn hidden_parents(_c: &Construction, _def: &Def) -> Vec<Id> {
    Vec::new()
}

impl Group for PolygonDef {
    fn kind(&self) -> Kind {
        match self {
            PolygonDef::AreaPoints(_) => Kind::Number,
        }
    }

    fn parents(&self) -> Parents {
        Parents::none()
    }

    fn list(&self) -> Option<IdList> {
        match *self {
            PolygonDef::AreaPoints(list) => Some(list),
        }
    }

    fn with_list(self, list: IdList) -> Self {
        match self {
            PolygonDef::AreaPoints(_) => PolygonDef::AreaPoints(list),
        }
    }

    fn check(&self, c: &Construction) -> Result<(), Error> {
        match *self {
            PolygonDef::AreaPoints(list) => c.expect_list(list, POINT),
        }
    }

    fn compute(&self, c: &Construction) -> Option<Value> {
        match *self {
            PolygonDef::AreaPoints(list) => {
                Some(Value::Number(signed_area(c, c.list(list))?.abs()))
            }
        }
    }

    /// The commands of this group need to store a list; see [`exec`].
    fn parse(_c: &Construction, _func: &str, _args: &[&str]) -> Option<Result<Self, String>> {
        None
    }
}

/// Runs `name = rhs` when it is a command of this group. Returns `None` for
/// every other line, so that the other groups and the built-in commands can
/// try; otherwise what [`crate::script::exec`] returns.
pub(crate) fn exec(
    c: &mut Construction,
    name: &str,
    rhs: &str,
) -> Option<Result<Option<Id>, String>> {
    let (func, args) = split_call(rhs)?;
    match func {
        // `CmdArea`: any number of points above two; three and four are
        // handled by the measuring group.
        "Area" if args.len() >= 5 => Some(add_points(c, name, &args, PolygonDef::AreaPoints)),
        _ => None,
    }
}

/// Adds a definition over the points named by `args`.
fn add_points(
    c: &mut Construction,
    name: &str,
    args: &[&str],
    make: fn(IdList) -> PolygonDef,
) -> Result<Option<Id>, String> {
    let ids = args
        .iter()
        .map(|a| object(c, a))
        .collect::<Result<Vec<_>, _>>()?;
    c.add_with_list(name, &ids, |list| Def::Polygon(make(list)))
        .map(Some)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::script::{exec as run, load};

    #[test]
    fn area_of_many_points() {
        let mut c = load(
            "A = (0, 0)\nB = (4, 0)\nC = (4, 3)\nD = (2, 5)\nE = (0, 3)\n\
             a = Area(A, B, C, D, E)\nb = Area(A, B, C, D, E, A)\n",
        )
        .unwrap();
        let a = c.id("a").unwrap();
        assert_eq!(c.value(a), Value::Number(16.0));
        assert_eq!(c.value(c.id("b").unwrap()), Value::Number(16.0));
        // clockwise order: the area is still positive
        run(&mut c, "r = Area(E, D, C, B, A)").unwrap();
        assert_eq!(c.value(c.id("r").unwrap()), Value::Number(16.0));
        // every point is a parent
        let d = c.id("D").unwrap();
        assert_eq!(c.move_point(d, 2.0, 7.0).unwrap(), 3);
        assert_eq!(c.value(a), Value::Number(20.0));
    }

    #[test]
    fn a_rejected_line_stores_nothing() {
        let mut c = load("A = (0, 0)\nB = (4, 0)\nk = Circle(A, B)\n").unwrap();
        let before = c.len();
        let e = run(&mut c, "a = Area(A, B, A, B, k)").unwrap_err();
        assert!(e.contains("`k` is a circle, expected a point"), "{e}");
        let e = run(&mut c, "a = Area(A, B, A, B, Z)").unwrap_err();
        assert!(e.contains("unknown object `Z`"), "{e}");
        assert_eq!(c.len(), before);
    }
}
