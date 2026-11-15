//! Shared helpers for the property and fuzz tests: a deterministic PRNG,
//! value accessors that do not depend on how lines are scaled, a table of
//! script commands, a random construction generator, panic capture and a
//! watchdog.
//!
//! Only the public API of the kernel is used.
#![allow(dead_code)]

use geo_kernel::algo::arc::ArcDef;
use geo_kernel::script::exec;
use geo_kernel::{ArcData, ConicData, ConicType, Construction, Def, Id, Kind, PathState, Value};
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

/// The value of an object together with its conic record
/// (`Construction::conic`), so that comparisons also see the matrix and the
/// parts of a conic, which [`Value::Conic`] does not hold, and with the
/// elements of a list written out in order: a nested list as its
/// `Value::List` followed by its elements.
#[derive(Clone, Debug)]
pub struct Snap {
    pub value: Value,
    pub conic: Option<ConicData>,
    /// The state of a point on a path (`Construction::path_state`),
    /// compared bit for bit.
    pub path: Option<PathState>,
    /// The record of an arc or sector (`Construction::arc`), compared by
    /// [`arc_bits`].
    pub arc: Option<ArcData>,
    pub elements: Vec<Value>,
}

impl Snap {
    pub fn is_defined(&self) -> bool {
        self.value.is_defined()
    }
}

/// The matrix, type and parts of a defined conic record, bit for bit (but
/// `-0.0` is `0.0`): the
/// parts the golden files record for its type (`golden/SCHEMA.md`). The
/// other fields of a record keep what an earlier type left in them, as in
/// GeoGebra's `GeoConic`, so they depend on the history of moves and are not
/// part of the conic. An undefined record is compared only as undefined, for
/// the same reason.
fn conic_bits(d: &ConicData) -> Vec<u64> {
    if !d.defined {
        return vec![0];
    }
    let mut numbers: Vec<f64> = d.matrix.to_vec();
    let eigenvectors = d.eigenvec.iter().flatten().copied();
    match d.ty {
        ConicType::Circle | ConicType::Ellipse | ConicType::Hyperbola => {
            numbers.extend(d.midpoint.iter().chain(&d.half_axes));
            numbers.extend(eigenvectors);
        }
        ConicType::Parabola => {
            numbers.extend(d.midpoint.iter().chain([&d.p]));
            numbers.extend(eigenvectors);
        }
        ConicType::SinglePoint => numbers.extend(d.midpoint),
        ConicType::IntersectingLines | ConicType::ParallelLines | ConicType::DoubleLine => {
            // The order of the two lines follows the order they had before
            // (GeoGebra keeps the line nearer the previous first line first),
            // so it depends on the history: compare them as a pair.
            let mut lines = d.lines;
            lines.sort_by_key(|l| l.map(f64::to_bits));
            numbers.extend(lines.iter().flatten())
        }
        ConicType::Empty | ConicType::NotClassified => {}
    }
    let mut bits = vec![1, d.ty as u64];
    // `-0.0` and `0.0` are the same, as in the comparison of values.
    bits.extend(
        numbers
            .iter()
            .map(|&v| if v == 0.0 { 0 } else { v.to_bits() }),
    );
    bits
}

/// The record of an arc or sector, bit for bit (but `-0.0` is `0.0`): its
/// conic by [`conic_bits`], its kind, orientation and parameter range, its
/// value and measures and the end points of a degenerate part.
fn arc_bits(a: &ArcData) -> Vec<u64> {
    let mut bits = conic_bits(&a.conic);
    bits.extend([a.sector, a.positive, a.defined].map(u64::from));
    let mut numbers = vec![a.start, a.end, a.extent, a.value, a.arc_length(), a.area()];
    if let Some([p, q]) = a.segment() {
        numbers.extend([p.0, p.1, q.0, q.1]);
    }
    bits.extend(
        numbers
            .iter()
            .map(|&v| if v == 0.0 { 0 } else { v.to_bits() }),
    );
    bits
}

