# Golden data format

Golden data records what GeoGebra computes for a script of commands and a
sequence of moves. The Rust tests replay the same script and moves on
`geo-kernel` and compare the results. No Java is needed to run the tests.

Golden values come only from running GeoGebra through `tools/ggb-harness`.
They are never written by hand and never produced by `geo-kernel` itself.
GeoGebra runs with its *continuity* setting off, which is the default of a
new construction: every snapshot depends only on the current positions, not
on the path the points took to get there.
The file `crates/geo-golden/tests/fixtures/format-sample.json` is a
hand-written sample of the format. It is not golden data.

## File

One JSON object per file, stored under `golden/<group>/`.

```json
{
  "schema": 2,
  "generator": "ggb-harness",
  "upstream_commit": "112919f2d450ec7430a5cd3b65dba226fdc43b74",
  "cases": []
}
```

| Field | Type | Meaning |
| --- | --- | --- |
| `schema` | integer | `1` stores angles as numbers. `2` records the angle type explicitly. |
| `generator` | string | `ggb-harness` for golden data, `format-sample` for the fixture. |
| `upstream_commit` | string or null | Full GeoGebra commit the values were recorded from. `null` only in the fixture. |
| `cases` | array of Case | See below. |

## Case

```json
{
  "id": "geometry-0001",
  "group": "geometry",
  "script": ["A = (0, 0)", "B = (6, 0)", "M = Midpoint(A, B)"],
  "steps": [
    { "action": "initial" },
    { "action": "move", "label": "A", "x": 2.0, "y": 2.0 }
  ],
  "snapshots": [
    { "A": { "type": "point", "defined": true, "x": 0.0, "y": 0.0 } },
    { "A": { "type": "point", "defined": true, "x": 2.0, "y": 2.0 } }
  ]
}
```

| Field | Type | Meaning |
| --- | --- | --- |
| `id` | string | Unique within the whole `golden/` tree. |
| `group` | string | Command group, equal to the directory name. |
| `script` | array of string | GeoGebra input-bar lines with English command names, run in order. Every line has the form `label = definition` or, for an equation, `label: lhs = rhs`, so every object has an explicit label. |
| `steps` | array of Step | The first step is always `initial`. |
| `snapshots` | array of Snapshot | Same length as `steps`. `snapshots[i]` is the state after `steps[i]`. |

## Step

| `action` | Other fields | Meaning |
| --- | --- | --- |
| `initial` | none | The state right after the script has run. |
| `move` | `label`, `x`, `y` | Move the free point `label` to `(x, y)`, or set the free vector `label` to `(x, y)`. A point made by `Point(path)` moves too: GeoGebra puts it at the point of its path closest to `(x, y)` (`GeoPoint.setCoords` with `Path.pointChanged`) and keeps its path parameter. |
| `set_number` | `label`, `value` | Set the free number `label` to `value`. |
| `reject` | `line` | Check that the command reports an error and creates no output. |

## Snapshot

A JSON object that maps every label created by the script to an Object.

A line whose command creates several objects is recorded under the labels
GeoGebra gives them: `p_{1}`, `p_{2}`, … when two or more are defined at
creation, the plain label `p` when only one is. Here `p` is the script label
up to its first `_`, and each output takes the next free index `n`; for
`n < 10` the short form `p_n` must be free too, so an existing `b_1` makes
the outputs `b_{2}` and `b_{3}`. Every snapshot of a case has the same keys,
and no key other than a script label or such an indexed label is allowed,
except for the objects `Polygon` creates:

A line `p = Polygon(...)` also creates the sides of the polygon and, for
`Polygon(A, B, n)`, its new vertices. They are recorded under the labels
GeoGebra gives them (for example `c`, `a`, `b` for `Polygon(A, B, C)`, or
`C`, `D`, `f`, `g`, `h`, `i` for `Polygon(A, B, 4)`). A key that is not a
script label is also allowed when it is one of the labels a `Polygon(...)`
line of the script can give, with the right type:

- `Polygon(A, B, n)`, where `A` and `B` are recorded as points and `n` is
  a number literal or recorded as a number: points labelled from `A`, ...,
  `W`, `Z`, and segments labelled from `f`, `g`, `h`, `i`, `j`, `k`, `l`,
  `m`, `n`, `p`, `q`, `r`, `s`, `t`, `a`, `b`, `c`, `d`, `e`;
