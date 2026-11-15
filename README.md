# geokernel

A native dynamic-geometry application written in Rust, built for speed.
GeoGebra is the reference for features and the source of algorithms; it is
not a target for one-to-one compatibility. `geokernel` is a working name.

This project is not affiliated with or endorsed by GeoGebra GmbH. The name
and logo GeoGebra belong to GeoGebra GmbH. Construction files saved by
GeoGebra (`.ggb`) can be opened; what is not supported yet is reported, not
dropped silently.

## Status

Early. The kernel covers points, lines, segments, rays, circles, general
conics (ellipse, hyperbola, parabola and the degenerate cases), vectors,
polygons, polylines, numbers, booleans, lists and functions of x, arithmetic
expressions with GeoGebra's exact decimal arithmetic, incremental updates,
a small script language and SVG output.
The desktop window has a toolbar with construction tools, an algebra view,
an input row with an on-screen keyboard, undo and redo, and lets you drag
free points. The commands so far: lines,
segments, rays, midpoints, perpendicular and parallel lines, bisectors,
tangents and polars of circles, circles (centre and point, centre and
radius, three points, incircle), conics (through five points, ellipse and
hyperbola from foci, parabola from focus and directrix, their centre, foci,
vertices, axes, directrix, eccentricity), intersections, measures (length, distance,
radius, area, slope, angle, ratios), vectors (between points, direction,
unit and orthogonal vectors), transformations (translation, reflection,
rotation, dilation, shear, stretch), polygons (by vertices, regular, their
area, perimeter, centroid, vertices and angles) and predicates (collinear,
parallel, perpendicular, concurrent, concyclic, equal, congruent, tangent).

Every command is checked against values recorded from GeoGebra. See
`docs/command-matrix.csv` for what is planned and done,
`crates/geo-golden/known-gaps.toml` for where the kernel still differs
from GeoGebra, and [DIFFERENCES.md](DIFFERENCES.md) for the differences
that are intended.

## Licence

`geokernel` is licensed under the [EUPL 1.2](LICENSE).

