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

### Where the data is recorded

`GoldenGenerator` runs GeoGebra's Java code, and two systems need not give
the same bits. HotSpot's intrinsics for `Math.exp`, `Math.pow`, `Math.sin`
and `Math.cos` give other last bits elsewhere (`Math.exp(1)` is
`2.718281828459045` on Linux x86_64 and `2.7182818284590455` on macOS
arm64). Record on Linux x86_64, where the committed data comes from. On
another system, run the harness in a container of that system:

```sh
tools/ggb-harness/harness.sh build
GEOGEBRA_SRC=<path to the GeoGebra checkout> tools/ggb-harness/linux-x86_64.sh golden lines
```

`linux-x86_64.sh` runs every command of `harness.sh` except `build` in a
Linux x86_64 container with JDK 17. It needs a container runtime that runs
x86_64 images: `GGB_HARNESS_RUNTIME` names it (default `docker`) and
`GGB_HARNESS_IMAGE` the image. `GRADLE_USER_HOME` is Gradle's directory
with the dependency jars.

After a run, check with `git diff` that the files whose outcome does not
change are byte-identical, and commit only the files the change is about.
A full run at the reference commit (November 2026) rewrote every file of
`golden/` byte for byte except:

- `axes/functions`, `calculus/degree` and `algebraedge/integer`, which are
  still in schema 2 and come out in schema 4 (`algebraedge/integer` also
  holds `Math.pow` results that HotSpot's intrinsics change by a unit in
  the last place);
- ten files whose numbers are printed with one digit more or less and parse
  to the same values (`cas/factor`, `cas/magnitude`, `calculus/coefficients`,
  `conics/transform`, `conicpaths/locus`, `expr/apply`, `infinity/outputs`,
  `locus/polygon`, `locus/segment`, `locusequation/boolean`);
- four that depend on the JVM: the order of identity-hashed sets and maps
  in `discrete/path`, `discrete/tree` and `discrete/upstream`, and the
  seeded search in `intersections/boundaryseed`.

The bounds of a text depend on the fonts of the system and are not
recorded this way.

### Selecting CAS topics

To record one or more CAS topics without rewriting other golden files, set
`GGB_HARNESS_TOPICS` to their comma-separated names before running the
Linux x86_64 script. The `cascell` group records one exact CAS view row per
case. The CAS-view command fixtures in `cas` use the same row field.

To regenerate just the CAS interaction recordings and the corresponding
construction files, run the case generator and select the group in the container:

```sh
python3 -I tools/ggb-harness/cases/make_casview.py
GEOGEBRA_SRC=<checkout> tools/ggb-harness/linux-x86_64.sh golden casview
GEOGEBRA_SRC=<checkout> tools/ggb-harness/linux-x86_64.sh ggb 1 50 casview
```

### Native CAS oracle

On macOS arm64 and Linux x86_64, `build` also downloads the published
`fr.ujf-grenoble:javagiac:70507` platform JAR from GeoGebra's Maven releases.
It verifies the pinned SHA-256 and caches the JAR and selected JNI library in
the ignored `build/giac` directory. This requires `curl`, `shasum`, and `unzip`.
The Linux artifact requires glibc 2.35 or newer. Use a JDK that matches the
machine architecture.

```sh
tools/ggb-harness/harness.sh cas-smoke
tools/ggb-harness/harness.sh golden cas
```

`cas-smoke` checks a prime command through the headless app's algebra processor.
The `cas`, `locusequation`, `envelope`, `curvepairs` and `odesymbolic` golden
groups, the `calculus` group's `degree` topic and the `funcanalysis` group's
`asymptote`, `discontinuity` and `curvatureupstream` topics select the native
backend automatically. The `odesymbolic` group's `nocas` topic uses the dummy
backend to verify behavior without symbolic evaluation. `GGB_HARNESS_CAS=1` selects
the native backend for other runs. The remaining historical cases default
to the dummy backend. `GoldenGenerator` takes a comma-separated list of topics
to record, or with a leading `!` to leave out, so one group can be recorded
with both backends.

