// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/Kernel.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoCurveCartesian.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoLocusND.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use super::Format;
use crate::algo::conic_nd::ConicData;
use crate::algo::measure::MeasureDef;
use crate::equation;
use crate::expr::eval::{Ctx, Env, Lists};
use crate::expr::{self, Entry, Node};
use crate::kernel::locus::LocusDef;
use crate::kernel::{Construction, Def, EquationForm, Id, Kind, Value};
use crate::tol::STANDARD_PRECISION;

impl Construction {
    /// A line, ray or conic equation in the object's current display form.
    pub fn formatted_equation(&self, id: Id) -> Option<String> {
        Format::new(false).equation::<false>(self, id, None)
    }
}

impl Construction {
    /// Mathematical definitions and rectangular numeric list values for the algebra panel.
    pub fn algebra_formula(&self, id: Id) -> Option<String> {
        let format = Format::new(true);
        match self.def(id) {
            Def::Expr(def) if matches!(def.kind(), Kind::Function | Kind::FunctionNVar) => {
                let entry = expr::entry(self, def);
                let meter = Lists::new(Vec::new());
                let body = format.entry_value::<true>(self, entry, Some(&meter), false)?;
                let variables = if def.kind() == Kind::FunctionNVar {
                    "x,y"
                } else {
                    entry
                        .curve_parameter
                        .as_deref()
                        .unwrap_or(if entry.function_of_y { "y" } else { "x" })
                };
                Some(format!(
                    r"{}\left({variables}\right) = {body}",
                    Format::label(self.label(id), true)
                ))
            }
            _ if self.kind(id) == Kind::List => {
                let rows = self.elements(id)?;
                let expr::Element::List(first) = rows.get(0)? else {
                    return None;
                };
                let columns = first.len();
                if columns == 0 || columns > 128 || rows.len().saturating_mul(columns) > 512 {
                    return None;
                }
                let mut out = String::from(r"\begin{pmatrix}");
                for (i, row) in rows.iter().enumerate() {
                    let expr::Element::List(row) = row else {
                        return None;
                    };
                    if row.len() != columns {
                        return None;
                    }
                    if i != 0 {
                        out.push_str(r"\\");
                    }
                    for (i, item) in row.iter().enumerate() {
                        let expr::Element::Value(kind @ (Kind::Number | Kind::Angle), value) = item
                        else {
                            return None;
                        };
                        if i != 0 {
                            out.push('&');
                        }
                        out.push_str(&format.shown(kind, value));
                    }
                }
                out.push_str(r"\end{pmatrix}");
                Some(out)
            }
            _ => None,
        }
    }
}

impl Format {
    pub(super) fn object_value<const M: bool>(self, ctx: &Ctx<'_, M>, id: Id) -> Option<String> {
        let value = ctx.c.value(id);
        if !value.is_defined() {
            return Some(self.shown(ctx.c.kind(id), value));
        }
        if let Some(eq) = self.equation::<M>(ctx.c, id, ctx.meter) {
            return Some(eq);
        }
        match ctx.c.def(id) {
            Def::Expr(def) if def.kind() == Kind::Function => {
                let format = Self {
                    simplify_coefficients: true,
                    ..self
                };
                format.entry_value::<M>(ctx.c, expr::entry(ctx.c, def), ctx.meter, true)
            }
            Def::Expr(def) if def.kind() == Kind::FunctionNVar => {
                let entry = expr::entry(ctx.c, def);
                if let Node::Side(equation, left) = entry.nodes[entry.root as usize] {
                    if let Some(entry) = expr::equation::side_entry(ctx.c, equation, left) {
                        return self.entry_value::<M>(ctx.c, entry, ctx.meter, true);
                    }
                    let display = self.equation::<M>(ctx.c, equation, ctx.meter)?;
                    let (lhs, rhs) = display.split_once(self.equals())?;
                    return Some(if left { lhs } else { rhs }.to_owned());
                }
                Some("?".into())
            }
            Def::Measure(MeasureDef::CartesianCurve { coordinates, .. }) => {
                let mut values = coordinates.into_iter().map(|id| {
                    let Def::Expr(def) = ctx.c.def(Id(id as usize)) else {
                        return None;
                    };
                    self.entry_value::<M>(ctx.c, expr::entry(ctx.c, def), ctx.meter, true)
                });
                Some(format!("({}, {})", values.next()??, values.next()??))
            }
            Def::Locus(locus) => {
                let (command, objects) = match locus {
                    LocusDef::Path { point, mover } => ("Locus", [Some(point), Some(mover)]),
                    LocusDef::Slider { point, slider } => ("Locus", [Some(point), Some(slider)]),
                    LocusDef::Ode { point, source } => ("Locus", [Some(source), Some(point)]),
                    LocusDef::Numeric(_) => return Some(self.shown(ctx.c.kind(id), value)),
                    LocusDef::SlopeField(source) => ("SlopeField", [Some(source), None]),
                    LocusDef::Hull(list) => ("Hull", [Some(list), None]),
                };
                let args = objects
                    .into_iter()
                    .flatten()
                    .map(|id| {
                        let label = ctx.c.label(id);
                        if label.starts_with('\0') {
                            self.object_value(ctx, id)
                        } else {
                            Some(Self::label(label, self.latex))
                        }
                    })
                    .collect::<Option<Vec<_>>>()?;
                Some(self.command(command, &args))
            }
            _ => Some(self.shown(ctx.c.kind(id), value)),
        }
    }

