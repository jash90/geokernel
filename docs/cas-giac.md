# Giac as an optional CAS backend

Status: Linux prototype for the solving commands. The feature is off by
default; the default build, its tests and every golden gate do not change.

## Why

Most of the open command forms are CAS forms: `Solve`, `NSolve`,
`Solutions` and `NSolutions` of conics, implicit curves, lines, lists and
start values; `Simplify` of conics, curves, implicit curves and lines;
`Expand`, `Factor` and `IFactor` of curves and implicit curves; and
`PlotSolve`. The reference evaluates every one of them with Giac. Eight review
rounds showed that exact agreement needs Giac's own behaviour: which roots it
finds, the order it lists them in and the exact form it prints. No EUPL source
fixes that behaviour, so the kernel cannot reproduce it by porting. The
reference embeds Giac as a library; this backend does the same.

## Which Giac

Exact agreement needs the same Giac build as the reference: **build 70507**,
the version the pinned reference loads and the Java harness already downloads
(`tools/ggb-harness/harness.sh`, `fetch_giac`). It is published on
`repo.geogebra.net` once per platform:

| Platform | Artifact | Library inside |
| --- | --- | --- |
| Linux x86_64 | `fr/ujf-grenoble/javagiac/70507/javagiac-70507-natives-linux-amd64.jar` | `libjavagiac64.so` (needs only libc and libm) |
| macOS (arm64, x86_64) | `.../javagiac-70507-natives-macosx-universal.jar` | `libjavagiac-arm64.jnilib`, `libjavagiac.jnilib` |
| Windows x86_64 | `.../javagiac-70507-natives-windows-amd64.jar` | `javagiac64.dll` (needs only the system C runtime) |
| Android (arm64-v8a, armeabi-v7a, x86, x86_64) | `org/geogebra/giac-android/70507/giac-android-70507.aar` | `jni/<abi>/libjavagiac.so` with `libc++_shared.so` |

Web builds of the reference use a JavaScript build of Giac; that is out of
scope here.

## How the kernel calls it

Every one of these libraries is a JNI library: its entry points are the
`Java_javagiac_giacJNI_*` functions that the reference's Java classes call.
The C++ API of Giac is exported by the Linux build but not by the Windows
build, so the JNI entry points are the only interface present on every
platform. They are also exactly the functions the reference calls.

The reference uses only three Giac operations (`GenImpl`, `ContextImpl` in
its JRE binding): build an expression from text in a context, evaluate it at
level 1, and print it in that context. The matching entry points are

| Operation | Entry point |
| --- | --- |
| `new context()` | `new_1context_1_1SWIG_10` |
| `new gen(String, context)` | `new_1gen_1_1SWIG_125` |
| `gen.eval(int, context)` | `gen_1eval` |
| `gen.print(context)` | `gen_1print_1_1SWIG_10` |
| destructors | `delete_1gen`, `delete_1context` |

These wrappers use their `JNIEnv` argument only to read the bytes of the input
string (`GetStringUTFChars`, `ReleaseStringUTFChars`), to build the output
string (`NewStringUTF`) and, for a null argument, to raise an exception
(`FindClass`, `ThrowNew`, `ExceptionClear`). `crates/geo-giac` supplies a
function table with exactly those slots; every other slot aborts with a
message. So the library is called **without a Java virtual machine and without
a C++ shim**: the crate loads it at run time with `libloading` and calls the
wrappers through that table.

Options considered and rejected:

- *Link the C++ library from the same release.* The release ships no headers
  and no separate C++ library; the Windows DLL does not export the C++ API, and
  mixing our C++ standard library with the copy statically linked into the
  Linux build is fragile.
- *A thin C shim over the C++ API.* Same export problem on Windows, plus a C++
  toolchain on every target.
- *A process boundary* (a helper program that owns Giac). Keeps Giac out of the
  kernel's address space, but every target needs a helper executable and
  Android cannot easily spawn one; it can be added later on top of the same
  crate if crash isolation turns out to matter.
- *Embedding a JVM* was excluded by the task.

### Protocol

The reference resets Giac before every evaluation (`CASgiacB.init`,
`EvalFunction.evalRaw`). `geo_giac::eval_raw` reproduces that sequence:

