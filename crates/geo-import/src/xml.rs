//! Reads the construction part of `geogebra.xml` into a flat list of items.
//!
//! What is read, and where GeoGebra writes it:
//!
//! * `<expression label= exp= type=>`: `GeoElement.getExpressionXML` writes
//!   it before the element of a free object that was typed as an expression
//!   (`A = (1, 2)`, `r = 3`), and `AlgoElement.getExpXML` for an object that
//!   is an arithmetic expression of others. The text is printed with the
//!   decimals of the file, so it is **not** the exact value.
//! * `<command name=><input a0= a1=/><output a0= a1=/></command>`:
//!   `AlgoElement.getCmdXML`. The outputs are listed in the algorithm's output
//!   order; an output without a label has an empty name.
//! * `<element type= label=>` with `<coords x= y= z=/>` (points, vectors:
//!   homogeneous, `z = 0` for a vector and for a point at infinity) or
//!   `<value val=/>` (numbers, angles, booleans) holding the exact value, and
//!   `<show object=/>`.
//! * `<kernel>`: `Kernel.getKernelXML` (decimals, angle unit, continuity).
//!
//! Everything else (the user interface, styles, scripts) is ignored.

use crate::{ImportError, Settings};
use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};

/// An element of the construction that GeoGebra wrote.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Element {
    /// The `type` attribute (`point`, `numeric`, `conic`, ...).
    pub kind: String,
    pub label: String,
    /// `<show object=>`; `true` when absent.
    pub shown: bool,
    /// `<coords>` as written: `x`, `y`, `z`.
    pub coords: Option<[String; 3]>,
    /// `<value val=>` as written.
    pub value: Option<String>,
}

/// One entry of the construction, in file order.
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Expression {
        label: String,
        exp: String,
    },
    Command {
        name: String,
        inputs: Vec<String>,
        /// One slot per output of the command; empty for an output that has
        /// no label.
        outputs: Vec<String>,
    },
    Element(Element),
}

/// The parsed construction.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Document {
    pub items: Vec<Item>,
    pub settings: Settings,
}

fn bad(e: impl std::fmt::Display) -> ImportError {
    ImportError::Xml(e.to_string())
}

/// The value of attribute `name`, with entities resolved.
fn attr(e: &BytesStart, name: &str) -> Result<Option<String>, ImportError> {
    for a in e.attributes() {
        let a = a.map_err(bad)?;
        if a.key.as_ref() == name {
            return Ok(Some(
                a.normalized_value(XmlVersion::Implicit1_0)
                    .map_err(bad)?
                    .into_owned(),
            ));
        }
    }
    Ok(None)
}

/// The attributes `a0`, `a1`, ... of an `<input>` or `<output>` tag, which
/// must be numbered without gaps.
fn numbered(e: &BytesStart, command: &str) -> Result<Vec<String>, ImportError> {
    let mut found: Vec<(usize, String)> = Vec::new();
    for a in e.attributes() {
        let a = a.map_err(bad)?;
        let key = a.key.as_ref().to_string();
        let Some(index) = key.strip_prefix('a').and_then(|n| n.parse::<usize>().ok()) else {
            continue;
        };
        let value = a
            .normalized_value(XmlVersion::Implicit1_0)
            .map_err(bad)?
            .into_owned();
        found.push((index, value));
    }
    found.sort_by_key(|(i, _)| *i);
    for (n, (i, _)) in found.iter().enumerate() {
        if *i != n {
            return Err(ImportError::BadOutputs {
                command: command.into(),
                detail: format!(
                    "the attributes are not numbered a0, a1, ...: a{n} is missing or repeated"
                ),
            });
        }
    }
    Ok(found.into_iter().map(|(_, v)| v).collect())
}

/// Parses the text of a `geogebra.xml`.
pub fn parse(text: &str) -> Result<Document, ImportError> {
    let mut reader = Reader::from_str(text);
    let mut doc = Document::default();
    let mut in_construction = false;
    let mut seen_construction = false;
    let mut command: Option<(String, Vec<String>, Vec<String>)> = None;
    let mut element: Option<Element> = None;
    let mut in_kernel = false;
    loop {
        let event = reader.read_event().map_err(bad)?;
        let (start, empty) = match &event {
            Event::Start(e) => (Some(e), false),
            Event::Empty(e) => (Some(e), true),
            _ => (None, false),
        };
        if let Some(e) = start {
            let name = e.name().as_ref().to_string();
            match name.as_str() {
                "construction" if !in_construction => {
                    in_construction = !empty;
                    seen_construction = true;
                }
                "kernel" if !in_construction => in_kernel = !empty,
                "continuous" | "decimals" | "significantfigures" | "angleUnit" if in_kernel => {
                    let val = attr(e, "val")?.unwrap_or_default();
                    match name.as_str() {
                        "continuous" => doc.settings.continuous = val == "true",
                        "decimals" => doc.settings.decimals = val.parse().ok(),
                        "significantfigures" => doc.settings.significant_figures = val.parse().ok(),
                        _ => doc.settings.angle_unit = Some(val),
                    }
                }
                "expression" if in_construction && command.is_none() && element.is_none() => {
                    // Outputs of commands that make several objects have no label
                    // here; an object needs one.
                    if let (Some(label), Some(exp)) = (attr(e, "label")?, attr(e, "exp")?) {
                        doc.items.push(Item::Expression { label, exp });
                    }
                }
                "command" if in_construction && command.is_none() && element.is_none() => {
                    let cmd = attr(e, "name")?.unwrap_or_default();
                    if empty {
                        doc.items.push(Item::Command {
                            name: cmd,
                            inputs: vec![],
                            outputs: vec![],
                        });
                    } else {
                        command = Some((cmd, vec![], vec![]));
                    }
                }
                "input" | "output" if command.is_some() => {
                    let c = command.as_mut().expect("checked above");
                    let list = numbered(e, &c.0)?;
                    if name == "input" {
                        c.1 = list;
                    } else {
                        c.2 = list;
                    }
                }
                "element" if in_construction && command.is_none() && element.is_none() => {
                    let el = Element {
                        kind: attr(e, "type")?.unwrap_or_default(),
                        label: attr(e, "label")?.unwrap_or_default(),
                        shown: true,
                        ..Element::default()
                    };
                    if empty {
                        doc.items.push(Item::Element(el));
                    } else {
                        element = Some(el);
                    }
                }
                "show" if element.is_some() => {
                    if let Some(object) = attr(e, "object")? {
                        element.as_mut().expect("checked above").shown = object != "false";
                    }
                }
                "coords" if element.is_some() => {
                    let get = |k: &str| attr(e, k).map(Option::unwrap_or_default);
                    element.as_mut().expect("checked above").coords =
                        Some([get("x")?, get("y")?, get("z")?]);
                }
                "value" if element.is_some() => {
                    element.as_mut().expect("checked above").value = attr(e, "val")?;
                }
                _ => {}
            }
        }
        match event {
            Event::End(e) => match e.name().as_ref() {
                "construction" => in_construction = false,
                "kernel" => in_kernel = false,
                "command" => {
                    if let Some((name, inputs, outputs)) = command.take() {
                        doc.items.push(Item::Command {
                            name,
                            inputs,
                            outputs,
                        });
                    }
                }
                "element" => {
                    if let Some(el) = element.take() {
                        doc.items.push(Item::Element(el));
                    }
                }
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
    }
    if in_construction || command.is_some() || element.is_some() {
        return Err(bad("unexpected end of the file inside the construction"));
    }
    if !seen_construction {
        return Err(ImportError::NoConstruction);
    }
    Ok(doc)
}
