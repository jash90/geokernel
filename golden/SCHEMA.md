# Golden data format (schema version 1)

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
  "schema": 1,
  "generator": "ggb-harness",
  "upstream_commit": "112919f2d450ec7430a5cd3b65dba226fdc43b74",
  "cases": []
}
```

| Field | Type | Meaning |
| --- | --- | --- |
| `schema` | integer | Always `1` for this version. |
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
| `script` | array of string | GeoGebra input-bar lines with English command names, run in order. Every line has the form `label = definition`, so every object has an explicit label. |
| `steps` | array of Step | The first step is always `initial`. |
| `snapshots` | array of Snapshot | Same length as `steps`. `snapshots[i]` is the state after `steps[i]`. |

## Step

| `action` | Other fields | Meaning |
| --- | --- | --- |
| `initial` | none | The state right after the script has run. |
| `move` | `label`, `x`, `y` | Move the free point `label` to `(x, y)`, or set the free vector `label` to `(x, y)`. |
| `set_number` | `label`, `value` | Set the free number `label` to `value`. |

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
| `vector` | `x`, `y` | The vector's coordinates. Where GeoGebra draws it from (its start point) is not recorded. |
| `ray` | `x`, `y`, `a`, `b`, `c` | Start point `(x, y)` and the raw GeoGebra coefficients of the line `a*x + b*y + c = 0` it lies on, not normalised. The ray points along `(b, -a)`. |
| `boolean` | `value` | `true` or `false`, a JSON boolean. `defined` is `false` only for the prover-only "unknown" state. |
| `polygon` | `vertices`, `area`, `signed_area` | `vertices` is the list of vertices in order, each `[x, y]`. `area` is GeoGebra's area (never negative), `signed_area` the area with its sign: positive when the vertices run counter-clockwise. |
| `polyline` | `vertices`, `length` | `vertices` is the list of vertices in order, each `[x, y]`, or `null` for an undefined point (GeoGebra leaves a hole there). `length` is the sum of the lengths of the pieces between two defined points. |

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

Free vectors come from a script line `v = (x, y)` whose label starts with a
lower-case letter; with an upper-case label the same line makes a point, as in
GeoGebra.

The type `boolean` (the predicate commands) was added the same way.

The types `vector` and `ray`, the moving of a free vector and these rules were
added after the first golden files were written. The addition is backwards
compatible: no existing file changes and the schema version stays 1.
The same holds for the types `polygon` and `polyline`, the lists in their
fields and the labels of the objects `Polygon` creates, added later still.

## Known gaps

A case that `geo-kernel` cannot reproduce yet is listed in
`crates/geo-golden/known-gaps.toml` with a reason and the exact objects that
disagree, as `step:label`. The test fails when a listed case disagrees
anywhere else or starts to agree, so an entry can neither hide a new
difference nor go stale.
