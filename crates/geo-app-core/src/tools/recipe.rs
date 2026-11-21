//! What each tool picks, when it is complete, the lines it runs and its hint.

use super::{NumberRequest, Pick, Tool};
use crate::labels::{next_greek, next_label, polygon_label, primed};
use crate::text::{self, Hint, Label};
use geo_kernel::kernel::Kind;

fn point(k: Option<Kind>) -> bool {
    matches!(k, None | Some(Kind::Point))
}

fn existing_point(k: Option<Kind>) -> bool {
    k == Some(Kind::Point)
}

/// A line or a segment (the kernel takes both wherever a line is expected).
fn line_like(k: Option<Kind>) -> bool {
    matches!(k, Some(Kind::Line | Kind::Segment))
}

fn circle(k: Option<Kind>) -> bool {
    k == Some(Kind::Circle)
}

/// What the reflections, rotations and dilations accept.
fn transformable(k: Option<Kind>) -> bool {
    matches!(
        k,
        Some(Kind::Point | Kind::Line | Kind::Segment | Kind::Circle)
    )
}

/// Whether `cand` (`None`: a new point on empty space) may be the next pick
/// after `picks`.
pub(super) fn accepts(tool: Tool, picks: &[Kind], cand: Option<Kind>) -> bool {
    let n = picks.len();
    let first = picks.first().copied();
    let has = |f: fn(Option<Kind>) -> bool| picks.iter().any(|&k| f(Some(k)));
    match tool {
        Tool::Point => n == 0 && cand.is_none(),
        // The kernel intersects whole lines and circles only.
        Tool::Intersect => n < 2 && matches!(cand, Some(Kind::Line | Kind::Circle)),
        Tool::Midpoint => match n {
            0 => point(cand) || matches!(cand, Some(Kind::Segment | Kind::Circle)),
            1 => point(cand),
            _ => false,
        },
        Tool::Line | Tool::Segment | Tool::Ray | Tool::Vector | Tool::Circle => {
            n < 2 && point(cand)
        }
        Tool::Bisector => match n {
            0 => point(cand) || cand == Some(Kind::Segment),
            1 => point(cand),
            _ => false,
        },
        Tool::Circle3 => n < 3 && point(cand),
        // Closing on the first vertex is handled by the tool state.
        Tool::Polygon => point(cand),
        Tool::RegularPolygon => n < 2 && point(cand),
        Tool::CircleRadius => n == 0 && point(cand),
        Tool::Perpendicular | Tool::Parallel => {
            n < 2 && ((point(cand) && !has(point)) || (line_like(cand) && !has(line_like)))
        }
        Tool::AngleBisector => match first {
            None => point(cand) || line_like(cand),
            Some(Kind::Point) => n < 3 && point(cand),
            Some(_) => n < 2 && line_like(cand),
        },
        Tool::Tangents => {
            n < 2
                && ((circle(cand) && !has(circle))
                    || ((existing_point(cand) || line_like(cand))
                        && !has(existing_point)
                        && !has(line_like)))
        }
        Tool::Polar => {
            n < 2
                && ((circle(cand) && !has(circle))
                    || (existing_point(cand) && !has(existing_point)))
        }
        Tool::Angle => match first {
            None => {
                point(cand)
                    || matches!(
                        cand,
                        Some(Kind::Line | Kind::Segment | Kind::Ray | Kind::Vector | Kind::Polygon)
                    )
            }
            Some(Kind::Point) => n < 3 && point(cand),
            Some(Kind::Vector) => n < 2 && cand == Some(Kind::Vector),
            Some(Kind::Line | Kind::Segment | Kind::Ray) => {
                n < 2 && matches!(cand, Some(Kind::Line | Kind::Segment | Kind::Ray))
            }
            _ => false,
        },
        Tool::Distance => match first {
            None => existing_point(cand) || line_like(cand) || circle(cand),
            Some(Kind::Point) => n == 1 && (existing_point(cand) || line_like(cand)),
            Some(Kind::Line) => n == 1 && existing_point(cand),
            Some(_) => false,
        },
        Tool::ReflectLine => match n {
            0 => transformable(cand),
            1 => line_like(cand),
            _ => false,
        },
        Tool::ReflectPoint | Tool::Rotate | Tool::Dilate => match n {
            0 => transformable(cand),
            1 => existing_point(cand),
            _ => false,
        },
        Tool::ReflectCircle => match n {
            0 => existing_point(cand),
            1 => circle(cand),
            _ => false,
        },
        Tool::Translate => match n {
            0 => transformable(cand),
            1 => cand == Some(Kind::Vector),
            _ => false,
        },
        Tool::Move
        | Tool::MoveView
        | Tool::ZoomIn
        | Tool::ZoomOut
        | Tool::ShowHide
        | Tool::Delete => false,
    }
}

