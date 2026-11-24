# Giac as an optional CAS backend

Status: optional backend for the solving commands. The feature is off by
default. Linux x86_64 is the reference for cross-platform validation.

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
recurses deeply. One call is in flight at a time, and a caller waits at most
the time limit plus five seconds. After that the call returns
`Error::Timeout`; the worker thread keeps running the stuck Giac call (it
cannot be interrupted from outside, and a second copy of the library would
share Giac's global state), every later call returns `Error::Busy` at once,
and the worker takes calls again as soon as Giac returns. Giac's own limit is
in whole seconds; a limit under a second rounds up to one.

Before loading, the crate checks the file's SHA-256 against the pinned
Giac 70507 build for the platform (Linux x86_64, macOS arm64 and x86_64,
Windows x86_64, the four Android ABIs) and refuses anything else with
`Error::Integrity`; `tools/giac/fetch.sh` checks the jar, the extracted
library and any cached copy against the same values. A null handle from Giac
is an error, never passed back to it.

### Backends and failures

With the feature compiled in, the backend is chosen once per process, at the
first CAS evaluation or the first parse of a form only Giac evaluates, and
kept for the process's lifetime (`geo_giac::backend()`):

| Situation | Backend | What CAS commands do |
| --- | --- | --- |
| No bundled library found, or the explicit path is empty or missing | `KernelOnly` | Exactly the default build: the kernel's own solver answers, and the parser refuses what the default build refuses (start values, numbers and unknown names as the second argument, `PlotSolve`). The golden data give the default build's numbers and use `known-gaps.toml` alone. |
| The library is configured, passes the hash check and loads | `Giac` | Translated forms go to Giac. A failed Giac call (time limit, busy worker, worker gone, null handle, Giac error) makes that result undefined, as a failed CAS call does in the reference; a later recompute can succeed once Giac answers again. A form without a translation keeps the kernel's solver, unless it is `PlotSolve` or has a second argument only Giac accepts: then it is undefined, never answered with that argument ignored. |
| The library is configured but has another hash, the platform has no pinned build, or it does not load | `Failed` | Every CAS result that would go to Giac is undefined, with no fallback; the reason is printed once on standard error (`geo_giac::backend_reason()`). The parser takes the same forms as with `Giac`. |

### Fidelity check

The fixture `tools/giac/cas-exchanges.jsonl` contains **1,966** complete
input/output pairs from the 662 CAS cases in `cases/cas.json` and
`cases/reference/cas.json`. It was recorded by the Java harness at reference
revision `112919f2d450ec7430a5cd3b65dba226fdc43b74` on Linux x86_64 with
Giac 70507. Four logged inputs had no corresponding printed result. To
recreate the fixture, record the two case files with
`GGB_HARNESS_JAVA_OPTS=-DggbHarness.giacLog=<file>` (see the
[harness README](../tools/ggb-harness/README.md)), then pass the log to
`python3 -I tools/giac/replay-fixture.py <file>`.

```sh
GEO_GIAC_LIBRARY="$(tools/giac/fetch.sh)" \
  cargo run -p geo-giac --features giac --example replay -- tools/giac/cas-exchanges.jsonl
```

The example exits with an error for any differing result. Set
`GEO_GIAC_REPLAY_TRACE=1` to log each case and input before evaluation.

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
  `ggbtmpvar<name>`); a list of equations gets its sorted variables.
  Beyond lists of variables the parser takes only the shapes the reference
  takes (one name, or `name = number`); inequalities, plain literals and
  start values in a list stay refused;
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
| default build | 7303 | 6819 | 485 |
| `giac` feature, library present | 7303 | 7133 | 171 |
| `giac` feature, no library | 7303 | 6819 | 485 |

Only one case that agrees without the feature differs with it:
`cas-plot-022` records the kernel's old refusal text for `PlotSolve`, a
command it now has; the lines are still refused (see
`known-gaps-giac.toml`). Of the 1,394 Giac
inputs the kernel sends for the recorded cases, all but 11 (the second
arguments `z = 1` and `z = 0`, which the reference's two-dimensional parser
reads as numbers, and powers of ten that the reference prints from a
different tree) are byte for byte the reference's.

Eight solving forms agree in every recorded case that may call them only
with the feature and are counted as done, marked `giac = true` in
`tools/command-matrix/form-evidence.toml` and in the matrix notes:
`Solve(list)`, `Solutions(list)`, `NSolve(list)`, `NSolutions(list)`,
`NSolutions(function, any)`, `NSolutions(line, any)`,
`NSolutions(conic, any)`, `PlotSolve(list)`. Ten more forms agree in the
cases that call them directly but stay open, because a case that is still a
gap with the feature may call them (`cas-implicit-013`, `cas-name-004`,
`cas-name-012`, `cas-degree-001`, `-003`, `-004`, `cas-hintobject-001`,
`-002`): `Solve(line)`, `Solve(conic)`, `Solve(function, any)`,
`Solutions(line)`, `Solutions(function, any)`, `NSolve(conic)`,
`NSolve(function, any)`, `NSolve(line, any)`, `NSolve(conic, any)`,
`NSolutions(conic)`. The implicit-curve forms stay
open because the same cases also call `PlotSolve` of an implicit curve,
whose curves the kernel does not read back yet.

Desktop feature checks run in `giac-linux`, `giac-macos` and `giac-windows`
(.github/workflows/ci.yml).

## Crate layout and feature

