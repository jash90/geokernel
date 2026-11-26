//! Element attributes without live fields. The reader and writer for these
//! tags are specified by ConsElementXMLHandler and the individual geos.

use crate::convert::StyleExpressions;
use crate::{xml, ImportError};
use geo_kernel::kernel::angle::AngleStyle;
use geo_kernel::kernel::image::RasterAsset;
use geo_kernel::{Construction, Def, Id, Kind};
use quick_xml::events::BytesStart;
use std::{fmt, sync::Arc};

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
    extra: Option<Box<TagExtra>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum TagExtra {
    PointInputs(Vec<(String, Kind)>),
    ChartImage(ChartImage),
}

#[derive(Clone, PartialEq, Eq)]
struct ChartImage(Arc<RasterAsset>);

impl fmt::Debug for ChartImage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ChartImage")
            .field("size", &self.0.size())
            .field("bytes", &self.0.bytes().len())
            .finish()
    }
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

    pub(crate) fn len(&self) -> usize {
        self.tags.len()
    }

    pub(crate) fn finish(&mut self, value_position: &mut usize) {
        // Keep the writer's grouping without losing which angle styles
        // preceded the value in the source.
        if self.class == "angle" {
            *value_position = self.tags[..*value_position]
                .iter()
                .filter(|tag| tag.is_angle_style())
                .count();
        }
        self.tags.sort_by_key(|tag| {
            if self.class == "angle" && tag.is_angle_style() {
                0
            } else if tag.name == "eigenvectors" {
                1
            } else {
                2
            }
        });
    }

    pub(crate) fn retain(&mut self, mut keep: impl FnMut(&mut Tag) -> bool) {
        self.tags.retain_mut(|tag| {
            tag.children.retain_mut(&mut keep);
            keep(tag)
        });
    }

    /// Whether this saved presentation has an anchor lost in an edit.
    pub fn has_missing_start_point(&self, c: &Construction) -> bool {
        self.tags.iter().any(|tag| tag.missing_start_point(c))
    }

    /// Detach references to points removed from the edited document.
    pub fn detach_missing_start_points(&mut self, c: &Construction) {
        self.tags.retain(|tag| !tag.missing_start_point(c));
    }

    pub(crate) fn angle_style(&self, initial: AngleStyle) -> AngleStyle {
        self.tags.iter().fold(initial, Tag::apply_angle_style)
    }

    /// The reference normalizes a free angle's raw value whenever a bounded
    /// style is active after reading the value. Switching back to unbounded
    /// leaves that normalization in place, without the reflex reflection.
    pub(crate) fn free_angle_raw(&self, raw: f64, value_position: usize) -> f64 {
        let (before, after) = self.tags.split_at(value_position);
        let style = before
            .iter()
            .fold(AngleStyle::default(), Tag::apply_angle_style);
        if style != AngleStyle::Unbounded
            || after
                .iter()
                .any(|tag| Tag::apply_angle_style(style, tag) != AngleStyle::Unbounded)
        {
            AngleStyle::Anticlockwise.value(raw)
        } else {
            raw
        }
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

    pub(crate) fn is_angle_style(&self) -> bool {
        matches!(
            self.name,
            "angleStyle" | "allowReflexAngle" | "forceReflexAngle"
        )
    }

    fn apply_angle_style(previous: AngleStyle, tag: &Self) -> AngleStyle {
        match tag.name {
            "angleStyle" => tag
                .attr("val")
                .and_then(xml::java_int)
                .map_or(previous, AngleStyle::from_xml),
            "allowReflexAngle" => {
                if tag.attr("val") == Some("true") {
                    AngleStyle::Anticlockwise
                } else {
                    AngleStyle::NotReflex
                }
            }
            "forceReflexAngle" if tag.attr("val") == Some("true") => AngleStyle::Reflex,
            "forceReflexAngle" if previous == AngleStyle::Reflex => AngleStyle::Anticlockwise,
            _ => previous,
        }
    }

    pub(crate) fn validate_import(
        &mut self,
        c: &Construction,
        id: Id,
        expressions: &mut StyleExpressions,
    ) -> bool {
        match self.name {
            "incrementY" => {
                let Some((_, val)) = self.attributes.iter_mut().find(|(key, _)| *key == "val")
                else {
                    return false;
                };
                let Ok(normalized) = expressions.point_increment(c, val) else {
                    return false;
                };
                *val = normalized;
                true
            }
            "startPoint" if self.start_point_label().is_some() && c.kind(id) == Kind::Vector => {
                let source = self.start_point_label().expect("checked above");
                if c.id(source)
                    .is_some_and(|point| c.kind(point) == Kind::Point)
                {
                    return true;
                }
                let Ok((source, inputs)) = expressions.point(c, source) else {
                    return false;
                };
                self.attributes
                    .retain(|(key, _)| !matches!(*key, "exp" | "label"));
                self.attributes.push(("exp", source));
                let inputs = inputs
                    .into_iter()
                    .map(|id| (c.label(id).to_owned(), c.kind(id)))
                    .collect();
                self.extra = Some(Box::new(TagExtra::PointInputs(inputs)));
                true
            }
            "tag" if self.chart_image_name().is_some() => is_chart(c, id) && self.valid_chart_bar(),
            _ => self.can_write(c, id, expressions),
        }
    }

    pub(crate) fn chart_image_name(&self) -> Option<&str> {
        (self.name == "tag" && self.attr("key") == Some("barImage"))
            .then(|| self.attr("value").filter(|value| !value.is_empty()))
            .flatten()
    }

    pub(crate) fn set_chart_image(&mut self, asset: Arc<RasterAsset>) {
        self.attributes.retain(|(key, _)| *key != "value");
        self.extra = Some(Box::new(TagExtra::ChartImage(ChartImage(asset))));
    }

    pub(crate) fn chart_image(&self) -> Option<&RasterAsset> {
        match self.extra.as_deref() {
            Some(TagExtra::ChartImage(image)) => Some(&image.0),
            _ => None,
        }
    }

    fn start_point_label(&self) -> Option<&str> {
        self.attr("exp").or_else(|| self.attr("label"))
    }

    fn missing_start_point(&self, c: &Construction) -> bool {
        if self.name != "startPoint" {
            return false;
        }
        if let Some(TagExtra::PointInputs(inputs)) = self.extra.as_deref() {
            return inputs
                .iter()
                .any(|(label, kind)| c.id(label).is_none_or(|input| c.kind(input) != *kind));
        }
        self.start_point_label()
            .is_some_and(|label| c.id(label).is_none_or(|id| c.kind(id) != Kind::Point))
    }

    pub(crate) fn can_write(
        &self,
        c: &Construction,
        id: Id,
        expressions: &mut StyleExpressions,
    ) -> bool {
        match self.name {
            "incrementY" => self
                .attr("val")
                .is_some_and(|val| expressions.point_increment(c, val).is_ok()),
            "startPoint" => {
                if self.start_point_label().is_some() {
                    !self.missing_start_point(c)
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
            "tags" => is_chart(c, id),
            "tag" => is_chart(c, id) && self.valid_chart_tag(),
            "angleStyle" => self.attr("val").and_then(xml::java_int).is_some(),
            _ => true,
        }
    }
    fn valid_chart_tag(&self) -> bool {
        if !self.valid_chart_bar() {
            return false;
        }
        if self.attr("key") == Some("barImage") {
            return self.chart_image().is_some();
        }
        let Some(value) = self.attr("value").filter(|value| !value.is_empty()) else {
            return false;
        };
        match self.attr("key") {
            Some("barAlpha") => xml::finite_java_float(value),
            Some("barHatchDistance" | "barHatchAngle") => xml::java_int(value).is_some(),
            Some("barFillType") => xml::java_int(value)
                .is_some_and(|n| (0..=i32::from(crate::export::Fill::IMAGE_PATTERN)).contains(&n)),
            Some("barSymbol") => true,
            Some("barColor") => value
                .strip_prefix("rgba(")
                .and_then(|rgba| rgba.strip_suffix(')'))
                .is_some_and(|rgba| {
                    let mut fields = rgba.split(',');
                    (0..3).all(|_| {
                        fields
                            .next()
                            .and_then(xml::java_int)
                            .is_some_and(|n| u8::try_from(n).is_ok())
                    }) && fields
                        .next()
                        .is_some_and(|n| xml::java_double(n).is_some_and(f64::is_finite))
                        && fields.next().is_none()
                }),
            _ => false,
        }
    }

    fn valid_chart_bar(&self) -> bool {
        self.attr("barNumber").and_then(xml::java_int).is_some()
    }
}

fn is_chart(c: &Construction, id: Id) -> bool {
    matches!(c.def(id), Def::Chart(chart) if chart.has_bar_style())
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
    let takes_angle_style = matches!(class, "angle" | "list");
    let (name, keys): (_, &[_]) = match name {
        "angleStyle" if takes_angle_style => ("angleStyle", &["val"]),
        "allowReflexAngle" if takes_angle_style => ("allowReflexAngle", &["val"]),
        "forceReflexAngle" if takes_angle_style => ("forceReflexAngle", &["val"]),
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
        "fixedSize" if matches!(class, "button" | "textfield") => ("fixedSize", &["val"]),
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
        "tags" if matches!(class, "piechart" | "numeric") => ("tags", &[]),
        "tag" if matches!(class, "piechart" | "numeric") => ("tag", &["key", "value", "barNumber"]),
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
        extra: None,
    }))
}
