//! Fuzzing of the script parser: random and mutated input lines must never
//! panic or hang, a failing line must leave the construction as it was, and
//! a line that succeeds must create an object and leave only finite values.
//!
//! Inputs come from fixed seeds. Each distinct failure is reported once,
//! with its input reduced as far as it still fails.

mod common;

use common::*;
use geo_kernel::script::{assignment_label, exec, load};
use geo_kernel::Construction;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// A context in which many of the labels used below exist, with one object
/// of every kind.
const CONTEXT: &[&str] = &[
    "A = (0, 0)",
    "B = (4, 0)",
    "C = (1, 3)",
    "D = (-2, 2.5)",
    "r = 2",
    "q = -1.5",
    "l = Line(A, B)",
    "m = Line(C, D)",
    "s = Segment(A, C)",
    "k = Circle(A, r)",
    "k2 = Circle(B, C)",
    "d = Distance(A, B)",
    "M = Midpoint(A, B)",
    "t = Tangent(B, k)",
    "A' = (1, 1)",
    "X_1 = (2, 2)",
    "u = (1, 2)",
    "v = Vector(A, B)",
    "g = Ray(A, B)",
    "e = Ellipse(A, B, C)",
    "P = Point(k)",
    "Q = Point(s, r)",
    "a = CircularArc(A, B, C)",
    "b = CircumcircularSector(B, C, D)",
    "R = Point(a)",
];

/// At the time of writing: 697 case scripts and 21 README lines.
const MIN_CASE_SCRIPTS: usize = 500;
const MIN_README_LINES: usize = 10;

/// The evaluation work one line may do (`geo_kernel::expr::work_done`):
/// two objects at the bound of one evaluation, about 0.6 s of a release
/// build and 5 s of the test build (a unit is about 3.5 ns and 31 ns). This
/// is what finds a blow-up in an expression (a loop, a costly function,
/// long exact decimals), the same on any machine. It is a number of its
/// own, not a multiple of the bound, so that a bound raised by mistake is
/// caught here. At the time of writing the most work a line of the corpus
/// or its mutations does is the nested `Sequence` of `lists-seq-011` cut at
/// the element bound (when the mutations reach it), 10 531 983 units and
/// 0.29 s of the test build; otherwise it is about 2 000 000 units. The
/// slowest line is a long chain of powers that takes 0.4 s to parse and
/// evaluates no list.
const MAX_LINE_WORK: u64 = 160_000_000;

/// A backstop for work the counter does not see (parsing, commands other
/// than expressions): twice the time of [`MAX_LINE_WORK`] in the test build.
const SLOW: Duration = Duration::from_secs(10);

/// What went wrong with one input line, as a short stable key and a detail.
struct Problem {
    key: String,
    detail: String,
}

/// Runs one line in `c` and checks the properties of `exec`.
/// Returns whether the line created objects.
fn check_line(c: &mut Construction, line: &str) -> Result<bool, Problem> {
    let len = c.len();
    let before = values(c);
    let replacement = assignment_label(line).and_then(|label| c.id(label));
    let started = Instant::now();
    let work = geo_kernel::expr::work_done();
    let result = match catch(|| exec(c, line)) {
        Ok(r) => r,
        Err(panic) => {
            return Err(Problem {
                key: format!("panic: {panic}"),
                detail: panic,
            })
        }
    };
    let work = geo_kernel::expr::work_done() - work;
    if work > MAX_LINE_WORK {
        return Err(Problem {
            key: "too much evaluation work".into(),
            detail: format!("{work} units of work"),
        });
    }
    if started.elapsed() > SLOW {
        return Err(Problem {
            key: "slow line".into(),
            detail: format!("took {:?}", started.elapsed()),
        });
    }
    match result {
        Ok(Some(first)) => {
            let expected = if c.len() == len {
                replacement
            } else {
                c.ids().skip(len).find(|&id| !c.label(id).starts_with('\0'))
            };
            if Some(first) != expected {
                return Err(Problem {
                    key: "Ok(Some(id)) is not the assigned object or first new output".into(),
                    detail: format!("id {} with {len} objects before", first.index()),
                });
            }
            if let Some(bad) = first_non_finite(c) {
                return Err(Problem {
                    key: "non-finite value after a successful line".into(),
                    detail: bad,
                });
            }
            for id in c.ids().skip(len) {
                if c.label(id).is_empty() {
                    return Err(Problem {
                        key: "an object with an empty label".into(),
                        detail: format!("object {}", id.index()),
                    });
                }
            }
        }
        Ok(None) => {
            if c.len() != len {
                return Err(Problem {
                    key: "Ok(None) but objects were created".into(),
                    detail: format!("{len} -> {}", c.len()),
                });
            }
        }
        Err(_) => {
            if c.len() != len {
                return Err(Problem {
                    key: "Err but objects were created".into(),
                    detail: format!("{len} -> {}", c.len()),
                });
            }
            if values(c) != before {
                return Err(Problem {
                    key: "Err but values changed".into(),
                    detail: String::new(),
                });
            }
        }
    }
    Ok(c.len() > len)
}

