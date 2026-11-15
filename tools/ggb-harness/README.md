# ggb-harness

Runs GeoGebra headlessly to produce the golden data in `golden/` (format:
`golden/SCHEMA.md`) and to time GeoGebra on the chain that
`geokernel bench` builds. Nothing here is needed to build or test the Rust
crates.

The harness only calls GeoGebra's API (classes under the EUPL 1.2 header).
It never copies GeoGebra code, jars, `.properties` files or artwork into this
repository; the GeoGebra checkout stays outside it and is never modified.

## Requirements

- A GeoGebra source checkout (the golden data was recorded at commit
  `112919f2d450ec7430a5cd3b65dba226fdc43b74`).
- JDK 17 (GeoGebra's Gradle toolchain asks for 17). Gradle itself is
  downloaded by GeoGebra's wrapper.
- Python 3, only to regenerate the case inputs.

## Commands

Run from the repository root; `GEOGEBRA_SRC` points at the checkout.

```sh
export GEOGEBRA_SRC=<path to the GeoGebra checkout>
tools/ggb-harness/harness.sh build    # build GeoGebra's common-jre, compile the harness
tools/ggb-harness/harness.sh golden   # rewrite golden/<group>/*.json for every group
tools/ggb-harness/harness.sh golden lines   # rewrite one group only
tools/ggb-harness/harness.sh chain    # time the bench() chain (optional: number of steps)
python3 tools/ggb-harness/cases/make_cases.py   # rewrite cases/geometry.json
python3 tools/ggb-harness/cases/make_lines.py   # rewrite cases/lines.json (one script per group)
```

`build` runs `gradlew -p source/shared :common-jre:ggbHarnessClasspath` with
the init script `gradle/classpath.init.gradle`. The script adds that one task,
which builds `common-jre` and its test fixtures and writes their runtime class
path to `build/classpath.txt` (ignored by git, it holds local paths). The
harness is then compiled with plain `javac` into `build/classes`.

`golden` stamps every file with the commit that `git rev-parse HEAD` reports
for the checkout. Two runs produce byte-identical files.

## How a case is replayed

`cases/<group>.json` holds the inputs of one group: `id`, `group`, `script`,
`steps`. It is written by `cases/make_<group>.py` (`make_cases.py` for the
group `geometry`) with the helpers in `cases/caselib.py`: fixed seed, fixed
order, one case per line. Ids have the form `<group>-<topic>-<nnn>`; the
cases of one topic go to `golden/<group>/<topic>.json`. New cases must be appended to a topic so
existing ids keep their meaning.

For every case `GoldenGenerator`:

1. creates a fresh headless app: `AppCommon` with `LocalizationCommon`,
   `AwtFactoryCommon` (from GeoGebra's test fixtures) and the Classic
   configuration `AppConfigDefault`;
2. runs each script line with
   `AlgebraProcessor.processAlgebraCommandNoExceptionHandling` and an error
   handler that throws; it checks that each line creates exactly one object
   with the requested label and the expected class, and that free points and
   numbers are independent. A command with several outputs (`Tangent(P, c)`,
   `AngularBisector(g, h)`) is recorded under the labels GeoGebra gives its
   outputs: `t_{1}`, `t_{2}` when two or more are defined at creation, the
   plain label when only one is; unlabelled outputs are not recorded (see
   `golden/SCHEMA.md`). A line where no output is defined at creation fails,
   because GeoGebra labels those outputs only later. `Vertex(p)`,
   `InteriorAngles(p)` and `Angle(p)` of a polygon are recorded the same way
   (`V_{1}`, `V_{2}`, ...). `p = Polygon(...)` must create the polygon `p`;
   its sides and the new vertices of a regular polygon are recorded under the
   labels GeoGebra gives them, and every one of them must be labelled, so the
   polygon must be defined when it is created;
3. records a snapshot, then applies each step and records a snapshot after it.
   `move` calls `GeoPoint.setCoords(x, y, 1)` (for a free vector
   `GeoVector.setCoords(x, y, 0)`) and `updateCascade()`;
   `set_number` calls `GeoNumeric.setValue` (it fails if GeoGebra clamps the
   value) and `updateCascade()`.

Snapshot encoding:

| GeoGebra object | Recorded as |
| --- | --- |
| `GeoPoint` | `point` with the inhomogeneous `x`, `y`; a point at infinity is undefined |
| `GeoLine` | `line` with the raw fields `x`, `y`, `z` as `a`, `b`, `c` |
| `GeoSegment` | `segment` with its start point and end point |
| `GeoConic` from `Circle` | `circle` with the midpoint and the first half-axis |
| `GeoNumeric` | `number` |
| `GeoVector` | `vector` with `x`, `y` |
| `GeoRay` | `ray` with the inhomogeneous start point `x`, `y` and the raw fields `x`, `y`, `z` as `a`, `b`, `c`; a start point at infinity is undefined |
| `GeoPolygon` | `polygon` with `getPoints()` as `vertices`, `getArea()` as `area` and `getAreaWithSign()` as `signed_area`; a vertex that is undefined or at infinity makes it undefined |
| `GeoPolyLine` | `polyline` with `getPointsND()` as `vertices` (`null` for an undefined point or a point at infinity) and `getLength()` as `length` |

`defined` is GeoGebra's `isDefined()` and, in addition, every recorded field
must be finite. A circle of radius 0 is a single-point conic in GeoGebra; it is
recorded as a defined `circle` with `r` 0 (GeoGebra's own midpoint and
half-axis). A circle around an infinite centre becomes a pair of parallel
lines with infinite half-axes and is therefore recorded as undefined. Any other
conic type stops the run.

Intersections are not continuous (`Kernel.isContinuous()` is false in this
configuration), so the harness records whatever GeoGebra's fixed permutation
gives.

## Rules for case scripts

- Only the constructs `geo-kernel` supports: free points `A = (x, y)`, free
  numbers, `Line(A, B)`, `Segment`, `Midpoint`, `Distance`,
  `PerpendicularLine(P, l)`, parallel `Line(P, l)`, `Circle(A, B)`,
  `Circle(A, r)` with `r` a number object, and `Intersect` of two lines, a
  line and a circle with an index, or two circles with an index.
- Points have upper-case labels, everything else lower case. A coordinate
  pair with a lower-case label is a free vector (GeoGebra's rule), so free
  points must have upper-case labels. Labels avoid
  `e`, `i`, `x`, `y`, `z` and letter-digit names such as `A1` (spreadsheet
  cells).
- GeoGebra gives an object that is undefined when it is created no label until
  it becomes defined, so no line may refer to such an object. The harness
  keeps a reference to it and records it like any other object.

## Chain timing

`ChainTiming` builds the same chain as `bench()` in `crates/geo-cli/src/main.rs`
(free points `A`, `B`; per step a line, a circle, a perpendicular, two
intersections, a midpoint and a line) with the views detached, checks that no
chain object is undefined, drags `A` 200 times as a warm-up and then three
timed rounds of 200 drags along the same path, and finally drags an unrelated
point 100,000 times. The argument is the number of steps (default 15,000, as in
the Rust bench). GeoGebra's memory grows with the square of the chain length,
so the default does not fit in memory; see `docs/perf/geogebra-chain.md`, where
the results are written up by hand.

## Construction files

```sh
tools/ggb-harness/harness.sh ggb [sample step [zip step]]
```

writes, for every case of every `cases/<group>.json`, the construction file
GeoGebra itself produces after the script (`app.getXML()`, the content of
`geogebra.xml` in a saved file) to `golden/ggb/<group>/<case id>.xml`, and every
`zip step`-th case (default 50) also as the `.ggb` archive of GeoGebra's own
writer, without a thumbnail. `sample step` keeps every n-th case of each group
(default 1). The class is `GgbExporter`. It uses only `Kernel.getApplication()`
of a `Session`. Nothing is edited after GeoGebra has written it; the XML is
byte-identical between runs, and an archive is rewritten only when its entries
change (the zip writer stamps them with the current time). See
`golden/SCHEMA.md`, "Stored construction files".
