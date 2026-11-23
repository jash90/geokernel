use super::{
    failure, flag, number, ExportError, ObjectStyle, Rounding, SliderAnimation, SliderLayout,
    Writer,
};
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
        Kind::PenStroke => "penstroke",
        _ => "",
    }
}

/// Kinds whose reference class keeps a trace flag.
fn traceable(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::Point
            | Kind::Line
            | Kind::Segment
            | Kind::Ray
            | Kind::Vector
            | Kind::Circle
            | Kind::Conic
            | Kind::Arc
            | Kind::Sector
            | Kind::Function
            | Kind::Polygon
            | Kind::PolyLine
            | Kind::List
            | Kind::Locus
            | Kind::PenStroke
            | Kind::Curve
            | Kind::ImplicitCurve
    )
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
        // Every number exactly: the reader parses each field as a double.
        if let Some(stroke) = c.stroke(id).filter(|s| s.defined && !s.points.is_empty()) {
            let coords = geo_kernel::algo::stroke::saved_coords(stroke);
            self.xml.tag("strokeBezierCoords", &[("val", &coords)])?;
        }
        let style = c.drawing_style(id);
        // An object from a file writes the tags its file had. A tag the file
        // left out stays out, so the reader keeps the same default.
        let saved = self.presentation.object_styles.get(label).copied();
        let shown = !self.presentation.hidden.contains(label);
        let label_shown = saved.and_then(|style| style.show_label);
        // A free number is a slider when it has a bound, or when its file
        // placed one for it.
        let numeric = c.slider_bounds(id).map(|n| n.map(|n| n.to_string()));
        let bounds = match c.def(id) {
            Def::Number(_) => self
                .presentation
                .slider_bounds
                .get(label)
                .copied()
                .unwrap_or_else(|| numeric.each_ref().map(|n| n.as_deref())),
            _ => [None; 2],
        };
        let bounded = bounds.iter().any(Option::is_some);
        // The reference shows a number by giving it an interval around its
        // current value, so the value, and the slider with its bounds, come
        // before `<show>`, as in the files of the reference application.
        let is_number = matches!(object_kind, Kind::Number | Kind::Angle);
        // Only a number or an angle takes the tag in the reference reader.
        let layout = saved
            .and_then(|style| style.slider.as_deref())
            .filter(|_| is_number);
        if is_number {
            self.angle(id, saved)?;
            self.value(id)?;
            if bounded || layout.is_some() {
                self.slider(id, bounds, layout)?;
            }
        }
        let reader_decides = match saved {
            // The file had no tag and the object is still visible.
            Some(saved) => saved.untagged && shown,
            // The canvas draws nothing for a free number without bounds. Shown,
            // the reference would make it a slider and clamp its value.
            None => shown && !bounded && matches!(c.def(id), Def::Number(_)),
        };
        if !reader_decides {
            let views = saved
                .and_then(|style| style.views)
                .filter(|&views| views != 0)
                .map(|views| views.to_string());
            let mut attrs = vec![
                ("object", flag(shown)),
                ("label", flag(label_shown.unwrap_or(style.show_label))),
            ];
            if let Some(views) = &views {
                attrs.push(("ev", views));
            }
            self.xml.tag("show", &attrs)?;
        }
        if let Some(condition) = self.presentation.conditions.get(label) {
            self.xml.tag("condition", &[("showObject", condition)])?;
        }
        self.color(id, saved)?;
        if let Some([r, g, b, a]) = saved.and_then(|style| style.background) {
            self.xml.tag(
                "bgColor",
                &[
                    ("r", &r.to_string()),
                    ("g", &g.to_string()),
                    ("b", &b.to_string()),
                    ("alpha", &a.to_string()),
                ],
            )?;
        }
        if saved.is_none_or(|saved| saved.layer.is_some()) {
            self.xml.val("layer", &style.layer.to_string())?;
        }
        if let Some(mode) = saved.map_or(Some(0), |style| style.label_mode) {
            self.xml.val("labelMode", &mode.to_string())?;
        }
        if let Some(mode) = saved.and_then(|style| style.tooltip_mode) {
            self.xml.val("tooltipMode", &mode.to_string())?;
        }
        if let Some(trace) = saved
            .and_then(|style| style.trace)
            .filter(|_| traceable(object_kind))
        {
            self.xml.val("trace", flag(trace))?;
        }
        if let Some(decoration) = saved.and_then(|style| style.decoration) {
            self.xml
                .tag("decoration", &[("type", &decoration.to_string())])?;
        }
        if let Some(order) = saved.and_then(|style| style.ordering) {
            self.xml.val("ordering", &number(order))?;
        }
        // The caption text must still be a text of the construction.
        if let Some(caption) = saved
            .and_then(|style| style.dynamic_caption.as_deref())
            .filter(|caption| c.id(caption).is_some_and(|id| c.kind(id) == Kind::Text))
        {
            self.xml.val("dynamicCaption", caption)?;
        }
        if matches!(object_kind, Kind::Segment | Kind::PolyLine) {
            let ends = saved.map_or([None; 2], |style| style.ends);
            for (tag, end) in ["startStyle", "endStyle"].into_iter().zip(ends) {
                if let Some(end) = end {
                    self.xml.val(tag, end.xml_name())?;
                }
            }
        }
        if object_kind == Kind::Vector {
            if let Some(head) = saved.and_then(|style| style.head_style) {
                self.xml.val("headStyle", &head.to_string())?;
            }
        }
        if is_number {
            if let Some(size) = saved.and_then(|style| style.knob_size) {
                self.xml.val("pointSize", &number(size))?;
            }
        }
        match saved {
            Some(saved) => {
                if let Some(line) = saved.line {
                    // A tag the reader left early is written as far as it got.
                    let mut attrs = vec![("type", line.kind.to_string())];
                    if let Some(thickness) = line.thickness {
                        attrs.push(("thickness", thickness.to_string()));
                    }
                    if let Some(hidden) = line.hidden_kind {
                        attrs.push(("typeHidden", hidden.to_string()));
                    }
                    if let Some(opacity) = line.opacity {
                        attrs.push(("opacity", opacity.to_string()));
                    }
                    if let Some(arrows) = line
                        .draw_arrow
                        .filter(|_| matches!(object_kind, Kind::Locus | Kind::PenStroke))
                    {
                        attrs.push(("drawArrow", flag(arrows).to_owned()));
                    }
                    let attrs: Vec<_> = attrs.iter().map(|(k, v)| (*k, v.as_str())).collect();
                    self.xml.tag("lineStyle", &attrs)?;
                }
            }
            None if !matches!(
                object_kind,
                Kind::Point
                    | Kind::Image
                    | Kind::Text
                    | Kind::Boolean
                    | Kind::Button
                    | Kind::InputBox
            ) =>
            {
                self.xml.tag(
                    "lineStyle",
                    &[
                        ("thickness", &style.line_thickness.to_string()),
                        ("type", "0"),
                        ("typeHidden", "1"),
                    ],
                )?;
            }
            None => {}
        }
        let offset = self
            .presentation
            .label_offsets
            .get(label)
            .filter(|_| object_kind != Kind::Text)
            .copied()
            .or_else(|| {
                if object_kind == Kind::Boolean {
                    c.checkbox_style(id)
                        .position
                        .map(|[x, y]| (x, y))
                        .or_else(|| {
                            if saved.is_none() && shown {
                                c.checkbox_position(id).map(|(x, y)| (x as i32, y as i32))
                            } else {
                                None
                            }
                        })
                } else {
                    None
                }
            });
        if let Some((x, y)) = offset {
            self.xml.pixels("labelOffset", x, y)?;
        }
        if !is_number {
            self.angle(id, saved)?;
        }
        if object_kind == Kind::Angle {
            if let Some(size) = saved.and_then(|style| style.arc_size) {
                self.xml.val("arcSize", &size.to_string())?;
            }
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
        if let Some(fixed) = saved.and_then(|style| style.fixed) {
            self.xml.val("fixed", flag(fixed))?;
        }
        if let Some(fixed) = saved.and_then(|style| style.fixed_size) {
            self.xml.val("fixedSize", flag(fixed))?;
        }
        if let Some(allowed) = saved.and_then(|style| style.selection_allowed) {
            self.xml.val("selectionAllowed", flag(allowed))?;
        }
        if let Some(trimmed) = saved.and_then(|style| style.show_trimmed) {
            self.xml.val("showTrimmed", flag(trimmed))?;
        }
        if let Some(breakpoint) = saved.and_then(|style| style.breakpoint) {
            self.xml.val("breakpoint", flag(breakpoint))?;
        }
        if let Some(visible) = saved.and_then(|style| style.algebra_label) {
            self.xml
                .tag("algebra", &[("labelVisible", flag(visible))])?;
        }
        if let Some(auxiliary) = saved.and_then(|style| style.auxiliary) {
            self.xml.val("auxiliary", flag(auxiliary))?;
        }
        let caption = c.control(id).map_or_else(
            || saved.map_or("", |style| style.caption.as_str()),
            |control| control.caption.as_str(),
        );
        if !caption.is_empty() {
            self.xml.val("caption", caption)?;
        }
        if object_kind == Kind::Boolean {
            // Keep the fixed setting of a canvas checkbox.
            let fixed = match saved {
                Some(saved) => saved.checkbox_fixed,
                None => c.checkbox_position(id).map(|_| c.checkbox_style(id).fixed),
            };
            if let Some(fixed) = fixed {
                self.xml.tag("checkbox", &[("fixed", flag(fixed))])?;
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
        if matches!(object_kind, Kind::Point | Kind::List) {
            if let Some(size) = saved.and_then(|style| style.point_size) {
                self.xml.val("pointSize", &size.to_string())?;
            }
            if let Some(point_style) = saved.and_then(|style| style.point_style) {
                self.xml.val("pointStyle", &point_style.to_string())?;
            }
        }
        if object_kind == Kind::Text {
            match saved.and_then(|style| style.rounding) {
                Some(Rounding::Decimals(digits)) => {
                    self.xml.val("decimals", &digits.to_string())?;
                }
                Some(Rounding::SignificantFigures(digits)) => {
                    self.xml.val("significantfigures", &digits.to_string())?;
                }
                None => {}
            }
        }

        if !is_number {
            self.value(id)?;
        }
        let animation = self
            .presentation
            .slider_animation
            .get(label)
            .copied()
            .or(saved.and_then(|style| style.animation.as_deref()));
        if bounded {
            let command_step = line
                .split_once(['=', ':'])
                .and_then(|(_, rhs)| {
                    geo_kernel::script::split_call(geo_kernel::script::split_comment(rhs).0.trim())
                })
                .and_then(|(name, args)| match (name, args.as_slice()) {
                    ("Slider", [_, _, step]) => Some(*step),
                    _ => None,
                });
            let step = c.slider_style(id).step.map(number);
            match animation {
                // the construction knows whether the slider still plays
                Some(animation) if c.animation(id).is_some() => {
                    let playing = c.animation(id).is_some_and(|a| a.playing);
                    self.animation(&SliderAnimation {
                        playing,
                        ..animation.clone()
                    })?
                }
                Some(animation) => self.animation(animation)?,
                // The file of the slider had no such tag.
                None if saved.is_some() => {}
                // An animation the construction runs (`SlowPlot`).
                None if c.animation(id).is_some() => {
                    // As the reference writes it: an automatic increment has
                    // no step, the default speed is left out.
                    let a = c.animation(id).expect("checked above");
                    let (step, speed, mode) =
                        (number(a.step), number(a.speed), a.mode.code().to_string());
                    let mut attrs = Vec::new();
                    if !a.auto_step {
                        attrs.push(("step", step.as_str()));
                    }
                    if a.speed != 1.0 {
                        attrs.push(("speed", speed.as_str()));
                    }
                    attrs.extend([("type", mode.as_str()), ("playing", flag(a.playing))]);
                    self.xml.tag("animation", &attrs)?;
                }
                None => self.xml.tag(
                    "animation",
                    &[
                        ("speed", "1"),
                        ("type", "0"),
                        ("playing", "false"),
                        ("step", command_step.or(step.as_deref()).unwrap_or("0.1")),
                    ],
                )?,
            }
        } else if let Some(animation) = animation {
            self.animation(animation)?;
        }
        if c.text_presentation(id).is_some() {
            self.text_style(id)?;
        }
        self.xml.buffer.push_str("</element>\n");
        Ok(())
    }

    /// The interval of an angle and its right-angle marker.
    fn angle(&mut self, id: Id, saved: Option<&ObjectStyle>) -> Result<(), ExportError> {
        if let Some(angle) = self.c.angle_style(id) {
            self.xml.val("angleStyle", &angle.xml_code().to_string())?;
        }
        if self.c.kind(id) == Kind::Angle {
            if let Some(emphasize) = saved.and_then(|style| style.emphasize_right_angle) {
                self.xml.val("emphasizeRightAngle", flag(emphasize))?;
            }
        }
        Ok(())
    }

    /// `<objColor>` with the opacity, the dynamic channels and the fill pattern.
    fn color(&mut self, id: Id, saved: Option<&ObjectStyle>) -> Result<(), ExportError> {
        let c = self.c;
        let label = c.label(id);
        let rgb = match saved {
            Some(saved) => saved.color,
            None => Some(
                self.presentation
                    .colors
                    .get(label)
                    .copied()
                    .unwrap_or([0; 3]),
            ),
        };
        // Without a colour the reference reader skips the whole tag.
        let Some([r, g, b]) = rgb else { return Ok(()) };
        let mut attrs = vec![
            ("r", r.to_string()),
            ("g", g.to_string()),
            ("b", b.to_string()),
        ];
        // The shortest decimal of the stored value: the opacity of an image
        // is a 32-bit float, which widened would read 0.30000001192092896
        // for a saved 0.3.
        let alpha = match (c.image(id), saved) {
            (Some(image), _) => Some(image.style.alpha.to_string()),
            (None, Some(saved)) => saved.alpha.map(|alpha| alpha.to_string()),
            (None, None) => Some(
                self.presentation
                    .opacity
                    .get(label)
                    .copied()
                    .unwrap_or(0.0)
                    .to_string(),
            ),
        };
        if let Some(alpha) = alpha {
            attrs.push(("alpha", alpha));
        }
        // Channels reach the reader's command processor, so only arithmetic
        // the kernel itself evaluates is written, in the kernel's syntax.
        // Inputs that are undefined right now do not drop them; an object
        // they name that is gone does.
        let dynamic = saved
            .and_then(|style| style.dynamic_color.as_deref())
            .and_then(|dynamic| self.expressions.dynamic_color(c, dynamic).ok());
        if let Some(dynamic) = dynamic {
            attrs.push(("dynamicr", dynamic.red));
            attrs.push(("dynamicg", dynamic.green));
            attrs.push(("dynamicb", dynamic.blue));
            if let Some(alpha) = dynamic.alpha {
                attrs.push(("dynamica", alpha));
            }
            attrs.push(("colorSpace", dynamic.space.to_string()));
        }
        if let Some(fill) = saved.and_then(|style| style.fill.as_deref()) {
            if fill.pattern != 0 || fill.hatch.is_some() {
                attrs.push(("fillType", fill.pattern.to_string()));
            }
            if let Some([angle, distance]) = fill.hatch {
                attrs.push(("hatchAngle", angle.to_string()));
                attrs.push(("hatchDistance", distance.to_string()));
            }
            if let Some(symbol) = &fill.symbol {
                attrs.push(("fillSymbol", symbol.clone()));
            }
            if let Some(inverse) = fill.inverse {
                attrs.push(("inverseFill", flag(inverse).to_owned()));
            }
        }
        let attrs: Vec<_> = attrs.iter().map(|(k, v)| (*k, v.as_str())).collect();
        self.xml.tag("objColor", &attrs)
    }

    fn slider(
        &mut self,
        id: Id,
        bounds: [Option<&str>; 2],
        layout: Option<&SliderLayout>,
    ) -> Result<(), ExportError> {
        let mut attrs: Vec<(&str, String)> = Vec::new();
        match layout {
            // A tag the reader left early is written as far as it got.
            Some(layout) => {
                attrs.push(("absoluteScreenLocation", flag(layout.absolute).into()));
                if let Some([x, y]) = layout.position {
                    attrs.extend([("x", number(x)), ("y", number(y))]);
                }
                if let Some(width) = layout.width {
                    attrs.push(("width", number(width)));
                }
                if let Some(flags) = layout.flags {
                    attrs.extend([
                        ("fixed", flag(flags.fixed).into()),
                        ("showAlgebra", flag(flags.show_algebra).into()),
                        ("horizontal", flag(flags.horizontal).into()),
                    ]);
                }
            }
            None => {
                let style = self.c.slider_style(id);
                let (x, y) = self.c.slider_position(id, style);
                attrs.extend([
                    ("absoluteScreenLocation", flag(style.absolute).into()),
                    ("x", number(x)),
                    ("y", number(y)),
                    ("width", number(style.width)),
                    ("fixed", flag(style.fixed).into()),
                    ("showAlgebra", "true".into()),
                    ("horizontal", flag(style.horizontal).into()),
                ]);
            }
        }
        if bounds.iter().any(Option::is_some) {
            // The reader supplies default bounds for visible sliders
            // before reading this tag. An undefined endpoint clears it.
            attrs.extend([
                ("min", bounds[0].unwrap_or("0 / 0").into()),
                ("max", bounds[1].unwrap_or("0 / 0").into()),
            ]);
        }
        let attrs: Vec<_> = attrs.iter().map(|(k, v)| (*k, v.as_str())).collect();
        self.xml.tag("slider", &attrs)
    }

    /// `<animation>` of a saved object. The step and the speed are numeric
    /// expressions; one that is no arithmetic over the construction any more
    /// (an object it names is gone) is left out, one whose inputs are
    /// undefined right now is kept.
    fn animation(&mut self, animation: &SliderAnimation) -> Result<(), ExportError> {
        let c = self.c;
        let mut numeric = |source: &Option<String>| {
            source
                .as_deref()
                .and_then(|source| self.expressions.number(c, source).ok())
        };
        let step = numeric(&animation.step);
        let speed = numeric(&animation.speed);
        let mode = animation.mode.to_string();
        let mut attrs = vec![
            ("type", mode.as_str()),
            ("playing", flag(animation.playing)),
        ];
        if let Some(step) = &step {
            attrs.push(("step", step));
        }
        if let Some(speed) = &speed {
            attrs.push(("speed", speed));
        }
        self.xml.tag("animation", &attrs)
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