`GGB_HARNESS_JAVA_OPTS` adds options to every Java run. With
`-DggbHarness.giacLog=<file>` the native backend appends one JSON line per
Giac exchange to that file: `translate` lines carry the GeoGebra command text
and the processed result, `raw` lines each string passed to Giac and what Giac
printed, all tagged with the case id. `crates/geo-giac` replays these
exchanges to check that the kernel's Giac binding gives the same output
(see `docs/cas-giac.md`).
The `prove` group also sets `-DggbHarness.prover`. The headless app's prover
never decides a statement: its `Prover.compute` is empty, so every `Prove` is
`false` and every `ProveDetails` is `{}`. `ProverOracle` installs a prover
that runs `Prover.decideStatement`, synchronously and without a timeout, as the
desktop app's prover does, with the algebraic engine selected. The numerical
engine and the external Wu prover of the desktop app are not on the harness
class path.
The desktop app abandons a proof after five seconds and pins Giac 70501; the
oracle has no deadline and uses the Giac library this script downloads.
Giac is a separate GPL reference backend used only to record and check fixtures.
The Rust workspace and release builds have no runtime or build dependency on it.

`golden` stamps every file with the commit that `git rev-parse HEAD` reports
for the checkout. Two runs produce byte-identical files.

## Export oracle

`harness.sh export-snapshot <directory>` opens each `.ggb` in a fresh headless
app and writes a `.ggb.snapshot` alongside it. GeoGebra warnings, errors and
unrecognized XML tags fail the run. The integration test exports constructions
replayed from harness goldens, including `cases/export.json`, and exports
reference-saved text, raster, control, path and style fixtures. It compares
every labelled object against both Rust constructions at the golden tolerance
and checks slider settings and the primary view directly in the Java reader.

The snapshot also holds the presentation of every object, read through
GeoGebra's getters (`styles`): visibility, view bits, condition, colour,
opacity, fill, dynamic colour, background, layer, drawing order, label,
caption, tooltip, line and point style, the fixed, auxiliary, selection, trace
and other flags, decoration, end and head styles, text font, rounding and
placement, angle arcs, slider placement and animation.

For a document opened from a file the test snapshots three archives: the
source file, the export of its import, and the export of the same document
without its saved styles. The first two must agree in every property of every
object. The third gives what the reader makes of an object without the saved
tag. This control archive restores the original baseline for label visibility
and thickness, which the journal now mirrors for editing. `WITNESSES` names
the fixture objects checked for each kind. The first object whose source
value differs from the control proves the property. The test compares the result with
`crates/geo-golden/tests/fixtures/export-proof.json` and fails when the file
is stale; `GEOKERNEL_EXPORT_BLESS=1` rewrites it. A normal `cargo test` reads
that file and derives the `export` column of `docs/kinds.csv` from it.

The export test calls `export-snapshot --element-tags` for the source and its
export. The snapshot adds the original element attributes and those the
reference writer emits after reload. Raster file entries are compared by
SHA-256 because the archive writer assigns new names. The `ElementTags` proof
records the source, the export and the reload for each witnessed inventory row.
Run the export test with `GEOKERNEL_EXPORT_CONTAINER=1` and
`GEOKERNEL_EXPORT_BLESS=1` to record the proof on Linux x86_64. The reference
files for the element properties come from `saved/make_saved.py` and the
`element-properties`, `element-properties-extra` and
`element-raster-properties` saved topics.

`export-snapshot --lenient <directory>` records what GeoGebra reports while
loading (`reported`) instead of stopping, and writes `{"failed": ...}` for a
file its reader gives up. The hand-written files of
`crates/geo-golden/tests/fixtures/export` are read this way: tags GeoGebra
does not write, such as a line style without a thickness or a fill whose angle
is no number. `<name>.applied.xml` beside a file the reader gives up is the
same file with that tag cut down to what the reader had applied by then; the
export must load without a report and agree with it.

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

An `append` step runs a new assignment after edits. The `undo` and `redo`
steps restore GeoGebra construction checkpoints, and `reload` reads the
current construction back from XML. An `anchor_text` step sets the start
point used by a saved text. The long polyline clip uses `observe` to record
the first and last segment by slot with reference label allocation suppressed.
The last slot still reflects the full reference output count.

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
   value) and `updateCascade()`. Use `infinity: 1` or `infinity: -1` in place
   of `value` to set an infinity. The `delete` step calls the object's removal
   operation and records the surviving labels.

Snapshot encoding:

