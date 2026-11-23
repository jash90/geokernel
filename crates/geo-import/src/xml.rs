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
//! * Presentation that only export uses ([`ObjectStyle`]): colour, opacity,
//!   filling, dynamic colour and background, line and point style, label and
//!   tooltip mode, the fixed, auxiliary, selection and trace flags, decoration,
//!   the rounding of a text, the arc of an angle, slider placement and
//!   animation.
//!
//! Other UI properties and styles are ignored.
//!
//! At most [`MAX_ITEMS`] items (an item counts once, plus once per output of
//! a command) are read, a command lists at most [`MAX_ARGUMENTS`] inputs and
//! as many outputs, an attribute value has at most [`MAX_ATTRIBUTE_BYTES`]
//! bytes, and tags nest at most [`MAX_DEPTH`] deep.

use crate::export::{
    DynamicColor, Fill, LineStyle, ObjectStyle, Rounding, SegmentEnd, SliderAnimation, SliderFlags,
    SliderLayout,
};
use crate::{ImportError, Settings};
use geo_kernel::kernel::control::ControlScript;
use geo_kernel::kernel::image::{ImageCorner, ImageStyle};
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

/// The deepest nesting of tags. A saved file nests five levels (a macro,
/// its construction, an element, a tag).
pub const MAX_DEPTH: usize = 32;

/// An element of the construction that GeoGebra wrote.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Element {
    /// The `type` attribute (`point`, `numeric`, `conic`, ...).
    pub kind: String,
    pub label: String,
    /// `<show object=>`; `None` without the tag.
    pub shown: Option<bool>,
    /// Tags at which the reference reader gives up the whole file: a number
    /// it reads outside any guard is missing or malformed.
    pub reader_stops: Vec<&'static str>,
    pub layer: u8,
    pub label_shown: Option<bool>,
    pub label_offset: Option<[i32; 2]>,
    pub thickness: Option<u8>,
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
    pub implicit_coefficients: Option<String>,
    /// `<strokeBezierCoords val=>` of a pen stroke as written.
    pub stroke_coords: Option<String>,
    /// The first `<strokeCoords val=>`: the vertices of an older pen stroke,
    /// which the reference smooths in its view unless Bezier points came.
    pub legacy_stroke: Option<String>,
    /// `<fixed val="true">`: not used, counted for a note.
    pub fixed: bool,
    /// `<auxiliary val="true">`: not used, counted for a note.
    pub auxiliary: bool,
    pub condition: Option<String>,
    pub caption: String,
    pub object_style: ObjectStyle,
    pub curve_parameter: Option<f64>,
    pub slider_animation: Option<SliderAnimation>,
    pub scripts: Vec<ControlScript>,
    pub slider: Option<[Option<String>; 2]>,
    /// A `<startPoint>` of a vector: not used, counted for a note.
    pub start_point: bool,
    pub text: Option<Box<TextStyle>>,
    pub image: Option<Box<SavedImage>>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SavedImage {
    pub file: Option<String>,
    pub screen: Option<[f64; 2]>,
    pub start_points: Vec<(i32, ImageCorner<String>)>,
    pub anchors_in_screen_coords: bool,
    pub centered: bool,
    pub style: ImageStyle,
    pub invalid_position: bool,
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
    pub view: Option<crate::export::View>,
    /// How many `<construction>` tags the file has. GeoGebra's reader appends
    /// the elements of a second one to the first, and so does the importer.
    pub constructions: usize,
    /// Items plus outputs, what [`MAX_ITEMS`] limits.
    pub weight: usize,
    /// Horizontal range of the primary graphics view, used by function paths.
    pub function_path_bounds: Option<[f64; 2]>,
    /// Base font size used by older text elements with an additive size.
    pub gui_font_size: Option<f64>,
    /// The file format is 3.19 or older: its reader ignores the opacity of
    /// a list.
    pub legacy_list_alpha: bool,
}

