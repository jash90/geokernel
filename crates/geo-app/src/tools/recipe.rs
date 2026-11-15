//! What each tool picks, when it is complete, the lines it runs and its hint.

use super::{NumberRequest, Pick, Tool};
use crate::describe::fixed;
use crate::labels::{next_greek, next_label, polygon_label, primed};
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
        Tool::Angle => n < 3 && existing_point(cand),
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
        Tool::Circle3 | Tool::Angle => n == 3,
        Tool::AngleBisector => n == 3 || (n == 2 && first != Some(Kind::Point)),
        Tool::CircleRadius => {
            return (n == 1).then_some(Finish::Ask(NumberRequest {
                title: "Okrąg: środek i promień",
                prompt: "Promień",
                default: 2.0,
                degrees: false,
            }))
        }
        // A polygon is finished by clicking its first vertex again.
        Tool::Polygon => false,
        Tool::RegularPolygon => {
            return (n == 2).then_some(Finish::Ask(NumberRequest {
                title: "Wielokąt foremny",
                prompt: "Liczba wierzchołków",
                default: 4.0,
                degrees: false,
            }))
        }
        Tool::Rotate => {
            return (n == 2).then_some(Finish::Ask(NumberRequest {
                title: "Obrót",
                prompt: "Kąt w stopniach",
                default: 45.0,
                degrees: true,
            }))
        }
        Tool::Dilate => {
            return (n == 2).then_some(Finish::Ask(NumberRequest {
                title: "Jednokładność",
                prompt: "Skala",
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
    decimals: usize,
    is_taken: &dyn Fn(&str) -> bool,
) -> Vec<String> {
    let mut lines = Vec::new();
    let mut allocated: Vec<String> = Vec::new();
    let taken = |l: &str, allocated: &[String]| is_taken(l) || allocated.iter().any(|a| a == l);
    let mut args: Vec<String> = Vec::new();
    let kinds: Vec<Kind> = picks.iter().map(Pick::kind).collect();
    for pick in picks {
        match pick {
            Pick::Object { label, .. } => args.push(label.clone()),
            Pick::New { x, y } => {
                let label = next_label(Kind::Point, |l| taken(l, &allocated));
                allocated.push(label.clone());
                lines.push(format!(
                    "{label} = ({}, {})",
                    fixed(*x, decimals),
                    fixed(*y, decimals)
                ));
                args.push(label);
            }
        }
    }
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

/// A one-line Polish hint for what the next click should be.
pub(super) fn hint(tool: Tool, picks: &[Kind]) -> &'static str {
    let n = picks.len();
    let first = picks.first().copied();
    match (tool, n) {
        (Tool::Move, _) => "Przeciągnij punkt, aby go przesunąć",
        (Tool::Point, _) => "Kliknij, aby umieścić punkt",
        (Tool::Intersect, 0) => "Kliknij pierwszy obiekt",
        (Tool::Intersect, _) => "Kliknij drugi obiekt",
        (Tool::Midpoint, 0) => "Kliknij dwa punkty, odcinek albo okrąg",
        (Tool::Midpoint, _) => "Kliknij drugi punkt",
        (Tool::Line, 0) => "Kliknij pierwszy punkt prostej",
        (Tool::Line, _) => "Kliknij drugi punkt prostej",
        (Tool::Segment, 0) => "Kliknij pierwszy koniec odcinka",
        (Tool::Segment, _) => "Kliknij drugi koniec odcinka",
        (Tool::Ray, 0) => "Kliknij początek półprostej",
        (Tool::Ray, _) => "Kliknij punkt na półprostej",
        (Tool::Vector, 0) => "Kliknij początek wektora",
        (Tool::Vector, _) => "Kliknij koniec wektora",
        (Tool::Perpendicular | Tool::Parallel, 0) => "Kliknij punkt lub prostą",
        (Tool::Perpendicular | Tool::Parallel, _) if first == Some(Kind::Point) => "Kliknij prostą",
        (Tool::Perpendicular | Tool::Parallel, _) => "Kliknij punkt",
        (Tool::Bisector, 0) => "Kliknij dwa punkty albo odcinek",
        (Tool::Bisector, _) => "Kliknij drugi punkt",
        (Tool::AngleBisector, 0) => "Kliknij trzy punkty albo dwie proste",
        (Tool::AngleBisector, _) if first == Some(Kind::Point) => "Kliknij kolejny punkt",
        (Tool::AngleBisector, _) => "Kliknij drugą prostą",
        (Tool::Tangents, 0) => "Kliknij punkt lub prostą, potem okrąg",
        (Tool::Tangents, _) if first == Some(Kind::Circle) => "Kliknij punkt lub prostą",
        (Tool::Tangents, _) => "Kliknij okrąg",
        (Tool::Polar, 0) => "Kliknij punkt, potem okrąg",
        (Tool::Polar, _) if first == Some(Kind::Circle) => "Kliknij punkt",
        (Tool::Polar, _) => "Kliknij okrąg",
        (Tool::Circle, 0) => "Kliknij środek okręgu",
        (Tool::Circle, _) => "Kliknij punkt na okręgu",
        (Tool::CircleRadius, _) => "Kliknij środek okręgu",
        (Tool::Circle3, _) => "Kliknij trzy punkty okręgu",
        (Tool::Polygon, 0) => "Kliknij pierwszy wierzchołek",
        (Tool::Polygon, 1 | 2) => "Kliknij kolejny wierzchołek",
        (Tool::Polygon, _) => "Kliknij kolejny wierzchołek albo pierwszy, aby zamknąć",
        (Tool::RegularPolygon, 0) => "Kliknij pierwszy wierzchołek",
        (Tool::RegularPolygon, _) => "Kliknij drugi wierzchołek",
        (Tool::Angle, 0) => "Kliknij punkt na pierwszym ramieniu",
        (Tool::Angle, 1) => "Kliknij wierzchołek kąta",
        (Tool::Angle, _) => "Kliknij punkt na drugim ramieniu",
        (Tool::Distance, 0) => "Kliknij dwa punkty, punkt i prostą, odcinek albo okrąg",
        (Tool::Distance, _) if first == Some(Kind::Point) => "Kliknij punkt lub prostą",
        (Tool::Distance, _) => "Kliknij punkt",
        (Tool::ReflectLine, 0)
        | (Tool::ReflectPoint, 0)
        | (Tool::Rotate, 0)
        | (Tool::Translate, 0)
        | (Tool::Dilate, 0) => "Kliknij obiekt do przekształcenia",
        (Tool::ReflectLine, _) => "Kliknij oś symetrii",
        (Tool::ReflectPoint, _) => "Kliknij środek symetrii",
        (Tool::Rotate, _) => "Kliknij środek obrotu",
        (Tool::Dilate, _) => "Kliknij środek jednokładności",
        (Tool::Translate, _) => "Kliknij wektor",
        (Tool::ReflectCircle, 0) => "Kliknij punkt",
        (Tool::ReflectCircle, _) => "Kliknij okrąg inwersji",
        (Tool::MoveView, _) => "Przeciągnij, aby przesunąć widok",
        (Tool::ZoomIn, _) => "Kliknij, aby powiększyć",
        (Tool::ZoomOut, _) => "Kliknij, aby pomniejszyć",
        (Tool::ShowHide, _) => "Kliknij obiekt, aby go pokazać lub ukryć",
        (Tool::Delete, _) => "Kliknij obiekt do usunięcia",
    }
}
