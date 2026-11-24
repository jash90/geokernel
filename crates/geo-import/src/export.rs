//! Writes a .ggb archive from a live construction and its definition journal.
//!
//! Definitions preserve dependencies and output slots. Element tags record
//! the current coordinates and presentation, including edits made after the
//! definitions ran.

mod element;

use crate::{archive, Settings};
use geo_kernel::kernel::image::RasterAsset;
use geo_kernel::{script, Construction, Def, Id, Kind};
use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::io::{Cursor, Write};

/// One definition and the labelled outputs it created, in slot order.
pub struct Definition<'a> {
    pub line: &'a str,
    pub outputs: &'a [String],
}

/// The primary graphics view in file coordinates (origin relative to the view).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub size: [u32; 2],
    pub origin: [f64; 2],
    pub scale: [f64; 2],
    pub axes: [bool; 2],
    pub grid: bool,
}

impl Default for View {
    fn default() -> Self {
        Self {
            size: [800, 600],
            origin: [215.0, 315.0],
            scale: [50.0; 2],
            axes: [true; 2],
            grid: false,
        }
    }
}

/// Presentation data kept outside the kernel by the document.
#[derive(Default)]
pub struct Presentation<'a> {
    pub view: View,
    pub settings: Settings,
    pub cas_rows: Vec<crate::SavedCasRow>,
    /// Colours of the objects that have no saved style.
    pub colors: BTreeMap<&'a str, [u8; 3]>,
    /// Fill opacity the application draws those objects with, from 0 to 1.
    pub opacity: BTreeMap<&'a str, f64>,
    pub hidden: HashSet<&'a str>,
    pub conditions: BTreeMap<&'a str, &'a str>,
    pub label_offsets: BTreeMap<&'a str, (i32, i32)>,
    pub object_styles: BTreeMap<&'a str, &'a ObjectStyle>,
    pub slider_animation: BTreeMap<&'a str, &'a SliderAnimation>,
    pub slider_bounds: BTreeMap<&'a str, [Option<&'a str>; 2]>,
}

/// Presentation of one object as its source file saved it. The editor does
/// not change it and export writes it back. `None` means that the file had no
/// such tag or attribute, so the reader keeps its own default.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ObjectStyle {
    pub caption: String,
    /// `<show label=>`.
    pub show_label: Option<bool>,
    /// The file had no `<show>` tag, so its reader decides the visibility.
    pub untagged: bool,
    /// View membership (`<show ev=>`): bit 0 leaves the first view, bit 1
    /// joins the second, 4 and 8 switch the 3D view on and off, 16 and 32
    /// the view of a plane.
    pub views: Option<i32>,
    /// `<layer val=>`. The reader gives an object without the tag the layer
    /// of the object before it, so its absence is kept.
    pub layer: Option<u8>,
    /// `<labelMode val=>`: name, name and value, value, caption, caption and value.
    pub label_mode: Option<i32>,
    /// `<tooltipMode val=>`.
    pub tooltip_mode: Option<i32>,
    /// `<objColor r= g= b=>`.
    pub color: Option<[u8; 3]>,
    /// Fill opacity from 0 to 1 (`<objColor alpha=>`).
    pub alpha: Option<f64>,
    pub fill: Option<Box<Fill>>,
    pub dynamic_color: Option<Box<DynamicColor>>,
    /// `<bgColor r= g= b= alpha=>`.
    pub background: Option<[u8; 4]>,
    /// `<lineStyle>`.
    pub line: Option<LineStyle>,
    /// `<pointSize val=>` of a point or a list.
    pub point_size: Option<i32>,
    /// `<pointStyle val=>`; -1 selects the document default.
    pub point_style: Option<i32>,
    /// `<fixed val=>`.
    pub fixed: Option<bool>,
    /// `<auxiliary val=>`.
    pub auxiliary: Option<bool>,
    /// `<selectionAllowed val=>`.
    pub selection_allowed: Option<bool>,
    /// `<trace val=>`.
    pub trace: Option<bool>,
    /// `<decoration type=>`: ticks and arcs of segments and angles.
    pub decoration: Option<i32>,
    /// `<ordering val=>`: the drawing order within a layer.
    pub ordering: Option<f64>,
    /// `<dynamicCaption val=>`: the label of the text shown as the caption.
    pub dynamic_caption: Option<String>,
    /// `<pointSize val=>` of a number: the size of the knob of its slider.
    pub knob_size: Option<f64>,
    /// `<fixedSize val=>`.
    pub fixed_size: Option<bool>,
    /// `<showTrimmed val=>`: only the parts of the intersecting lines near
    /// an intersection point are drawn.
    pub show_trimmed: Option<bool>,
    /// `<breakpoint val=>` of the construction protocol.
    pub breakpoint: Option<bool>,
    /// `<algebra labelVisible=>`.
    pub algebra_label: Option<bool>,
    /// `<startStyle val=>` and `<endStyle val=>` of a segment or a broken line.
    pub ends: [Option<SegmentEnd>; 2],
    /// `<headStyle val=>` of a vector: 0 plain, 1 arrow.
    pub head_style: Option<u8>,
    /// Rounding of one text (`<decimals>` or `<significantfigures>`).
    pub rounding: Option<Rounding>,
    /// `<arcSize val=>` of an angle, in pixels.
    pub arc_size: Option<i32>,
    /// `<emphasizeRightAngle val=>`.
    pub emphasize_right_angle: Option<bool>,
    /// `<checkbox fixed=>`.
    pub checkbox_fixed: Option<bool>,
    /// Placement of a slider (`<slider>` without its bounds).
    pub slider: Option<Box<SliderLayout>>,
    /// `<animation>` of an object that is not a slider.
    pub animation: Option<Box<SliderAnimation>>,
}

