//! Shared helpers for the property and fuzz tests: a deterministic PRNG,
//! value accessors that do not depend on how lines are scaled, a table of
//! script commands, a random construction generator, panic capture and a
//! watchdog.
//!
//! Only the public API of the kernel is used.
#![allow(dead_code)]

use geo_kernel::script::exec;
use geo_kernel::{Construction, Def, Id, Kind, Value};
use std::cell::{Cell, RefCell};
use std::panic::{self, AssertUnwindSafe};
use std::sync::mpsc;
use std::sync::Once;
use std::time::Duration;

// ------------------------------------------------------------------- PRNG

/// SplitMix64: small, fast, and the same sequence on every platform.
#[derive(Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed)
    }

    /// The current state; for a fresh generator, the seed.
    pub fn state(&self) -> u64 {
        self.0
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `[0, 1)`.
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Uniform in `[lo, hi)`.
    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.unit()
    }

    /// Uniform in `0..n`; `n` must not be zero.
    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }

    pub fn chance(&mut self, p: f64) -> bool {
        self.unit() < p
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }

    pub fn point(&mut self, half: f64) -> (f64, f64) {
        (self.range(-half, half), self.range(-half, half))
    }
}

/// Seed of the `i`-th case of a test, spread so that neighbouring cases do
/// not share a prefix of the sequence.
pub fn case_seed(salt: u64, i: u64) -> u64 {
    Rng::new(salt ^ i.wrapping_mul(0xD6E8_FEB8_6659_FD93)).next_u64()
}

// ------------------------------------------------------ value accessors

pub fn point_of(v: &Value) -> Option<(f64, f64)> {
    match *v {
        Value::Point { x, y } => Some((x, y)),
        _ => None,
    }
}

pub fn line_of(v: &Value) -> Option<(f64, f64, f64)> {
    match *v {
        Value::Line { a, b, c } => Some((a, b, c)),
        _ => None,
    }
}

pub fn circle_of(v: &Value) -> Option<(f64, f64, f64)> {
    match *v {
        Value::Circle { cx, cy, r } => Some((cx, cy, r)),
        _ => None,
    }
}

pub fn segment_of(v: &Value) -> Option<((f64, f64), (f64, f64))> {
    match *v {
        Value::Segment { x1, y1, x2, y2 } => Some(((x1, y1), (x2, y2))),
        _ => None,
    }
}

pub fn number_of(v: &Value) -> Option<f64> {
    match *v {
        Value::Number(n) => Some(n),
        _ => None,
    }
}

pub fn val(c: &Construction, label: &str) -> Value {
    match c.id(label) {
        Some(id) => c.value(id),
        None => panic!("no object `{label}`"),
    }
}

pub fn pt(c: &Construction, label: &str) -> (f64, f64) {
    point_of(&val(c, label)).unwrap_or_else(|| panic!("`{label}` is {:?}", val(c, label)))
}

pub fn line(c: &Construction, label: &str) -> (f64, f64, f64) {
    line_of(&val(c, label)).unwrap_or_else(|| panic!("`{label}` is {:?}", val(c, label)))
}

pub fn circle(c: &Construction, label: &str) -> (f64, f64, f64) {
    circle_of(&val(c, label)).unwrap_or_else(|| panic!("`{label}` is {:?}", val(c, label)))
}

pub fn segment(c: &Construction, label: &str) -> ((f64, f64), (f64, f64)) {
    segment_of(&val(c, label)).unwrap_or_else(|| panic!("`{label}` is {:?}", val(c, label)))
}

pub fn num(c: &Construction, label: &str) -> f64 {
    number_of(&val(c, label)).unwrap_or_else(|| panic!("`{label}` is {:?}", val(c, label)))
}

/// Every number stored in a value, found through its `Debug` text so that a
/// new kind of object is covered without changing this file.
pub fn numbers_in(v: &Value) -> Vec<f64> {
    let text = format!("{v:?}");
    text.split(|ch: char| !(ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '+')))
        .filter_map(|tok| tok.parse::<f64>().ok())
        .collect()
}

pub fn is_all_finite(v: &Value) -> bool {
    numbers_in(v).iter().all(|n| n.is_finite())
}

