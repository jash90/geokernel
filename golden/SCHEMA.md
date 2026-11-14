# Golden data format (schema version 1)

Golden data records what GeoGebra computes for a script of commands and a
sequence of moves. The Rust tests replay the same script and moves on
`geo-kernel` and compare the results. No Java is needed to run the tests.

Golden values come only from running GeoGebra through `tools/ggb-harness`.
They are never written by hand and never produced by `geo-kernel` itself.
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
| `move` | `label`, `x`, `y` | Move the free point `label` to `(x, y)`. |
| `set_number` | `label`, `value` | Set the free number `label` to `value`. |

## Snapshot

A JSON object that maps every label created by the script to an Object.

Every Object has `type` and `defined`. When `defined` is `false` the object
carries no numeric fields. When it is `true` it carries exactly these:

| `type` | Fields | Meaning |
| --- | --- | --- |
| `point` | `x`, `y` | Cartesian coordinates. |
| `line` | `a`, `b`, `c` | Raw GeoGebra coefficients of `a*x + b*y + c = 0`, not normalised. |
| `segment` | `x1`, `y1`, `x2`, `y2` | Start point and end point, in that order. |
| `circle` | `cx`, `cy`, `r` | Centre and radius. |
| `number` | `value` | The numeric value. |

All numbers are finite JSON numbers that round-trip to the same `f64`.
A value that is not finite in GeoGebra is recorded as `defined: false`.

## Comparison rules

These rules are applied by the Rust side (`crates/geo-golden`).

- Two numbers `p` and `q` agree when `|p - q| <= 1e-9 * max(1, |p|, |q|)`.
- An object that is undefined in GeoGebra must be undefined in `geo-kernel`,
  and the other way round.
- The object types must be equal.
- Lines: both sides are normalised so that `(a, b)` has length 1, then the
  triples must agree up to a common sign. Whether the sign agrees is counted
  and reported separately; it does not fail the comparison.
- Segments are compared with their end points in order.

## Known gaps

A case that `geo-kernel` cannot reproduce yet is listed in
`crates/geo-golden/known-gaps.toml` with a reason. The test fails when a
listed case starts to agree, so the list cannot go stale.