/// `<lineStyle type= thickness= typeHidden= opacity= drawArrow=>`.
///
/// The reference reader applies the attributes in this order and stops at
/// the first one it cannot read, so a later field is only set when every
/// earlier attribute was read.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LineStyle {
    /// Dash pattern code: 0 full, 10 and 15 dashed, 20 dotted, 30 dash-dotted.
    pub kind: i32,
    /// Any value of 0 or more.
    pub thickness: Option<i32>,
    pub hidden_kind: Option<i32>,
    /// Stroke opacity from 0 to 255.
    pub opacity: Option<i32>,
    /// A locus drawn as arrows (a slope field).
    pub draw_arrow: Option<bool>,
}

/// The fill attributes of `<objColor>`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Fill {
    /// Index of the pattern (`fillType`): 0 plain, 1 hatched, 2 cross-hatched,
    /// 3 chessboard, 4 dotted, 5 honeycomb, 6 brick, 7 weaving, 8 symbols.
    pub pattern: u8,
    /// `hatchAngle` in degrees and `hatchDistance` in pixels.
    pub hatch: Option<[i32; 2]>,
    /// `fillSymbol`.
    pub symbol: Option<String>,
    /// `inverseFill`.
    pub inverse: Option<bool>,
}

impl Fill {
    /// The last pattern the writer supports; an image fill needs an asset.
    pub const MAX_PATTERN: u8 = 8;
    /// The index of the image fill.
    pub const IMAGE_PATTERN: u8 = 9;
    /// The distance of a hatch whose distance the reader never set.
    pub const DEFAULT_DISTANCE: i32 = 10;
}

/// The decoration of one end of a segment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SegmentEnd {
    Default,
    Line,
    Arrow,
    CrowsFoot,
    ArrowOutline,
    ArrowFilled,
    CircleOutline,
    Circle,
    SquareOutline,
    Square,
    DiamondOutline,
    Diamond,
}

impl SegmentEnd {
    const NAMES: [(&'static str, SegmentEnd); 12] = [
        ("default", Self::Default),
        ("line", Self::Line),
        ("arrow", Self::Arrow),
        ("crows_foot", Self::CrowsFoot),
        ("arrow_outline", Self::ArrowOutline),
        ("arrow_filled", Self::ArrowFilled),
        ("circle_outline", Self::CircleOutline),
        ("circle", Self::Circle),
        ("square_outline", Self::SquareOutline),
        ("square", Self::Square),
        ("diamond_outline", Self::DiamondOutline),
        ("diamond", Self::Diamond),
    ];

    /// The saved name, in any letter case.
    pub fn from_xml(name: &str) -> Option<Self> {
        Self::NAMES
            .iter()
            .find(|(known, _)| known.eq_ignore_ascii_case(name))
            .map(|&(_, end)| end)
    }

    pub fn xml_name(self) -> &'static str {
        Self::NAMES
            .iter()
            .find(|&&(_, end)| end == self)
            .map_or("default", |&(name, _)| name)
    }
}

