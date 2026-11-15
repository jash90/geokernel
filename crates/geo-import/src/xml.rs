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
//! * `<matrix A0= .. A5=>` of a conic, and the cheap flags `<fixed>`,
//!   `<auxiliary>`, `<condition>`, `<startPoint>`, which no value depends on
//!   (they are only counted, for a note).
//!
//! Everything else (the user interface, styles, scripts) is ignored.
//!
//! At most [`MAX_ITEMS`] items are read.

use crate::{ImportError, Settings};
use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};

/// The most items (expressions, commands, elements) a construction may have.
pub const MAX_ITEMS: usize = 200_000;

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
    /// `<matrix A0= ... A5=>` of a conic as written.
    pub matrix: Option<[String; 6]>,
    /// `<fixed val="true">`: not used, counted for a note.
    pub fixed: bool,
    /// `<auxiliary val="true">`: not used, counted for a note.
    pub auxiliary: bool,
    /// A `<condition>` (conditional visibility): not used, counted for a note.
    pub conditional: bool,
    /// A `<startPoint>` of a vector: not used, counted for a note.
    pub start_point: bool,
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
    Element(Box<Element>),
}

/// The parsed construction.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Document {
    pub items: Vec<Item>,
    pub settings: Settings,
    /// How many `<construction>` tags the file has. GeoGebra's reader appends
    /// the elements of a second one to the first, and so does the importer.
    pub constructions: usize,
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

fn push(doc: &mut Document, item: Item) -> Result<(), ImportError> {
    if doc.items.len() >= MAX_ITEMS {
        return Err(ImportError::TooManyItems);
    }
    doc.items.push(item);
    Ok(())
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
                    doc.constructions += 1;
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
                        push(&mut doc, Item::Expression { label, exp })?;
                    }
                }
                "command" if in_construction && command.is_none() && element.is_none() => {
                    let cmd = attr(e, "name")?.unwrap_or_default();
                    if empty {
                        push(
                            &mut doc,
                            Item::Command {
                                name: cmd,
                                inputs: vec![],
                                outputs: vec![],
                            },
                        )?;
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
                        push(&mut doc, Item::Element(Box::new(el)))?;
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
                "matrix" if element.is_some() => {
                    let get = |k: &str| attr(e, k).map(Option::unwrap_or_default);
                    element.as_mut().expect("checked above").matrix = Some([
                        get("A0")?,
                        get("A1")?,
                        get("A2")?,
                        get("A3")?,
                        get("A4")?,
                        get("A5")?,
                    ]);
                }
                "fixed" | "auxiliary" if element.is_some() => {
                    let on = attr(e, "val")?.is_some_and(|v| v == "true");
                    let el = element.as_mut().expect("checked above");
                    if name == "fixed" {
                        el.fixed = on;
                    } else {
                        el.auxiliary = on;
                    }
                }
                "condition" if element.is_some() => {
                    element.as_mut().expect("checked above").conditional = true;
                }
                "startPoint" if element.is_some() => {
                    element.as_mut().expect("checked above").start_point = true;
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
                        push(
                            &mut doc,
                            Item::Command {
                                name,
                                inputs,
                                outputs,
                            },
                        )?;
                    }
                }
                "element" => {
                    if let Some(el) = element.take() {
                        push(&mut doc, Item::Element(Box::new(el)))?;
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