| GeoGebra object | Recorded as |
| --- | --- |
| `GeoPoint` | `point` with the inhomogeneous `x`, `y`; a point at infinity is undefined |
| `GeoLine` | `line` with the raw fields `x`, `y`, `z` as `a`, `b`, `c` |
| `GeoSegment` | `segment` with its start point and end point |
| `GeoConic` from `Circle` | `circle` with the midpoint and the first half-axis |
| `GeoNumeric` | `number`; one that holds `Infinity` or `-Infinity` is defined and carries `infinity` (1 or -1) in place of `value` |
| `GeoAngle` | `angle` with a radian `value` |
| `GeoVector` | `vector` with `x`, `y` |
| `GeoRay` | `ray` with the inhomogeneous start point `x`, `y` and the raw fields `x`, `y`, `z` as `a`, `b`, `c`; a start point at infinity is undefined |
| `GeoFunction` | `function` with `samples`, the values of `GeoFunction.value` at the sample points (`null` for NaN, `{ "infinity": 1 }` or `-1` for an infinity) |
| `GeoPolygon` | `polygon` with `getPoints()` as `vertices`, `getArea()` as `area` and `getAreaWithSign()` as `signed_area`; a vertex that is undefined or at infinity makes it undefined |
| `GeoPolyLine` | `polyline` with `getPointsND()` as `vertices` (`null` for an undefined point or a point at infinity) and `getLength()` as `length` |
| `GeoLocus` | `locus` with `getPoints()` as `points`, each `[x, y, getLineTo()]` (a coordinate is `null` for NaN and `{ "infinity": 1 }` or `-1` for an infinity) |

New files use schema 4, which records angles separately in every group, an
infinite number as such and the sign of every infinite sample, pie fraction,
curve or locus coordinate, so that `null` there means NaN only
(`golden/SCHEMA.md`). The snapshots of the export oracle and the verified
t-pairs keep the schema 2 encoding, where an infinite number is written as
undefined and every value that is not finite as `null`.
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

### Toolbar pointer recordings

```sh
python3 -I tools/ggb-harness/cases/make_tool_pointer.py
GEOGEBRA_SRC=<checkout> tools/ggb-harness/linux-x86_64.sh tool-pointer
```

The recorder invokes the headless Euclidian controller with the pointer
positions in the two input files. Generated pointer inputs live under
`cases/pointer/`, outside the construction corpus globs. It records objects
after each release and dialog submission, records the dialog calls, and
invokes kernel undo to check the resulting checkpoints. Oversize-shape
witnesses record the total object count and a small set of observed points
at each checkpoint. Fixed Segment and Fixed Angle answers invoke
the reference's number dialog handlers. The output
`crates/geo-mcp/tests/fixtures/tool_pointer_capture.json` is separate from
the kernel golden corpus.

The older toolbar recording preserves the five original desktop sequences.
Its Create List drag produces no list in the controller. The desktop replay
still checks that gesture; Text and InputBox now use the captures below.

### Interaction parity recordings

```sh
python3 -I tools/ggb-harness/cases/make_parity_scenarios.py
GEOGEBRA_SRC=<checkout> tools/ggb-harness/linux-x86_64.sh golden parity-scenarios
```

This command writes only `crates/geo-mcp/tests/fixtures/parity_scenario_capture.json`.
Its header identifies the GeoGebra revision and the SHA-256 of the recorder
sources. The scenario names identify the inputs and checkpoints. The command
compiles the recorder against the existing build. The capture is separate
from the kernel golden corpus. A parity trace with more than four undo
checkpoints is rejected before taking its undo snapshots. Each recording
checks this limit with four and five placements through the Point tool.
Point observations between these frames share a 4,096-unit budget per
case. Each observation charges for all labels it scans, with a minimum
charge of one.

The controller gets the same integer screen positions throughout a gesture,
including after a change of view. Text submission uses `TextBuilder`, as in
`TextBuilderTest`, and InputBox submission uses `ButtonDialogModel.apply`. An unchanged redefine submits the definition
supplied by the reference to its shared `RedefineInputHandler`.
The headless app exercises the shared input preview helper for an empty
algebra entry. Native key delivery and the virtual keyboard still use the
original web observation and desktop replay. See the [interaction parity
observations](../../docs/interaction-parity.md) for the GUI and platform
boundaries of the recordings.

### Canvas review observations

```sh
python3 tools/ggb-harness/cases/make_canvas.py
python3 tools/ggb-harness/cases/make_controls.py
python3 tools/ggb-harness/saved/make_saved.py
GEOGEBRA_SRC=<checkout> tools/ggb-harness/linux-x86_64.sh golden canvas controls
GEOGEBRA_SRC=<checkout> tools/ggb-harness/linux-x86_64.sh canvas-review
```