/// Colour channels computed from numeric expressions (`dynamicr`, ...).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DynamicColor {
    pub red: String,
    pub green: String,
    pub blue: String,
    pub alpha: Option<String>,
    /// `colorSpace`: 0 RGB, 1 HSB, 2 HSL.
    pub space: i32,
}

impl DynamicColor {
    pub fn channels(&self) -> impl Iterator<Item = &str> {
        [&self.red, &self.green, &self.blue]
            .into_iter()
            .chain(&self.alpha)
            .map(String::as_str)
    }
}

/// Rounding saved on one text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rounding {
    Decimals(i32),
    SignificantFigures(i32),
}

/// Where and how a slider is drawn (`<slider>` without its bounds).
///
/// The reference reader applies the placement mode, the position, the width
/// and then the flags, and stops at the first attribute it cannot read.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SliderLayout {
    pub absolute: bool,
    /// Pixels when `absolute`, otherwise world coordinates.
    pub position: Option<[f64; 2]>,
    /// `None` when the reader stopped at or before the width.
    pub width: Option<f64>,
    /// `fixed`, `showAlgebra` and `horizontal`, read after the width.
    pub flags: Option<SliderFlags>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SliderFlags {
    pub fixed: bool,
    pub show_algebra: bool,
    pub horizontal: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SliderAnimation {
    pub step: Option<String>,
    pub speed: Option<String>,
    pub mode: u8,
    pub playing: bool,
}

#[derive(Debug)]
pub struct ExportError(String);

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ExportError {}

impl ExportError {
    pub fn incomplete_import(skipped: usize, mismatched: usize) -> Self {
        failure(format!("Export refused: the source had {skipped} skipped objects and {mismatched} saved value mismatches."))
    }
}

fn failure(message: impl Into<String>) -> ExportError {
    ExportError(message.into())
}

/// Serializes the document before the caller opens the destination file.
pub fn ggb<'a>(
    construction: &Construction,
    definitions: impl IntoIterator<Item = Definition<'a>>,
    presentation: &Presentation<'_>,
) -> Result<Vec<u8>, ExportError> {
    // `AlgoElement.getCmdOutputXML` saves such an output as an empty slot.
    if construction
        .ids()
        .any(|id| construction.is_freed_output(id))
    {
        return Err(failure(
            "an interval intersection has an output whose label it freed, \
             which the export cannot save as an empty output slot",
        ));
    }
    let mut writer = Writer {
        c: construction,
        presentation,
        xml: Xml::default(),
        images: BTreeMap::new(),
        expressions: crate::convert::StyleExpressions::default(),
    };
    writer.xml.buffer.push_str(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<geogebra format=\"5.0\" app=\"classic\">\n",
    );
    writer.settings()?;
    writer
        .xml
        .buffer
        .push_str("<construction title=\"\" author=\"\" date=\"\">\n");
    let mut seen = HashSet::new();
    for definition in definitions {
        let ids = definition
            .outputs
            .iter()
            .map(|label| {
                let id = construction
                    .id(label)
                    .ok_or_else(|| failure(format!("missing output `{label}`")))?;
                if !seen.insert(id) {
                    return Err(failure(format!("duplicate output `{label}`")));
                }
                Ok(id)
            })
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(&first) = ids.first() {
            writer.definition(first, &ids, definition.line)?;
            for id in ids {
                writer.element(id, definition.line)?;
            }
        }
        if writer.xml.buffer.len() as u64 > archive::MAX_XML_BYTES {
            return Err(failure("construction XML exceeds the import size limit"));
        }
    }
    if let Some(id) = construction
        .ids()
        .find(|&id| !construction.label(id).starts_with('\0') && !seen.contains(&id))
    {
        return Err(failure(format!(
            "missing definition for `{}`",
            construction.label(id)
        )));
    }
    if presentation.cas_rows.len() > crate::MAX_CAS_ROWS {
        return Err(failure("too many CAS rows"));
    }
    for row in &presentation.cas_rows {
        writer.xml.cas_row(row)?;
        if writer.xml.buffer.len() as u64 > archive::MAX_XML_BYTES {
            return Err(failure("construction XML exceeds the import size limit"));
        }
    }
    writer.xml.buffer.push_str("</construction>\n</geogebra>\n");
    if writer.xml.buffer.len() as u64 > archive::MAX_XML_BYTES
        || writer.images.len() + 1 > archive::MAX_ZIP_ENTRIES
    {
        return Err(failure("construction exceeds the import archive limits"));
    }

    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut write = |name: &str, bytes: &[u8]| -> Result<(), ExportError> {
        zip.start_file(name, options)
            .map_err(|e| failure(e.to_string()))?;
        zip.write_all(bytes).map_err(|e| failure(e.to_string()))
    };
    for (name, asset) in writer.images.values() {
        write(name, asset.bytes())?;
    }
    write(archive::XML_FILE, writer.xml.buffer.as_bytes())?;
    zip.finish()
        .map(Cursor::into_inner)
        .map_err(|e| failure(e.to_string()))
}