1. `caseval("init geogebra")`;
2. `restart`, `warn_equal_in_prog(0)`, `proba_epsilon:=0;` and every helper of
   `CASgiac.CustomFunctions` whose name occurs in the input, followed by its
   prerequisites in the reference's order;
3. `caseval("timeout 5")`, `caseval("ckevery 20")`;
4. `srand(seed)`: 9 for inputs that call `fsolve` (as in the reference). The
   reference seeds other inputs randomly; the kernel passes a fixed seed so
   runs are reproducible;
5. `caseval(evalfa(input))`, printed; surrounding quotes removed; results
   longer than 10,000 UTF-16 units become `?`.

One worker thread with a 256 MiB stack owns the library and its context:
Giac's context is not thread-safe (the reference serialises calls too) and it
recurses deeply.

### Fidelity check

The harness records every Giac exchange when run with
`GGB_HARNESS_JAVA_OPTS=-DggbHarness.giacLog=<file>` (see
`tools/ggb-harness/README.md`). On Linux x86_64 all **1,962** exchanges of the
662 recorded CAS cases (`cases/cas.json`, `cases/reference/cas.json`) are
reproduced byte for byte by `geo_giac::eval_raw`
(`cargo run -p geo-giac --features giac --example replay`).

## What the kernel ports

Giac is used, not ported. What the kernel ports from the reference's EUPL
sources is the conversation with Giac:

- the command translation: the algorithm's GeoGebra command text (for example
  `AlgoSolve.compute`), the per-command Giac templates of `Ggb2giac` and the
  Giac serialisation of expression trees (`StringTemplate.giacTemplate`);
- the output side: `CASgiac.postProcess`, the Giac-mode parse of the result
  and the conversion back into construction objects
  (`AlgebraProcessor.evaluateToList` for the solving commands).

The kernel's own exact CAS stays the default. With the feature on, a CAS
form that has a Giac translation is evaluated through Giac; a form without one
keeps the default path.

### What is translated (`crates/geo-kernel/src/expr/cas/giac.rs`)

`Solve`, `Solutions`, `NSolve`, `NSolutions` and `PlotSolve` from the
algebra input, as `AlgoSolve` sends them (`PlotSolve` exists only with the
feature; it takes one list, equation or function, as `CmdSolve` does):

- arguments print as the reference's Giac template prints the definition it
  parsed back: every operand bracketed (`(L)+(R)`), `-(x)` for a negated
  term, typed decimals as `(digits/10^k)` (`convertScientificNotationGiac`),
  computed numbers through their 15-significant-digit text rounded half up,
  `+ -` turned into `-`; lines without a definition as `GeoLine`'s implicit
  or explicit text; function objects as their expression; `%%n` placeholders
  with the numeric template (decimals as written);
- the second argument is printed from its text (variables, lists of them,
  start values `variable = value`, numbers, other names as
  `ggbtmpvar<name>`); a list of equations gets its sorted variables;
- the one- and two-argument `Ggb2giac` templates of the four commands, and
  `PointList.1` around `Solve.1` for `PlotSolve`;
- Giac's printed list is read back as `evaluateToList` does for these
  results: lines from linear equations, numbers, points, functions of the one
  free variable (polynomials exactly, other expressions sampled from Giac's
  printed form), an undefined implicit curve for an equation that is neither
  linear, nor `y = f(x)`, nor a polynomial, nested lists, `AlgoSolve`'s extra
  list around the solutions of a list of equations, and an undefined list for
  an unevaluated `fsolve` or an unknown name.

