// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/cas/CmdCASCommand1Arg.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/cas/AlgoCasBaseSingleArgument.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoCasBase.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! `Expand`, `Factor`, `IFactor` and `Simplify` of an object that is no
//! function. A curve is rewritten coordinate by coordinate into a curve
//! over the same interval (`AlgoCasBase` copies the input and applies the
//! command to it).

use super::Op;
use crate::algo::measure::MeasureDef;
use crate::expr::{add_curve_coordinate, reparse, Lit, Node, NONE};
use crate::{Construction, Def, Id, Kind};

/// The call `Command(object)` of one of the four commands when the object
/// is a curve; `None`, with nothing added, for every other input.
pub(crate) fn define_object(
    c: &mut Construction,
    label: &str,
    input: &str,
) -> Option<Result<Id, String>> {
    if !["Expand", "Factor", "IFactor", "Simplify"]
        .iter()
        .any(|name| input.starts_with(name))
    {
        return None;
    }
    let (name, args) = crate::script::split_call(input)?;
    let op = match name {
        "Expand" => Op::Expand,
        "Factor" => Op::Factor,
        "IFactor" => Op::IFactor,
        "Simplify" => Op::Simplify,
        _ => return None,
    };
    let &[argument] = args.as_slice() else {
        return None;
    };
    let source = c.id(argument.trim())?;
    match c.kind(source) {
        Kind::Curve => Some(curve(c, label, op, source)),
        _ => None,
    }
}

/// The curve whose coordinates are the command applied to the coordinates
/// of `source`. It shares the bounds, so it is undefined with its source.
fn curve(c: &mut Construction, label: &str, op: Op, source: Id) -> Result<Id, String> {
    let Def::Measure(MeasureDef::CartesianCurve {
        coordinates,
        bounds,
    }) = c.def(source)
    else {
        return Err(format!(
            "`{}` needs a curve given by two coordinate functions",
            op.name()
        ));
    };
    let start = c.checkpoint();
    let result = (|| {
        let mut rewritten = [0; 2];
        for (slot, coordinate) in rewritten.iter_mut().zip(coordinates) {
            let coordinate = Id(coordinate as usize);
            let nodes = [Node::FunctionObject(coordinate), Node::Cas(op, 0, NONE)];
            let literals = vec![Lit::default(); nodes.len()];
            let parsed = reparse(c, Kind::Function, &nodes, &literals, 1)
                .ok_or("cannot rewrite the coordinates of the curve")?;
            let hidden = format!("\0arg{}", c.len());
            let id = add_curve_coordinate(c, &hidden, parsed, coordinate)?;
            *slot = u32::try_from(id.0).map_err(|_| "too many objects".to_owned())?;
        }
        c.add(
            label,
            Def::Measure(MeasureDef::CartesianCurve {
                coordinates: rewritten,
                bounds,
            }),
        )
        .map_err(|e| e.to_string())
    })();
    if result.is_err() {
        c.discard_since(start);
    }
    result
}