struct Writer<'a> {
    c: &'a Construction,
    presentation: &'a Presentation<'a>,
    xml: Xml,
    images: BTreeMap<u64, (String, &'a RasterAsset)>,
    /// The budget of the style expressions written in this export.
    expressions: crate::convert::StyleExpressions,
}

impl Writer<'_> {
    fn settings(&mut self) -> Result<(), ExportError> {
        let View {
            size: [width, height],
            origin: [x, y],
            scale: [sx, sy],
            axes,
            grid,
        } = self.presentation.view;
        if width == 0
            || height == 0
            || ![x, y, sx, sy].iter().all(|n| n.is_finite())
            || sx <= 0.0
            || sy <= 0.0
        {
            return Err(failure("invalid graphics view"));
        }
        self.xml.buffer.push_str("<euclidianView>\n");
        self.xml.tag(
            "size",
            &[
                ("width", &width.to_string()),
                ("height", &height.to_string()),
            ],
        )?;
        self.xml.tag(
            "coordSystem",
            &[
                ("xZero", &x.to_string()),
                ("yZero", &y.to_string()),
                ("scale", &sx.to_string()),
                ("yscale", &sy.to_string()),
            ],
        )?;
        self.xml.tag(
            "evSettings",
            &[
                ("axes", flag(axes[0] && axes[1])),
                ("grid", flag(grid)),
                ("gridIsBold", "false"),
                ("gridType", "0"),
                ("pointCapturing", "3"),
            ],
        )?;
        for (i, show) in axes.into_iter().enumerate() {
            self.xml.tag(
                "axis",
                &[
                    ("id", &i.to_string()),
                    ("show", flag(show)),
                    ("label", ""),
                    ("unitLabel", ""),
                    ("tickStyle", "1"),
                    ("showNumbers", "true"),
                ],
            )?;
        }
        let settings = &self.presentation.settings;
        self.xml.buffer.push_str("</euclidianView>\n<kernel>\n");
        self.xml.val("continuous", flag(settings.continuous))?;
        self.xml.val("usePathAndRegionParameters", "true")?;
        if let Some(figures) = settings.significant_figures {
            self.xml.val("significantfigures", &figures.to_string())?;
        } else {
            self.xml
                .val("decimals", &settings.decimals.unwrap_or(2).to_string())?;
        }
        self.xml.val(
            "angleUnit",
            settings.angle_unit.as_deref().unwrap_or("degree"),
        )?;
        // an animation that runs starts again when the file is opened
        if self.c.is_animating() {
            self.xml.val("startAnimation", "true")?;
        }
        // Object scripts are kept as data by the application.
        self.xml
            .buffer
            .push_str("</kernel>\n<scripting blocked=\"true\" disabled=\"true\"/>\n");
        Ok(())
    }

    fn definition(&mut self, id: Id, outputs: &[Id], line: &str) -> Result<(), ExportError> {
        let c = self.c;
        let def = c.def(id);
        let label = c.label(id);
        if c.relation_reports(id).is_some() {
            return Err(failure(format!(
                "relation report `{label}` has no saved object form"
            )));
        }
        let line = script::split_comment(line).0;
        if line.starts_with("$implicitCoefficients(") {
            return Ok(());
        }
        let separator = line
            .find([':', '='])
            .ok_or_else(|| failure(format!("missing definition for `{label}`")))?;
        let (lhs, source) = (line[..separator].trim(), line[separator + 1..].trim());
        let source = c
            .construction_source(id)
            .or_else(|| script::saved_definition(source))
            .unwrap_or(source);
        let call = script::split_call(source);

        match def {
            Def::Image(_) | Def::Boolean(_) | Def::Button | Def::Angle(_) => return Ok(()),
            // `AlgoLocusStroke` saves an empty command; the element holds the
            // points
            Def::PenStroke => return self.expression(id, "PenStroke[]"),
            Def::Number(_) if c.construction_source(id).is_none() => return Ok(()),
            Def::FreePoint { .. } if c.construction_source(id).is_none() => return Ok(()),
            Def::InputBox(link) => {
                let args = link.map(|id| c.label(id)).into_iter().collect::<Vec<_>>();
                return self.command("Textfield", &args, outputs);
            }
            _ => {}
        }

        if c.is_independent_text(id) {
            return self.expression(id, &text_expression(c.text(id).unwrap_or_default()));
        }
        if call
            .as_ref()
            .is_some_and(|(name, _)| *name == "$pointOnPath")
        {
            let (_, args) = call.expect("saved path point has arguments");
            return self.command("Point", &args[..1], outputs);
        }
        if matches!(c.kind(id), Kind::Function | Kind::FunctionNVar) && lhs.ends_with(')') {
            return self.expression(id, &format!("{lhs} = {}", syntax(source)?));
        }
        if let Some((name, inputs)) = call {
            let name = if name == "$regularPolygon" {
                "Polygon"
            } else {
                name
            };
            if name == "$point" || name == "$vector" {
                return self.expression(id, &syntax(&format!("({})", inputs.join(", ")))?);
            }
            if name == "Intersect"
                && inputs.len() == 2
                && c.inputs(id).any(|id| c.kind(id) == Kind::Curve)
                && lhs
                    .strip_prefix('{')
                    .and_then(|labels| labels.strip_suffix('}'))
                    .is_some_and(|labels| labels.split(',').any(|label| label.trim().is_empty()))
            {
                return Err(failure(format!(
                    "curve intersection `{label}` has an unlabelled output slot"
                )));
            }
            if name.chars().next().is_some_and(char::is_uppercase)
                && name.chars().all(char::is_alphanumeric)
                && c.id(name).is_none()
            {
                let name = match name {
                    "PerpendicularLine" => "OrthogonalLine",
                    "PerpendicularBisector" => "LineBisector",
                    "AngleBisector" => "AngularBisector",
                    "CircularArc" => "CircleArc",
                    "CircularSector" => "CircleSector",
                    "CircumcircularArc" => "CircumcircleArc",
                    "CircumcircularSector" => "CircumcircleSector",
                    "Reflect" => "Mirror",
                    "Curve" => "CurveCartesian",
                    other => other,
                };
                let inputs = inputs
                    .iter()
                    .copied()
                    .filter(|arg| !arg.is_empty())
                    .collect::<Vec<_>>();
                if name == "Polygon"
                    && inputs.len() == 3
                    && outputs.get(1).is_some_and(|&id| c.kind(id) == Kind::Point)
                {
                    // The kernel allocates the generated points before the
                    // edges that use them. The saved output slots put edges first.
                    let ordered = std::iter::once(id)
                        .chain(
                            outputs[1..]
                                .iter()
                                .copied()
                                .filter(|&id| c.kind(id) == Kind::Segment),
                        )
                        .chain(
                            outputs[1..]
                                .iter()
                                .copied()
                                .filter(|&id| c.kind(id) != Kind::Segment),
                        )
                        .collect::<Vec<_>>();
                    return self.command(name, &inputs, &ordered);
                }
                return self.command(name, &inputs, outputs);
            }
        }
        if outputs.len() != 1 {
            return Err(failure(format!(
                "no command for multi-output object `{label}`"
            )));
        }
        self.expression(id, &syntax(source)?)
    }

    fn expression(&mut self, id: Id, source: &str) -> Result<(), ExportError> {
        self.xml.tag(
            "expression",
            &[
                ("label", self.c.label(id)),
                ("exp", source),
                (
                    "type",
                    match self.c.kind(id) {
                        Kind::FunctionNVar => "function",
                        kind => element::kind(kind),
                    },
                ),
            ],
        )
    }

    fn command(&mut self, name: &str, inputs: &[&str], outputs: &[Id]) -> Result<(), ExportError> {
        self.xml.open("command", &[("name", name)])?;
        self.xml.numbered(
            "input",
            inputs
                .iter()
                .map(|input| syntax(input))
                .collect::<Result<Vec<_>, _>>()?,
        )?;
        self.xml
            .numbered("output", outputs.iter().map(|&id| self.c.label(id)))?;
        self.xml.buffer.push_str("</command>\n");
        Ok(())
    }
}

