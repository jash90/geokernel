//! Element attributes without live fields. The reader and writer for these
//! tags are specified by ConsElementXMLHandler and the individual geos.

use crate::convert::StyleExpressions;
use crate::{xml, ImportError};
use geo_kernel::{Construction, Kind};
use quick_xml::events::BytesStart;

/// Additional presentation attached to a saved object. Storing its original
/// class prevents old properties from being applied after a change of kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Properties {
    class: String,
    tags: Vec<Tag>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Tag {
    pub(crate) name: &'static str,
    pub(crate) attributes: Vec<(&'static str, String)>,
    pub(crate) children: Vec<Tag>,
}

impl Properties {
    pub(crate) fn new(class: &str) -> Self {
        Self {
            class: class.to_owned(),
            tags: Vec::new(),
        }
    }

    pub(crate) fn push(&mut self, tag: Tag) {
        self.tags.push(tag);
    }

    pub(crate) fn push_chart_tag(&mut self, tag: Tag) {
        self.tags
            .last_mut()
            .expect("open chart tags")
            .children
            .push(tag);
    }

    pub(crate) fn finish(&mut self) {
        // The reference classifies a conic when it reads the matrix.
        self.tags.sort_by_key(|tag| tag.name != "eigenvectors");
    }

    pub(crate) fn retain(&mut self, mut keep: impl FnMut(&Tag) -> bool) {
        self.tags.retain_mut(|tag| {
            tag.children.retain(&mut keep);
            keep(tag)
        });
    }

    pub(crate) fn for_class(&self, class: &str) -> &[Tag] {
        if self.class == class {
            &self.tags
        } else {
            &[]
        }
    }
}

impl Tag {
    fn attr(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find_map(|(key, value)| (*key == name).then_some(value.as_str()))
    }

    pub(crate) fn can_write(&self, c: &Construction, expressions: &mut StyleExpressions) -> bool {
        match self.name {
            "incrementY" => self
                .attr("val")
                .is_some_and(|val| expressions.number(c, val).is_ok()),
            "startPoint" => {
                if let Some(label) = self.attr("exp").or_else(|| self.attr("label")) {
                    // The reference would evaluate any expression here. Only
                    // an existing named point is safe for these retained tags.
                    c.id(label).is_some_and(|id| c.kind(id) == Kind::Point)
                } else {
                    ["x", "y", "z"].into_iter().all(|key| {
                        self.attr(key)
                            .is_some_and(|value| value.parse::<f64>().is_ok())
                    })
                }
            }
            "spreadsheetTrace" => [
                ("traceColumn1", geo_kernel::spreadsheet::MAX_COLUMNS),
                ("traceColumn2", geo_kernel::spreadsheet::MAX_COLUMNS),
                ("traceRow1", geo_kernel::spreadsheet::MAX_ROWS),
                ("traceRow2", geo_kernel::spreadsheet::MAX_ROWS),
                ("tracingRow", geo_kernel::spreadsheet::MAX_ROWS),
                ("numRows", geo_kernel::spreadsheet::MAX_ROWS),
                ("headerOffset", geo_kernel::spreadsheet::MAX_ROWS),
            ]
            .into_iter()
            .all(|(key, limit)| {
                self.attr(key)
                    .and_then(xml::java_int)
                    .is_some_and(|n| (-1..=i32::from(limit)).contains(&n))
            }),
            "tag" => matches!(
                self.attr("key"),
                Some(
                    "barAlpha"
                        | "barHatchDistance"
                        | "barFillType"
                        | "barHatchAngle"
                        | "barSymbol"
                        | "barColor"
                )
            ),
            _ => true,
        }
    }
}

/// Attributes are decoded and escaped by the same code as modeled fields.
/// Do not pass unchecked XML fragments to the writer. In particular, some
/// element tags load external content or hand expressions to the reference
/// application and need their own import and export rules.
pub(crate) fn read(
    class: &str,
    name: &str,
    event: &BytesStart,
) -> Result<Option<Tag>, ImportError> {
    let (name, keys): (_, &[_]) = match name {
        "allowReflexAngle" => ("allowReflexAngle", &["val"]),
        "forceReflexAngle" => ("forceReflexAngle", &["val"]),
        "coordStyle" => ("coordStyle", &["style"]),
        "eigenvectors" => (
            "eigenvectors",
            &["x0", "y0", "z0", "x1", "y1", "z1", "x2", "y2", "z2"],
        ),
        "startPoint" if matches!(class, "vector" | "button" | "list" | "textfield") => (
            "startPoint",
            &["x", "y", "z", "exp", "label", "number", "absolute"],
        ),
        "spreadsheetTrace" => (
            "spreadsheetTrace",
            &[
                "val",
                "traceColumn1",
                "traceColumn2",
                "traceRow1",
                "traceRow2",
                "tracingRow",
                "numRows",
                "headerOffset",
                "doColumnReset",
                "doRowLimit",
                "showLabel",
                "showTraceList",
                "doTraceGeoCopy",
                "pause",
            ],
        ),
        "font" if matches!(class, "button" | "textfield" | "list") => {
            ("font", &["serif", "sizeM", "size", "style"])
        }
        "textAlign" if class == "textfield" => ("textAlign", &["val"]),
        "tempUserInput" if class == "textfield" => ("tempUserInput", &["eval", "display"]),
        "contentSerif" if class == "textfield" => ("contentSerif", &["val"]),
        "dimensions" => ("dimensions", &["width", "height", "angle", "unscaled"]),
        "comboBox" => ("comboBox", &["val"]),
        "listType" => ("listType", &["val"]),
        "selectedIndex" => ("selectedIndex", &["val"]),
        "incrementY" => ("incrementY", &["val"]),
        "absoluteScreenLocation" if !matches!(class, "text" | "image") => {
            ("absoluteScreenLocation", &["x", "y"])
        }
        "isMask" => ("isMask", &["val"]),
        "fading" => ("fading", &["val"]),
        "parentLabel" => ("parentLabel", &["val"]),
        "tableview" => ("tableview", &["column", "points"]),
        "keepTypeOnTransform" => ("keepTypeOnTransform", &["val"]),
        "outlyingIntersections" => ("outlyingIntersections", &["val"]),
        "showOnAxis" => ("showOnAxis", &["val"]),
        "symbolic" => ("symbolic", &["val"]),
        "simplifyCoefficients" => ("simplifyCoefficients", &["val"]),
        "slopeTriangleSize" => ("slopeTriangleSize", &["val"]),
        "levelOfDetailQuality" => ("levelOfDetailQuality", &["val"]),
        "autocolor" => ("autocolor", &["val"]),
        "tags" if class == "piechart" => ("tags", &[]),
        "tag" if class == "piechart" => ("tag", &["key", "value", "barNumber"]),
        _ => return Ok(None),
    };
    let mut attributes = Vec::new();
    for attr in event.attributes().with_checks(false) {
        let attr = attr.map_err(xml::bad)?;
        if let Some(&key) = keys.iter().find(|key| **key == attr.key.as_ref()) {
            if !attributes.iter().any(|(seen, _)| *seen == key) {
                attributes.push((key, xml::value_of(&attr)?));
            }
        }
    }
    Ok(Some(Tag {
        name,
        attributes,
        children: Vec::new(),
    }))
}
