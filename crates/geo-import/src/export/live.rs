//! Native command definitions for geometry with labelled inputs.

use super::{failure, number, ExportError, Writer};
use geo_kernel::algo::{
    arc::ArcDef,
    conic::ConicDef,
    general_conic::GeneralConicDef,
    lines::LineDef,
    measure::MeasureDef,
    path::PathDef,
    polygon::{Count, PolygonDef},
    transform::TransformDef,
    vector::VectorDef,
};
use geo_kernel::{Def, Id, LocusDef};

impl Writer<'_> {
    pub(super) fn live_definition(
        &mut self,
        def: Def,
        outputs: &[Id],
    ) -> Result<bool, ExportError> {
        use Def::*;
        let c = self.c;
        let mut command = |name, ids: &[Id]| self.live_command(name, ids, outputs);
        match def {
            Line(a, b) => command("Line", &[a, b]),
            Segment(a, b) if outputs.len() == 1 => command("Segment", &[a, b]),
            Midpoint(a, b) => command("Midpoint", &[a, b]),
            Distance(a, b) => command("Distance", &[a, b]),
            IntersectLines(a, b) => command("Intersect", &[a, b]),
            Parallel { through, to } => command("Line", &[through, to]),
            Perpendicular { through, to } => command("OrthogonalLine", &[through, to]),
            CircleThrough { center, through } => command("Circle", &[center, through]),
            CircleRadius { center, radius } => command("Circle", &[center, radius]),
            Lines(LineDef::BisectorPoints(a, b)) => command("LineBisector", &[a, b]),
            Lines(LineDef::BisectorSegment(s)) => command("LineBisector", &[s]),
            Lines(LineDef::AngleBisectorPoints { leg1, vertex, leg2 }) => {
                command("AngularBisector", &[leg1, vertex, leg2])
            }
            Conic(ConicDef::ThroughThree(a, b, d)) => command("Circle", &[a, b, d]),
            Conic(ConicDef::SegmentRadius { center, segment }) => {
                command("Circle", &[center, segment])
            }
            Conic(ConicDef::LiteralRadius { center, radius })
                if !c.label(center).starts_with('\0') =>
            {
                self.command("Circle", &[c.label(center), &number(radius)], outputs)?;
                Ok(true)
            }
            Conic(ConicDef::Incircle(a, b, d)) => command("Incircle", &[a, b, d]),
            Conic(ConicDef::Center(shape)) => command("Center", &[shape]),
            Conic(ConicDef::MidpointSegment(s)) => command("Midpoint", &[s]),
            Vector(VectorDef::Between(a, b)) => command("Vector", &[a, b]),
            Vector(VectorDef::Position(a)) => command("Vector", &[a]),
            Vector(VectorDef::RayPoints(a, b) | VectorDef::RayVector(a, b)) => {
                command("Ray", &[a, b])
            }
            Vector(VectorDef::LineVector(a, b)) => command("Line", &[a, b]),
            Vector(VectorDef::OrthoLineVector(a, b)) => command("OrthogonalLine", &[a, b]),
            Vector(VectorDef::Translate { src, by, .. }) => command("Translate", &[src, by]),
            Vector(VectorDef::TranslateTo { vector, start }) => {
                command("Translate", &[vector, start])
            }
            Vector(VectorDef::Length(v)) => command("Length", &[v]),
            Transform(TransformDef::MirrorPoint { src, center, .. }) => {
                command("Mirror", &[src, center])
            }
            Transform(TransformDef::MirrorLine { src, mirror, .. }) => {
                command("Mirror", &[src, mirror])
            }
            Transform(TransformDef::Invert { src, circle }) => command("Mirror", &[src, circle]),
            Transform(TransformDef::Rotate {
                src, angle, center, ..
            }) => match center {
                Some(center) => command("Rotate", &[src, angle, center]),
                None => command("Rotate", &[src, angle]),
            },
            Transform(TransformDef::Dilate {
                src,
                factor,
                center,
                ..
            }) => match center {
                Some(center) => command("Dilate", &[src, factor, center]),
                None => command("Dilate", &[src, factor]),
            },
            Measure(MeasureDef::Length(id)) => command("Length", &[id]),
            Measure(MeasureDef::Radius(id)) => command("Radius", &[id]),
            Measure(MeasureDef::CircleArea(id)) => command("Area", &[id]),
            Measure(MeasureDef::Slope(id)) => command("Slope", &[id]),
            Measure(MeasureDef::DistancePointPath { point, path }) => {
                command("Distance", &[point, path])
            }
            Polygon(PolygonDef::Polygon(ids)) => command("Polygon", c.list(ids)),
            Polygon(PolygonDef::PolyLine(ids)) => command("PolyLine", c.list(ids)),
            Polygon(PolygonDef::Area(p)) => command("Area", &[p]),
            Polygon(PolygonDef::Perimeter(p)) => command("Perimeter", &[p]),
            Polygon(PolygonDef::Centroid(p)) => command("Centroid", &[p]),
            GeneralConic(GeneralConicDef::FivePoints(ids)) => command("Conic", c.list(ids)),
            GeneralConic(GeneralConicDef::FociPoint {
                f1,
                f2,
                through,
                hyperbola,
            }) => command(
                if hyperbola { "Hyperbola" } else { "Ellipse" },
                &[f1, f2, through],
            ),
            GeneralConic(GeneralConicDef::Parabola { focus, directrix }) => {
                command("Parabola", &[focus, directrix])
            }
            Path(PathDef::On { path }) => command("Point", &[path]),
            Path(PathDef::OnParam { path, param }) => command("Point", &[path, param]),
            Path(PathDef::InRegion { region }) => command("PointIn", &[region]),
            Path(PathDef::Parameter { point }) => command("PathParameter", &[point]),
            Arc(ArcDef::Circle {
                center,
                start,
                end,
                sector,
            }) => command(
                if sector { "CircleSector" } else { "CircleArc" },
                &[center, start, end],
            ),
            Arc(ArcDef::Circum { a, b, c, sector }) => command(
                if sector {
                    "CircumcircleSector"
                } else {
                    "CircumcircleArc"
                },
                &[a, b, c],
            ),
            Arc(ArcDef::Semicircle { a, b }) => command("Semicircle", &[a, b]),
            Locus(LocusDef::Path { point, mover }) => command("Locus", &[point, mover]),
            Locus(LocusDef::Slider { point, slider }) => command("Locus", &[point, slider]),
            _ => Ok(false),
        }
    }

    /// GeoGebra saves Segment(point, length) as a path point on an
    /// anonymous circle followed by an ordinary two-point segment.
    pub(super) fn live_fixed_segment(
        &mut self,
        outputs: &[Id],
        source: &str,
    ) -> Result<bool, ExportError> {
        let [endpoint, segment] = *outputs else {
            return Ok(false);
        };
        let c = self.c;
        let Def::Path(PathDef::FixedEndpoint { start, length }) = c.def(endpoint) else {
            return Ok(false);
        };
        if c.def(segment) != Def::Segment(start, endpoint) {
            return Ok(false);
        }
        let literal;
        let length = match length {
            Count::Literal(length) => {
                literal = number(length);
                &literal
            }
            Count::Object(id) => super::syntax(c.label(id))?,
        };
        let start = super::syntax(c.label(start))?;
        if start
            .len()
            .saturating_add(length.len())
            .saturating_add("Circle[, ]".len())
            > crate::xml::MAX_ATTRIBUTE_BYTES
        {
            return Err(failure(
                "fixed segment circle exceeds the import attribute size limit",
            ));
        }
        let circle = format!("Circle[{start}, {length}]");
        self.command("Point", &[&circle], &[endpoint])?;
        self.element(endpoint, source)?;
        self.command("Segment", &[start, c.label(endpoint)], &[segment])?;
        self.element(segment, source)?;
        Ok(true)
    }

    fn live_command(
        &mut self,
        name: &str,
        inputs: &[Id],
        outputs: &[Id],
    ) -> Result<bool, ExportError> {
        if inputs.len() > crate::xml::MAX_ARGUMENTS {
            return Err(failure("command exceeds the import argument limit"));
        }
        let labels: Vec<_> = inputs.iter().map(|&id| self.c.label(id)).collect();
        if labels.iter().any(|label| label.starts_with('\0')) {
            return Ok(false);
        }
        self.command(name, &labels, outputs)?;
        Ok(true)
    }
}