/// Lower journal-only inline arguments before writing the command parameters.
fn syntax(mut source: &str) -> Result<String, ExportError> {
    if let Some(inner) = source
        .strip_prefix("$arg(")
        .and_then(|s| s.strip_suffix(')'))
    {
        source = inner;
    }
    if source.contains('\0') || source.starts_with('$') {
        return Err(failure(format!("unsupported definition `{source}`")));
    }
    Ok(source.to_owned())
}

// File expressions use literal backslashes. An ASCII quote inside a text
// is expressed as its character code so it cannot close the quoted literal.
fn text_expression(text: &str) -> String {
    text.split('"')
        .map(|part| format!("\"{part}\""))
        .collect::<Vec<_>>()
        .join(" + UnicodeToLetter[34] + ")
}

fn number(value: f64) -> String {
    match value {
        f64::INFINITY => "Infinity".into(),
        f64::NEG_INFINITY => "-Infinity".into(),
        other => other.to_string(),
    }
}

fn flag(value: bool) -> &'static str {
    if value {
        "true"
    } else {
        "false"
    }
}

#[derive(Default)]
struct Xml {
    buffer: String,
    items: usize,
}

impl Xml {
    fn attributes(&mut self, name: &str, attrs: &[(&str, &str)]) -> Result<(), ExportError> {
        self.items += match name {
            "command" | "element" | "cellPair" => 1,
            "expression" if attrs.iter().any(|(key, _)| *key == "label") => 1,
            "output" => attrs.len(),
            _ => 0,
        };
        if self.items > crate::xml::MAX_ITEMS {
            return Err(failure("construction exceeds the import object limit"));
        }
        self.buffer.push('<');
        self.buffer.push_str(name);
        for (key, value) in attrs {
            if value.len() > crate::xml::MAX_ATTRIBUTE_BYTES {
                return Err(failure(
                    "construction XML attribute exceeds the import size limit",
                ));
            }
            self.buffer.push(' ');
            self.buffer.push_str(key);
            self.buffer.push_str("=\"");
            let start = self.buffer.len();
            for ch in value.chars() {
                match ch {
                    '&' => self.buffer.push_str("&amp;"),
                    '<' => self.buffer.push_str("&lt;"),
                    '>' => self.buffer.push_str("&gt;"),
                    '"' => self.buffer.push_str("&quot;"),
                    '\n' => self.buffer.push_str("&#10;"),
                    '\r' => self.buffer.push_str("&#13;"),
                    '\t' => self.buffer.push_str("&#9;"),
                    '\0'..='\u{1f}' | '\u{fffe}' | '\u{ffff}' => {
                        return Err(failure(
                            "a construction attribute contains an invalid XML character",
                        ));
                    }
                    _ => self.buffer.push(ch),
                }
            }
            if self.buffer.len() - start > crate::xml::MAX_ATTRIBUTE_BYTES {
                return Err(failure(
                    "construction XML attribute exceeds the import size limit",
                ));
            }
            self.buffer.push('"');
        }
        Ok(())
    }

