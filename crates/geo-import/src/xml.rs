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
//! * Conic `<matrix A0= .. A5=>` and `<eqnStyle>`, numeric `<slider>` bounds,
//!   captions, object scripts, and `<condition showObject=>` visibility rules.
//! * `<fixed>`, `<auxiliary>`, and non-text `<startPoint>` are counted for a note.
//! * Text placement and style: `<startPoint>`, `<absoluteScreenLocation>`,
//!   `<labelOffset>`, `<font>`, and `<isLaTeX>`.
//!
//! Other UI properties and styles are ignored.
//!
//! At most [`MAX_ITEMS`] items (an item counts once, plus once per output of
//! a command) are read, a command lists at most [`MAX_ARGUMENTS`] inputs and
//! as many outputs, and an attribute value has at most
//! [`MAX_ATTRIBUTE_BYTES`] bytes.

use crate::{ImportError, Settings};
use geo_kernel::kernel::control::ControlScript;
use geo_kernel::kernel::text::TextPosition;
use geo_kernel::kernel::EquationForm;
use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};

/// The most items (expressions, commands, elements) a construction may have.
pub const MAX_ITEMS: usize = 200_000;

/// The most inputs, and the most outputs, one command may list. Every output
/// also counts as an item toward [`MAX_ITEMS`].
pub const MAX_ARGUMENTS: usize = 10_000;

/// The longest value one attribute may have, in bytes.
pub const MAX_ATTRIBUTE_BYTES: usize = 1 << 20;

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
    /// GeoGebra's angle interval code (`<angleStyle val=>`).
    pub angle_style: Option<i32>,
    /// The saved display form of a line or conic.
    pub equation_form: Option<EquationForm>,
    /// `<matrix A0= ... A5=>` of a conic as written.
    pub matrix: Option<[String; 6]>,
    /// `<fixed val="true">`: not used, counted for a note.
    pub fixed: bool,
    /// `<auxiliary val="true">`: not used, counted for a note.
    pub auxiliary: bool,
    pub condition: Option<String>,
    pub caption: String,
    pub scripts: Vec<ControlScript>,
    pub slider: Option<[Option<String>; 2]>,
    /// A `<startPoint>` of a vector: not used, counted for a note.
    pub start_point: bool,
    pub text: Option<Box<TextStyle>>,
}

/// The presentation properties of a saved text.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextStyle {
    pub position: Option<TextPosition<String>>,
    pub latex: Option<bool>,
    pub offset: Option<[i32; 2]>,
    pub font: Option<SavedFont>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SavedFont {
    pub multiplier: Option<f64>,
    pub legacy_size: Option<i32>,
    pub serif: bool,
    pub style: u8,
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
    /// Items plus outputs, what [`MAX_ITEMS`] limits.
    pub weight: usize,
    /// Horizontal range of the primary graphics view, used by function paths.
    pub function_path_bounds: Option<[f64; 2]>,
    /// Base font size used by older text elements with an additive size.
    pub gui_font_size: Option<f64>,
}

fn bad(e: impl std::fmt::Display) -> ImportError {
    ImportError::Xml(e.to_string())
}

/// The value of attribute `name`, with entities resolved.
fn attr(e: &BytesStart, name: &str) -> Result<Option<String>, ImportError> {
    // No duplicate check: it is quadratic in the number of attributes.
    for a in e.attributes().with_checks(false) {
        let a = a.map_err(bad)?;
        if a.key.as_ref() == name {
            return Ok(Some(value_of(&a)?));
        }
    }
    Ok(None)
}

/// The value of an attribute, with entities resolved, at most
/// [`MAX_ATTRIBUTE_BYTES`] long.
fn value_of(a: &quick_xml::events::attributes::Attribute) -> Result<String, ImportError> {
    if a.value.len() > MAX_ATTRIBUTE_BYTES {
        return Err(ImportError::AttributeTooLong);
    }
    Ok(a.normalized_value(XmlVersion::Implicit1_0)
        .map_err(bad)?
        .into_owned())
}

fn read_text_style(el: &mut Element, name: &str, e: &BytesStart) -> Result<(), ImportError> {
    let text = el.text.get_or_insert_with(Box::default);
    let read = |key: &str| attr(e, key);
    let number = |key: &str| {
        read(key).map(|v| {
            v.and_then(|s| s.parse::<f64>().ok())
                .filter(|v| v.is_finite())
        })
    };
    match name {
        "startPoint" => {
            let absolute = read("absolute")?.as_deref() == Some("true");
            text.position = Some(if let Some(point) = read("exp")?.or(read("label")?) {
                if absolute {
                    TextPosition::ScreenPoint(point)
                } else {
                    TextPosition::Point(point)
                }
            } else {
                let coords = match (number("x")?, number("y")?, number("z")?) {
                    (Some(x), Some(y), Some(z)) if z != 0.0 => Some((x / z, y / z)),
                    _ => None,
                };
                match coords.filter(|(x, y)| x.is_finite() && y.is_finite()) {
                    Some((x, y)) if absolute => TextPosition::Screen {
                        x: f64::from(x as i32),
                        y: f64::from(y as i32),
                    },
                    Some((x, y)) => TextPosition::World { x, y },
                    None => TextPosition::Undefined,
                }
            });
        }
        "absoluteScreenLocation" => {
            text.position = match (number("x")?, number("y")?) {
                (Some(x), Some(y)) => Some(TextPosition::Screen {
                    x: f64::from(x as i32),
                    y: f64::from(y as i32),
                }),
                _ => None,
            };
        }
        "labelOffset" => {
            text.offset = match (read("x")?, read("y")?) {
                (Some(x), Some(y)) => x.parse().ok().zip(y.parse().ok()).map(|(x, y)| [x, y]),
                _ => None,
            };
        }
        "isLaTeX" => text.latex = read("val")?.and_then(|v| v.parse().ok()),
        "font" => {
            text.font = Some(SavedFont {
                multiplier: number("sizeM")?.filter(|v| *v > 0.0),
                legacy_size: read("size")?.and_then(|v| v.parse().ok()),
                serif: read("serif")?.as_deref() == Some("true"),
                style: read("style")?.and_then(|v| v.parse().ok()).unwrap_or(0),
            });
        }
        _ => unreachable!("only text properties are read here"),
    }
    Ok(())
}