/// The first object whose value holds a non-finite number.
pub fn first_non_finite(c: &Construction) -> Option<String> {
    c.ids().find_map(|id| {
        let v = c.value(id);
        (!is_all_finite(&v)).then(|| format!("`{}` = {v:?}", c.label(id)))
    })
}

pub fn values(c: &Construction) -> Vec<Value> {
    c.ids().map(|id| c.value(id)).collect()
}

/// Describes the first difference between two value lists (`==` on `f64`,
/// so `-0.0` and `0.0` are the same).
pub fn first_difference(c: &Construction, a: &[Value], b: &[Value]) -> Option<String> {
    if a.len() != b.len() {
        return Some(format!("{} objects against {}", a.len(), b.len()));
    }
    c.ids()
        .zip(a.iter().zip(b))
        .find(|(_, (x, y))| x != y)
        .map(|(id, (x, y))| format!("`{}`: {x:?} against {y:?}", c.label(id)))
}

// ------------------------------------------------------ geometry helpers

pub const TOL: f64 = 1e-9;

/// `|a - b| <= TOL * max(1, scale)`.
pub fn near(a: f64, b: f64, scale: f64) -> bool {
    (a - b).abs() <= TOL * scale.abs().max(1.0)
}

pub fn expect_near(what: &str, a: f64, b: f64, scale: f64) -> Result<(), String> {
    if near(a, b, scale) {
        Ok(())
    } else {
        Err(format!(
            "{what}: {a:e} against {b:e} (difference {:e}, tolerance {:e})",
            (a - b).abs(),
            TOL * scale.abs().max(1.0)
        ))
    }
}

pub fn dist(p: (f64, f64), q: (f64, f64)) -> f64 {
    (p.0 - q.0).hypot(p.1 - q.1)
}

/// Distance from a point to the line `a*x + b*y + c = 0`, whatever the
/// scale of the coefficients.
pub fn dist_to_line(p: (f64, f64), l: (f64, f64, f64)) -> f64 {
    (l.0 * p.0 + l.1 * p.1 + l.2).abs() / l.0.hypot(l.1)
}

/// Two points of a line, 6 units apart around the foot of the origin.
pub fn points_on(l: (f64, f64, f64)) -> [(f64, f64); 2] {
    let n2 = l.0 * l.0 + l.1 * l.1;
    let foot = (-l.0 * l.2 / n2, -l.1 * l.2 / n2);
    let len = n2.sqrt();
    let dir = (l.1 / len, -l.0 / len);
    [
        (foot.0 - 3.0 * dir.0, foot.1 - 3.0 * dir.1),
        (foot.0 + 3.0 * dir.0, foot.1 + 3.0 * dir.1),
    ]
}

/// Unit normal and offset, with a sign that does not depend on the input's.
pub fn canonical_line(l: (f64, f64, f64)) -> (f64, f64, f64) {
    let len = l.0.hypot(l.1);
    let (a, b, c) = (l.0 / len, l.1 / len, l.2 / len);
    if a < 0.0 || (a == 0.0 && b < 0.0) {
        (-a, -b, -c)
    } else {
        (a, b, c)
    }
}

/// Same line up to scale and sign. Lines whose normal is nearly horizontal
/// or vertical may flip sign, so both signs are tried.
pub fn same_line(l: (f64, f64, f64), m: (f64, f64, f64), scale: f64) -> bool {
    let (p, q) = (canonical_line(l), canonical_line(m));
    let eq =
        |s: f64| near(p.0, s * q.0, 1.0) && near(p.1, s * q.1, 1.0) && near(p.2, s * q.2, scale);
    eq(1.0) || eq(-1.0)
}

pub fn same_point(p: (f64, f64), q: (f64, f64), scale: f64) -> bool {
    near(p.0, q.0, scale) && near(p.1, q.1, scale)
}