    fn open(&mut self, name: &str, attrs: &[(&str, &str)]) -> Result<(), ExportError> {
        self.attributes(name, attrs)?;
        self.buffer.push_str(">\n");
        Ok(())
    }

    fn tag(&mut self, name: &str, attrs: &[(&str, &str)]) -> Result<(), ExportError> {
        self.attributes(name, attrs)?;
        self.buffer.push_str("/>\n");
        Ok(())
    }

    fn cas_row(&mut self, row: &crate::SavedCasRow) -> Result<(), ExportError> {
        self.open("cascell", &[])?;
        self.open("cellPair", &[])?;
        if row.as_text {
            self.tag("useAsText", &[])?;
        }
        if !row.input.is_empty() {
            self.open("inputCell", &[])?;
            let mut attrs = vec![("value", row.input.as_str())];
            let input = &row.input_options;
            attrs.extend(
                [
                    ("prefix", input.prefix.as_deref()),
                    ("eval", input.eval.as_deref()),
                    ("postfix", input.postfix.as_deref()),
                    ("evalCmd", input.command.as_deref()),
                    ("pointList", input.point_list.then_some("true")),
                ]
                .into_iter()
                .filter_map(|(key, value)| Some((key, value?))),
            );
            self.cas_expression(&attrs)?;
            self.buffer.push_str("</inputCell>\n");
        }
        if let Some(value) = &row.output {
            self.open("outputCell", &[])?;
            let output = &row.output_options;
            let mut attrs = vec![("value", value.as_str())];
            attrs.extend(
                [
                    ("evalCommand", output.command.as_deref()),
                    ("evalComment", output.comment.as_deref()),
                    ("error", output.error.then_some("true")),
                    ("native", output.native.then_some("true")),
                ]
                .into_iter()
                .filter_map(|(key, value)| Some((key, value?))),
            );
            self.cas_expression(&attrs)?;
            self.buffer.push_str("</outputCell>\n");
        }
        self.buffer.push_str("</cellPair>\n</cascell>\n");
        Ok(())
    }