#[derive(Default)]
struct SavedView {
    second: bool,
    size: Option<[u32; 2]>,
    width: Option<f64>,
    x: Option<f64>,
    y: Option<f64>,
    sx: Option<f64>,
    sy: Option<f64>,
    axes: [Option<bool>; 2],
    both_axes: Option<bool>,
    grid: Option<bool>,
}

impl SavedView {
    fn file_view(&self) -> Option<crate::export::View> {
        let size = self.size?;
        let origin = [self.x?, self.y?];
        let scale = [self.sx?, self.sy.or(self.sx)?];
        (size.iter().all(|&n| n > 0)
            && origin.iter().all(|x| x.is_finite())
            && scale.iter().all(|s| s.is_finite() && *s > 0.0))
        .then_some(crate::export::View {
            size,
            origin,
            scale,
            axes: self
                .axes
                .map(|axis| axis.or(self.both_axes).unwrap_or(true)),
            grid: self.grid.unwrap_or(false),
        })
    }

    fn read(&mut self, name: &str, e: &BytesStart) -> Result<(), ImportError> {
        let value = |key| attr(e, key);
        let number = |key| value(key).map(|s| s.and_then(|n| n.parse::<f64>().ok()));
        let flag = |key| value(key).map(|s| s.and_then(|n| n.parse::<bool>().ok()));
        match name {
            "viewNumber" => self.second = value("viewNo")?.is_some_and(|n| n != "1"),
            "size" => {
                let width = value("width")?;
                self.width = width.as_deref().and_then(|n| n.parse().ok());
                self.size = width
                    .and_then(|n| n.parse().ok())
                    .zip(value("height")?.and_then(|n| n.parse().ok()))
                    .map(|(w, h)| [w, h]);
            }
            "coordSystem" => {
                self.x = number("xZero")?;
                self.y = number("yZero")?;
                self.sx = number("scale")?;
                self.sy = number("yscale")?;
            }
            "evSettings" => {
                self.both_axes = flag("axes")?;
                self.grid = flag("grid")?;
            }
            "axis" => {
                if let Some(axis) = value("id")?
                    .and_then(|n| n.parse::<usize>().ok())
                    .and_then(|i| self.axes.get_mut(i))
                {
                    *axis = flag("show")?;
                }
            }
            _ => {}
        }
        Ok(())
    }
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

fn read_image(el: &mut Element, name: &str, e: &BytesStart) -> Result<(), ImportError> {
    let image = el.image.get_or_insert_with(Box::default);
    let read = |key| attr(e, key);
    let number = |key| read(key).map(|s| s.and_then(|s| s.parse::<f64>().ok()));
    let flag = |key| read(key).map(|s| s.is_some_and(|s| s == "true"));
    match name {
        "file" => image.file = read("name")?,
        "inBackground" => image.style.background = flag("val")?,
        "interpolate" => image.style.interpolate = flag("val")?,
        "centered" => image.centered = flag("val")?,
        "objColor" => {
            if let Some(alpha) = number("alpha")? {
                image.style.alpha = (alpha as f32).clamp(0.0, 1.0);
            }
        }
        "cropBox" if flag("cropped")? => {
            if let (Some(x), Some(y), Some(w), Some(h)) = (
                number("x")?,
                number("y")?,
                number("width")?,
                number("height")?,
            ) {
                image.style.crop = Some([x, y, w, h]);
            }
        }
        "absoluteScreenLocation" => {
            if let (Some(x), Some(y)) = (number("x")?, number("y")?) {
                image.screen = Some([f64::from(x as i32), f64::from(y as i32)]);
                image.anchors_in_screen_coords = true;
                image.start_points.retain(|(index, corner)| {
                    *index != 0 || matches!(corner, ImageCorner::Point(_))
                });
            } else {
                image.invalid_position = true;
            }
        }
        "startPoint" => {
            let index = read("number")?.and_then(|s| s.parse().ok()).unwrap_or(0);
            image.anchors_in_screen_coords = flag("absolute")?;
            if let Some(source) = read("exp")?.or(read("label")?) {
                image.start_points.push((index, ImageCorner::Point(source)));
            } else if let (Some(x), Some(y)) = (number("x")?, number("y")?) {
                let z = number("z")?.unwrap_or(1.0);
                if index == 0 {
                    image.start_points.retain(|(index, corner)| {
                        *index != 0 || matches!(corner, ImageCorner::Point(_))
                    });
                    if image.anchors_in_screen_coords {
                        image.screen = None;
                    }
                }
                image
                    .start_points
                    .push((index, ImageCorner::Coordinates([x / z, y / z])));
            }
        }
        _ => {}
    }
    Ok(())
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

/// `Integer.parseInt`: a sign and decimal digits that fit 32 bits.
fn java_int(text: &str) -> Option<i32> {
    text.parse().ok()
}

/// `Double.parseDouble`: white space around the number and a type suffix
/// are allowed, and the infinities and NaN have one spelling each.
fn java_double(text: &str) -> Option<f64> {
    let text = text.trim_matches(|ch: char| ch <= ' ');
    let text = text.strip_suffix(['f', 'F', 'd', 'D']).unwrap_or(text);
    let unsigned = text.trim_start_matches(['+', '-']);
    let named = unsigned
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic());
    if named && !matches!(unsigned, "NaN" | "Infinity") {
        return None;
    }
    text.parse().ok()
}

/// The number parser of the reference for sizes and positions, which also
/// takes `undefined` and `null` for NaN.
fn reference_double(text: &str) -> Option<f64> {
    match text {
        "NaN" | "undefined" | "null" => Some(f64::NAN),
        "Infinity" => Some(f64::INFINITY),
        "-Infinity" => Some(f64::NEG_INFINITY),
        other => java_double(other),
    }
}

/// A colour channel: any 32 bit integer, of which the low byte counts.
fn color_channel(text: &str) -> Option<u8> {
    java_int(text).map(|value| (value & 0xff) as u8)
}

/// `<lineStyle>`: the dash pattern first, then the thickness, the hidden
/// pattern, the opacity and the arrows of a locus. The reference reader
/// keeps what it set before the first attribute it cannot read, and sets
/// nothing without a dash pattern.
fn read_line_style(e: &BytesStart) -> Result<Option<LineStyle>, ImportError> {
    let Some(kind) = attr(e, "type")?.and_then(|v| java_int(&v)) else {
        return Ok(None);
    };
    let mut line = LineStyle {
        kind,
        ..LineStyle::default()
    };
    let Some(thickness) = attr(e, "thickness")?.and_then(|v| java_int(&v)) else {
        return Ok(Some(line));
    };
    line.thickness = Some(thickness.max(0));
    if let Some(text) = attr(e, "typeHidden")? {
        let Some(hidden) = java_int(&text) else {
            return Ok(Some(line));
        };
        line.hidden_kind = Some(hidden);
    }
    if let Some(text) = attr(e, "opacity")? {
        let Some(opacity) = java_int(&text) else {
            return Ok(Some(line));
        };
        line.opacity = Some(opacity);
    }
    line.draw_arrow = attr(e, "drawArrow")?.map(|v| v == "true");
    Ok(Some(line))
}

/// The placement of `<slider>`: the mode, the position, the width and then
/// the flags, up to the first number the reference reader cannot read.
fn read_slider_layout(e: &BytesStart) -> Result<SliderLayout, ImportError> {
    let mut layout = SliderLayout {
        absolute: attr(e, "absoluteScreenLocation")?.is_some_and(|v| v == "true"),
        ..SliderLayout::default()
    };
    if let Some(x) = attr(e, "x")? {
        let y = attr(e, "y")?;
        let (Some(x), Some(y)) = (
            reference_double(&x),
            y.as_deref().and_then(reference_double),
        ) else {
            return Ok(layout);
        };
        layout.position = Some([x, y]);
    }
    let Some(width) = attr(e, "width")?.and_then(|v| reference_double(&v)) else {
        return Ok(layout);
    };
    layout.width = Some(width);
    let flag = |key| attr(e, key).map(|v| v.is_some_and(|v| v == "true"));
    layout.flags = Some(SliderFlags {
        fixed: flag("fixed")?,
        show_algebra: flag("showAlgebra")?,
        horizontal: flag("horizontal")?,
    });
    Ok(layout)
}

/// The dynamic channels, fill pattern and opacity of an `<objColor>` tag
/// whose colour was accepted, in the order of the reference reader: dynamic
/// colour, hatch angle, inversion, hatch distance, pattern, symbol, opacity.
/// It reads the numbers of the fill outside any guard: `false` tells that it
/// gives up the file at this tag, and what it had not yet set is not kept.
fn read_fill(
    style: &mut ObjectStyle,
    e: &BytesStart,
    read_alpha: bool,
) -> Result<bool, ImportError> {
    let read = |key| attr(e, key);
    if let (Some(red), Some(green), Some(blue)) =
        (read("dynamicr")?, read("dynamicg")?, read("dynamicb")?)
    {
        // Three empty channels mean no dynamic colour; a single empty one is 0.
        if !(red.is_empty() && green.is_empty() && blue.is_empty()) {
            let channel = |text: String| if text.is_empty() { "0".into() } else { text };
            style.dynamic_color = Some(Box::new(DynamicColor {
                red: channel(red),
                green: channel(green),
                blue: channel(blue),
                alpha: read("dynamica")?.filter(|text| !text.is_empty()),
                // A colour space that is no number leaves the default.
                space: read("colorSpace")?.and_then(|v| java_int(&v)).unwrap_or(0),
            }));
        }
    }
    let mut fill = Fill::default();
    let mut touched = false;
    let complete = (|| -> Result<bool, ImportError> {
        let angle = match read("hatchAngle")? {
            Some(text) => {
                let Some(angle) = java_int(&text) else {
                    return Ok(false);
                };
                // The distance keeps its default until it is read.
                fill.hatch = Some([angle, Fill::DEFAULT_DISTANCE]);
                touched = true;
                Some(angle)
            }
            None => None,
        };
        if let Some(inverse) = read("inverseFill")? {
            fill.inverse = Some(inverse == "true");
            touched = true;
        }
        if let Some(angle) = angle {
            let Some(distance) = read("hatchDistance")?.and_then(|v| java_int(&v)) else {
                return Ok(false);
            };
            fill.hatch = Some([angle, distance]);
            // A hatch angle without a pattern is the hatched fill of older files.
            fill.pattern = 1;
        }
        if let Some(text) = read("fillType")? {
            let Some(index) = java_int(&text).filter(|i| (0..=9).contains(i)) else {
                return Ok(false);
            };
            fill.pattern = index as u8;
            touched = true;
        }
        if let Some(symbol) = read("fillSymbol")? {
            fill.symbol = Some(symbol);
            touched = true;
        }
        if read("image")?.is_some() {
            fill.pattern = Fill::IMAGE_PATTERN;
            touched = true;
        }
        if read_alpha {
            if let Some(text) = read("alpha")? {
                let Some(alpha) = java_double(&text) else {
                    return Ok(false);
                };
                // A value outside 0 to 1 is ignored; NaN passes that test.
                if !(alpha < 0.0 || alpha > 1.0) {
                    style.alpha = Some(alpha);
                }
            }
        }
        Ok(true)
    })()?;
    if touched {
        style.fill = Some(Box::new(fill));
    }
    Ok(complete)
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
    let mut depth = 0usize;
    loop {
        let event = reader.read_event().map_err(bad)?;
        let (start, empty) = match &event {
            Event::Start(e) => (Some(e), false),
            Event::Empty(e) => (Some(e), true),
            _ => (None, false),
        };
        match &event {
            Event::Start(_) => {
                depth += 1;
                if depth > MAX_DEPTH {
                    return Err(ImportError::TooDeep);
                }
            }
            Event::End(_) => depth = depth.saturating_sub(1),
            _ => {}
        }
        if let Some(e) = start {
            let name = e.name().as_ref().to_string();
            match name.as_str() {
                "geogebra" if depth <= 1 => {
                    // The reader of a format up to 3.19 ignores list opacity.
                    doc.legacy_list_alpha = attr(e, "format")?
                        .and_then(|format| reference_double(&format))
                        .is_some_and(|format| format <= 3.19);
                }
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
                    view = Some(SavedView::default());
                }
                "viewNumber" | "size" | "coordSystem" | "evSettings" | "axis" if view.is_some() => {
                    view.as_mut().expect("checked above").read(&name, e)?;
                }
                "continuous" | "decimals" | "significantfigures" | "angleUnit"
                | "startAnimation"
                    if in_kernel =>
                {
                    let val = attr(e, "val")?.unwrap_or_default();
                    match name.as_str() {
                        "continuous" => doc.settings.continuous = val == "true",
                        "decimals" => doc.settings.decimals = val.parse().ok(),
                        "significantfigures" => doc.settings.significant_figures = val.parse().ok(),
                        "startAnimation" => doc.settings.start_animation = val == "true",
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
                        ..Element::default()
                    };
                    if empty {
                        push(&mut doc, Item::Element(Box::new(el)))?;
                    } else {
                        element = Some(el);
                    }
                }
                "show" if element.is_some() => {
                    let el = element.as_mut().expect("checked above");
                    // Only `true` shows the object, as in the reference reader.
                    el.shown = Some(attr(e, "object")?.is_some_and(|object| object == "true"));
                    el.label_shown = Some(attr(e, "label")?.is_some_and(|label| label == "true"));
                    // The views change only when the bits are a number.
                    match attr(e, "ev")? {
                        None => el.object_style.views = Some(0),
                        Some(bits) => {
                            if let Some(bits) = java_int(&bits) {
                                el.object_style.views = Some(bits);
                            }
                        }
                    }
                }
                "layer" if element.is_some() => {
                    if let Some(layer) = attr(e, "val")?.and_then(|v| v.parse::<i32>().ok()) {
                        let el = element.as_mut().expect("checked above");
                        el.layer = layer.clamp(0, 9) as u8;
                        el.object_style.layer = Some(el.layer);
                    }
                }
                "lineStyle" if element.is_some() => {
                    let el = element.as_mut().expect("checked above");
                    if let Some(line) = read_line_style(e)? {
                        el.thickness = line.thickness.map(|v| v.clamp(0, 255) as u8);
                        el.object_style.line = Some(line);
                    }
                }
                "ordering" if element.is_some() => {
                    // `Double.parseDouble`, ignored when it fails.
                    if let Some(order) = attr(e, "val")?.and_then(|v| java_double(&v)) {
                        element
                            .as_mut()
                            .expect("checked above")
                            .object_style
                            .ordering = Some(order);
                    }
                }
                "dynamicCaption" if element.is_some() => {
                    element
                        .as_mut()
                        .expect("checked above")
                        .object_style
                        .dynamic_caption = attr(e, "val")?;
                }
                "startStyle" | "endStyle" if element.is_some() => {
                    // An unknown name clears the style in the reference, which
                    // then cannot save the segment; it is not kept here.
                    let end = attr(e, "val")?.and_then(|v| SegmentEnd::from_xml(&v));
                    let ends = &mut element.as_mut().expect("checked above").object_style.ends;
                    ends[usize::from(name == "endStyle")] = end;
                }
                "headStyle" if element.is_some() => {
                    if let Some(index) = attr(e, "val")?
                        .and_then(|v| java_int(&v))
                        .filter(|index| (0..=1).contains(index))
                    {
                        element
                            .as_mut()
                            .expect("checked above")
                            .object_style
                            .head_style = Some(index as u8);
                    }
                }
                "labelMode" | "tooltipMode" | "pointSize" | "pointStyle" | "arcSize"
                    if element.is_some() =>
                {
                    let el = element.as_mut().expect("checked above");
                    let text = attr(e, "val")?;
                    let value = text.as_deref().and_then(java_int);
                    let style = &mut el.object_style;
                    match name.as_str() {
                        "labelMode" => style.label_mode = value.or(style.label_mode),
                        "tooltipMode" => style.tooltip_mode = value.or(style.tooltip_mode),
                        // On a number the tag is the size of the slider knob.
                        "pointSize" if matches!(el.kind.as_str(), "numeric" | "angle") => {
                            match text.as_deref().map(reference_double) {
                                Some(Some(size)) => style.knob_size = Some(size),
                                // The number is read outside any guard.
                                _ => el.reader_stops.push("pointSize"),
                            }
                        }
                        "pointSize" => style.point_size = value.or(style.point_size),
                        "pointStyle" => style.point_style = value.or(style.point_style),
                        _ => style.arc_size = value.or(style.arc_size),
                    }
                }
                "decoration" if element.is_some() => {
                    element
                        .as_mut()
                        .expect("checked above")
                        .object_style
                        .decoration = attr(e, "type")?.and_then(|v| v.parse().ok());
                }
                "selectionAllowed" | "trace" | "emphasizeRightAngle" if element.is_some() => {
                    let style = &mut element.as_mut().expect("checked above").object_style;
                    let on = attr(e, "val")?.map(|v| v == "true");
                    match name.as_str() {
                        "selectionAllowed" => style.selection_allowed = on,
                        "trace" => style.trace = on,
                        _ => style.emphasize_right_angle = on,
                    }
                }
                "fixedSize" | "showTrimmed" | "breakpoint" if element.is_some() => {
                    let style = &mut element.as_mut().expect("checked above").object_style;
                    let on = attr(e, "val")?.map(|v| v == "true");
                    match name.as_str() {
                        "fixedSize" => style.fixed_size = on,
                        "showTrimmed" => style.show_trimmed = on,
                        _ => style.breakpoint = on,
                    }
                }
                "algebra" if element.is_some() => {
                    // Anything but `false` shows the label.
                    element
                        .as_mut()
                        .expect("checked above")
                        .object_style
                        .algebra_label =
                        Some(attr(e, "labelVisible")?.is_none_or(|v| v != "false"));
                }
                "checkbox" if element.is_some() => {
                    element
                        .as_mut()
                        .expect("checked above")
                        .object_style
                        .checkbox_fixed = attr(e, "fixed")?.map(|v| v == "true");
                }
                "decimals" | "significantfigures" if element.is_some() => {
                    if let Some(digits) = attr(e, "val")?.and_then(|v| v.parse().ok()) {
                        element
                            .as_mut()
                            .expect("checked above")
                            .object_style
                            .rounding = Some(if name == "decimals" {
                            Rounding::Decimals(digits)
                        } else {
                            Rounding::SignificantFigures(digits)
                        });
                    }
                }
                "bgColor" if element.is_some() => {
                    let channel = |key| -> Result<Option<u8>, ImportError> {
                        Ok(attr(e, key)?.and_then(|v| color_channel(&v)))
                    };
                    // All four channels are required, as in the reference reader.
                    if let (Some(r), Some(g), Some(b), Some(a)) = (
                        channel("r")?,
                        channel("g")?,
                        channel("b")?,
                        channel("alpha")?,
                    ) {
                        element
                            .as_mut()
                            .expect("checked above")
                            .object_style
                            .background = Some([r, g, b, a]);
                    }
                }
                "curveParam" if element.is_some() => {
                    element.as_mut().expect("checked above").curve_parameter = attr(e, "t")?
                        .and_then(|v| v.parse::<f64>().ok())
                        .filter(|v| v.is_finite());
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
                        el.object_style.fixed = Some(on);
                    } else {
                        el.auxiliary = on;
                        el.object_style.auxiliary = Some(on);
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
                "animation" if element.is_some() => {
                    element.as_mut().expect("checked above").slider_animation =
                        Some(SliderAnimation {
                            step: attr(e, "step")?,
                            speed: attr(e, "speed")?,
                            mode: attr(e, "type")?
                                .and_then(|v| v.parse().ok())
                                .unwrap_or_default(),
                            playing: attr(e, "playing")?.is_some_and(|v| v == "true"),
                        });
                }
                "slider" if element.is_some() => {
                    let el = element.as_mut().expect("checked above");
                    el.slider = Some([attr(e, "min")?, attr(e, "max")?]);
                    el.object_style.slider = Some(Box::new(read_slider_layout(e)?));
                }
                "file"
                | "inBackground"
                | "interpolate"
                | "centered"
                | "cropBox"
                | "absoluteScreenLocation"
                | "startPoint"
                    if element.as_ref().is_some_and(|el| el.kind == "image") =>
                {
                    read_image(element.as_mut().expect("checked above"), &name, e)?;
                }
                "objColor" if element.is_some() => {
                    let el = element.as_mut().expect("checked above");
                    if el.kind == "image" {
                        read_image(el, &name, e)?;
                    }
                    let channel = |key| -> Result<Option<u8>, ImportError> {
                        Ok(attr(e, key)?.and_then(|v| color_channel(&v)))
                    };
                    if let (Some(r), Some(g), Some(b)) =
                        (channel("r")?, channel("g")?, channel("b")?)
                    {
                        el.object_style.color = Some([r, g, b]);
                        // Before format 3.2 the opacity of a list is not read.
                        let read_alpha = !(el.kind == "list" && doc.legacy_list_alpha);
                        if !read_fill(&mut el.object_style, e, read_alpha)? {
                            el.reader_stops.push("objColor");
                        }
                    }
                }
                "labelOffset" if element.as_ref().is_some_and(|el| el.kind != "text") => {
                    let x = attr(e, "x")?.and_then(|n| n.parse().ok());
                    let y = attr(e, "y")?.and_then(|n| n.parse().ok());
                    element.as_mut().expect("checked above").label_offset =
                        x.zip(y).map(|(x, y)| [x, y]);
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
                "strokeBezierCoords"
                    if element.as_ref().is_some_and(|el| el.kind == "penstroke") =>
                {
                    let el = element.as_mut().expect("checked above");
                    // the reader replaces the points with every tag it reads
                    if let Some(coords) = attr(e, "val")?.filter(|v| !v.is_empty()) {
                        el.stroke_coords = Some(coords);
                    }
                }
                "strokeCoords" if element.as_ref().is_some_and(|el| el.kind == "penstroke") => {
                    let el = element.as_mut().expect("checked above");
                    if let Some(coords) = attr(e, "val")?.filter(|v| !v.is_empty()) {
                        el.legacy_stroke.get_or_insert(coords);
                    }
                }
                "coefficients" if element.as_ref().is_some_and(|el| el.kind == "implicitpoly") => {
                    if attr(e, "rep")?.as_deref() == Some("array") {
                        element
                            .as_mut()
                            .expect("checked above")
                            .implicit_coefficients = attr(e, "data")?;
                    }
                }
                "userinput" if element.as_ref().is_some_and(|el| el.kind == "implicitpoly") => {
                    let input = attr(e, "show")?.as_deref() == Some("true");
                    element.as_mut().expect("checked above").equation_form = Some(if input {
                        EquationForm::User
                    } else {
                        EquationForm::Implicit
                    });
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
                    if let Some(view) = view.take().filter(|v| !v.second) {
                        doc.view = view.file_view();
                        doc.function_path_bounds = (|| {
                            let (width, xzero, scale) = (view.width?, view.x?, view.sx?);
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