/// How a complete set of picks finishes.
pub(super) enum Finish {
    Now,
    Ask(NumberRequest),
}

/// `Some` once `picks` complete the tool.
pub(super) fn finish(tool: Tool, picks: &[Kind]) -> Option<Finish> {
    let n = picks.len();
    let first = picks.first().copied();
    let done = match tool {
        Tool::Point => n == 1,
        Tool::Midpoint | Tool::Bisector => n == 2 || (n == 1 && first != Some(Kind::Point)),
        Tool::Distance => n == 2 || (n == 1 && matches!(first, Some(Kind::Segment | Kind::Circle))),
        Tool::Circle3 => n == 3,
        Tool::Angle => match first {
            Some(Kind::Point) => n == 3,
            Some(Kind::Polygon) => n == 1,
            Some(_) => n == 2,
            None => false,
        },
        Tool::AngleBisector => n == 3 || (n == 2 && first != Some(Kind::Point)),
        Tool::CircleRadius => {
            return (n == 1).then_some(Finish::Ask(NumberRequest {
                title: text::tool_caption(Tool::CircleRadius),
                prompt: text::label(Label::Radius),
                default: 2.0,
                degrees: false,
            }))
        }
        // A polygon is finished by clicking its first vertex again.
        Tool::Polygon => false,
        Tool::RegularPolygon => {
            return (n == 2).then_some(Finish::Ask(NumberRequest {
                title: text::tool_caption(Tool::RegularPolygon),
                prompt: text::label(Label::VertexCount),
                default: 4.0,
                degrees: false,
            }))
        }
        Tool::Rotate => {
            return (n == 2).then_some(Finish::Ask(NumberRequest {
                title: text::tool_caption(Tool::Rotate),
                prompt: text::label(Label::AngleInDegrees),
                default: 45.0,
                degrees: true,
            }))
        }
        Tool::Dilate => {
            return (n == 2).then_some(Finish::Ask(NumberRequest {
                title: text::tool_caption(Tool::Dilate),
                prompt: text::label(Label::Scale),
                default: 2.0,
                degrees: false,
            }))
        }
        _ => n == 2,
    };
    done.then_some(Finish::Now)
}

