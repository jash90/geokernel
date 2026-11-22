use super::{failure, flag, number, ExportError, Writer};
use geo_kernel::algo::path::PathDef;
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
        Kind::ImplicitCurve => "implicitpoly",
        Kind::Button => "button",
        Kind::InputBox => "textfield",
        Kind::Image => "image",
        Kind::PieChart => "piechart",
        _ => "",
    }
}

impl Writer<'_> {
    pub(super) fn element(&mut self, id: Id, line: &str) -> Result<(), ExportError> {
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
        let saved = self.presentation.object_styles.get(label).copied();
        let shown = !self.presentation.hidden.contains(label);
        self.xml.tag(
            "show",
            &[
                ("object", flag(shown)),
                (
                    "label",
                    flag(
                        saved
                            .and_then(|style| style.show_label)
                            .unwrap_or(style.show_label),
                    ),
                ),
            ],
        )?;
        if let Some(condition) = self.presentation.conditions.get(label) {
            self.xml.tag("condition", &[("showObject", condition)])?;
        }
        let [r, g, b] = self
            .presentation
            .colors
            .get(label)
            .copied()
            .or_else(|| saved.and_then(|style| style.color))
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
        self.xml.val(
            "labelMode",
            &saved.map_or(0, |style| style.label_mode).to_string(),
        )?;
        if !matches!(
            object_kind,
            Kind::Point | Kind::Image | Kind::Text | Kind::Boolean | Kind::Button | Kind::InputBox
        ) {
            self.xml.tag(
                "lineStyle",
                &[
                    (
                        "thickness",
                        &saved
                            .and_then(|style| style.thickness)
                            .unwrap_or(style.line_thickness)
                            .to_string(),
                    ),
                    (
                        "type",
                        &saved.map_or(0, |style| style.line_type).to_string(),
                    ),
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
        if object_kind == Kind::ImplicitCurve {
            if let Some(coefficients) = c.implicit_coefficients(id) {
                let data = format!("{coefficients:?}");
                self.xml
                    .tag("coefficients", &[("rep", "array"), ("data", &data)])?;
            }
            let user = c.equation_form(id) == Some(&geo_kernel::kernel::EquationForm::User);
            self.xml.tag("userinput", &[("show", flag(user))])?;
        } else if let Some(form) = c.equation_form(id) {
            let mut attrs = vec![("style", form.xml_style())];
            if let geo_kernel::kernel::EquationForm::Parametric(parameter) = form {
                attrs.push(("parameter", parameter));
            }
            self.xml.tag("eqnStyle", &attrs)?;
        }
        let caption = c.control(id).map_or_else(
            || saved.map_or("", |style| style.caption.as_str()),
            |control| control.caption.as_str(),
        );
        if !caption.is_empty() {
            self.xml.val("caption", caption)?;
        }
        if c.control(id).is_some() && object_kind == Kind::Boolean {
            self.xml.tag("checkbox", &[("fixed", "false")])?;
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
                // Stack sliders in rows, with another column as the view fills.
                let rows = usize::try_from(self.presentation.view.size[1] / 40)
                    .unwrap_or(usize::MAX)
                    .saturating_sub(1)
                    .max(1);
                let x = (30 + 130 * (self.sliders / rows)).to_string();
                let y = (30 + 40 * (self.sliders % rows)).to_string();
                self.sliders += 1;
                let attrs = [
                    ("width", "100"),
                    ("x", &x),
                    ("y", &y),
                    ("fixed", "false"),
                    ("horizontal", "true"),
                    ("showAlgebra", "true"),
                    ("absoluteScreenLocation", "true"),
                    // The reader supplies default bounds for visible sliders
                    // before reading this tag. An undefined endpoint clears it.
                    ("min", bounds[0].unwrap_or("0 / 0")),
                    ("max", bounds[1].unwrap_or("0 / 0")),
                ];
                self.xml.tag("slider", &attrs)?;
                let animation = self.presentation.slider_animation.get(label).copied();
                let command_step = line
                    .split_once(['=', ':'])
                    .and_then(|(_, rhs)| {
                        geo_kernel::script::split_call(
                            geo_kernel::script::split_comment(rhs).0.trim(),
                        )
                    })
                    .and_then(|(name, args)| match (name, args.as_slice()) {
                        ("Slider", [_, _, step]) => Some(*step),
                        _ => None,
                    });
                let step = animation.map_or(command_step.or(Some("0.1")), |a| a.step.as_deref());
                let speed = animation.and_then(|a| a.speed.as_deref()).unwrap_or("1");
                let mode = animation.map_or(0, |a| a.mode).to_string();
                let mut attrs = vec![
                    ("speed", speed),
                    ("type", mode.as_str()),
                    ("playing", flag(animation.is_some_and(|a| a.playing))),
                ];
                if let Some(step) = step {
                    attrs.push(("step", step));
                }
                self.xml.tag("animation", &attrs)?;
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
        if let Def::Path(PathDef::On { path } | PathDef::OnParam { path, .. }) = c.def(id) {
            if let Some(state) = c.path_state(id).filter(|_| c.kind(path) == Kind::Curve) {
                self.xml
                    .tag("curveParam", &[("t", &number(state.parameter()))])?;
            }
        }
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
                            let x = number(x);
                            let y = number(y);
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
