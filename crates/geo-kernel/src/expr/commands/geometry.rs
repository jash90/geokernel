// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/MyList.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! The geometry group of the command matrix, as far as expressions compute
//! it: `Length` of a list (of anything else it is the command of the script
//! parser).

use crate::expr::eval::{Ctx, Val};
use crate::expr::parse::{list_form, Parser};
use crate::expr::{Elem, ListOp, Node, Ty};

/// The commands of this group (see `commands::is_command`).
pub(super) const NAMES: &[&str] = &["Length"];

/// `Length(list)`.
pub(super) fn parse(p: &mut Parser, text: &str) -> Result<Option<u32>, String> {
    if text != "Length" {
        return Ok(None);
    }
    p.list_command(|_, args, lists| list_form(text, ListOp::Length, false, args, lists))
}

/// The type of a list command of this group (see `commands::list_type`).
pub(super) fn list_type(
    op: ListOp,
    _elem: Elem,
    _numbers: bool,
    _nodes: &[Node],
    _tys: &[Ty],
    _l: u32,
    _n: u32,
) -> Result<Option<Ty>, String> {
    Ok(match op {
        ListOp::Length => Some(Ty::Num(0)),
        _ => None,
    })
}

/// The value of a list command of this group (see `commands::list_value`).
pub(super) fn list_value<const M: bool>(
    ctx: &Ctx<'_, M>,
    _i: u32,
    op: ListOp,
    l: u32,
    _n: u32,
) -> Option<Val> {
    Some(match op {
        ListOp::Length => Val::N(ctx.list(l).map_or(f64::NAN, |(_, len)| f64::from(len))),
        _ => return None,
    })
}
