//! Bind inline conic coefficients to the expression arena. The expression
//! supplies inputs to one conic, without adding a separate list object.

use crate::algo::general_conic::{coefficient_matrix, GeneralConicDef};
use crate::expr::{self, eval, parse, Entry, ExprDef, List, Node, Slot, Ty};
use crate::kernel::angle::AngleStyle;
use crate::kernel::{Construction, Def, Error, Id, Kind};
use std::cell::RefCell;

thread_local! {
    // Conic records are computed through a shared construction reference.
    // Reuse scratch space on this thread for the temporary coefficient list.
    static LIST_SCRATCH: RefCell<(Vec<Slot>, Vec<Slot>)> = const { RefCell::new((Vec::new(), Vec::new())) };
}

impl GeneralConicDef {
    pub(crate) fn add_from_arguments(
        c: &mut Construction,
        label: &str,
        args: &str,
    ) -> Result<Id, String> {
        // The five-point form takes construction labels through the normal
        // object lookup, including labels reserved inside arithmetic.
        let mut names = args.split(',').map(str::trim);
        let points = std::array::from_fn(|_| names.next().and_then(|name| c.id(name)));
        if let ([Some(p), Some(q), Some(r), Some(s), Some(t)], None) = (points, names.next()) {
            return c
                .add_with_list(label, &[p, q, r, s, t], |list| {
                    Def::GeneralConic(Self::FivePoints(list))
                })
                .map_err(|e| e.to_string());
        }

        // The expression parser already splits a list at its top-level
        // commas and handles any lists and calls inside its elements.
        let mut parsed = parse::parse(c, &format!("{{{args}}}"))?;
        let Node::List(first, len) = parsed.nodes[parsed.root as usize] else {
            return Err("invalid `Conic` arguments".into());
        };
        let items = first as usize..(first + len) as usize;
        match len {
            0 => return Err("missing `Conic` arguments".into()),
            1 => {
                let Node::Item(arg) = parsed.nodes[first as usize] else {
                    unreachable!("a list contains item nodes");
                };
                if !matches!(parsed.tys[arg as usize], Ty::List(_)) {
                    return Err("`Conic` expects a coefficient list".into());
                }
                if let Node::Obj(id) = parsed.nodes[arg as usize] {
                    return c
                        .add(label, Def::GeneralConic(Self::CoefficientList(id)))
                        .map_err(|e| e.to_string());
                }
                parsed.root = arg;
            }
            5 => return Err("`Conic` expects five point objects".into()),
            _ => {
                // CmdConic wraps separate GeoNumeric arguments in a list.
                // Only a list argument can supply booleans or angles as coefficients.
                if !parsed.tys[items]
                    .iter()
                    .all(|ty| matches!(ty, Ty::Num(dimension) if *dimension != 1))
                {
                    return Err("`Conic` expects numeric coefficients".into());
                }
            }
        }
        if parsed.uses_var {
            return Err("`Conic` coefficients must be independent of the function variable".into());
        }
        add_inline_list(c, label, parsed).map_err(|e| e.to_string())
    }

    pub(crate) fn coefficient_expression_matrix(
        c: &Construction,
        expression: ExprDef,
    ) -> Option<[f64; 6]> {
        let (arena, mut out) = LIST_SCRATCH.take();
        let lists = eval::Lists::new(arena);
        out.clear();
        let matrix = eval::root_list(c, expr::entry(c, expression), &lists, &mut out)
            .and_then(|len| coefficient_matrix(List::new(&out, len)));
        LIST_SCRATCH.set((lists.into_arena(), out));
        matrix
    }
}

fn add_inline_list(
    c: &mut Construction,
    label: &str,
    mut parsed: parse::Parsed,
) -> Result<Id, Error> {
    let index = u32::try_from(c.exprs.entries.len()).map_err(|_| Error::UnknownObject)?;
    let parents = std::mem::take(&mut parsed.parents);
    c.exprs.entries.push(Entry::new(parsed, false));
    let expression = ExprDef {
        kind: Kind::List,
        index,
        angle_style: AngleStyle::Unbounded,
    };
    c.exprs.pending = Some(index);
    let result = c.add_with_list(label, &parents, |reads| {
        Def::GeneralConic(GeneralConicDef::CoefficientExpr { expression, reads })
    });
    c.exprs.pending = None;
    if result.is_err() {
        c.exprs.entries.pop();
    }
    result
}