fn context() -> Construction {
    let mut c = Construction::new();
    for line in CONTEXT {
        exec(&mut c, line).unwrap_or_else(|e| panic!("context line `{line}`: {e}"));
    }
    c
}

/// Collects distinct problems. The first input seen for a key is reduced.
#[derive(Default)]
struct Findings {
    by_key: BTreeMap<String, (String, String, usize)>,
    runs: usize,
    created: usize,
}

impl Findings {
    /// Runs `line` after `prefix` in a fresh construction and records any
    /// problem. The prefix lines are replayed unchecked.
    fn run(&mut self, prefix: &[String], line: &str) {
        self.runs += 1;
        let mut c = Construction::new();
        for p in prefix {
            let _ = catch(|| exec(&mut c, p));
        }
        self.run_in(&mut c, prefix, line);
    }

    fn run_in(&mut self, c: &mut Construction, prefix: &[String], line: &str) {
        let problem = match check_line(c, line) {
            Ok(created) => {
                self.created += usize::from(created);
                return;
            }
            Err(p) => p,
        };
        if let Some(entry) = self.by_key.get_mut(&problem.key) {
            entry.2 += 1;
            return;
        }
        let key = problem.key.clone();
        let reduced = shrink(line, &|candidate| {
            let mut c = Construction::new();
            for p in prefix {
                let _ = catch(|| exec(&mut c, p));
            }
            matches!(check_line(&mut c, candidate), Err(p) if p.key == key)
        });
        let detail = format!("{}; prefix has {} line(s)", problem.detail, prefix.len());
        self.by_key.insert(key, (reduced, detail, 1));
    }

    /// Fails on any problem, and when fewer than `min_created` lines
    /// succeeded (the success checks would otherwise run on too little).
    fn report(&self, name: &str, min_created: usize) {
        println!(
            "{name}: {} inputs, {} created objects, {} distinct problem(s)",
            self.runs,
            self.created,
            self.by_key.len()
        );
        let text: Vec<String> = self
            .by_key
            .iter()
            .map(|(key, (input, detail, n))| {
                format!("{key} ({n} time(s))\n  reduced input: {input:?}\n  {detail}")
            })
            .collect();
        assert!(self.by_key.is_empty(), "{name}:\n{}", text.join("\n"));
        assert!(
            self.created >= min_created,
            "{name}: only {} lines created objects (at least {min_created} expected)",
            self.created
        );
    }
}

fn context_lines() -> Vec<String> {
    CONTEXT.iter().map(|s| s.to_string()).collect()
}

// ------------------------------------------------------------- corpus

fn repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

