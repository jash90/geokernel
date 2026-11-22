//! The commands of the `geogebra` group of the command matrix that
//! expressions compute themselves.

use super::super::parse::Parser;
use crate::expr::Node;
use crate::kernel::Kind;

pub(super) const NAMES: &[&str] = &["Object"];

/// `Object(text)`: the object the text names, resolved while parsing. The
/// text has to be a string literal or a text object with fixed content;
/// a name that is computed at run time, which GeoGebra's `AlgoObject`
/// re-resolves on every update, is not supported.
pub(super) fn parse(p: &mut Parser<'_>, name: &str) -> Result<Option<u32>, String> {
    if name != "Object" {
        return Ok(None);
    }
    let args = p.arguments()?;
    let [arg] = args[..] else {
        return Err(format!("`Object` does not take {} argument(s)", args.len()));
    };
    let label = match p.nodes[arg as usize] {
        Node::Text => p.literal_text(arg).map(str::to_owned),
        Node::Obj(id) if p.kind(id) == Kind::Text => {
            crate::expr::text_value(p.construction(), id).map(str::to_owned)
        }
        _ => None,
    };
    let Some(label) = label else {
        return Err("`Object` needs the name of an object, as text".into());
    };
    match p.lookup(&label) {
        Some(id) => Ok(Some(p.object_reference(id))),
        None => Err(format!("unknown object `{label}`")),
    }
}