/// The script lines for complete `picks`: new free points first, then the
/// number the tool asked for (`number`, in the kernel's unit), then the
/// result.
pub(super) fn build(
    tool: Tool,
    picks: &[Pick],
    number: Option<f64>,
    is_taken: &dyn Fn(&str) -> bool,
) -> Vec<String> {
    let mut lines = Vec::new();
    let mut allocated: Vec<String> = Vec::new();
    let taken = |l: &str, allocated: &[String]| is_taken(l) || allocated.iter().any(|a| a == l);
    let kinds: Vec<Kind> = picks.iter().map(Pick::kind).collect();
    let args: Vec<String> = picks.iter().map(|p| p.label().to_owned()).collect();
    let number_label = number.map(|v| {
        let label = if tool == Tool::Rotate {
            next_greek(|l| taken(l, &allocated))
        } else {
            next_label(Kind::Number, |l| taken(l, &allocated))
        };
        allocated.push(label.clone());
        lines.push(format!("{label} = {v}"));
        label
    });
    let num = number_label.as_deref().unwrap_or("");
    let a = |i: usize| args[i].as_str();
    // The point (or line) first, the circle second.
    let circle_last = |args: &[String]| {
        if kinds[0] == Kind::Circle {
            format!("{}, {}", args[1], args[0])
        } else {
            format!("{}, {}", args[0], args[1])
        }
    };
    // Perpendicular and parallel take the point first, then the line.
    let point_first = || {
        if kinds[0] == Kind::Point {
            format!("{}, {}", a(0), a(1))
        } else {
            format!("{}, {}", a(1), a(0))
        }
    };
    enum Name {
        Fresh(Kind),
        /// `t1`, `q1`, ... by the number of vertices.
        Polygon(usize),
        Greek,
        Prime,
    }
    let (rhs, name) = match tool {
        Tool::Point => return lines,
        Tool::Intersect => (
            format!("Intersect({}, {})", a(0), a(1)),
            Name::Fresh(Kind::Point),
        ),
        Tool::Midpoint if kinds[0] == Kind::Circle => {
            (format!("Center({})", a(0)), Name::Fresh(Kind::Point))
        }
        Tool::Midpoint => (
            format!("Midpoint({})", args.join(", ")),
            Name::Fresh(Kind::Point),
        ),
        Tool::Line => (format!("Line({}, {})", a(0), a(1)), Name::Fresh(Kind::Line)),
        Tool::Segment => (
            format!("Segment({}, {})", a(0), a(1)),
            Name::Fresh(Kind::Segment),
        ),
        Tool::Ray => (format!("Ray({}, {})", a(0), a(1)), Name::Fresh(Kind::Ray)),
        Tool::Vector => (
            format!("Vector({}, {})", a(0), a(1)),
            Name::Fresh(Kind::Vector),
        ),
        Tool::Circle => (
            format!("Circle({}, {})", a(0), a(1)),
            Name::Fresh(Kind::Circle),
        ),
        Tool::Circle3 => (
            format!("Circle({})", args.join(", ")),
            Name::Fresh(Kind::Circle),
        ),
        Tool::CircleRadius => (
            format!("Circle({}, {num})", a(0)),
            Name::Fresh(Kind::Circle),
        ),
        Tool::Perpendicular => (
            format!("Perpendicular({})", point_first()),
            Name::Fresh(Kind::Line),
        ),
        Tool::Parallel => (
            format!("Parallel({})", point_first()),
            Name::Fresh(Kind::Line),
        ),
        Tool::Bisector => (
            format!("PerpendicularBisector({})", args.join(", ")),
            Name::Fresh(Kind::Line),
        ),
        Tool::AngleBisector => (
            format!("AngleBisector({})", args.join(", ")),
            Name::Fresh(Kind::Line),
        ),
        Tool::Tangents => (
            format!("Tangent({})", circle_last(&args)),
            Name::Fresh(Kind::Line),
        ),
        Tool::Polar => (
            format!("Polar({})", circle_last(&args)),
            Name::Fresh(Kind::Line),
        ),
        Tool::Polygon => (
            format!("Polygon({})", args.join(", ")),
            Name::Polygon(args.len()),
        ),
        Tool::RegularPolygon => (
            format!("Polygon({}, {}, {num})", a(0), a(1)),
            Name::Polygon(number.map_or(0, |n| n.round().max(0.0) as usize)),
        ),
        Tool::Angle if kinds[0] == Kind::Polygon => {
            (format!("InteriorAngles({})", a(0)), Name::Greek)
        }
        Tool::Angle => (format!("Angle({})", args.join(", ")), Name::Greek),
        Tool::Distance => match kinds.as_slice() {
            [Kind::Segment] => (format!("Length({})", a(0)), Name::Fresh(Kind::Number)),
            [Kind::Circle] => (
                format!("Circumference({})", a(0)),
                Name::Fresh(Kind::Number),
            ),
            _ => (
                format!("Distance({})", point_first()),
                Name::Fresh(Kind::Number),
            ),
        },
        Tool::ReflectLine | Tool::ReflectPoint | Tool::ReflectCircle => {
            (format!("Reflect({}, {})", a(0), a(1)), Name::Prime)
        }
        Tool::Rotate => (format!("Rotate({}, {num}, {})", a(0), a(1)), Name::Prime),
        Tool::Dilate => (format!("Dilate({}, {num}, {})", a(0), a(1)), Name::Prime),
        Tool::Translate => (format!("Translate({}, {})", a(0), a(1)), Name::Prime),
        Tool::Move
        | Tool::MoveView
        | Tool::ZoomIn
        | Tool::ZoomOut
        | Tool::ShowHide
        | Tool::Delete => return lines,
    };
    let label = match name {
        Name::Fresh(kind) => next_label(kind, |l| taken(l, &allocated)),
        Name::Greek => next_greek(|l| taken(l, &allocated)),
        Name::Polygon(n) => polygon_label(n, |l| taken(l, &allocated)),
        Name::Prime => primed(a(0), kinds[0], |l| taken(l, &allocated)),
    };
    lines.push(format!("{label} = {rhs}"));
    lines
}