/// The strings of every `"script": [...]` array of a JSON text.
fn script_arrays(json: &str) -> Vec<Vec<String>> {
    let b: Vec<char> = json.chars().collect();
    let mut scripts = Vec::new();
    let needle: Vec<char> = "\"script\"".chars().collect();
    let mut i = 0;
    while i + needle.len() < b.len() {
        if b[i..i + needle.len()] != needle[..] {
            i += 1;
            continue;
        }
        i += needle.len();
        let skip_ws = |i: &mut usize| {
            while *i < b.len() && b[*i].is_whitespace() {
                *i += 1;
            }
        };
        skip_ws(&mut i);
        if b.get(i) != Some(&':') {
            panic!("`\"script\"` is not followed by a colon in the case file");
        }
        i += 1;
        skip_ws(&mut i);
        if b.get(i) != Some(&'[') {
            panic!("`\"script\"` is not an array in the case file");
        }
        i += 1;
        let mut lines = Vec::new();
        loop {
            skip_ws(&mut i);
            match b.get(i) {
                Some('"') => {
                    i += 1;
                    let mut s = String::new();
                    while i < b.len() && b[i] != '"' {
                        if b[i] == '\\' && i + 1 < b.len() {
                            i += 1;
                            match b[i] {
                                'n' => s.push('\n'),
                                't' => s.push('\t'),
                                'u' if i + 4 < b.len() => {
                                    let hex: String = b[i + 1..i + 5].iter().collect();
                                    if let Some(ch) =
                                        u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32)
                                    {
                                        s.push(ch);
                                    }
                                    i += 4;
                                }
                                other => s.push(other),
                            }
                        } else {
                            s.push(b[i]);
                        }
                        i += 1;
                    }
                    i += 1;
                    lines.push(s);
                }
                Some(',') => i += 1,
                _ => break,
            }
        }
        scripts.push(lines);
    }
    scripts
}