    fn cas_expression(&mut self, attrs: &[(&str, &str)]) -> Result<(), ExportError> {
        if attrs
            .iter()
            .any(|(_, value)| value.len() > crate::MAX_CAS_INPUT_BYTES)
        {
            return Err(failure("CAS cell attribute exceeds the import size limit"));
        }
        self.tag("expression", attrs)
    }

    fn val(&mut self, name: &str, value: &str) -> Result<(), ExportError> {
        self.tag(name, &[("val", value)])
    }

    fn numbered(
        &mut self,
        name: &str,
        values: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<(), ExportError> {
        let values: Vec<_> = values
            .into_iter()
            .enumerate()
            .map(|(i, v)| (format!("a{i}"), v))
            .collect();
        if values.len() > crate::xml::MAX_ARGUMENTS {
            return Err(failure("command exceeds the import argument limit"));
        }
        let attrs: Vec<_> = values
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_ref()))
            .collect();
        self.tag(name, &attrs)
    }

    fn coords(&mut self, name: &str, [x, y, z]: [f64; 3]) -> Result<(), ExportError> {
        self.tag(
            name,
            &[("x", &number(x)), ("y", &number(y)), ("z", &number(z))],
        )
    }

    fn pixels(&mut self, name: &str, x: i32, y: i32) -> Result<(), ExportError> {
        self.tag(name, &[("x", &x.to_string()), ("y", &y.to_string())])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinates_use_reference_spellings_for_nonfinite_numbers() {
        let mut xml = Xml::default();
        xml.coords("coords", [f64::INFINITY, f64::NEG_INFINITY, f64::NAN])
            .unwrap();
        assert_eq!(
            xml.buffer,
            "<coords x=\"Infinity\" y=\"-Infinity\" z=\"NaN\"/>\n"
        );
    }
}
