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
    pub javascript: &'a [u8],
    pub colors: BTreeMap<&'a str, [u8; 3]>,
    pub hidden: HashSet<&'a str>,
    pub conditions: BTreeMap<&'a str, &'a str>,
    pub label_offsets: BTreeMap<&'a str, (i32, i32)>,
    pub slider_bounds: BTreeMap<&'a str, [Option<&'a str>; 2]>,
}

#[derive(Debug)]
pub struct ExportError(String);

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ExportError {}

fn failure(message: impl Into<String>) -> ExportError {
    ExportError(message.into())
}

/// Serializes the document before the caller opens the destination file.
pub fn ggb<'a>(
    construction: &Construction,
    definitions: impl IntoIterator<Item = Definition<'a>>,
    presentation: &Presentation<'_>,
) -> Result<Vec<u8>, ExportError> {
    let mut writer = Writer {
        c: construction,
        presentation,
        xml: Xml::default(),
        images: BTreeMap::new(),
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
                writer.element(id)?;
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
    writer.xml.buffer.push_str("</construction>\n</geogebra>\n");
    if writer.xml.buffer.len() as u64 > archive::MAX_XML_BYTES
        || writer.images.len() + 1 + usize::from(!presentation.javascript.is_empty())
            > archive::MAX_ZIP_ENTRIES
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
    if !presentation.javascript.is_empty() {
        if presentation.javascript.len() as u64 > archive::MAX_JAVASCRIPT_BYTES {
            return Err(failure(
                "the JavaScript library exceeds the import size limit",
            ));
        }
        write(archive::JAVASCRIPT_FILE, presentation.javascript)?;
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
            if name == "$point" || name == "$vector" {
                return self.expression(id, &syntax(&format!("({})", inputs.join(", ")))?);
            }
            if name.chars().next().is_some_and(char::is_uppercase)
                && name.chars().all(char::is_alphanumeric)
                && c.id(name).is_none()
            {
                let name = match name {
                    "PerpendicularLine" => "OrthogonalLine",
                    "PerpendicularBisector" => "LineBisector",
                    "ParallelLine" => "Line",
                    "AngleBisector" => "AngularBisector",
                    "CircularArc" => "CircleArc",
                    "CircularSector" => "CircleSector",
                    "CircumcircularArc" => "CircumcircleArc",
                    "CircumcircularSector" => "CircumcircleSector",
                    "Reflect" => "Mirror",
                    "Curve" => "CurveCartesian",
                    "Polyline" => "PolyLine",
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
            "command" | "expression" | "element" => 1,
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
            &[
                ("x", &x.to_string()),
                ("y", &y.to_string()),
                ("z", &z.to_string()),
            ],
        )
    }

    fn pixels(&mut self, name: &str, x: i32, y: i32) -> Result<(), ExportError> {
        self.tag(name, &[("x", &x.to_string()), ("y", &y.to_string())])
    }
}