/// Scripts from the golden-case generators, when the repository layout is
/// present.
fn case_scripts() -> Vec<Vec<String>> {
    let dir = repo_path("tools/ggb-harness/cases");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read the case directory {dir:?}: {e}"))
        .map(|e| e.expect("directory entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    let scripts: Vec<Vec<String>> = files
        .iter()
        .flat_map(|f| {
            let text = std::fs::read_to_string(f)
                .unwrap_or_else(|e| panic!("cannot read case file {f:?}: {e}"));
            script_arrays(&text)
        })
        .collect();
    assert!(
        scripts.len() >= MIN_CASE_SCRIPTS,
        "only {} case scripts found in {dir:?} (at least {MIN_CASE_SCRIPTS} expected)",
        scripts.len()
    );
    scripts
}

/// Lines of the code blocks of the README that look like script lines.
fn readme_lines() -> Vec<String> {
    let path = repo_path("README.md");
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {path:?}: {e}"));
    let mut inside = false;
    let mut lines = Vec::new();
    for line in text.lines() {
        if line.starts_with("```") {
            inside = !inside;
        } else if inside && line.contains(" = ") {
            lines.push(line.trim().to_string());
        }
    }
    assert!(
        lines.len() >= MIN_README_LINES,
        "only {} script lines found in the README code blocks (at least {MIN_README_LINES} expected)",
        lines.len()
    );
    lines
}

/// Lines that exercise every command form, written out so that the corpus
/// does not depend on the files above.
fn builtin_lines() -> Vec<String> {
    let mut lines = vec![
        "P = (1.5, -2)".to_string(),
        "n = 3.25".to_string(),
        "e = Intersect(l, k, 2)".to_string(),
        "e = Intersect(k, k2)".to_string(),
        "e = Intersect(l, m)".to_string(),
        "w_{1} = Tangent(M, k)".to_string(),
        "w = AngularBisector(l, m)".to_string(),
        "zl = {1, {2, n}, (1, 2), 1 < n}".to_string(),
        "zl = Sequence(Sequence(j + i2, j, 1, i2), i2, 1, n)".to_string(),
        "zn = Sum(Sequence(n)) + Length({P}) + Element({1, 2}, 2)".to_string(),
        "zb = If(n < 3, 1, n > 4, P, (0, 0))".to_string(),
        "zl = Join(Sort({n, 1, 2}), Reverse(Append(First({P, P}, 1), P)))".to_string(),
        "zn = Product({n, 2}) + Mean({n}) + Max({1, n}) + x(Sum({P, P}) - {1, n})".to_string(),
        "zb = !(n == 3) || true && n \u{2264} 2".to_string(),
        "e2 = Intersect(l, e, 2)".to_string(),
        "e2 = Tangent(D, e)".to_string(),
        "e2 = Directrix(e)".to_string(),
        "# only a comment".to_string(),
        "".to_string(),
    ];
    for cmd in COMMANDS {
        let mut rng = Rng::new(case_seed(0xC0DE, lines.len() as u64));
        let c = context();
        if let Some(call) = pick_args(&mut rng, &c, cmd, usize::MAX) {
            lines.push(format!("zz = {}", call.text));
        }
    }
    lines
}

#[test]
fn the_corpus_is_valid() {
    // Case scripts are valid scripts: they must load unchanged.
    let scripts = case_scripts();
    if scripts.is_empty() {
        println!("no case scripts found; the corpus is the built-in one only");
    }
    for (i, lines) in scripts.iter().enumerate() {
        let src = lines.join("\n");
        if let Err(e) = load(&src) {
            panic!("case script {i} does not load: {e}\n{src}");
        }
    }
    println!(
        "{} case scripts, {} README lines",
        scripts.len(),
        readme_lines().len()
    );
}

// ---------------------------------------------------------- generators

const TOKENS: &[&str] = &[
    "A",
    "B",
    "C",
    "D",
    "l",
    "m",
    "s",
    "k",
    "k2",
    "r",
    "q",
    "d",
    "M",
    "t_{1}",
    "A'",
    "X_1",
    "u",
    "v",
    "g",
    "(",
    ")",
    "(",
    ")",
    ",",
    ", ",
    "=",
    " = ",
    " ",
    "  ",
    "\t",
    "#",
    "{",
    "}",
    "'",
    "_",
    ".",
    "-",
    "+",
    "e",
    "E",
    "1",
    "2",
    "0",
    "3.5",
    "-0",
    "1e999",
    "-1e999",
    "1e-400",
    "NaN",
    "inf",
    "-inf",
    "Infinity",
    "0x10",
    "1_0",
    "١٢٣",
    "é",
    "ł",
    "ż",
    "日本",
    "🦀",
    "\u{0}",
    "\r",
    "\u{7f}",
    "\u{202e}",
    "\u{301}",
    "\u{feff}",
    "Line",
    "Circle",
    "Intersect",
    "Tangent",
    "Angle",
    "Area",
    "Mirror",
    "Rotate",
    "Dilate",
    "Midpoint",
    "Center",
    "AngularBisector",
    "Polar",
    "Distance",
    "Incircle",
    "Segment",
    "Shear",
    "Stretch",
    "Diameter",
    "Slope",
    "Perpendicular",
    "Parallel",
    "Radius",
    "Length",
    "CrossRatio",
    "AffineRatio",
    "Vector",
    "Ray",
    "Translate",
    "Direction",
    "UnitVector",
    "OrthogonalVector",
    "<",
    " < ",
    "<=",
    "==",
    ">",
    ">=",
    "!=",
    "\u{2264}",
    "\u{2260}",
    "&&",
    "||",
    "!",
    "\u{ac}",
    "\u{2227}",
    "\u{2228}",
    "\u{2295}",
    "->",
    "true",
    "false",
    "True",
    "If",
    "If(",
    "{",
    "}",
    "{}",
    "{1, 2}",
    "l(1)",
    "l(-1)",
    "Sum",
    "Sequence",
    "Sequence(",
    "Element",
    "Sum(",
    "Length(",
    "^",
    "*",
    "/",
    "x",
    "k",
    "1 < 2",
    "Product",
    "Mean",
    "Min(",
    "Reverse(",
    "First(",
    "Last",
    "Sort(",
    "Append(",
    "Join(",
    "{{1}, {2}}",
    "Conic",
    "Ellipse",
    "Hyperbola",
    "Parabola",
    "Focus",
    "Vertex",
    "Axes",
    "MajorAxis",
    "Directrix",
    "Eccentricity",
    "FirstAxisLength",
    "Parameter",
    "Point",
    "PathParameter",
    "CircularArc",
    "CircleSector",
    "CircumcircularArc",
    "Semicircle",
    "Arc",
    "Sector",
    "Length",
    "Perimeter",
    "a",
    "b",
    "Foo",
    "",
];

const LABELS: &[&str] = &[
    "A", "B", "C", "D", "l", "m", "s", "k", "k2", "r", "q", "d", "M", "t_{1}", "t_{2}", "A'",
    "X_1", "u", "v", "g", "e", "1", "2",
];

fn random_char(rng: &mut Rng) -> char {
    let roll = rng.unit();
    if roll < 0.6 {
        (0x20 + rng.below(0x5f) as u32) as u8 as char
    } else if roll < 0.7 {
        char::from_u32(rng.below(0x20) as u32).unwrap_or('?')
    } else if roll < 0.9 {
        char::from_u32(0x80 + rng.below(0x800) as u32).unwrap_or('?')
    } else {
        char::from_u32(rng.below(0x11_0000) as u32).unwrap_or('?')
    }
}

fn garbage_line(rng: &mut Rng) -> String {
    match rng.below(3) {
        0 => (0..rng.below(80)).map(|_| random_char(rng)).collect(),
        1 => (0..rng.below(40)).map(|_| *rng.pick(TOKENS)).collect(),
        _ => {
            // name = Command(args), with every part possibly wrong
            let name = match rng.below(6) {
                0 => String::new(),
                1 => "x_{1}".into(),
                2 => "t'".into(),
                3 => (0..rng.below(6)).map(|_| random_char(rng)).collect(),
                4 => rng.pick(TOKENS).to_string(),
                _ => format!("g{}", rng.below(100)),
            };
            let cmd = if rng.chance(0.8) {
                rng.pick(COMMANDS).name.to_string()
            } else {
                rng.pick(TOKENS).to_string()
            };
            let n_args = rng.below(7);
            let wrong = rng.pick(&[0.0, 0.05, 0.3]);
            let args: Vec<&str> = (0..n_args)
                .map(|_| {
                    if rng.chance(*wrong) {
                        *rng.pick(TOKENS)
                    } else {
                        *rng.pick(LABELS)
                    }
                })
                .collect();
            let sep = if rng.chance(0.9) {
                ","
            } else {
                *rng.pick(&[";", " ", ",,", ""])
            };
            let close = if rng.chance(0.9) { ")" } else { "" };
            format!("{name} = {cmd}({}{close}", args.join(sep))
        }
    }
}

/// A valid command with arguments of the right kinds, from the objects of `c`.
fn template_line(rng: &mut Rng, c: &Construction) -> String {
    loop {
        let cmd = rng.pick(COMMANDS);
        if let Some(call) = pick_args(rng, c, cmd, usize::MAX) {
            return format!("g{} = {}", rng.below(100), call.text);
        }
    }
}

fn garbage_script(rng: &mut Rng) -> String {
    let n = 1 + rng.below(6);
    let eol = if rng.chance(0.3) { "\r\n" } else { "\n" };
    (0..n)
        .map(|_| garbage_line(rng))
        .collect::<Vec<_>>()
        .join(eol)
}

const NUMBER_SPECIALS: &[&str] = &[
    "1e999",
    "-1e999",
    "NaN",
    "nan",
    "inf",
    "-inf",
    "+inf",
    "Infinity",
    "-Infinity",
    "-0",
    "+0",
    "-0.0",
    "0x1p3",
    "1e-400",
    "1e400",
    "1e309",
    "1.7976931348623157e308",
    "1.7976931348623159e308",
    "4.9e-324",
    "2e-324",
    ".",
    ".5",
    "5.",
    "e5",
    "1e",
    "1e+",
    "--1",
    "1,5",
    "1_0",
    "١٢٣",
    "1E5",
    "  7  ",
    "+",
    "-",
    "0.0000000000000000000000000000001",
    "٣.١٤",
    "1 2",
    "1e1e1",
    "..1",
    "0e0",
];

/// Free objects and commands with numbers at and past the limits of `f64`.
fn number_lines() -> Vec<String> {
    let mut lines = Vec::new();
    for n in NUMBER_SPECIALS {
        lines.push(format!("P = ({n}, 1)"));
        lines.push(format!("P = (1, {n})"));
        lines.push(format!("P = ({n}, {n})"));
        lines.push(format!("r = {n}"));
        lines.push(format!("P = ({n})"));
        lines.push(format!("P = {n}"));
    }
    let nines = "9".repeat(400);
    let zeros = format!("0.{}1", "0".repeat(400));
    for n in [nines, zeros] {
        lines.push(format!("P = ({n}, {n})"));
        lines.push(format!("r = {n}"));
        // exact decimals of the size cap and beyond
        lines.push(format!("r = {n} {n} - {n}^2"));
        lines.push(format!("r = ({n} + 1)^999999 / {n}"));
    }
    // at the work bound of one operation: 3000-digit divisors and products,
    // several per line, and alignments over 100000 digits
    let long = format!("1.{}7", "3".repeat(2998));
    lines.push(format!(
        "r = 1 / {long} + 2 / {long} + 3 / {long} + q / {long}"
    ));
    lines.push(format!(
        "r = {long} {long} + {long} {long} - {long} q {long}"
    ));
    lines.push(format!("r = ({long})^999 / ({long})^998"));
    lines.push("r = (1E-99990 + 1) + (1E-99990 + 2) + (1E-99990 + 3) - 6".to_string());
    lines.push("r = (1E-100001 + 1) (1E-100001 + q) - 1 + 0.1".to_string());
    for exponent in ["999999999", "-999999999", "99999999999999999999"] {
        lines.push(format!("r = 1E{exponent} + 1"));
        lines.push(format!("r = 1.5E{exponent} * 0.1^999999"));
    }
    lines
}

/// Lines that are long, deeply nested or have very many arguments.
fn size_lines() -> Vec<String> {
    let mut lines = Vec::new();
    for n in [100usize, 1_000, 100_000] {
        let open = "(".repeat(n);
        let close = ")".repeat(n);
        lines.push(format!("A = {open}{close}"));
        lines.push(format!("r = {}", vec!["(1)"; n].join(" + ")));
        lines.push(format!("r = sin^2{}", "(1)^2".repeat(n)));
        lines.push(format!("A = {open}1, 2{close}"));
        lines.push(format!("A = Line({open}B{close})"));
        lines.push(format!("A = {}B{}", "Line(".repeat(n), close));
        lines.push(format!("A = Line({}B)", "A, ".repeat(n)));
        lines.push(format!("A = Circle({})", vec!["B"; n].join(",")));
        lines.push(format!("A = ({}, 1)", "1".repeat(n)));
        lines.push(format!("{} = (1, 1)", "x".repeat(n)));
        lines.push(format!("{} = (1, 1)", "_".repeat(n)));
        lines.push(format!("A = (1,2){}", ",".repeat(n)));
        lines.push("=".repeat(n));
        lines.push(format!("# {}", "x".repeat(n)));
        lines.push(format!("{}#", " ".repeat(n)));
        lines.push(format!("A = Line(B,{})", " ".repeat(n)));
        lines.push(format!("A = {}", "é".repeat(n)));
        lines.push("\u{301}".repeat(n));
    }
    lines
}

// ------------------------------------------------------------ mutation

fn mutate(rng: &mut Rng, line: &str) -> String {
    let mut chars: Vec<char> = line.chars().collect();
    let labels: Vec<String> = CONTEXT
        .iter()
        .filter_map(|l| l.split(" = ").next().map(str::to_string))
        .chain(["t_{1}".to_string(), "t_{2}".to_string()])
        .collect();
    match rng.below(12) {
        0 if !chars.is_empty() => {
            chars.remove(rng.below(chars.len()));
        }
        1 if !chars.is_empty() => {
            let i = rng.below(chars.len());
            chars.insert(i, chars[i]);
        }
        2 if chars.len() > 1 => {
            let i = rng.below(chars.len() - 1);
            chars.swap(i, i + 1);
        }
        3 => {
            let i = rng.below(chars.len() + 1);
            for (k, ch) in rng.pick(TOKENS).chars().enumerate() {
                chars.insert(i + k, ch);
            }
        }
        4 if !chars.is_empty() => {
            let i = rng.below(chars.len());
            chars[i] = random_char(rng);
        }
        5 if !chars.is_empty() => chars.truncate(rng.below(chars.len())),
        6 if chars.len() > 2 => {
            // duplicate a stretch
            let i = rng.below(chars.len());
            let j = (i + 1 + rng.below(chars.len() - i)).min(chars.len());
            let piece: Vec<char> = chars[i..j].to_vec();
            for (k, ch) in piece.into_iter().enumerate() {
                chars.insert(j + k, ch);
            }
        }
        7..=9 => return mutate_arguments(rng, line, &labels),
        _ => return mutate_numbers(rng, line),
    }
    chars.into_iter().collect()
}

/// Deletes, duplicates or swaps arguments, or replaces one with another
/// label, which is often one of the wrong kind.
fn mutate_arguments(rng: &mut Rng, line: &str, labels: &[String]) -> String {
    let (Some(open), Some(close)) = (line.find('('), line.rfind(')')) else {
        return line.to_string();
    };
    if close <= open {
        return line.to_string();
    }
    let mut args: Vec<String> = line[open + 1..close]
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();
    match rng.below(5) {
        0 if !args.is_empty() => {
            args.remove(rng.below(args.len()));
        }
        1 if !args.is_empty() => {
            let i = rng.below(args.len());
            args.insert(i, args[i].clone());
        }
        2 if args.len() > 1 => {
            let (i, j) = (rng.below(args.len()), rng.below(args.len()));
            args.swap(i, j);
        }
        3 => args.push(rng.pick(labels).clone()),
        _ if !args.is_empty() => {
            let i = rng.below(args.len());
            args[i] = rng.pick(labels).clone();
        }
        _ => {}
    }
    format!("{}({}{}", &line[..open], args.join(", "), &line[close..])
}

/// Replaces a number in the line by a special one.
fn mutate_numbers(rng: &mut Rng, line: &str) -> String {
    let bytes = line.as_bytes();
    let digits: Vec<usize> = (0..bytes.len())
        .filter(|&i| bytes[i].is_ascii_digit())
        .collect();
    let Some(&at) = digits.get(rng.below(digits.len().max(1))) else {
        return format!("{line}{}", rng.pick(NUMBER_SPECIALS));
    };
    let mut start = at;
    while start > 0 && (bytes[start - 1].is_ascii_digit() || bytes[start - 1] == b'.') {
        start -= 1;
    }
    let mut end = at + 1;
    while end < bytes.len() && (bytes[end].is_ascii_digit() || bytes[end] == b'.') {
        end += 1;
    }
    format!(
        "{}{}{}",
        &line[..start],
        rng.pick(NUMBER_SPECIALS),
        &line[end..]
    )
}

// --------------------------------------------------------------- tests

#[test]
fn random_text_never_panics() {
    with_timeout("random text", 120, || {
        let mut findings = Findings::default();
        let prefix = context_lines();
        let pool = context();
        for i in 0..scaled(20_000) {
            let mut rng = Rng::new(case_seed(0xF022, i));
            // a third of the inputs are valid command templates, so that the
            // checks on successful lines run on many successes
            let line = if i % 3 == 0 {
                template_line(&mut rng, &pool)
            } else {
                garbage_line(&mut rng)
            };
            findings.run(&prefix, &line);
        }
        // several lines in one construction, as `load` runs them
        for i in 0..scaled(3_000) {
            let mut rng = Rng::new(case_seed(0xF023, i));
            let src = garbage_script(&mut rng);
            let outcome = catch(|| load(&src));
            match outcome {
                Err(panic) => {
                    findings.by_key.entry(format!("panic: {panic}")).or_insert((
                        shrink(&src, &|s| catch(|| load(s)).is_err()),
                        "found by `load`".into(),
                        1,
                    ));
                }
                Ok(Ok(c)) => {
                    if let Some(bad) = first_non_finite(&c) {
                        findings
                            .by_key
                            .entry("load: non-finite value".into())
                            .or_insert((src.clone(), bad, 1));
                    }
                }
                Ok(Err(_)) => {}
            }
            findings.runs += 1;
        }
        findings.report("random text", 3000);
    });
}

#[test]
fn numbers_at_the_limits_and_oversized_lines() {
    with_timeout("numbers and sizes", 120, || {
        let mut findings = Findings::default();
        let prefix = context_lines();
        for line in number_lines().iter().chain(&size_lines()) {
            findings.run(&prefix, line);
        }
        // objects defined from the special numbers must stay finite under
        // every command
        for n in NUMBER_SPECIALS {
            let mut c = context();
            for def in [
                format!("P = ({n}, {n})"),
                format!("r = {n}"),
                format!("Q = ({n}, 1)"),
            ] {
                let _ = catch(|| exec(&mut c, &def));
            }
            let mut rng = Rng::new(case_seed(0xFA11, n.len() as u64));
            for round in 0..300 {
                let cmd = rng.pick(COMMANDS);
                if let Some(call) = pick_args(&mut rng, &c, cmd, usize::MAX) {
                    let line = format!("z{round} = {}", call.text);
                    match check_line(&mut c, &line) {
                        Ok(created) => findings.created += usize::from(created),
                        Err(p) => {
                            findings.by_key.entry(p.key).or_insert((line, p.detail, 1));
                        }
                    }
                }
            }
            findings.runs += 300;
        }
        findings.report("numbers and sizes", 3000);
    });
}

#[test]
fn mutated_corpus_lines_never_panic() {
    with_timeout("mutated corpus", 180, || {
        let mut findings = Findings::default();

        // case scripts: mutate one line, run the whole script
        let scripts: Vec<Vec<String>> = case_scripts();
        for (i, script) in scripts.iter().enumerate() {
            if script.is_empty() {
                continue;
            }
            for round in 0..scaled(25) {
                let mut rng = Rng::new(case_seed(0xAB1E, (i as u64) << 8 | round));
                let target = rng.below(script.len());
                let mut mutated = mutate(&mut rng, &script[target]);
                if rng.chance(0.3) {
                    mutated = mutate(&mut rng, &mutated);
                }
                let mut c = Construction::new();
                for (j, line) in script.iter().enumerate() {
                    if j == target {
                        findings.run_in(&mut c, &script[..j], &mutated);
                        findings.runs += 1;
                    } else {
                        findings.run_in(&mut c, &script[..j], line);
                    }
                }
            }
        }

        // README, built-in and context lines: mutate in the common context
        let mut corpus = readme_lines();
        corpus.extend(builtin_lines());
        corpus.extend(CONTEXT.iter().map(|s| s.to_string()));
        let prefix = context_lines();
        for i in 0..scaled(20_000) {
            let mut rng = Rng::new(case_seed(0xAB1F, i));
            let mut line = rng.pick(&corpus).clone();
            for _ in 0..1 + rng.below(3) {
                line = mutate(&mut rng, &line);
            }
            findings.run(&prefix, &line);
        }
        findings.report("mutated corpus", 20_000);
    });
}

#[test]
fn the_nested_sequence_mutation_does_bounded_work() {
    // A mutation of `lists-seq-011` (`b = 4` to `b = 1E5`) once built about
    // 5e9 elements in its last line and took 92 s; it is in the corpus only
    // for some seeds, so it is checked here on its own.
    let script = [
        "a = 1",
        "b = 1E5",
        "s = 0.5",
        "A = (1, 2)",
        "l = Sequence(Sequence(j + k, j, 1, k), k, a, b)",
    ];
    let mut c = Construction::new();
    for line in script {
        if let Err(p) = check_line(&mut c, line) {
            panic!("`{line}`: {}: {}", p.key, p.detail);
        }
    }
    assert_eq!(c.value(c.id("l").unwrap()), geo_kernel::Value::Undefined);
}

#[test]
fn a_costly_function_in_a_sequence_does_bounded_work() {
    // A function of about 240 000 nodes, functions expanded, applied for
    // each value of a range of 30 000 took 19 s of a release build before
    // evaluation work was bounded. At 1400 values the list does twice
    // `MAX_LINE_WORK` unbounded, so that a bound raised past it fails here
    // through the counter, before the clock.
    let script = [
        "f(x) = x + x + x + x",
        "g(x) = f(f(f(f(f(f(f(f(x))))))))",
        "s = Sequence(g(k), k, 1, 1400)",
    ];
    let mut c = Construction::new();
    for line in script {
        if let Err(p) = check_line(&mut c, line) {
            panic!("`{line}`: {}: {}", p.key, p.detail);
        }
    }
    assert_eq!(c.value(c.id("s").unwrap()), geo_kernel::Value::Undefined);
}

#[test]
fn unmodified_corpus_lines_behave() {
    // The unmutated lines go through the same checks, so that a failure of
    // a mutated one can be told from a failure of the corpus itself.
    let mut findings = Findings::default();
    let prefix = context_lines();
    for line in readme_lines().iter().chain(&builtin_lines()) {
        findings.run(&prefix, line);
    }
    findings.report("unmodified corpus", 50);
}