    pub(super) fn user_equation<const M: bool>(
        self,
        c: &Construction,
        id: Id,
        meter: Option<&Lists>,
        substitute: bool,
    ) -> Option<String> {
        let side = |left| {
            let format = Self {
                simplify_coefficients: false,
                ..self
            };
            format.entry_value::<M>(
                c,
                expr::equation::side_entry(c, id, left)?,
                meter,
                substitute,
            )
        };
        Some(format!("{}{}{}", side(true)?, self.equals(), side(false)?))
    }

    fn entry_value<const M: bool>(
        self,
        c: &Construction,
        entry: &Entry,
        meter: Option<&Lists>,
        substitute: bool,
    ) -> Option<String> {
        let ctx = Ctx {
            meter,
            ..Ctx::new(c, entry, &Env::None)
        };
        self.expression::<M>(&ctx, entry.root, substitute, 0)
    }

    fn equation<const M: bool>(
        self,
        c: &Construction,
        id: Id,
        meter: Option<&Lists>,
    ) -> Option<String> {
        let kind = c.kind(id);
        if kind == Kind::ImplicitCurve {
            if !c.value(id).is_defined() {
                return Some("?".into());
            }
            if c.equation_form(id) == Some(&EquationForm::User) {
                if let Some(text) = self.user_equation::<M>(c, id, meter, false) {
                    return Some(text);
                }
            }
            return Some(expr::equation::implicit::raw_equation(
                c.implicit_coefficients(id)?,
                |v| self.number(v),
            ));
        }
        if !matches!(kind, Kind::Line | Kind::Ray | Kind::Circle | Kind::Conic) {
            return None;
        }
        let value = c.value(id);
        if !value.is_defined() {
            return Some("?".into());
        }
        let form = c.equation_form(id);
        if form == Some(&EquationForm::User) {
            if let Some(text) = self.user_equation::<M>(c, id, meter, true) {
                return Some(text);
            }
        }
        match value {
            Value::Line { a, b, c: d } | Value::Ray { a, b, c: d, .. } => {
                if let Some(EquationForm::Parametric(parameter)) = form {
                    let start = match value {
                        Value::Ray { x, y, .. } => Some((x, y)),
                        _ => match c.def(id) {
                            Def::Line(point, _)
                            | Def::Parallel { through: point, .. }
                            | Def::Perpendicular { through: point, .. } => match c.value(point) {
                                Value::Point { x, y } => Some((x, y)),
                                _ => None,
                            },
                            _ => None,
                        },
                    };
                    let (x, y) = start.unwrap_or_else(|| {
                        if a.abs() > b.abs() {
                            (-d / a, 0.0)
                        } else {
                            (0.0, -d / b)
                        }
                    });
                    return Some(format!(
                        "X{}({}, {}) + {} ({}, {})",
                        self.equals(),
                        self.number(x),
                        self.number(y),
                        parameter,
                        self.number(b),
                        self.number(-a)
                    ));
                }
                Some(self.line(a, b, d, form))
            }
            Value::Circle { cx, cy, r } if matches!(form, None | Some(EquationForm::Specific)) => {
                Some(self.circle(cx, cy, r))
            }
            Value::Circle { .. } | Value::Conic { .. } => {
                let conic = c.conic(id)?;
                if let Some(form) = form {
                    if let Some(text) =
                        equation::styled_conic_equation(&conic, form, self.latex, &|value| {
                            self.number(value)
                        })
                    {
                        return Some(text);
                    }
                }
                Some(self.conic_equation(&conic))
            }
            _ => None,
        }
    }

