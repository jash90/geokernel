//! The constructions the benchmark draws: an example file and generated
//! grids of a fixed cell, so that every size is reproducible.

/// A construction and the objects the scripted states act on.
pub struct Scene {
    pub name: String,
    pub script: String,
    /// A free point with dependents: dragged, and hovered around.
    pub drag: String,
    /// A free point picked as the first point of the segment tool.
    pub pick: String,
    /// An object without dependents, deleted and restored.
    pub delete: String,
    /// Two points to make a midpoint of (the object added and undone).
    pub midpoint: (String, String),
}

const FOUR_LINES: &str = include_str!("../../../../examples/four-lines.geo");

/// The objects of one cell: three free points, a free-standing segment, a
/// line, a circle, a triangle with its three sides, a midpoint and an
/// intersection. Twelve objects.
const CELL_OBJECTS: usize = 12;
/// Distance between neighbouring cells in world units.
const PITCH: f64 = 5.0;

fn cell(i: usize, x: f64, y: f64) -> String {
    let p = |s: &str| format!("{s}_{{{i}}}");
    let (pp, q, r) = (p("P"), p("Q"), p("R"));
    [
        format!("{pp} = ({x}, {y})"),
        format!("{q} = ({}, {y})", x + 2.0),
        format!("{r} = ({x}, {})", y + 2.0),
        format!("{} = Segment({pp}, {r})", p("s")),
        format!("{} = Line({q}, {r})", p("l")),
        format!("{} = Circle({pp}, {q})", p("k")),
        format!("{} = Polygon({pp}, {q}, {r})", p("t")),
        format!("{} = Midpoint({q}, {r})", p("M")),
        format!("{} = Intersect({}, {})", p("I"), p("l"), p("k")),
    ]
    .join("\n")
}

/// About `objects` objects in a square grid of cells around the origin.
pub fn grid(name: &str, objects: usize) -> Scene {
    let cells = objects.div_ceil(CELL_OBJECTS).max(1);
    let cols = (cells as f64).sqrt().ceil() as usize;
    let half = (cols as f64 - 1.0) * PITCH / 2.0;
    let mut lines = Vec::with_capacity(cells);
    for i in 0..cells {
        let x = (i % cols) as f64 * PITCH - half;
        let y = (i / cols) as f64 * PITCH - half;
        lines.push(cell(i, x, y));
    }
    // The cell nearest the middle of the grid.
    let mid = (cols / 2) * cols + cols / 2;
    let target = mid.min(cells - 1);
    let p = |s: &str| format!("{s}_{{{target}}}");
    Scene {
        name: name.to_owned(),
        script: lines.join("\n") + "\n",
        drag: p("P"),
        pick: p("Q"),
        delete: p("I"),
        midpoint: (p("Q"), p("R")),
    }
}

pub fn four_lines() -> Scene {
    Scene {
        name: "four-lines".to_owned(),
        script: FOUR_LINES.to_owned(),
        drag: "A".to_owned(),
        pick: "B".to_owned(),
        delete: "E".to_owned(),
        midpoint: ("A".to_owned(), "B".to_owned()),
    }
}

/// Every scene, smallest first.
pub fn all() -> Vec<Scene> {
    vec![
        four_lines(),
        grid("n100", 100),
        grid("n1000", 1_000),
        grid("n10000", 10_000),
    ]
}