- any other `Polygon(...)` with two or more arguments: segments labelled
  after a vertex, the vertex label in lower case; for a one-character
  vertex label also indexed, and for a longer one also the segment labels
  above.

"Labelled from" a letter means the letter itself, or the letter followed
by `_<n>` or `_{<n>}` with `n` a positive integer without leading zeros.
The comparison then checks that `geo-kernel` gave the object the same
label.

Every Object has `type` and `defined`. When `defined` is `false` the object
carries no numeric fields. When it is `true` it carries exactly these:

| `type` | Fields | Meaning |
| --- | --- | --- |
| `point` | `x`, `y` | Cartesian coordinates. |
| `line` | `a`, `b`, `c` | Raw GeoGebra coefficients of `a*x + b*y + c = 0`, not normalised. |
| `segment` | `x1`, `y1`, `x2`, `y2` | Start point and end point, in that order. |
| `circle` | `cx`, `cy`, `r` | Centre and radius. |
| `number` | `value` | The numeric value. |
| `angle` | `value` | The angle in radians, recorded in schema 2. |
| `vector` | `x`, `y` | The vector's coordinates. Where GeoGebra draws it from (its start point) is not recorded. |
| `ray` | `x`, `y`, `a`, `b`, `c` | Start point `(x, y)` and the raw GeoGebra coefficients of the line `a*x + b*y + c = 0` it lies on, not normalised. The ray points along `(b, -a)`. |
| `boolean` | `value` | `true` or `false`, a JSON boolean. `defined` is `false` only for the prover-only "unknown" state. |
| `function` | `samples` | The values at `x` = -8, -4, -2, -1, -0.5, 0, 0.5, 1, 2, 4, 8, as pairs `[x, y]` in this order: `{ "type": "function", "defined": true, "samples": [[-8.0, 82.0], [-4.0, 26.0], ..., [0.5, null], ...] }`. `y` is `null` where GeoGebra's value (`GeoFunction.value`) is not finite. An undefined function carries no `samples`. |
| `polygon` | `vertices`, `area`, `signed_area` | `vertices` is the list of vertices in order, each `[x, y]`. `area` is GeoGebra's area (never negative), `signed_area` the area with its sign: positive when the vertices run counter-clockwise. |
| `polyline` | `vertices`, `length` | `vertices` is the list of vertices in order, each `[x, y]`, or `null` for an undefined point (GeoGebra leaves a hole there). `length` is the sum of the lengths of the pieces between two defined points. |
| `list` | `elements` | The elements in order, each an Object of its own (`number`, `angle`, `point`, `vector`, `boolean` or `list`, defined or not): `{ "type": "list", "defined": true, "elements": [{ "type": "number", "defined": true, "value": 1.0 }, { "type": "list", "defined": true, "elements": [] }] }`. An element that is a point or vector carries `x` and `y` like an object. |
| `conic` | `conic_type`, `matrix`, and the parts of the type (below) | A general conic: an object made by `Conic`, `Ellipse`, `Hyperbola` or `Parabola`, or the image of one under a transformation. Every other conic object of GeoGebra is a `circle`, an `arc` or a `sector`. |
| `locus` | `points` | The points of a locus made by `Locus(Q, P)` (`P` a point on a path) or `Locus(Q, a)` (`a` a slider), in order, each `[x, y, lineTo]`: `lineTo` is `true` when the point is joined to the one before it and `false` when it starts a new piece. `x` and `y` are both `null` for an undefined point, which GeoGebra may append at the end. See below for the view the points depend on. |
| `arc`, `sector` | `value`, `conic_type`, and further fields of the type (below) | A part of a circle or conic (GeoGebra's `GeoConicPart`), made by `CircularArc`, `CircumcircularArc`, `Semicircle` or `Arc` (an `arc`), or by `CircularSector`, `CircumcircularSector` or `Sector` (a `sector`). `value` is the length of an arc and the area of a sector. |

A `conic` carries `conic_type`, a string, and `matrix`, the six numbers
`[A00, A11, A22, A01, A02, A12]` of
`A00*x² + A11*y² + A22 + 2*A01*x*y + 2*A02*x + 2*A12*y = 0` raw as GeoGebra
holds them (`GeoConicND.getMatrix`): not normalised, and scaled by a power of
two when GeoGebra rescales huge or tiny coefficients. The other fields depend
on `conic_type`:

| `conic_type` | Further fields |
| --- | --- |
| `circle`, `ellipse`, `hyperbola` | `midpoint` `[x, y]`, `half_axes` `[a, b]` (first and second axis), `eigenvectors` `[[x, y], [x, y]]` (the directions of the first and second axis) |
| `parabola` | `vertex` `[x, y]`, `p` (the parameter), `eigenvectors` (the first points from the directrix to the focus) |
| `single_point` | `point` `[x, y]` |
| `intersecting_lines`, `parallel_lines`, `double_line` | `lines` `[[x, y, z], [x, y, z]]`, the two lines raw in GeoGebra's order |
| `empty` | none |

An `arc` or `sector` carries `value` and `conic_type`, the type of the conic
it lies on, named as for a `conic`. The further fields depend on that type:

| `conic_type` | Further fields |
| --- | --- |
| `circle`, `ellipse` | `orientation` (`"positive"` or `"negative"`), `start` and `end` (the parameters where the part starts and ends, each in `[0, 2 pi]`, as GeoGebra keeps them: swapped for a negative orientation), `arc_length`, `area` (GeoGebra's `getArcLength` and `getArea`: for an arc of a circle the area between the arc and its chord), `midpoint`, `half_axes` and `eigenvectors` of the conic as for a `conic` |
| `parallel_lines` | `orientation`: a circumcircular part of three collinear points, which is the segment between the first and the last point (positive) or the two rays from them away from each other (negative) |
| `single_point` | none: a circumcircular part of three equal points, with `value` 0 |

GeoGebra keeps the start and end parameters, the arc length, the area and,
for a single point, the orientation of a part of any other type from an
earlier update, so they depend on the history of the construction and are
not recorded.

GeoGebra samples a locus in steps it measures in pixels of the graphics
view: a step is refined while the next point is more than 5 pixels away, a
point is joined to the one before it only when it is close, and the area a
bit larger than the view decides which steps far outside it are refined. So
the points depend on the window of the view. The harness records them in the
window of the headless application, which it checks whenever it records a
locus: x from -4.3 to 11.700000000000001, y from -5.7 to 6.3, 50 pixels per
unit on both axes, only the first graphics view shown. The desktop app supplies
its current sampling window. A locus is recomputed
when its path or a free object its point depends on changes, but not when
the point on the path moves (GeoGebra leaves that point out of the inputs it
updates from). A slider is a script line `a = Slider(min, max)` or
`a = Slider(min, max, increment)` with constant number expressions: a free
number that starts at 0 clamped to `[min, max]`. A one-value interval fixes
the number at that value. Reversed bounds give an initially undefined
number and cannot drive a locus.
GeoGebra also stops sampling a locus when one update of the construction
takes longer than 500 ms, which makes the points depend on the machine; no
case comes near it, and `geo-kernel` has no such limit.

All numbers are finite JSON numbers that round-trip to the same `f64`.
A value that is not finite in GeoGebra is recorded as `defined: false`.
This includes a point at infinity and a circle whose centre is at infinity,
both of which GeoGebra itself reports as defined. A polygon with a vertex at
infinity is recorded as undefined; in a polyline such a vertex is `null`, and
its length is not finite, so the polyline is undefined.

## Comparison rules

These rules are applied by the Rust side (`crates/geo-golden`).

- Two numbers `p` and `q` agree when `|p - q| <= 1e-9 * max(1, |p|, |q|)`.
- An object that is undefined in GeoGebra must be undefined in `geo-kernel`,
  and the other way round.
- The object types must be equal.
- Lines: both sides are normalised so that `(a, b)` has length 1, then the
  triples must agree up to a common sign. Whether the sign agrees is counted
  and reported separately; it does not fail the comparison.
- Lines and rays, scale: whether the kernel holds GeoGebra's raw
  coefficients exactly (every component within `1e-9` times the largest
  component magnitude of the two triples, same sign) is also counted for the
  `ggb-harness` cases and reported per group and defining command; it does
  not fail the comparison.
- Functions: every sample is compared like a number; a `null` sample must
  meet a value that is not finite in `geo-kernel`, and a finite value a
  finite one.
- Segments are compared with their end points in order.
- Vectors are compared by `x` and `y`.
- Booleans are compared exactly.
- Rays: the start points must agree, and the coefficients are normalised like
  those of a line, but the sign must agree too: a ray with the opposite sign
  points the other way and fails the comparison. Rays are not counted in the
  line signs.
- Polygons: `area`, `signed_area` and the vertices
  (`Construction::vertices`) must agree, in the same order and number.
- Polylines: `length` and the vertices must agree, in the same order and
  number, with `null` on both sides at the same places.
- Arcs and sectors: `value` must agree with the kernel's value, the
  conic type and the orientation must be equal, and the further numbers of a
  part of a circle or an ellipse must agree with the kernel's record
  (`Construction::arc`), raw.
- Loci: the number of points must be equal, and each point must agree with
  the kernel's (`Construction::locus`) like two numbers per coordinate, with
  the same `lineTo`; a `null` point must meet coordinates that are not
  finite.
- Lists: the number of elements must be equal, and each element is
  compared like an object of its type, nested lists recursively
  (`Construction::elements`).
- Conics: `conic_type` must be equal, and every number of `matrix` and of
  the parts must agree, raw: the matrix with GeoGebra's scale, the
  eigenvectors with their signs and the two lines in GeoGebra's order.
  Whether the matrices also agree up to a common factor is counted and
  reported separately; it does not decide the comparison.

Free vectors come from a script line `v = (x, y)` whose label starts with a
lower-case letter; with an upper-case label the same line makes a point, as in
GeoGebra.

The type `boolean` (the predicate commands) was added the same way.

The types `vector` and `ray`, the moving of a free vector and these rules were
added after the first golden files were written. The addition is backwards
compatible: no existing file changes and the schema version stays 1.
The same holds for the types `polygon` and `polyline`, the lists in their
fields and the labels of the objects `Polygon` creates, added later still.

Script lines may also be arithmetic expressions (group `expr`), such as
`c = a + 2*b`, `d = Distance(A, B) / 2` or `M = (A + B) / 2`. An expression
whose result is a coordinate pair is a vector when its label starts with a
lower-case letter and a point otherwise, whatever its operands are, as in
GeoGebra. An expression that reads no object (`c = 2 + 3`) is evaluated once
and becomes a free number, point or vector, which steps may set or move.
A line `f(x) = ...` defines a function of `x`, recorded under the label `f`;
other lines may apply it (`v = f(2)`, `g(x) = f(x) + 1`).
Expressions and the type `function` were added after the first golden files
were written; no existing file changed and the schema version stays 1.
The type `conic` was added the same way, later still: no existing file
changed and the schema version stays 1. So were the types `arc` and
`sector` and the `move` of a point on a path, later again.

Expressions may also compare and combine booleans and pick a branch with
`If` (group `logic`), and build lists (group `lists`): literals such as
`l = {1, 2, 3}` or `{A, B}`, element access `l(n)` and `Element(l, n)`,
list commands such as `Length`, `Sum`, `Sort`, `Join` and `Sequence`, and
element-wise arithmetic. A constant boolean
or list is an object like any other but cannot be set. The type `list` was
added in schema 1.

The type `locus` and the `Slider` line were added the same way, in schema
2: no existing file changed and the schema version stays 2.

Schema 2 distinguishes `angle` from `number`, including inside lists. Only
schema 1 permits an expected number to match an angle in the kernel.

Script lines may also be equations in `x` and `y` (group `equations`),
written `label: lhs = rhs` as GeoGebra's input bar reads them: a linear
equation makes a `line` (the raw coefficients of its normal form `lhs -
rhs`), a quadratic one a `conic` (recorded like every general conic), and
`y = <rhs in x>` of any other kind a `function`. An equation whose
coefficients read objects is a dependent object; the steps change the
numbers it reads and move the points it reads. Equations and these forms
were added the same way: no existing file changed and the schema version
stays 1.

## Known gaps

A case that `geo-kernel` cannot reproduce yet is listed in
`crates/geo-golden/known-gaps.toml` with a reason and the exact objects that
disagree, as `step:label`. The test fails when a listed case disagrees
anywhere else or starts to agree, so an entry can neither hide a new
difference nor go stale.

## Stored construction files (`golden/ggb/`)

For every case of the groups written by `ggb-harness`, `golden/ggb/<group>/<case id>.xml`
holds the construction file GeoGebra itself writes after the script has run, that is,
the text `app.getXML()` returns, which is the content of `geogebra.xml` in a saved
`.ggb` file. It is the state of `snapshots[0]` of the same case, so the importer
(`crates/geo-import`) can be checked against the golden values. About every 50th case
also has a `<case id>.ggb`, the zip archive of GeoGebra's own writer without a thumbnail
(`geogebra.xml`, `geogebra_defaults2d.xml`, `geogebra_javascript.js`), for the zip path.
Nothing in these files is edited after GeoGebra has written it;
`tools/ggb-harness/harness.sh ggb` rewrites them. The XML is byte-identical between runs;
the `.ggb` files differ only in the modification times of their zip entries.

The header carries no path, user name or host name: `<geogebra format="5.0"
version=... app="classic" platform="a" ...>` without an `id`, and
`<construction title="" author="" date="">` with empty fields.

What the importer reads, and where GeoGebra writes it (`MyXMLHandler`,
`GeoElement.getXML`, `AlgoElement.getXML`):

- A free point, vector or number is written as `<expression label= exp=>` followed by
  its `<element>`. The `exp` text is rounded to the decimals of the file; the exact value
  is in the element: `<coords x= y= z=>` (homogeneous, `z = 0` for a vector and for a
  point at infinity) or `<value val=>`. A point and a vector differ by the element
  `type`, not by the label.
- A dependent object is `<command name=><input a0= a1=/><output a0= a1=/></command>`
  followed by one `<element>` per labelled output. An output without a label (an
  object that was undefined when it was created) has an empty name and no element.
- The images of the end points of a transformed segment are separate commands
  followed by `Segment`, not one `Mirror` of the segment.
- `AlgoDistanceLineLine.setInputOutput` writes `input[0] = h; input[1] = g`, the
  reverse of the order it computes in. `GeoLine.distance(GeoLine)` is 0 for lines
  that are not parallel and treats two lines as parallel when
  `isZero(g.x * y - g.y * x)` on the raw coefficients, so the order matters only
  for lines whose coefficients are about 1e-6 (a line through two points 1e-6
  apart): a point of `g` is chosen and the distance is measured with the
  normalisation of `this`. Reading the file back evaluates the saved order, so
  GeoGebra computes another value than before saving for these lines (see
  `crates/geo-import/known-import-gaps.toml`, which pins all five values).
- `Intersect` of a line and a conic, or of two conics, is saved without an index
  and lists both points as outputs (`a0`, `a1`); the importer maps output `n` to
  `Intersect(a, b, n + 1)`. `Polygon` lists the polygon, its sides and (for
  `Polygon(A, B, n)`) the new vertices, in an order other than the one the
  kernel creates them in; the importer matches them by label.
- The same files after the free points were moved are in `golden/ggb/saved/`:
  `<id>.xml` and `<id>.snapshot` (the golden field format of every labelled
  object, taken in the same state; extension `.snapshot` so the golden loader
  skips it). They come from `tools/ggb-harness/saved/saved.json`
  (`harness.sh saved`).
- Several `<construction>` sections: GeoGebra's `MyXMLHandler` appends the
  elements of each, and so does the importer (with a note).
- An angle command's inputs are objects; numbers in command inputs are written
  as text (`30°`) or plain radians whatever the `<angleUnit>`, which only
  affects display, so the unit does not change what the importer computes
  (`saved/` has cases under both units).
- `<fixed>`, `<auxiliary>`, `<condition>` and a vector's `<startPoint>` are
  counted and reported as notes; no value depends on them.
- `<kernel>`: `<continuous val=>`, `<decimals val=>`, `<angleUnit val=>`.
- `<show object=>` of an element: `false` for an object that is hidden.