/// Twice the signed area of a triangle.
pub fn cross3(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> f64 {
    (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
}

/// A triangle that is far from degenerate: sides of at least 2 and an area
/// of at least 4.
pub fn triangle(rng: &mut Rng, half: f64) -> [(f64, f64); 3] {
    loop {
        let t = [rng.point(half), rng.point(half), rng.point(half)];
        let sides = [dist(t[0], t[1]), dist(t[1], t[2]), dist(t[2], t[0])];
        if sides.iter().all(|&s| s >= 2.0) && cross3(t[0], t[1], t[2]).abs() >= 8.0 {
            return t;
        }
    }
}

pub fn id_at(c: &Construction, index: usize) -> Id {
    c.ids().nth(index).expect("object exists")
}

pub fn script_of(lines: &[String]) -> String {
    lines.join("\n")
}

/// Builds a construction from script text; panics with the script on error.
pub fn build(src: &str) -> Construction {
    match geo_kernel::script::load(src) {
        Ok(c) => c,
        Err(e) => panic!("script failed: {e}\n{src}"),
    }
}

/// Number of cases, multiplied by the `GEO_TEST_SCALE` environment variable
/// (default 1) for longer local runs.
pub fn scaled(n: u64) -> u64 {
    let factor = std::env::var("GEO_TEST_SCALE")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(1);
    n * factor.max(1)
}

/// Runs `f` for `n` cases with fixed seeds. A failure panics with the seed,
/// so that the case can be replayed with `Rng::new(seed)`.
pub fn for_seeds(name: &str, salt: u64, n: u64, mut f: impl FnMut(&mut Rng) -> Result<(), String>) {
    let mut failures = Vec::new();
    for i in 0..scaled(n) {
        let seed = case_seed(salt, i);
        let mut rng = Rng::new(seed);
        if let Err(msg) = f(&mut rng) {
            failures.push(format!("seed {seed:#018x} (case {i}): {msg}"));
            if failures.len() >= 3 {
                break;
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{name}: {} failing case(s), first ones:\n{}",
        failures.len(),
        failures.join("\n---\n")
    );
}

/// Appends the script to an error, for replaying.
pub fn with_script(src: &str, r: Result<(), String>) -> Result<(), String> {
    r.map_err(|e| format!("{e}\nscript:\n{src}"))
}

// -------------------------------------------------------- command table

/// Kinds a command argument can take.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum K {
    Point,
    Line,
    Segment,
    Circle,
    Number,
    /// A literal intersection index, `1` or `2`.
    Index,
}

impl K {
    pub fn fits(self, kind: Kind) -> bool {
        matches!(
            (self, kind),
            (K::Point, Kind::Point)
                | (K::Line, Kind::Line)
                | (K::Segment, Kind::Segment)
                | (K::Circle, Kind::Circle)
                | (K::Number, Kind::Number)
        )
    }
}

type Slot = &'static [K];

const PT: Slot = &[K::Point];
const LN: Slot = &[K::Line];
const SG: Slot = &[K::Segment];
const CI: Slot = &[K::Circle];
const NU: Slot = &[K::Number];
const IX: Slot = &[K::Index];
/// A line or a segment.
const LL: Slot = &[K::Line, K::Segment];
/// Everything a transformation accepts.
const SRC: Slot = &[K::Point, K::Line, K::Segment, K::Circle];
/// What a shear or stretch accepts (a circle becomes an ellipse).
const AFFINE: Slot = &[K::Point, K::Line, K::Segment];

/// One accepted form of a script command. Adding a command to the tests is
/// adding a line to [`COMMANDS`].
pub struct Cmd {
    pub name: &'static str,
    pub slots: &'static [Slot],
    /// The command can create several objects, and which ones depends on
    /// the positions when it runs.
    pub multi: bool,
    /// The result is known to depend on the history of moves; see the
    /// ignored tests in `properties.rs`. Such commands are left out of the
    /// random constructions unless asked for.
    pub history_dependent: bool,
}

const fn cmd(name: &'static str, slots: &'static [Slot]) -> Cmd {
    Cmd {
        name,
        slots,
        multi: false,
        history_dependent: false,
    }
}

const fn multi(name: &'static str, slots: &'static [Slot]) -> Cmd {
    Cmd {
        name,
        slots,
        multi: true,
        history_dependent: false,
    }
}

const fn history_dependent(name: &'static str, slots: &'static [Slot]) -> Cmd {
    Cmd {
        name,
        slots,
        multi: false,
        history_dependent: true,
    }
}

pub const COMMANDS: &[Cmd] = &[
    // built-in
    cmd("Line", &[PT, PT]),
    cmd("Line", &[PT, LL]),
    cmd("Segment", &[PT, PT]),
    cmd("Midpoint", &[PT, PT]),
    cmd("Distance", &[PT, PT]),
    cmd("Perpendicular", &[PT, LL]),
    cmd("PerpendicularLine", &[PT, LL]),
    cmd("OrthogonalLine", &[PT, LL]),
    cmd("Parallel", &[PT, LL]),
    cmd("Circle", &[PT, PT]),
    cmd("Circle", &[PT, NU]),
    cmd("Intersect", &[LN, LN]),
    cmd("Intersect", &[LN, LN, IX]),
    cmd("Intersect", &[LN, CI]),
    cmd("Intersect", &[CI, LN, IX]),
    cmd("Intersect", &[CI, CI]),
    cmd("Intersect", &[CI, CI, IX]),
    // lines
    cmd("LineBisector", &[PT, PT]),
    cmd("PerpendicularBisector", &[PT, PT]),
    cmd("LineBisector", &[SG]),
    cmd("AngularBisector", &[PT, PT, PT]),
    cmd("AngleBisector", &[PT, PT, PT]),
    multi("AngularBisector", &[LL, LL]),
    multi("AngleBisector", &[LL, LL]),
    multi("Tangent", &[PT, CI]),
    multi("Tangent", &[CI, PT]),
    multi("Tangent", &[LL, CI]),
    cmd("Polar", &[PT, CI]),
    // conics
    cmd("Circle", &[PT, PT, PT]),
    cmd("Circle", &[PT, SG]),
    cmd("Incircle", &[PT, PT, PT]),
    cmd("Center", &[CI]),
    cmd("Center", &[SG]),
    cmd("Midpoint", &[CI]),
    cmd("Midpoint", &[SG]),
    cmd("Diameter", &[LL, CI]),
    // measures
    cmd("Length", &[SG]),
    cmd("Radius", &[CI]),
    cmd("Circumference", &[CI]),
    cmd("Perimeter", &[CI]),
    cmd("Area", &[CI]),
    cmd("Area", &[PT, PT, PT]),
    cmd("Area", &[PT, PT, PT, PT]),
    history_dependent("Slope", &[LL]),
    cmd("Distance", &[PT, LL]),
    cmd("Distance", &[LL, PT]),
    history_dependent("Distance", &[LL, LL]),
    cmd("AffineRatio", &[PT, PT, PT]),
    cmd("CrossRatio", &[PT, PT, PT, PT]),
    cmd("Angle", &[PT, PT, PT]),
    // transformations
    cmd("Mirror", &[SRC, PT]),
    cmd("Mirror", &[SRC, LL]),
    cmd("Reflect", &[SRC, LL]),
    cmd("Mirror", &[PT, CI]),
    cmd("Rotate", &[SRC, NU]),
    cmd("Rotate", &[SRC, NU, PT]),
    cmd("Dilate", &[SRC, NU]),
    cmd("Dilate", &[SRC, NU, PT]),
    cmd("Shear", &[AFFINE, LL, NU]),
    cmd("Stretch", &[AFFINE, LL, NU]),
];

/// A command call, as script text and as the objects it reads.
pub struct Call {
    pub text: String,
    pub multi: bool,
    pub args: Vec<String>,
}

/// Picks arguments for `cmd` among the objects of `c` (restricted to the
/// first `limit` objects when given). `None` when some slot has no
/// candidate.
pub fn pick_args(rng: &mut Rng, c: &Construction, cmd: &Cmd, limit: usize) -> Option<Call> {
    let mut args = Vec::new();
    let mut texts = Vec::new();
    for slot in cmd.slots {
        if slot.contains(&K::Index) {
            texts.push(if rng.chance(0.5) { "1" } else { "2" }.to_string());
            continue;
        }
        let candidates: Vec<Id> = c
            .ids()
            .take(limit)
            .filter(|&id| slot.iter().any(|k| k.fits(c.kind(id))))
            .collect();
        if candidates.is_empty() {
            return None;
        }
        let label = c.label(*rng.pick(&candidates)).to_string();
        texts.push(label.clone());
        args.push(label);
    }
    Some(Call {
        text: format!("{}({})", cmd.name, texts.join(", ")),
        multi: cmd.multi,
        args,
    })
}

// ---------------------------------------------- random constructions

const POINT_NAMES: [&str; 6] = ["A", "B", "C", "D", "E", "F"];
const NUMBER_NAMES: [&str; 2] = ["r", "q"];

/// A coordinate that often lands on a lattice or on an existing coordinate,
/// so that coincident and collinear points come up, and sometimes at an
/// extreme magnitude.
pub fn random_coordinate(rng: &mut Rng, existing: &[f64]) -> f64 {
    let roll = rng.unit();
    if roll < 0.35 {
        rng.below(11) as f64 - 5.0
    } else if roll < 0.45 && !existing.is_empty() {
        *rng.pick(existing)
    } else if roll < 0.48 {
        *rng.pick(&[1e150, -1e150, 1e-150, 1e300, -1e300, 1e-300, 0.0, -0.0])
    } else {
        rng.range(-10.0, 10.0)
    }
}

pub fn random_number(rng: &mut Rng) -> f64 {
    let roll = rng.unit();
    if roll < 0.3 {
        rng.below(9) as f64 - 2.0
    } else if roll < 0.35 {
        *rng.pick(&[0.0, -0.0, 1e-300, 1e300, -1e300, 1e150])
    } else {
        rng.range(-4.0, 8.0)
    }
}

#[derive(Clone, Debug)]
pub enum Step {
    Point { id: Id, x: f64, y: f64 },
    Number { id: Id, v: f64 },
}

impl Step {
    pub fn id(&self) -> Id {
        match *self {
            Step::Point { id, .. } | Step::Number { id, .. } => id,
        }
    }

    pub fn apply(&self, c: &mut Construction) -> Result<usize, geo_kernel::Error> {
        match *self {
            Step::Point { id, x, y } => c.move_point(id, x, y),
            Step::Number { id, v } => c.set_number(id, v),
        }
    }

    pub fn describe(&self, c: &Construction) -> String {
        match *self {
            Step::Point { id, x, y } => format!("move {} to ({x:?}, {y:?})", c.label(id)),
            Step::Number { id, v } => format!("set {} to {v:?}", c.label(id)),
        }
    }
}

/// A generated construction.
pub struct Case {
    pub seed: u64,
    pub c: Construction,
    /// Script lines that built it, in order (free objects included).
    pub lines: Vec<String>,
    /// Which lines are commands that can create several objects.
    pub multi_lines: Vec<bool>,
    /// For each object, the objects it was built from (empty for free ones).
    pub deps: Vec<Vec<usize>>,
    pub free: Vec<Id>,
    /// Commands the generator tried with matching kinds that the script
    /// layer rejected.
    pub rejected: Vec<String>,
}

/// `all_commands` also uses the commands marked `history_dependent`.
pub fn generate(seed: u64, n_commands: usize, all_commands: bool) -> Case {
    let mut rng = Rng::new(seed);
    let mut c = Construction::new();
    let mut lines = Vec::new();
    let mut multi_lines = Vec::new();
    let mut deps: Vec<Vec<usize>> = Vec::new();
    let mut free = Vec::new();
    let mut rejected = Vec::new();

    let n_points = 3 + rng.below(4);
    let n_numbers = 1 + rng.below(2);
    let mut xs: Vec<f64> = Vec::new();
    for name in POINT_NAMES.iter().take(n_points) {
        let x = random_coordinate(&mut rng, &xs);
        let y = random_coordinate(&mut rng, &xs);
        xs.push(x);
        xs.push(y);
        let line = format!("{name} = ({x:?}, {y:?})");
        let id = exec(&mut c, &line)
            .expect("free point")
            .expect("free point is created");
        lines.push(line);
        multi_lines.push(false);
        deps.push(Vec::new());
        free.push(id);
    }
    for name in NUMBER_NAMES.iter().take(n_numbers) {
        let v = random_number(&mut rng);
        let line = format!("{name} = {v:?}");
        let id = exec(&mut c, &line)
            .expect("free number")
            .expect("free number is created");
        lines.push(line);
        multi_lines.push(false);
        deps.push(Vec::new());
        free.push(id);
    }

    let mut made = 0;
    let mut attempts = 0;
    while made < n_commands && attempts < n_commands * 20 {
        attempts += 1;
        let cmd = rng.pick(COMMANDS);
        if cmd.history_dependent && !all_commands {
            continue;
        }
        let Some(call) = pick_args(&mut rng, &c, cmd, usize::MAX) else {
            continue;
        };
        let label = format!("o{made}");
        let line = format!("{label} = {}", call.text);
        let before = c.len();
        match exec(&mut c, &line) {
            Ok(Some(_)) => {
                let arg_ids: Vec<usize> = call
                    .args
                    .iter()
                    .map(|a| c.id(a).expect("argument exists").index())
                    .collect();
                for _ in before..c.len() {
                    deps.push(arg_ids.clone());
                }
                lines.push(line);
                multi_lines.push(call.multi);
                made += 1;
            }
            Ok(None) => unreachable!("a command line creates an object"),
            Err(e) => rejected.push(format!("{line}: {e}")),
        }
    }
    Case {
        seed,
        c,
        lines,
        multi_lines,
        deps,
        free,
        rejected,
    }
}

pub fn random_step(rng: &mut Rng, c: &Construction, free: &[Id]) -> Step {
    let id = *rng.pick(free);
    match c.def(id) {
        Def::Number(_) => Step::Number {
            id,
            v: random_number(rng),
        },
        _ => {
            let xs: Vec<f64> = free
                .iter()
                .filter_map(|&f| match c.value(f) {
                    Value::Point { x, y } => Some([x, y]),
                    _ => None,
                })
                .flatten()
                .collect();
            Step::Point {
                id,
                x: random_coordinate(rng, &xs),
                y: random_coordinate(rng, &xs),
            }
        }
    }
}

// ------------------------------------------------------ panic capture

thread_local! {
    static CAPTURING: Cell<bool> = const { Cell::new(false) };
    static LAST_PANIC: RefCell<Option<String>> = const { RefCell::new(None) };
}

static HOOK: Once = Once::new();

/// Runs `f`, turning a panic into `Err("message at file:line")` without
/// printing it. Panics of other threads still use the default hook.
pub fn catch<T>(f: impl FnOnce() -> T) -> Result<T, String> {
    HOOK.call_once(|| {
        let previous = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            if CAPTURING.with(Cell::get) {
                let payload = info.payload();
                let msg = payload
                    .downcast_ref::<&str>()
                    .map(|s| s.to_string())
                    .or_else(|| payload.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "non-string panic".into());
                let at = info
                    .location()
                    .map(|l| format!("{}:{}", file_name(l.file()), l.line()))
                    .unwrap_or_default();
                LAST_PANIC.with(|p| *p.borrow_mut() = Some(format!("{msg} at {at}")));
            } else {
                previous(info);
            }
        }));
    });
    CAPTURING.with(|c| c.set(true));
    let result = panic::catch_unwind(AssertUnwindSafe(f));
    CAPTURING.with(|c| c.set(false));
    result.map_err(|_| {
        LAST_PANIC
            .with(|p| p.borrow_mut().take())
            .unwrap_or_else(|| "panic".into())
    })
}

fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// Runs `f` on its own thread and fails if it takes longer than `secs`.
/// A panic in `f` is passed on.
pub fn with_timeout(name: &str, secs: u64, f: impl FnOnce() + Send + 'static) {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let r = panic::catch_unwind(AssertUnwindSafe(f));
        let _ = tx.send(r);
    });
    match rx.recv_timeout(Duration::from_secs(secs)) {
        Ok(Ok(())) => {}
        Ok(Err(p)) => panic::resume_unwind(p),
        Err(_) => panic!("{name} did not finish within {secs} s (a hang or a very slow input)"),
    }
}

/// Removes chunks of `input` while `fails` stays true.
pub fn shrink(input: &str, fails: &dyn Fn(&str) -> bool) -> String {
    let mut chars: Vec<char> = input.chars().collect();
    let mut budget = 3000;
    let mut chunk = chars.len().div_ceil(2).max(1);
    loop {
        let mut i = 0;
        while i < chars.len() && budget > 0 {
            budget -= 1;
            let end = (i + chunk).min(chars.len());
            let candidate: String = chars[..i].iter().chain(&chars[end..]).collect();
            if fails(&candidate) {
                chars.drain(i..end);
            } else {
                i += chunk;
            }
        }
        if chunk == 1 || budget == 0 {
            break;
        }
        chunk = chunk.div_ceil(2);
    }
    chars.into_iter().collect()
}