/// The attributes `a0`, `a1`, ... of an `<input>` or `<output>` tag, which
/// must be numbered without gaps.
fn numbered(e: &BytesStart, command: &str) -> Result<Vec<String>, ImportError> {
    let mut found: Vec<(usize, String)> = Vec::new();
    for a in e.attributes().with_checks(false) {
        let a = a.map_err(bad)?;
        let key = a.key.as_ref().to_string();
        let Some(index) = key.strip_prefix('a').and_then(|n| n.parse::<usize>().ok()) else {
            continue;
        };
        if found.len() >= MAX_ARGUMENTS {
            return Err(ImportError::TooManyArguments(command.to_string()));
        }
        found.push((index, value_of(&a)?));
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
    let weight = match &item {
        Item::Command { outputs, .. } => 1 + outputs.len(),
        _ => 1,
    };
    if doc.weight + weight > MAX_ITEMS {
        return Err(ImportError::TooManyItems);
    }
    doc.weight += weight;
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
    let mut in_gui = false;
    let mut view = None;
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
                "gui" if !in_construction => in_gui = !empty,
                "font" if in_gui && !in_construction => {
                    doc.gui_font_size = attr(e, "size")?
                        .and_then(|s| s.parse::<u32>().ok())
                        .filter(|v| *v > 0)
                        .map(f64::from);
                }
                "euclidianView"
                    if !in_construction && !empty && doc.function_path_bounds.is_none() =>
                {
                    view = Some((None, None, None, None));
                }
                "viewNumber" if view.is_some() => {
                    view.as_mut().expect("checked above").0 = attr(e, "viewNo")?;
                }
                "size" if view.is_some() => {
                    view.as_mut().expect("checked above").1 = attr(e, "width")?;
                }
                "coordSystem" if view.is_some() => {
                    let view = view.as_mut().expect("checked above");
                    view.2 = attr(e, "xZero")?;
                    view.3 = attr(e, "scale")?;
                }
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
                    element.as_mut().expect("checked above").condition = attr(e, "showObject")?;
                }
                "caption" if element.is_some() => {
                    element.as_mut().expect("checked above").caption =
                        attr(e, "val")?.unwrap_or_default();
                }
                "ggbscript" | "javascript" if element.is_some() => {
                    let scripts = &mut element.as_mut().expect("checked above").scripts;
                    for (attribute, event) in [
                        ("val", "click"),
                        ("onUpdate", "update"),
                        ("onDragEnd", "dragEnd"),
                        ("onChange", "change"),
                    ] {
                        if let Some(text) = attr(e, attribute)? {
                            scripts.push(ControlScript {
                                language: name.clone(),
                                event: event.into(),
                                text,
                            });
                        }
                    }
                }
                "slider" if element.is_some() => {
                    element.as_mut().expect("checked above").slider =
                        Some([attr(e, "min")?, attr(e, "max")?]);
                }
                "startPoint" | "absoluteScreenLocation" | "labelOffset" | "isLaTeX" | "font"
                    if element.as_ref().is_some_and(|el| el.kind == "text") =>
                {
                    read_text_style(element.as_mut().expect("checked above"), &name, e)?;
                }
                "startPoint" if element.is_some() => {
                    element.as_mut().expect("checked above").start_point = true;
                }
                "value" if element.is_some() => {
                    element.as_mut().expect("checked above").value = attr(e, "val")?;
                }
                "angleStyle" if element.is_some() => {
                    element.as_mut().expect("checked above").angle_style =
                        attr(e, "val")?.and_then(|v| v.parse().ok());
                }
                "eqnStyle"
                    if element
                        .as_ref()
                        .is_some_and(|el| matches!(el.kind.as_str(), "line" | "ray" | "conic")) =>
                {
                    let style = attr(e, "style")?;
                    let parameter = attr(e, "parameter")?;
                    element.as_mut().expect("checked above").equation_form = style
                        .and_then(|style| EquationForm::from_xml(&style, parameter.as_deref()));
                }
                _ => {}
            }
        }
        match event {
            Event::End(e) => match e.name().as_ref() {
                "construction" => in_construction = false,
                "kernel" => in_kernel = false,
                "gui" => in_gui = false,
                "euclidianView" => {
                    if let Some((number, width, xzero, scale)) = view.take() {
                        doc.function_path_bounds = (|| {
                            if number.as_deref().is_some_and(|n| n != "1") {
                                return None;
                            }
                            let (width, xzero, scale): (f64, f64, f64) = (
                                width?.parse().ok()?,
                                xzero?.parse().ok()?,
                                scale?.parse().ok()?,
                            );
                            let (min, max) = (-xzero / scale, (width - xzero) / scale);
                            (scale > 0.0 && min.is_finite() && max.is_finite() && min < max)
                                .then_some([min, max])
                        })();
                    }
                }
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
