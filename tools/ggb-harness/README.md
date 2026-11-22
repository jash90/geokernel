# ggb-harness

Runs GeoGebra headlessly to produce the golden data in `golden/` (format:
`golden/SCHEMA.md`) and to time GeoGebra on the chain that
`geokernel bench` builds. The regular Rust tests use the checked-in golden
corpus. The export oracle also runs the Java harness live.

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
python3 tools/ggb-harness/cases/make_angle.py  # rewrite cases/angle.json
python3 tools/ggb-harness/cases/make_expr.py    # rewrite cases/expr.json
python3 tools/ggb-harness/cases/make_logic.py   # rewrite cases/logic.json
python3 tools/ggb-harness/cases/make_lists.py   # rewrite cases/lists.json
python3 tools/ggb-harness/cases/make_equations.py  # rewrite cases/equations.json
python3 -I tools/extract-t-pairs/extract_t_pairs.py \
    --source "$GEOGEBRA_SRC" --out golden/t-pairs   # re-extract the upstream pairs
tools/ggb-harness/harness.sh tpairs   # replay them in GeoGebra, rewrite golden/t-pairs/verified.jsonl
```

`build` runs `gradlew -p source/shared :common-jre:ggbHarnessClasspath` with
the init script `gradle/classpath.init.gradle`. The script adds that one task,
which builds `common-jre` and its test fixtures and writes their runtime class
path to `build/classpath.txt` (ignored by git, it holds local paths). The
harness is then compiled with plain `javac` into `build/classes`.

`golden` stamps every file with the commit that `git rev-parse HEAD` reports
for the checkout. Two runs produce byte-identical files.

## Export oracle

`harness.sh export-snapshot <directory>` opens each `.ggb` in a fresh headless
app and writes a `.ggb.snapshot` alongside it. GeoGebra warnings, errors and
unrecognized XML tags fail the run. The integration test exports constructions
replayed from harness goldens, including `cases/export.json`, and exports
reference-saved text, raster, control and path fixtures. It compares every
labelled object against both Rust constructions at the golden tolerance and
checks object styles, slider settings and the primary view directly in the
Java reader.

After building the harness, run the ignored
`geogebra_reloads_exports_and_matches_the_kernel` test in
`geo-golden`'s `export` integration suite with `GEOGEBRA_SRC` set.
The `check` CI job runs this test with the pinned reference revision.

## How a case is replayed

`cases/<group>.json` holds the inputs of one group: `id`, `group`, `script`,
`steps`. It is written by `cases/make_<group>.py` (`make_cases.py` for the
group `geometry`) with the helpers in `cases/caselib.py`: fixed seed, fixed
order, one case per line. Ids have the form `<group>-<topic>-<nnn>`; the
cases of one topic go to `golden/<group>/<topic>.json`. New cases must be appended to a topic so
existing ids keep their meaning.

Cases whose scripts use a command the kernel cannot run yet live in
`cases/reference/`. The golden runner includes them, and the Rust golden
test checks their exact failures against `known-gaps.toml`. Use a separate
topic from the runnable cases, since each topic has one output file.

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
   polygon must be defined when it is created. A fixed `Angle(A, B, n)`
   records the angle and the rotated point under its own label.
   `Angle(a)` of an existing angle returns the same object; the harness
   records it once under its original label. `a = Slider(min, max)` must
   make a free number. Whenever a line makes a locus, the harness checks
   that GeoGebra samples it in the window of the headless application
   (`Session.LOCUS_VIEW`: x from -4.3 to 11.700000000000001, y from -5.7 to
   6.3, 50 pixels per unit, no second graphics view and no 3D view, not
   continuous), as the points depend on it;
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
| `GeoAngle` | `angle` with a radian `value` |
| `GeoVector` | `vector` with `x`, `y` |
| `GeoRay` | `ray` with the inhomogeneous start point `x`, `y` and the raw fields `x`, `y`, `z` as `a`, `b`, `c`; a start point at infinity is undefined |
| `GeoFunction` | `function` with `samples`, the values of `GeoFunction.value` at the sample points (`null` where not finite) |
| `GeoPolygon` | `polygon` with `getPoints()` as `vertices`, `getArea()` as `area` and `getAreaWithSign()` as `signed_area`; a vertex that is undefined or at infinity makes it undefined |
| `GeoPolyLine` | `polyline` with `getPointsND()` as `vertices` (`null` for an undefined point or a point at infinity) and `getLength()` as `length` |
| `GeoLocus` | `locus` with `getPoints()` as `points`, each `[x, y, getLineTo()]` (`x` and `y` `null` when not finite) |

New files use schema 2, which records angles separately in every group.
Schema 1 files use the legacy numeric encoding for angles.

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
- Arithmetic expressions over these objects (`c = a + 2*b`,
  `d = Distance(A, B) / 2`, `M = (A + B) / 2`, `k = x(A) + sin(a)`). A line
  that is not a literal or a single call of a known command is an expression:
  the harness accepts a number, a point or a vector from it (a point or a
  vector by the case of the label) and checks that it is free exactly when it
  names no scripted object. A line `f(x) = ...` must make a function; it is
  recorded with its values at the sample points of `golden/SCHEMA.md`
  (`GeoFunction.value`). A command inside the arguments of another command
  (`Slope(Line(A, B))`) is accepted by GeoGebra but not by `geo-kernel`.
- Points have upper-case labels, everything else lower case. A coordinate
  pair with a lower-case label is a free vector (GeoGebra's rule), so free
  points must have upper-case labels. Labels avoid
  `e`, `i`, `x`, `y`, `z` and letter-digit names such as `A1` (spreadsheet
  cells).
- GeoGebra gives an object that is undefined when it is created no label until
  it becomes defined, so no line may refer to such an object. The harness
  keeps a reference to it and records it like any other object.

## Test-driven development with the upstream tests

`golden/t-pairs/` holds the input/expected pairs of GeoGebra's own kernel
tests (`CommandsTest`, `ArithmeticTest`, `RedefineTest`, ...), and
`verified.jsonl` records what GeoGebra actually returned for every step when
`tools/ggb-harness/harness.sh tpairs` replayed the methods. To refresh
records of one snapshot type at the same upstream commit, pass the type as an
argument, for example `tools/ggb-harness/harness.sh tpairs text`.
The Rust runner
`crates/geo-golden/tests/tpairs.rs` replays the same steps on `geo-kernel`
and compares the values and the strings against that file. Steps the kernel
cannot pass yet are listed in `crates/geo-golden/t-pairs-gaps.toml`.

Before implementing a command or fixing one:

```sh
# the red pairs of that command, with the reason of each:
GEOKERNEL_TPAIRS_COMMAND=Midpoint cargo test -p geo-golden --test tpairs -- --nocapture
# implement, then watch the pairs leave the gaps list:
cargo test -p geo-golden --test tpairs
```

A gap entry whose step starts passing fails the test as stale: delete it.
After a change that is expected to move pairs, regenerate the list with
`GEOKERNEL_TPAIRS_BLESS=1 cargo test -p geo-golden --test tpairs` (the run
itself stays red; the next plain run must be green) and review the diff —
the reason strings are generated, the categories may need a human eye.

The upstream tests sometimes use the old input syntax (`Length[f]`,
`label:input`); the runner rewrites those forms before executing, exactly as
GeoGebra's parser accepts both. What it cannot repair it reports: an
incomplete context (the method moved a point or changed a setting between
the steps), a redefine (the step assigns to an existing label), or an
inline equation as an argument (`Intersect(x = 4, y = 4)`).

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

A partial run (a sample step above 1) never deletes a file. A full run (sample
step 1) removes the files of the group directories it did not produce (for
example the archives of another zip step) and prints each one as `pruned <path>`.

### Files saved after moves

```sh
tools/ggb-harness/harness.sh saved
tools/ggb-harness/harness.sh saved inline controls elements
python3 tools/ggb-harness/saved/make_saved.py   # rewrite saved/saved.json
```

`SavedExporter` runs the raw GeoGebra input lines of `saved/saved.json` through the
algebra processor (so `P = Intersect(f, k)` gives both points), applies the
`move` and other edit steps, then writes the construction file
(`golden/ggb/saved/<id>.xml` or `<id>.ggb` when `archive` is true) and what GeoGebra computes in that state
(`<id>.snapshot`, golden field format, not read by the golden loader). A case
may set `angle_unit` (`degree` or `radiant`). `crates/geo-import/tests/saved.rs`
imports each file and compares it with its snapshot: this covers, for exactly the states the cases reach, that the
n-th output slot of a saved `Intersect` or `Tangent` is the n-th object of the
kernel after points were moved, that angles do not depend on the angle unit,
and that imported texts match before and after a point move and a number edit.

### Reading files back

```sh
tools/ggb-harness/harness.sh reload <file.xml> <label>...
```

loads a saved file into a fresh GeoGebra (`getXMLio().processXMLString`) and prints
the numbers it computes for the labels, as `<file> <label> = <value>`. It is how
the entries of `crates/geo-import/known-import-gaps.toml` were checked: for
`conic-diameter-009.xml` and `d` it prints `1.7888543819998317`, the value the
importer gives, not the `0.8944271909999159` that GeoGebra computed before saving.