The project takes the port route: parts of it are translated from the
[GeoGebra source code](https://github.com/geogebra/geogebra), whose Java
files are licensed by GeoGebra GmbH under the EUPL 1.2. That makes this
project a derivative work, distributed under the same licence, with sources
available and the original authors credited.

Rules that follow from this choice:

- Only files that carry the EUPL 1.2 header are used as a source.
- Translation files (`.properties`) and artwork from the GeoGebra repository
  are not copied; they are under a non-commercial licence. Command names in
  other languages and all icons are our own work.
- Deliberate differences from GeoGebra are recorded in
  [DIFFERENCES.md](DIFFERENCES.md).

The reference commit for ported code and recorded test data is
`112919f2d450ec7430a5cd3b65dba226fdc43b74`.

## Layout

| Path | Contents |
| --- | --- |
| `crates/geo-kernel` | The kernel: construction graph, object types, incremental updates, script parser, SVG output. No dependencies and no knowledge of any user interface. |
| `crates/geo-cli` | The `geokernel` binary: `run`, `demo`, `bench`. |
| `crates/geo-app` | The `geokernel-app` desktop window (egui on wgpu): toolbar with construction tools, algebra view, graphics view with pan and zoom, on-screen keyboard, undo and redo. Opens scripts and GeoGebra files. |
| `crates/geo-app-core` | The window's logic without any UI toolkit: pointer and touch gestures, layout and size profiles, tools, the document with its undo history, snapshots. Builds for Android. |
| `crates/geo-mcp` | The `geokernel-mcp` server: drives the window without a window through its own frame loop, so an agent or a script can click, drag, type and read the exact state. See [crates/geo-mcp/README.md](crates/geo-mcp/README.md). |
| `crates/geo-import` | Reads GeoGebra construction files (`.ggb` or bare XML) into a kernel script, and reports what it skipped and what differs from the values saved in the file. |
| `crates/geo-golden` | Tests that compare the kernel with values recorded from GeoGebra. |
| `golden/` | Recorded golden data and its format, [golden/SCHEMA.md](golden/SCHEMA.md). |
| `examples/` | Example scripts. |
| `tools/` | Generators for golden data and tables. Not needed to build or test. |
| `docs/` | The command matrix and performance notes. |

## Build and run

```
cargo test --workspace
cargo run --release -p geo-cli -- demo out        # triangle before and after moving A
cargo run --release -p geo-cli -- run examples/triangle.geo out.svg
cargo run --release -p geo-cli -- run drawing.ggb out.svg   # a file saved by GeoGebra
cargo run --release -p geo-cli -- bench           # 105 thousand objects, dragging a point
cargo run --release -p geo-app                    # window with the triangle; pick a tool, drag the blue points, type commands
cargo run --release -p geo-app -- drawing.ggb     # open a script or a file saved by GeoGebra
cargo run --release -p geo-app -- --touch         # touch-sized targets and the narrow layout, on a desktop
cargo run --release -p geo-mcp -- --batch calls.jsonl   # drive the window headlessly, one tool call per line
```

## Script syntax

Commands use GeoGebra's English names and argument order.

```
A = (0, 0)              # free point
v = (1, 2)              # free vector: a lower-case label, as in GeoGebra
r = 3                   # free number (a slider)
l = Line(A, B)
s = Segment(A, B)
M = Midpoint(A, B)
p = PerpendicularLine(M, s)  # also Perpendicular, OrthogonalLine
q = Line(M, l)          # parallel to a line or segment; also Parallel(M, l)
k = Circle(A, B)        # centre A, through B
m = Circle(A, r)        # centre A, radius r
n = Circle(A, B, C)     # through three points
P = Intersect(l, k, 2)  # second intersection point (the first by default)
d = Distance(A, B)
b = LineBisector(A, B)
t = Tangent(P, k)       # creates t_{1} and t_{2}, as GeoGebra does
a = Angle(A, B, C)      # radians
R = Rotate(A, a, B)     # A rotated by the angle a about B
u = Vector(A, B)
h = Ray(A, B)
T = Translate(k, u)     # the circle k moved by the vector u
w = Ellipse(A, B, C)    # foci A and B, through C; also Hyperbola, Parabola(A, l), Conic of five points
F = Focus(w)            # creates F_{1} and F_{2}; also Center, Vertex, Axes, Eccentricity
q = Polygon(A, B, C)    # also creates the sides, named as GeoGebra names them
g = Polygon(A, B, 5)    # regular polygon on the side AB
e = AreCollinear(A, B, C)    # a boolean
c = 2 r + sqrt(x(A)^2 + y(A)^2)   # an expression; recomputed when r or A change
N = (A + B) / 2         # points and vectors in expressions
f(x) = x^2 + r          # a function of x; f(2) in other expressions
v = If(r > 2, r, 0)     # comparisons, boolean operators and If
L = Sequence(i^2, i, 1, 5)   # lists: {1, 4, 9, 16, 25}; also Sum, Sort, First, Join, L(2)
```

The group files in `crates/geo-kernel/src/algo/` list every command and
form; `docs/command-matrix.csv` has the overview.

## Using the kernel as a library

```rust
use geo_kernel::{Construction, Def};

let mut c = Construction::new();
let a = c.add("A", Def::FreePoint { x: 0.0, y: 0.0 })?;
let b = c.add("B", Def::FreePoint { x: 4.0, y: 0.0 })?;
let m = c.add("M", Def::Midpoint(a, b))?;
let recomputed = c.move_point(a, 2.0, 2.0)?; // recomputes only M
```

## Project rules

1. GeoGebra's code is a specification and a source of test cases, not input
   for an automatic converter.
2. Every ported command has a test that compares its result with GeoGebra.
3. Performance budgets are checked automatically from the start.
4. The kernel does not know the interface: the same library runs in the
   application, in tests and later in a browser.