/// Values with `==` on `f64` (so `-0.0` and `0.0` are the same), conic
/// records by [`conic_bits`], arc records by [`arc_bits`], list elements
/// with `==`.
impl PartialEq for Snap {
    fn eq(&self, other: &Snap) -> bool {
        self.value == other.value
            && self.conic.as_ref().map(conic_bits) == other.conic.as_ref().map(conic_bits)
            && self.path == other.path
            && self.arc.as_ref().map(arc_bits) == other.arc.as_ref().map(arc_bits)
            && self.elements == other.elements
    }
}

fn write_elements(list: geo_kernel::expr::List<'_>, out: &mut Vec<Value>) {
    for e in list.iter() {
        match e {
            geo_kernel::expr::Element::Value(_, v) => out.push(v),
            geo_kernel::expr::Element::List(inner) => {
                out.push(Value::List {
                    len: inner.len() as u32,
                });
                write_elements(inner, out);
            }
        }
    }
}

pub fn snap(c: &Construction, id: Id) -> Snap {
    let mut elements = Vec::new();
    if let Some(list) = c.elements(id) {
        write_elements(list, &mut elements);
    }
    Snap {
        value: c.value(id),
        conic: c.conic(id),
        path: c.path_state(id),
        arc: c.arc(id),
        elements,
    }
}

pub fn values(c: &Construction) -> Vec<Snap> {
    c.ids().map(|id| snap(c, id)).collect()
}

/// Describes the first difference between two value lists (see [`Snap`]).
pub fn first_difference(c: &Construction, a: &[Snap], b: &[Snap]) -> Option<String> {
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

/// A number as a script literal. Rust prints `1e300`, but only an upper-case
/// `E` starts an exponent in the script language (`1e300` is `1·e300`).
pub fn lit(v: f64) -> String {
    format!("{v:?}").replace('e', "E")
}

/// `src` with every exponent written as Rust prints it (`1e300`, `2.5e-7`)
/// turned into a script literal (`1E300`).
pub fn upper_exponents(src: &str) -> String {
    let b: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    for (i, &ch) in b.iter().enumerate() {
        let after_digit = i > 0 && b[i - 1].is_ascii_digit();
        let before_exponent = match b.get(i + 1) {
            Some('-') => b.get(i + 2).is_some_and(char::is_ascii_digit),
            Some(c) => c.is_ascii_digit(),
            None => false,
        };
        out.push(if ch == 'e' && after_digit && before_exponent {
            'E'
        } else {
            ch
        });
    }
    out
}

/// Builds a construction from script text; panics with the script on error.
/// Exponents as Rust prints them are accepted (see [`upper_exponents`]).
pub fn build(src: &str) -> Construction {
    let src = &upper_exponents(src);
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
    Conic,
    Number,
    /// An arc or a sector.
    Arc,
    /// An arc or a sector that points can be put on: not a part of a
    /// general conic (`Arc(conic, s, t)` with a `Conic`).
    PointArc,
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
                | (K::Conic, Kind::Conic)
                | (K::Number, Kind::Number)
                | (K::Arc | K::PointArc, Kind::Arc | Kind::Sector)
        )
    }

    /// Whether the object `id` of `c` can be this argument.
    pub fn fits_object(self, c: &Construction, id: Id) -> bool {
        let general_part = || matches!(c.def(id), Def::Arc(ArcDef::ConicParameters { conic, .. }) if c.kind(conic) == Kind::Conic);
        self.fits(c.kind(id)) && !(self == K::PointArc && general_part())
    }
}

type Slot = &'static [K];

