//! Bind inline conic coefficients to the expression arena. The expression
//! supplies inputs to one conic, without adding a separate list object.

use crate::algo::general_conic::{coefficient_matrix, GeneralConicDef};
use crate::expr::{self, eval, parse, Element, Entry, ExprDef, List, Node, Slot, Ty};
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
                    return Err("`Conic` expects a coefficient list".into());
                };
                if !matches!(parsed.tys[arg as usize], Ty::List(_)) {
                    return Err("`Conic` expects a coefficient list".into());
                }
                if let Node::Obj(id) = parsed.nodes[arg as usize] {
                    validate_matrix(c.elements(id))?;
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
        add_inline_list(c, label, parsed)
    }

    pub(crate) fn coefficient_expression_matrix(
        c: &Construction,
        expression: ExprDef,
    ) -> Option<[f64; 6]> {
        with_list(c, expr::entry(c, expression), |list| {
            list.and_then(coefficient_matrix)
        })
    }
}

fn with_list<T>(c: &Construction, entry: &Entry, read: impl FnOnce(Option<List<'_>>) -> T) -> T {
    let (arena, mut out) = LIST_SCRATCH.take();
    let lists = eval::Lists::new(arena);
    out.clear();
    let len = eval::root_list(c, entry, &lists, &mut out);
    let result = read(len.map(|len| List::new(&out, len)));
    LIST_SCRATCH.set((lists.into_arena(), out));
    result
}

/// The three-row form reads the first three entries of every list row.
/// A missing entry in a list row is an input error at creation.
fn validate_matrix(list: Option<List<'_>>) -> Result<(), String> {
    if list.is_some_and(|list| {
        list.len() == 3
            && list
                .iter()
                .any(|item| matches!(item, Element::List(row) if row.len() < 3))
    }) {
        return Err("a conic coefficient matrix row needs at least three entries".into());
    }
    Ok(())
}

fn add_inline_list(
    c: &mut Construction,
    label: &str,
    mut parsed: parse::Parsed,
) -> Result<Id, String> {
    let index =
        u32::try_from(c.exprs.entries.len()).map_err(|_| Error::UnknownObject.to_string())?;
    let parents = std::mem::take(&mut parsed.parents);
    let entry = Entry::new(c, parsed, false);
    with_list(c, &entry, validate_matrix)?;
    c.exprs.entries.push(entry);
    let expression = ExprDef {
        kind: Kind::List,
        allows_infinite_point: false,
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
    result.map_err(|e| e.to_string())
}