- `crates/geo-giac`: the loader, the JNI table and the protocol. Without its
  `giac` feature it compiles to an empty shell whose calls return
  `Error::Disabled`; it contains no Giac code either way.
- Set `GEO_GIAC_LIBRARY` to override the library location. When it is unset,
  desktop builds look for the platform's pinned library next to the
  executable, then in `Contents/Frameworks` for a macOS bundle. An empty
  override selects the kernel alone. `tools/giac/fetch.sh` verifies the
  pinned desktop artifact and prints the library path.
- On Android, call `geo_giac::initialize_from_android_dir` with
  `ApplicationInfo.nativeLibraryDir` before any backend query or evaluation.
  The app uses the ABI selected at install time. The worker checks both
  library hashes, preloads the C++ runtime, and loads Giac by absolute
  path. An incomplete or relative directory is rejected before backend
  selection.
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

Evidence as of 2026-11-24:

| Platform | Check | Result |
| --- | --- | --- |
| Linux x86_64 | `giac-linux` | Passed on main on 2026-11-24 (6m 15s). |
| macOS arm64 | `giac-macos` on `macos-latest` | Local binding tests passed. The full native replay and golden replay reproduced the upstream fault described below. |
| macOS x86_64 | Manual desktop packaging | Pinned Intel library available. Full CI fidelity has yet to be established. |
| Windows x86_64 | `giac-windows` on `windows-latest` | Pinned DLL and JNI exports checked. Runtime CI evidence pending. |
| Android | AAR extraction and host unit tests | Pinned AAR verified and both libraries extracted for all four ABIs. Directory resolution and refusal tests passed. Emulator replay pending. |
| Web | Outside scope | The reference uses a JavaScript build of Giac. |

### macOS arm64 crash

On macOS 27.2.1 (arm64), Giac 70507's `libjavagiac-arm64.jnilib` crashed
during the native replay at `cas-order-018`. The call was
`Java_javagiac_giacJNI_gen_1eval` for this input:

```text
caseval(evalfa(ggbsort(normal(solve((((((x)^(2))+(((6)*(x))*(y)))-((3)*((y)^(2))))-((6)*(y)))-(3)=0,x,'!=')))))
```

The crash report records `EXC_BAD_ACCESS` / `SIGBUS` in `__gmpn_mul_1`
under `mpfr_exp`. Other replay and golden runs recorded `SIGSEGV` inside
`__gmpn_sqr_diag_addlsh1` or `__gmpn_sub_n`. The same isolated exchange
passed three fresh processes. Disassembling the pinned binary shows that
its GMP assembly uses `x18` as a loop counter, for example
`lsr x18, x3, #2` and `sub x18, x18, #1` in `__gmpn_sub_n`.
The Apple ARM64 ABI reserves `x18`. macOS can clear it on a context
switch. A counter cleared
mid-loop can underflow and run past the operand buffers, consistent with the
register state and memory faults in the crash reports. See the [Apple ARM64 ABI](https://developer.apple.com/documentation/xcode/writing-arm64-code-for-apple-platforms)
and the [GMP fault report](https://gnats.netbsd.org/60904).

The fix belongs in
the published arm64 library's GMP build. The CI replay keeps reporting
crashes and output differences. The current macOS packaging workflow builds
for Intel.

### Android emulator replay

`tools/giac/android.sh` verifies the SHA-256 of
`giac-android-70507.aar` and prints the extracted `jniLibs` root in the
ignored build directory. The four ABI folders each contain
`libjavagiac.so` and `libc++_shared.so`.

The next validation step is the following replay on a booted x86_64
emulator. Set `ANDROID_NDK_HOME` and use the host prebuilt directory for
your NDK installation (the example uses the Linux host).

```sh
rustup target add x86_64-linux-android
export CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin/x86_64-linux-android24-clang"
cargo build -p geo-giac --features giac --target x86_64-linux-android --example replay
jni_libs="$(tools/giac/android.sh)"
adb shell mkdir -p /data/local/tmp/geokernel-giac
adb push target/x86_64-linux-android/debug/examples/replay tools/giac/cas-exchanges.jsonl \
  "$jni_libs/x86_64/libjavagiac.so" "$jni_libs/x86_64/libc++_shared.so" /data/local/tmp/geokernel-giac/
adb shell chmod 755 /data/local/tmp/geokernel-giac/replay
adb shell /data/local/tmp/geokernel-giac/replay /data/local/tmp/geokernel-giac/cas-exchanges.jsonl /data/local/tmp/geokernel-giac
```

## Desktop distribution

The manual **Package desktop builds** workflow builds two archives per
desktop platform (Linux x86_64, Windows x86_64 and macOS x86_64):

- `geokernel-<platform>.tar.gz` contains the CLI and desktop app under
  EUPL-1.2.
- `geokernel-giac-<platform>.tar.gz` contains the same programs with
  the Giac feature and the verified native library under GPL-3.0-or-later.
  The library is beside the executables. In the macOS app bundle, the
  library is in `Contents/Frameworks` and linked from `Contents/MacOS`.

Each includes `NOTICE`, `LICENSE` and `REVISION`. The Giac variant also
includes the GPL licence, the application's source archive and the written
offer for Giac's exact corresponding source. Before any release, the project
owner must arrange delivery of that source for this binary build and, for
online distribution, make it available alongside the downloads under GPLv3
section 6(d). CI produces unsigned build artifacts.