Use GeoGebra revision `112919f2d450ec7430a5cd3b65dba226fdc43b74`.
The second container command compiles the review probes against the existing
harness build, saves the rounding and sector fixtures, and reports observations
of list drawability, tool selections, pen events and shape hits. The InputBox probe uses the model boundary for a linked submission and the
headless selection manager for keyboard order. Native focus delivery and the
five original desktop replay event sequences are outside these probes.

| Probe | Observed result |
| --- | --- |
| `controls-reviewopen-001` and `saved-input-box-001` | Loading the saved file replaces the field instance and loads `a = 3.5`. |
| `controls-reviewblur-001` | Submitting `3.5` through the box updates `a` to 3.5 and `b` to 7. |
| `controls-reviewdrag-001` | Moving the linked slider from 2 to 5 changes the box text from 2 to 5 and the dependent number from 4 to 10. |
| `controls-reviewtext-001` | The editor returns 257 characters. Removing the first character and submitting leaves 256 characters ending in `Z`. |
| `controls-reviewsliderclick-001` | A fixed slider starts at 2. Submitting 3.5 and clicking the high end of its track advances it to 4. |
| `controls-reviewtab-001` | The headless keyboard order is first, second, third. |
| `canvas-reviewstrokes-001` | Two pointer gestures preserve both paths in one stroke object, with a y range from -2 to 0.8333333333333334. The golden construction case separately records both PenStroke commands. |
| `canvas-reviewchart-001` | All 17 copies and the 257-member plot hit at the first dot. |
| `canvas-reviewrectangle-001` | The 7,000-vertex polyline and point produce no list from the rectangle gesture. |
| `canvas-reviewstretch-001` | The rectangle selects the original polygon, all 70 stretched polygons and all 71 vectors. |
| `canvas-reviewbounds-001` | The last point in the third 256-member list is hittable. |
| `canvas-reviewlocus-001` | The lone sample and the edge beyond 10,001 samples both hit. |

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
may set `angle_unit` (`degree` or `radiant`). A `style` step sets presentation
properties through the setters of the element (colour, opacity, line, point
and fill style, layer, flags, dynamic colour, slider placement and so on);
the `styles` topic has one fixture per family of kinds for the export oracle.
A run with topic names (`harness.sh saved styles`) rewrites those cases only. `crates/geo-import/tests/saved.rs`
imports each file and compares it with its snapshot: this covers, for exactly the states the cases reach, that the
n-th output slot of a saved `Intersect` or `Tangent` is the n-th object of the
kernel after points were moved, that angles do not depend on the angle unit,
and that imported texts match before and after a point move and a number edit.

### Reading files back

```sh
tools/ggb-harness/harness.sh reload <file.xml> <label>...
```

loads a saved file into a fresh GeoGebra (`getXMLio().processXMLString`) and prints
the values it computes for the labels, as `<file> <label> = <value>`.
`GEOGEBRA_SRC=<checkout> python3 -I tools/ggb-harness/saved/make_distance_reload.py`
runs this probe in the Linux x86_64 container on the four files whose saved
Distance argument order changes a computed value. For
`conic-diameter-009.xml` and `d`, reload gives `1.7888543819998317`.
The file still caches the pre-save value `0.8944271909999159`. The import
oracle reads the recording in `golden/ggb/distance-reload.snapshot`.


### Classic editor tools

```sh
python3 -I tools/ggb-harness/cases/make_editor_tools.py
GEOGEBRA_SRC=<checkout> tools/ggb-harness/linux-x86_64.sh editor-tools
```

The probe compiles against the existing harness classpath and the reference
test fixtures. It writes `crates/geo-mcp/tests/fixtures/editor_tool_scenarios.json`
and leaves the construction golden corpus alone. It observes the controller's
pointer sequences, modes, construction changes and undo history. The inspector
boundary records the selected function and its initial nine table rows. The
image chooser boundary supplies the generated PNG from the input fixture and
forwards the hit point to the reference image placement helper. The headless
probe does not run either native dialog, so the image observation checks the
anchor and return to Move. The Rust replay opens the same PNG through its file
prompt and the UI test checks undo and redo.

The `regressions` entries record reference outcomes for dedicated Rust tests.
They cover inspector endpoints, styled zoom, rotation about a dependent
centre, copied style after reload, labels, cancellation and freehand fits.
They remain separate from the two-cadence pointer replay counts.
