use crate::algo::predicate::{relation, RelationReport};
use crate::expr::{self, eval::Lists, Lit, Node};
use crate::kernel::{Construction, Def, Id, Kind, Value};

pub(crate) fn define_relation(
    c: &mut Construction,
    label: &str,
    input: &str,
) -> Option<Result<Id, String>> {
    if !input.starts_with("Relation") {
        return None;
    }
    let ("Relation", args) = crate::script::split_call(input)? else {
        return None;
    };
    let start = c.checkpoint();
    let result = (|| {
        let mut list = None;
        let ids = match *args.as_slice() {
            [only] if only.starts_with('{') && only.ends_with('}') => {
                let inner = &only[1..only.len() - 1];
                let call = format!("Relation({inner})");
                let (_, args) = crate::script::split_call(&call).ok_or("invalid relation list")?;
                arguments(c, args)?
            }
            [name] => {
                let id = c.id(name).ok_or("expected a list of objects")?;
                list = Some(id);
                let Def::Expr(def) = c.def(id) else {
                    return Err("expected a list of objects".into());
                };
                let entry = expr::entry(c, def);
                let Node::List(start, len @ 2..=4) = entry.nodes[entry.root as usize] else {
                    return Err("expected a list of two to four objects".into());
                };
                (start..start + len)
                    .map(|i| {
                        let Node::Item(item) = entry.nodes[i as usize] else {
                            unreachable!()
                        };
                        match entry.nodes[item as usize] {
                            Node::Obj(id) | Node::ItemObject(id) => Ok(id),
                            _ => Err("expected a list of named objects".into()),
                        }
                    })
                    .collect::<Result<Vec<Id>, String>>()?
            }
            [a, b] => arguments(c, [a, b])?,
            _ => return Err("Relation needs two objects or a list".into()),
        };
        if !(2..=4).contains(&ids.len()) {
            return Err("Relation needs two to four objects".into());
        }
        let mut objects = [ids[0]; 4];
        objects[..ids.len()].copy_from_slice(&ids);
        let node = Node::Relation {
            objects,
            count: ids.len() as u8,
            list,
        };
        let parsed = expr::reparse(c, Kind::Text, &[node], &[Lit::default()], 0)
            .ok_or("cannot build a relation report")?;
        expr::add(c, label, parsed, Kind::Text)
    })();
    if result.is_err() {
        c.discard_since(start);
    }
    Some(result)
}

fn arguments<'a>(
    c: &mut Construction,
    labels: impl IntoIterator<Item = &'a str>,
) -> Result<Vec<Id>, String> {
    labels
        .into_iter()
        .map(|label| crate::script::relation_or_region_argument(c, label))
        .collect()
}

pub(crate) fn relation_reports(c: &Construction, id: Id) -> Option<Vec<RelationReport>> {
    let (Def::Expr(def), Value::Text) = (c.def(id), c.value(id)) else {
        return None;
    };
    let entry = expr::entry(c, def);
    let Node::Relation { objects, count, .. } = entry.nodes[entry.root as usize] else {
        return None;
    };
    compare_objects(c, &objects[..usize::from(count)])
}

pub(crate) fn compare_objects(c: &Construction, objects: &[Id]) -> Option<Vec<RelationReport>> {
    let meter = Lists::new(Vec::new());
    let reports = relation::numerical(c, objects, |n| !meter.charge(n));
    (!meter.over()).then_some(reports)
}
