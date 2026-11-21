// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoAxis.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/Construction.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use crate::kernel::{Def, Id, Value};

pub(super) struct Axis {
    pub(super) label: &'static str,
    pub(super) def: Def,
    pub(super) value: Value,
}

// Built-ins live outside the construction order. Ordinary ids still index
// the same slots and the axes cannot be removed by rolling back any step.
const X_ID: Id = Id(usize::MAX - 1);
const Y_ID: Id = Id(usize::MAX);

const X_AXIS: Axis = Axis {
    label: "xAxis",
    def: Def::XAxis,
    value: Value::Line {
        a: 0.0,
        b: 1.0,
        c: 0.0,
    },
};
const Y_AXIS: Axis = Axis {
    label: "yAxis",
    def: Def::YAxis,
    value: Value::Line {
        a: -1.0,
        b: 0.0,
        c: 0.0,
    },
};

pub(super) fn lookup(label: &str) -> Option<Id> {
    match label {
        "xAxis" => Some(X_ID),
        "yAxis" => Some(Y_ID),
        _ => None,
    }
}

pub(super) fn get(id: Id) -> Option<&'static Axis> {
    match id {
        X_ID => Some(&X_AXIS),
        Y_ID => Some(&Y_AXIS),
        _ => None,
    }
}

pub(super) fn definition(def: Def) -> Option<&'static Axis> {
    match def {
        Def::XAxis => Some(&X_AXIS),
        Def::YAxis => Some(&Y_AXIS),
        _ => None,
    }
}
