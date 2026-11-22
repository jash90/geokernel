use super::{failure, flag, ExportError, Writer};
use geo_kernel::kernel::image::{ImageCorner, ImagePlacement};
use geo_kernel::kernel::text::TextPosition;
use geo_kernel::{Def, Id, Kind, Value};

pub(super) fn kind(kind: Kind) -> &'static str {
    match kind {
        Kind::Point => "point",
        Kind::Line => "line",
        Kind::Segment => "segment",
        Kind::Circle | Kind::Conic => "conic",
        Kind::Number => "numeric",
        Kind::Angle => "angle",
        Kind::Vector => "vector",
        Kind::Ray => "ray",
        Kind::Boolean => "boolean",
        Kind::Function => "function",
        Kind::FunctionNVar => "functionnvar",
        Kind::Text => "text",
        Kind::Polygon => "polygon",
        Kind::PolyLine => "polyline",
        Kind::List => "list",
        Kind::Arc | Kind::Sector => "conicpart",
        Kind::Locus => "locus",
        Kind::Curve => "curvecartesian",
        Kind::Button => "button",
        Kind::InputBox => "textfield",
        Kind::Image => "image",
        Kind::PieChart => "piechart",
        _ => "",
    }
}

impl Writer<'_> {
    pub(super) fn element(&mut self, id: Id) -> Result<(), ExportError> {
        let c = self.c;
        let label = c.label(id);
        let object_kind = c.kind(id);
        let class = kind(object_kind);
        if class.is_empty() {
            return Err(failure(format!(
                "unsupported object `{label}` ({object_kind})"
            )));
        }
        self.xml
            .open("element", &[("type", class), ("label", label)])?;

        if c.image(id).is_some() {
            self.image(id)?;
        }
        let style = c.drawing_style(id);
        let shown = !self.presentation.hidden.contains(label);
        self.xml.tag(
            "show",
            &[("object", flag(shown)), ("label", flag(style.show_label))],
        )?;
        if let Some(condition) = self.presentation.conditions.get(label) {
            self.xml.tag("condition", &[("showObject", condition)])?;
        }
        let [r, g, b] = self
            .presentation
            .colors
            .get(label)
            .copied()
            .unwrap_or([0; 3]);
        let alpha = c.image(id).map_or(0.0, |image| image.style.alpha);
        self.xml.tag(
            "objColor",
            &[
                ("r", &r.to_string()),
                ("g", &g.to_string()),
                ("b", &b.to_string()),
                ("alpha", &alpha.to_string()),
            ],
        )?;
        self.xml.val("layer", &style.layer.to_string())?;
        self.xml.val("labelMode", "0")?;
        if !matches!(
            object_kind,
            Kind::Point | Kind::Image | Kind::Text | Kind::Boolean | Kind::Button | Kind::InputBox
        ) {
            self.xml.tag(
                "lineStyle",
                &[
                    ("thickness", &style.line_thickness.to_string()),
                    ("type", "0"),
                    ("typeHidden", "1"),
                ],
            )?;
        }
        if let Some(&(x, y)) = self
            .presentation
            .label_offsets
            .get(label)
            .filter(|_| object_kind != Kind::Text)
        {
            self.xml.pixels("labelOffset", x, y)?;
        }
        if let Some(angle) = c.angle_style(id) {
            self.xml.val("angleStyle", &angle.xml_code().to_string())?;
        }
        if let Some(form) = c.equation_form(id) {
            let mut attrs = vec![("style", form.xml_style())];
            if let geo_kernel::kernel::EquationForm::Parametric(parameter) = form {
                attrs.push(("parameter", parameter));
            }
            self.xml.tag("eqnStyle", &attrs)?;
        }
        if let Some(control) = c.control(id) {
            if !control.caption.is_empty() {
                self.xml.val("caption", &control.caption)?;
            }
            if object_kind == Kind::Boolean {
                self.xml.tag("checkbox", &[("fixed", "false")])?;
            }
        }
        for script in c.object_scripts(label) {
            let tag = match script.language.as_str() {
                "ggbscript" => "ggbscript",
                "javascript" => "javascript",
                _ => return Err(failure(format!("unsupported script language on `{label}`"))),
            };
            let attr = match script.event.as_str() {
                "click" => "val",
                "update" => "onUpdate",
                "dragEnd" => "onDragEnd",
                "change" => "onChange",
                _ => return Err(failure(format!("unsupported script event on `{label}`"))),
            };
            self.xml.tag(tag, &[(attr, &script.text)])?;
        }

        self.value(id)?;
        if let Def::Number(_) = c.def(id) {
            let numeric = c.slider_bounds(id).map(|n| n.map(|n| n.to_string()));
            let bounds = self
                .presentation
                .slider_bounds
                .get(label)
                .copied()
                .unwrap_or_else(|| numeric.each_ref().map(|n| n.as_deref()));
            if bounds.iter().any(Option::is_some) {
                let mut attrs = vec![
                    ("width", "100"),
                    ("x", "30"),
                    ("y", "30"),
                    ("fixed", "false"),
                    ("horizontal", "true"),
                    ("showAlgebra", "true"),
                    ("absoluteScreenLocation", "true"),
                ];
                attrs.extend(
                    ["min", "max"]
                        .into_iter()
                        .zip(bounds)
                        .filter_map(|(key, v)| v.map(|v| (key, v))),
                );
                self.xml.tag("slider", &attrs)?;
            }
        }
        if c.text_presentation(id).is_some() {
            self.text_style(id)?;
        }
        self.xml.buffer.push_str("</element>\n");
        Ok(())
    }

    fn value(&mut self, id: Id) -> Result<(), ExportError> {
        let c = self.c;
        match c.stored_value(id) {
            Value::Point { x, y } => self.xml.coords("coords", [x, y, 1.0])?,
            Value::HomogeneousPoint { x, y, z } => self.xml.coords("coords", [x, y, z])?,
            Value::Vector { x, y } => self.xml.coords("coords", [x, y, 0.0])?,
            Value::Line { a, b, c } | Value::Ray { a, b, c, .. } => {
                self.xml.coords("coords", [a, b, c])?;
            }
            Value::Segment { x1, y1, x2, y2 } => {
                self.xml
                    .coords("coords", [y1 - y2, x2 - x1, x1 * y2 - y1 * x2])?;
            }
            Value::Number(n) => self.xml.val("value", &number(n))?,
            Value::Bool(b) => self.xml.val("value", flag(b))?,
            Value::Undefined if c.kind(id).is_number() => {
                self.xml
                    .val("value", &number(c.infinite_number(id).unwrap_or(f64::NAN)))?;
            }
            _ => {}
        }

        if let Some(conic) = c.arc(id).map(|arc| arc.conic).or_else(|| c.conic(id)) {
            let values = conic.matrix.map(number);
            self.xml.tag(
                "matrix",
                &[
                    ("A0", &values[0]),
                    ("A1", &values[1]),
                    ("A2", &values[2]),
                    ("A3", &values[3]),
                    ("A4", &values[4]),
                    ("A5", &values[5]),
                ],
            )?;
        }
        Ok(())
    }

    fn text_style(&mut self, id: Id) -> Result<(), ExportError> {
        let c = self.c;
        let text = c.text_presentation(id).expect("text checked by caller");
        self.xml.val("isLaTeX", flag(text.latex))?;
        if text.fixed {
            self.xml.val("fixed", "true")?;
        }
        let style = u8::from(text.bold) | (u8::from(text.italic) << 1);
        self.xml.tag(
            "font",
            &[
                ("serif", flag(text.serif)),
                ("sizeM", &text.size.to_string()),
                ("style", &style.to_string()),
            ],
        )?;
        if text.offset != [0, 0] {
            self.xml
                .pixels("labelOffset", text.offset[0], text.offset[1])?;
        }
        match text.position {
            TextPosition::Origin => Ok(()),
            TextPosition::World { x, y } => self.xml.coords("startPoint", [x, y, 1.0]),
            TextPosition::Point(point) | TextPosition::ScreenPoint(point) => self.xml.tag(
                "startPoint",
                &[
                    ("exp", c.label(point)),
                    (
                        "absolute",
                        flag(matches!(text.position, TextPosition::ScreenPoint(_))),
                    ),
                ],
            ),
            TextPosition::Screen { x, y } => {
                self.xml
                    .pixels("absoluteScreenLocation", x as i32, y as i32)
            }
            TextPosition::Undefined => self.xml.coords("startPoint", [f64::NAN; 3]),
        }
    }

    fn image(&mut self, id: Id) -> Result<(), ExportError> {
        let c = self.c;
        let image = c.image(id).expect("image checked by caller");
        let bytes = image.asset.bytes();
        let extension = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            "png"
        } else if bytes.starts_with(b"\xff\xd8\xff") {
            "jpg"
        } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
            "gif"
        } else {
            return Err(failure(format!(
                "unsupported raster data for `{}`",
                c.label(id)
            )));
        };
        let next = self.images.len() + 1;
        let (name, _) = self.images.entry(image.asset.id()).or_insert_with(|| {
            (
                format!("images/image{next}.{extension}"),
                image.asset.as_ref(),
            )
        });
        self.xml.tag("file", &[("name", name)])?;
        self.xml.val("inBackground", flag(image.style.background))?;
        self.xml.val("interpolate", flag(image.style.interpolate))?;
        match image.placement {
            ImagePlacement::Screen([x, y]) => {
                self.xml
                    .pixels("absoluteScreenLocation", x as i32, y as i32)?
            }
            ImagePlacement::World {
                corners,
                centered,
                screen_coords,
            } => {
                if centered {
                    self.xml.val("centered", "true")?;
                }
                for (slot, corner) in corners.into_iter().enumerate() {
                    let slot = if centered && slot == 0 { 3 } else { slot };
                    let mut attrs = Vec::new();
                    if screen_coords {
                        attrs.push(("absolute", "true"));
                    }
                    let index = slot.to_string();
                    attrs.push(("number", &index));
                    match corner {
                        Some(ImageCorner::Point(point)) => {
                            attrs.push(("exp", c.label(point)));
                            self.xml.tag("startPoint", &attrs)?;
                        }
                        Some(ImageCorner::Coordinates([x, y])) => {
                            let x = x.to_string();
                            let y = y.to_string();
                            attrs.extend_from_slice(&[("x", &x), ("y", &y), ("z", "1")]);
                            self.xml.tag("startPoint", &attrs)?;
                        }
                        None => {}
                    }
                }
            }
        }
        if let Some([x, y, w, h]) = image.style.crop {
            self.xml.tag(
                "cropBox",
                &[
                    ("cropped", "true"),
                    ("x", &x.to_string()),
                    ("y", &y.to_string()),
                    ("width", &w.to_string()),
                    ("height", &h.to_string()),
                ],
            )?;
        }
        Ok(())
    }
}

fn number(value: f64) -> String {
    match value {
        f64::INFINITY => "Infinity".into(),
        f64::NEG_INFINITY => "-Infinity".into(),
        other => other.to_string(),
    }
}
