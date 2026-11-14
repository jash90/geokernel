# geokernel

A native dynamic-geometry application written in Rust, built for speed.
GeoGebra is the reference for features and the source of algorithms; it is
not a target for one-to-one compatibility. `geokernel` is a working name.

This project is not affiliated with or endorsed by GeoGebra GmbH. Importing
GeoGebra files is planned and not implemented yet. The name and logo GeoGebra
belong to GeoGebra GmbH.

## Status

Early. The kernel covers points, lines, segments, circles and numbers with
13 kinds of definitions, incremental updates, a small script language and
SVG output, plus a desktop window in which free points can be dragged.
See `docs/command-matrix.csv` for what is planned and done, and
`crates/geo-golden/known-gaps.toml` for where the kernel still differs
from GeoGebra.

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
| `crates/geo-app` | The `geokernel-app` desktop window (egui on wgpu): draws a construction and lets you drag its free points. |
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
cargo run --release -p geo-cli -- bench           # 105 thousand objects, dragging a point
cargo run --release -p geo-app                    # window with the triangle; drag the blue points
```

## Script syntax

```
A = (0, 0)              # free point
r = 3                   # free number (a slider)
l = Line(A, B)
s = Segment(A, B)
M = Midpoint(A, B)
p = Perpendicular(M, s)
q = Parallel(M, l)
u = PerpendicularLine(M, s)  # GeoGebra name for Perpendicular
w = Line(M, l)          # GeoGebra form: parallel to a line or segment
k = Circle(A, B)        # centre A, through B
m = Circle(A, r)        # centre A, radius r
P = Intersect(l, k, 2)  # second intersection point (the first by default)
d = Distance(A, B)
```

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