const PT: Slot = &[K::Point];
const LN: Slot = &[K::Line];
const SG: Slot = &[K::Segment];
const CI: Slot = &[K::Circle];
const CO: Slot = &[K::Conic];
const NU: Slot = &[K::Number];
const IX: Slot = &[K::Index];
const AR: Slot = &[K::Arc];
const PA: Slot = &[K::PointArc];
/// A line or a segment.
const LL: Slot = &[K::Line, K::Segment];
/// Everything a transformation accepts.
const SRC: Slot = &[K::Point, K::Line, K::Segment, K::Circle, K::Conic];
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
    history_dependent("Intersect", &[CI, CI]),
    history_dependent("Intersect", &[CI, CI, IX]),
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
    // general conics
    cmd("Conic", &[PT, PT, PT, PT, PT]),
    cmd("Ellipse", &[PT, PT, PT]),
    cmd("Hyperbola", &[PT, PT, PT]),
    cmd("Ellipse", &[PT, PT, NU]),
    cmd("Hyperbola", &[PT, PT, NU]),
    cmd("Parabola", &[PT, LL]),
    cmd("Intersect", &[LN, CO]),
    cmd("Intersect", &[CO, LN, IX]),
    cmd("Center", &[CO]),
    multi("Focus", &[CO]),
    multi("Vertex", &[CO]),
    multi("Axes", &[CO]),
    cmd("MajorAxis", &[CO]),
    cmd("MinorAxis", &[CO]),
    multi("Directrix", &[CO]),
    cmd("FirstAxisLength", &[CO]),
    cmd("SecondAxisLength", &[CO]),
    cmd("Eccentricity", &[CO]),
    cmd("LinearEccentricity", &[CO]),
    cmd("Parameter", &[CO]),
    multi("Tangent", &[PT, CO]),
    multi("Tangent", &[LL, CO]),
    cmd("Polar", &[PT, CO]),
    cmd("Diameter", &[LL, CO]),
];