    pub(super) fn conic_equation(self, conic: &ConicData) -> String {
        let eq = equation::conic_equation(conic, &|value| self.number(value));
        if self.latex {
            eq.replace('²', "^{2}").replace(" = ", self.equals())
        } else {
            eq
        }
    }

    fn equals(self) -> &'static str {
        if self.latex {
            r"\, = \,"
        } else {
            " = "
        }
    }

    fn coefficient(self, value: f64) -> String {
        match value {
            1.0 => String::new(),
            -1.0 => "-".into(),
            _ => self.number(value),
        }
    }

    pub(super) fn line(self, a: f64, b: f64, c: f64, form: Option<&EquationForm>) -> String {
        let eq = self.equals();
        let general = form == Some(&EquationForm::General);
        if b.abs() < STANDARD_PRECISION {
            if general {
                let constant = self.number((c / a).abs());
                let mut lhs = String::from("x");
                if constant != "0" {
                    let sign = if c / a < 0.0 { '-' } else { '+' };
                    lhs.push_str(&format!(" {sign}  {constant}"));
                }
                return format!("{lhs}{eq}0");
            }
            return format!("x{eq}{}", self.number(-c / a));
        }
        if general && a.abs() < STANDARD_PRECISION {
            let constant = c / b;
            let sign = if constant < 0.0 { '-' } else { '+' };
            return format!("y {sign} {}{eq}0", self.number(constant.abs()));
        }
        if !general && (a.abs() < STANDARD_PRECISION || form == Some(&EquationForm::Explicit)) {
            let slope = -a / b;
            let intercept = -c / b;
            if slope.abs() <= 0.005 {
                return format!("y{eq}{}", self.number(intercept));
            }
            let mut rhs = self.coefficient(slope);
            if self.latex {
                rhs.push(' ');
            }
            rhs.push('x');
            if intercept.abs() > 0.005 {
                let sign = if intercept < 0.0 { '-' } else { '+' };
                rhs.push_str(&format!(" {sign} {}", self.number(intercept.abs())));
            }
            return format!("y{eq}{rhs}");
        }
        let scale = if general {
            1.0
        } else {
            equation::equation_scale(&[a, b, c])
        };
        let mut lhs = format!("{}x", self.coefficient(a / scale));
        let b = b / scale;
        if b.abs() > 0.005 {
            let sign = if b < 0.0 { '-' } else { '+' };
            lhs.push_str(&format!(" {sign} {}y", self.coefficient(b.abs())));
        }
        if general && c.abs() > 0.005 {
            let sign = if c < 0.0 { '-' } else { '+' };
            lhs.push_str(&format!(" {sign} {}", self.number(c.abs())));
        }
        let rhs = if general { 0.0 } else { -c / scale };
        format!("{lhs}{eq}{}", self.number(rhs))
    }

    pub(super) fn circle(self, cx: f64, cy: f64, r: f64) -> String {
        let square = |var: &str, center: f64| {
            let center = self.number(center);
            let base = match center.as_str() {
                "0" => var.to_owned(),
                s if s.starts_with('-') => format!("({var} + {})", &s[1..]),
                s => format!("({var} - {s})"),
            };
            format!("{base}{}", if self.latex { "^{2}" } else { "²" })
        };
        format!(
            "{} + {} = {}",
            square("x", cx),
            square("y", cy),
            self.number(r * r)
        )
    }
}