Not translated yet: `Simplify`,
`Expand`, `Factor` and `IFactor` of lines, conics, curves and implicit curves,
conics made by commands (`Circle`, `Conic`, `Parabola`, `Ellipse` print as
`GeoConic`'s value text, which is not ported), results that the reference
reads as conics, defined implicit curves or parametric curves (and the points
`(f(y), 0)` it reads as lines), and solutions in a third free variable.

## Results (Linux x86_64)

`GEO_GIAC_LIBRARY=$(tools/giac/fetch.sh) cargo test -p geo-golden --features
giac --test golden -- --nocapture` runs the golden data with the feature;
`crates/geo-golden/known-gaps-giac.toml` lists, on top of
`known-gaps.toml`, the cases that agree only with Giac (`agrees = true`) and
the reason for those that still differ.

| | ggb-harness cases | agree | known gaps |
| --- | --- | --- | --- |
| default build | 7065 | 6455 | 611 |
| `giac` feature | 7065 | 6768 | 298 |

Only one case that agrees without the feature differs with it:
`cas-plot-022` records the kernel's old refusal text for `PlotSolve`, a
command it now has; the lines are still refused (see
`known-gaps-giac.toml`). Of the 1,394 Giac
inputs the kernel sends for the recorded cases, all but 11 (the second
arguments `z = 1` and `z = 0`, which the reference's two-dimensional parser
reads as numbers, and powers of ten that the reference prints from a
different tree) are byte for byte the reference's.

Eighteen solving forms agree in every recorded case only with the feature
and are counted as done, marked `giac = true` in
`tools/command-matrix/form-evidence.toml` and in the matrix notes:
`Solve(line)`, `Solve(conic)`, `Solve(list)`, `Solve(function, any)`,
`Solutions(line)`, `Solutions(list)`, `Solutions(function, any)`,
`NSolve(conic)`, `NSolve(list)`, `NSolve(function, any)`,
`NSolve(line, any)`, `NSolve(conic, any)`, `NSolutions(conic)`,
`NSolutions(list)`, `NSolutions(function, any)`, `NSolutions(line, any)`,
`NSolutions(conic, any)`, `PlotSolve(list)`. The implicit-curve forms stay
open because the same cases also call `PlotSolve` of an implicit curve,
whose curves the kernel does not read back yet.

CI runs the binding, the kernel and the golden data with the feature on Linux
(`giac-linux` in `.github/workflows/ci.yml`).

## Crate layout and feature

- `crates/geo-giac`: the loader, the JNI table and the protocol. Without its
  `giac` feature it compiles to an empty shell whose calls return
  `Error::Disabled`; it contains no Giac code either way.
- The library path is read from `GEO_GIAC_LIBRARY` at run time.
  `tools/giac/fetch.sh` downloads and checks the pinned artifact for the
  current platform and prints that path.
- `geo-kernel` gets an optional `giac` feature that depends on
  `geo-giac/giac`. `cargo build --workspace` and every existing gate build
  without it.

## Licence

Giac is free software under the GNU General Public License, version 3 or
later. Nothing of Giac is in this repository or compiled into any crate.
However:

- A program built with the `giac` feature and **distributed together with a
  Giac library** (or designed so that it cannot work without one) is a combined
  work with Giac and must be distributed under the terms of the GPL version 3
  or later, including the complete corresponding source of the whole program.
- The kernel's own source is EUPL-1.2. EUPL-1.2 lists the GPL version 3 as a
  compatible licence (Article 5 and its Appendix), so the EUPL code may be
  distributed as part of such a GPL-licensed combined work.
- Builds without the feature contain no Giac and stay EUPL-1.2 only.
- The Giac binaries are downloaded from the publisher at build or test time
  and are never committed. Whoever redistributes them must also offer Giac's
  source as the GPL requires.

See `NOTICE`.

## Platforms

| Platform | State |
| --- | --- |
| Linux x86_64 | Prototype: loads and agrees on every recorded exchange. |
| macOS arm64 / x86_64 | Same entry points; needs a run on a Mac. The harness already uses the arm64 library. |
| Windows x86_64 | Same entry points exported (checked in the DLL's export table); needs a run on Windows. `extern "system"` is the JNI calling convention there. |
| Android | Same entry points in all four ABIs; `libc++_shared.so` from the same AAR must be packaged beside it. Loading needs the app's native library directory instead of `GEO_GIAC_LIBRARY`. |
| Web | Not covered: the reference uses a JavaScript build of Giac. |

Open questions for the other platforms: how the app ships and locates the
library (bundle resources on macOS, next to the executable on Windows,
`jniLibs` on Android), code signing of the downloaded library on macOS, and a
CI runner per platform to repeat the fidelity check.