/// Points on paths and their parameters, arcs and sectors and their
/// measures. [`generate`] adds them after the other commands, from a stream
/// of their own, so that the objects before them are those of the same
/// seed without them. The state of a point on a path depends on the history
/// of moves; a test that rebuilds a construction restores it
/// (`Construction::restore_path_state`).
pub const PATH_COMMANDS: &[Cmd] = &[
    cmd("Point", &[LL]),
    cmd("Point", &[CI]),
    cmd("Point", &[LL, NU]),
    cmd("Point", &[CI, NU]),
    cmd("PathParameter", &[PT]),
    cmd("CircularArc", &[PT, PT, PT]),
    cmd("CircularSector", &[PT, PT, PT]),
    cmd("CircumcircularArc", &[PT, PT, PT]),
    cmd("CircumcircleSector", &[PT, PT, PT]),
    cmd("Semicircle", &[PT, PT]),
    cmd("Arc", &[CI, PT, PT]),
    cmd("Sector", &[CI, NU, NU]),
    cmd("Arc", &[CO, NU, NU]),
    cmd("Point", &[PA]),
    cmd("Point", &[PA, NU]),
    cmd("Length", &[AR]),
    cmd("Area", &[AR]),
    cmd("Perimeter", &[AR]),
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
            .filter(|&id| slot.iter().any(|k| k.fits_object(c, id)))
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
    /// For each object, whether an expression line made it. `Undefined` does
    /// not always spread through an expression (`1 / (1 / a)` is 0 when `a`
    /// is 0, as in GeoGebra), so contagion is not checked on these.
    pub expr: Vec<bool>,
    /// For each object, whether it may stay defined although an object it
    /// reads is undefined: an `If` takes the next branch when a condition
    /// cannot be decided, and a list holds an undefined element (one of
    /// `Sequence(expr, k, a, b)` too, where only `a` and `b` must be defined).
    pub lenient: Vec<bool>,
    /// Pairs of expression objects that must have the same value: one reads
    /// list or boolean objects, the other is the same expression with their
    /// definitions written in its place.
    pub twins: Vec<(usize, usize)>,
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
    let mut expr: Vec<bool> = Vec::new();
    let mut lenient: Vec<bool> = Vec::new();
    let mut twins: Vec<(usize, usize)> = Vec::new();
    // the right-hand side and the objects read of each list or boolean
    let mut inline: Vec<(String, String, Vec<String>)> = Vec::new();
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
        let line = format!("{name} = ({}, {})", lit(x), lit(y));
        let id = exec(&mut c, &line)
            .expect("free point")
            .expect("free point is created");
        lines.push(line);
        multi_lines.push(false);
        deps.push(Vec::new());
        expr.push(false);
        lenient.push(false);
        free.push(id);
    }
    for name in NUMBER_NAMES.iter().take(n_numbers) {
        let v = random_number(&mut rng);
        let line = format!("{name} = {}", lit(v));
        let id = exec(&mut c, &line)
            .expect("free number")
            .expect("free number is created");
        lines.push(line);
        multi_lines.push(false);
        deps.push(Vec::new());
        expr.push(false);
        lenient.push(false);
        free.push(id);
    }

    let mut made = 0;
    let mut attempts = 0;
    while made < n_commands && attempts < n_commands * 20 {
        attempts += 1;
        if rng.chance(0.25) {
            let Some((line, used)) = expression_line(&mut rng, &c, made) else {
                continue;
            };
            let read_kinds: Vec<Kind> = used
                .iter()
                .map(|u| c.kind(c.id(u).expect("read object")))
                .collect();
            let reads_list_or_boolean = read_kinds
                .iter()
                .any(|&k| matches!(k, Kind::List | Kind::Boolean));
            match exec(&mut c, &line) {
                Ok(Some(id)) => {
                    let ids = |used: &[String], c: &Construction| -> Vec<usize> {
                        used.iter()
                            .map(|u| c.id(u).expect("read object").index())
                            .collect()
                    };
                    deps.push(ids(&used, &c));
                    expr.push(true);
                    let kind = c.kind(id);
                    let loose = matches!(kind, Kind::List | Kind::Boolean)
                        || reads_list_or_boolean
                        || line.contains("If(");
                    lenient.push(loose);
                    assert_eq!(id.index() + 1, c.len(), "an expression makes one object");
                    let (label, rhs) = line.split_once(" = ").expect("label = rhs");
                    let (label, rhs) = (label.to_string(), rhs.to_string());
                    lines.push(line);
                    multi_lines.push(false);
                    made += 1;
                    if reads_list_or_boolean && !label.contains('(') {
                        // the same expression with the definitions of the
                        // lists and booleans it reads written in their place
                        let mut twin_rhs = rhs.clone();
                        let mut twin_used: Vec<String> = Vec::new();
                        for u in &used {
                            match inline.iter().find(|(l, ..)| l == u) {
                                Some((l, def, inner)) => {
                                    twin_rhs = replace_label(&twin_rhs, l, &format!("({def})"));
                                    twin_used.extend(inner.iter().cloned());
                                }
                                None => twin_used.push(u.clone()),
                            }
                        }
                        let twin_line = format!("{label}w = {twin_rhs}");
                        match exec(&mut c, &twin_line) {
                            Ok(Some(twin)) => {
                                deps.push(ids(&twin_used, &c));
                                expr.push(true);
                                lenient.push(loose);
                                twins.push((id.index(), twin.index()));
                                lines.push(twin_line);
                                multi_lines.push(false);
                            }
                            Ok(None) => unreachable!("an expression line creates an object"),
                            Err(e) => rejected.push(format!("{twin_line}: {e}")),
                        }
                    }
                    if matches!(kind, Kind::List | Kind::Boolean) {
                        inline.push((label, rhs, used));
                    }
                }
                Ok(None) => unreachable!("an expression line creates an object"),
                Err(e) => rejected.push(format!("{line}: {e}")),
            }
            continue;
        }
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
                    expr.push(false);
                    lenient.push(false);
                }
                lines.push(line);
                multi_lines.push(call.multi);
                made += 1;
            }
            Ok(None) => unreachable!("a command line creates an object"),
            Err(e) => rejected.push(format!("{line}: {e}")),
        }
    }
    let mut path_rng = Rng::new(seed ^ 0x5EED_0A7B_9A7B_0A7B);
    for _ in 0..path_rng.below(8) {
        let cmd = path_rng.pick(PATH_COMMANDS);
        let Some(call) = pick_args(&mut path_rng, &c, cmd, usize::MAX) else {
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
                    expr.push(false);
                    lenient.push(false);
                }
                lines.push(line);
                multi_lines.push(false);
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
        expr,
        lenient,
        twins,
        free,
        rejected,
    }
}