/// What the next click should be (the text is in `crate::text`).
pub(super) fn hint(tool: Tool, picks: &[Kind]) -> Hint {
    let n = picks.len();
    let first = picks.first().copied();
    match (tool, n) {
        (Tool::Move, _) => Hint::DragPoint,
        (Tool::Point, _) => Hint::ClickToPlacePoint,
        (Tool::Intersect, 0) => Hint::FirstObject,
        (Tool::Intersect, _) => Hint::SecondObject,
        (Tool::Midpoint, 0) => Hint::TwoPointsSegmentOrCircle,
        (Tool::Midpoint, _) => Hint::SecondPoint,
        (Tool::Line, 0) => Hint::FirstPointOfLine,
        (Tool::Line, _) => Hint::SecondPointOfLine,
        (Tool::Segment, 0) => Hint::FirstEndOfSegment,
        (Tool::Segment, _) => Hint::SecondEndOfSegment,
        (Tool::Ray, 0) => Hint::StartOfRay,
        (Tool::Ray, _) => Hint::PointOnRay,
        (Tool::Vector, 0) => Hint::StartOfVector,
        (Tool::Vector, _) => Hint::EndOfVector,
        (Tool::Perpendicular | Tool::Parallel, 0) => Hint::PointOrLine,
        (Tool::Perpendicular | Tool::Parallel, _) if first == Some(Kind::Point) => Hint::Line,
        (Tool::Perpendicular | Tool::Parallel, _) => Hint::Point,
        (Tool::Bisector, 0) => Hint::TwoPointsOrSegment,
        (Tool::Bisector, _) => Hint::SecondPoint,
        (Tool::AngleBisector, 0) => Hint::ThreePointsOrTwoLines,
        (Tool::AngleBisector, _) if first == Some(Kind::Point) => Hint::NextPoint,
        (Tool::AngleBisector, _) => Hint::SecondLine,
        (Tool::Tangents, 0) => Hint::PointOrLineThenCircle,
        (Tool::Tangents, _) if first == Some(Kind::Circle) => Hint::PointOrLine,
        (Tool::Tangents, _) => Hint::Circle,
        (Tool::Polar, 0) => Hint::PointThenCircle,
        (Tool::Polar, _) if first == Some(Kind::Circle) => Hint::Point,
        (Tool::Polar, _) => Hint::Circle,
        (Tool::Circle, 0) => Hint::CenterOfCircle,
        (Tool::Circle, _) => Hint::PointOnCircle,
        (Tool::CircleRadius, _) => Hint::CenterOfCircle,
        (Tool::Circle3, _) => Hint::ThreePointsOfCircle,
        (Tool::Polygon, 0) => Hint::FirstVertex,
        (Tool::Polygon, 1 | 2) => Hint::NextVertex,
        (Tool::Polygon, _) => Hint::NextVertexOrClose,
        (Tool::RegularPolygon, 0) => Hint::FirstVertex,
        (Tool::RegularPolygon, _) => Hint::SecondVertex,
        (Tool::Angle, 0) => Hint::AngleObjects,
        (Tool::Angle, _) if first == Some(Kind::Vector) => Hint::Vector,
        (Tool::Angle, _) if first != Some(Kind::Point) => Hint::SecondLine,
        (Tool::Angle, 1) => Hint::VertexOfAngle,
        (Tool::Angle, _) => Hint::PointOnSecondArm,
        (Tool::Distance, 0) => Hint::DistanceObjects,
        (Tool::Distance, _) if first == Some(Kind::Point) => Hint::PointOrLine,
        (Tool::Distance, _) => Hint::Point,
        (Tool::ReflectLine, 0)
        | (Tool::ReflectPoint, 0)
        | (Tool::Rotate, 0)
        | (Tool::Translate, 0)
        | (Tool::Dilate, 0) => Hint::ObjectToTransform,
        (Tool::ReflectLine, _) => Hint::MirrorLine,
        (Tool::ReflectPoint, _) => Hint::MirrorPoint,
        (Tool::Rotate, _) => Hint::CenterOfRotation,
        (Tool::Dilate, _) => Hint::CenterOfDilation,
        (Tool::Translate, _) => Hint::Vector,
        (Tool::ReflectCircle, 0) => Hint::Point,
        (Tool::ReflectCircle, _) => Hint::InversionCircle,
        (Tool::MoveView, _) => Hint::DragView,
        (Tool::ZoomIn, _) => Hint::ClickToZoomIn,
        (Tool::ZoomOut, _) => Hint::ClickToZoomOut,
        (Tool::ShowHide, _) => Hint::ObjectToShowOrHide,
        (Tool::Delete, _) => Hint::ObjectToDelete,
    }
}