/// `text` with every identifier equal to `label` replaced by `by`.
fn replace_label(text: &str, label: &str, by: &str) -> String {
    let mut out = String::new();
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut String| {
        out.push_str(if word == label { by } else { word });
        word.clear();
    };
    for ch in text.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            word.push(ch);
        } else {
            flush(&mut word, &mut out);
            out.push(ch);
        }
    }
    flush(&mut word, &mut out);
    out
}

/// A random expression line over the objects of `c`: a number, a point, a
/// vector, a boolean, a list or a function, sometimes with a command inside
/// (`Distance(A, B)`), with the labels of the objects it reads. Some read
/// list and boolean objects made by earlier lines.
fn expression_line(rng: &mut Rng, c: &Construction, made: usize) -> Option<(String, Vec<String>)> {
    let of = |kind: Kind| -> Vec<String> {
        c.ids()
            .filter(|&id| c.kind(id) == kind)
            .map(|id| c.label(id).to_string())
            .collect()
    };
    let (numbers, points, vectors, functions, lists, booleans) = (
        of(Kind::Number),
        of(Kind::Point),
        of(Kind::Vector),
        of(Kind::Function),
        of(Kind::List),
        of(Kind::Boolean),
    );
    let n = rng.pick(&numbers).clone();
    let m = rng.pick(&numbers).clone();
    let p = rng.pick(&points).clone();
    let q = rng.pick(&points).clone();
    // readers of list and boolean objects, when there are any, a third of
    // the time
    let pick = if !(lists.is_empty() && booleans.is_empty()) && rng.chance(0.33) {
        24 + rng.below(7)
    } else {
        rng.below(32)
    };
    let (label, rhs, used) = match pick {
        0 => (format!("o{made}"), format!("{n} * 2 + {m}"), vec![n, m]),
        1 => (
            format!("o{made}"),
            format!("sqrt(abs({n})) - {m} / 3"),
            vec![n, m],
        ),
        2 => (
            format!("o{made}"),
            format!("x({p}) + y({q}) {n}"),
            vec![p, q, n],
        ),
        3 => (
            format!("o{made}"),
            format!("Distance({p}, {q}) / 2 + {n}"),
            vec![p, q, n],
        ),
        4 => (
            format!("o{made}"),
            format!("{n}^2 - floor({m})"),
            vec![n, m],
        ),
        5 => (format!("o{made}"), format!("1 / ({n} - {m})"), vec![n, m]),
        6 => {
            if functions.is_empty() {
                return None;
            }
            let f = rng.pick(&functions).clone();
            (format!("o{made}"), format!("{f}({n}) + 1"), vec![f, n])
        }
        7 => {
            if vectors.is_empty() {
                return None;
            }
            let u = rng.pick(&vectors).clone();
            (format!("o{made}"), format!("abs({u}) + x({u})"), vec![u])
        }
        8 => (format!("P{made}"), format!("({p} + {q}) / 2"), vec![p, q]),
        9 => (format!("P{made}"), format!("{p} + ({n}, 1)"), vec![p, n]),
        10 => (format!("P{made}"), format!("({n}; 30°) + {p}"), vec![n, p]),
        11 => (
            format!("P{made}"),
            format!("Midpoint({p}, {q}) + (1, {n})"),
            vec![p, q, n],
        ),
        12 => (
            format!("v{made}"),
            format!("{p} - {q} + ({n}, {m}) * 2"),
            vec![p, q, n, m],
        ),
        13 => (
            format!("o{made}"),
            format!("If({n} < {m}, {n} + 1, x({p}) > {m}, x({p}), 2 {m})"),
            vec![n, m, p],
        ),
        14 => (
            format!("P{made}"),
            format!("If({n} >= 0 && x({p}) != {m}, {p}, {q} + (1, 0))"),
            vec![n, p, m, q],
        ),
        15 => (
            format!("s{made}"),
            format!("{{{n}, 2 {m}, {{{n}^2, 1 / {m}}}, x({p}) < {n}}}"),
            vec![n, m, p],
        ),
        16 => (
            format!("s{made}"),
            format!("Sequence(jj {n} + x({p}), jj, {m}, {m} + 6)"),
            vec![n, p, m],
        ),
        17 => (
            format!("s{made}"),
            format!("{{{p}, {q}}} + ({n}, 1) - 2 {{{n}, {m}}}"),
            vec![p, q, n, m],
        ),
        18 => (
            format!("o{made}"),
            format!(
                "Sum({{{n}, {m}, x({p})}}, 3) + Length(Sequence({m})) - Sum({{x({p}), y({q})}})"
            ),
            vec![n, m, p, q],
        ),
        19 => (
            format!("s{made}"),
            format!("Sort(Append(Reverse({{{n}, {m}, x({p})}}), y({q})))"),
            vec![n, m, p, q],
        ),
        20 => (
            format!("o{made}"),
            format!("Product({{{n}, {m}}}) + Mean({{{n}, x({p})}}) - Max({{{m}, y({q})}})"),
            vec![n, m, p, q],
        ),
        21 => (
            format!("P{made}"),
            format!("Sum({{{p}, {q}}}) + Sum(First({{{q}, {p}}}, 1))"),
            vec![p, q],
        ),
        22 => (
            format!("s{made}"),
            format!("Join({{{n}}}, Last({{{m}, x({p})}}, 1), {{}})"),
            vec![n, m, p],
        ),
        23 => (
            format!("b{made}"),
            format!("{n} < {m} || x({p}) > {m}"),
            vec![n, m, p],
        ),
        24 | 25 => {
            if booleans.is_empty() {
                return None;
            }
            let b = rng.pick(&booleans).clone();
            if pick == 24 {
                (
                    format!("o{made}"),
                    format!("If({b}, {n}, {m}) + 1"),
                    vec![b, n, m],
                )
            } else {
                (
                    format!("P{made}"),
                    format!("If({b} && {n} > 0, {p}, {q})"),
                    vec![b, n, p, q],
                )
            }
        }
        26..=30 => {
            if lists.is_empty() {
                return None;
            }
            let l = rng.pick(&lists).clone();
            match pick {
                26 => (
                    format!("o{made}"),
                    format!("Length({l}) {n} + Length(Reverse({l}))"),
                    vec![l, n],
                ),
                27 => (format!("v{made}"), format!("{l} - {p}"), vec![l, p]),
                28 => (
                    format!("o{made}"),
                    format!("x({p} - {l}) + {n}"),
                    vec![p, l, n],
                ),
                29 => (
                    format!("s{made}"),
                    format!("Append(First({l}, 2), {n})"),
                    vec![l, n],
                ),
                _ => {
                    if booleans.is_empty() {
                        return None;
                    }
                    let b = rng.pick(&booleans).clone();
                    (
                        format!("s{made}"),
                        format!("If({b}, {l}, {{{n}, {b}}})"),
                        vec![b, l, n],
                    )
                }
            }
        }
        _ => {
            let body = if rng.chance(0.5) {
                format!("x^2 + {n}")
            } else {
                format!("{n} sin(x) + x({p})")
            };
            let used = if body.contains("sin") {
                vec![n, p]
            } else {
                vec![n]
            };
            return Some((format!("f{made}(x) = {body}"), used));
        }
    };
    Some((format!("{label} = {rhs}"), used))
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
