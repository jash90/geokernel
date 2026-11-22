// Parts translated from GeoGebra, commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
// source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdPenStroke.java
// source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoLocusStroke.java
// Copyright (c) GeoGebra GmbH. Licensed under the EUPL 1.2.

//! Pen strokes (`PenStroke`): the points of a freehand stroke, smoothed
//! once into lines and cubic Bezier pieces and then frozen. A stroke reads
//! no objects; it only remembers the coordinates of its creation.

use std::sync::Arc;

use crate::kernel::{Construction, Id, Value};

/// How a stroke point continues the path (the codes of saved files).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Segment {
    /// A straight line from the point before.
    Line,
    /// The start of a subpath.
    Move,
    /// The end point of a cubic piece whose two control points come before.
    Curve,
    /// A control point of a cubic piece.
    Control,
}

impl Segment {
    /// The code of the segment type in saved files.
    pub fn code(self) -> u8 {
        match self {
            Self::Line => 0,
            Self::Move => 1,
            Self::Curve => 3,
            Self::Control => 4,
        }
    }

    /// The segment type of a saved code; unknown codes draw lines, as the
    /// reference reader does.
    pub fn from_code(code: f64) -> Self {
        // the reader truncates the code to an integer
        match code as i32 {
            1 => Self::Move,
            3 => Self::Curve,
            4 => Self::Control,
            _ => Self::Line,
        }
    }
}

/// One point of a stroke. A subpath break has a non-finite `x`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StrokePoint {
    pub x: f64,
    pub y: f64,
    pub segment: Segment,
}

impl StrokePoint {
    const BREAK: Self = Self {
        x: f64::NAN,
        y: f64::NAN,
        segment: Segment::Line,
    };

    fn new(x: f64, y: f64, segment: Segment) -> Self {
        Self { x, y, segment }
    }

    /// Whether the point is no subpath break.
    pub fn is_defined(&self) -> bool {
        self.x.is_finite()
    }

    fn with(self, segment: Segment) -> Self {
        Self { segment, ..self }
    }
}

/// The frozen points of a stroke; undefined for `PenStroke()`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StrokeData {
    pub points: Arc<Vec<StrokePoint>>,
    pub defined: bool,
}

impl StrokeData {
    pub(crate) fn value(&self) -> Value {
        if self.defined {
            Value::PenStroke {
                len: u32::try_from(self.points.len()).unwrap_or(u32::MAX),
            }
        } else {
            Value::Undefined
        }
    }

    /// The finite points without subpath breaks.
    pub fn defined_points(&self) -> impl Iterator<Item = &StrokePoint> {
        self.points.iter().filter(|p| p.is_defined())
    }

    /// Bounds of the defined points: xmin, xmax, ymin, ymax.
    pub fn bounds(&self) -> Option<[f64; 4]> {
        self.defined_points()
            .filter(|p| p.y.is_finite())
            .fold(None, |bounds, p| {
                let [x0, x1, y0, y1] = bounds.unwrap_or([p.x, p.x, p.y, p.y]);
                Some([x0.min(p.x), x1.max(p.x), y0.min(p.y), y1.max(p.y)])
            })
    }

    /// The subpaths as polylines, each cubic piece flattened into `steps`
    /// chords. A subpath of one point is a run of one point; a piece with a
    /// non-finite coordinate ends its run.
    pub fn runs(&self, steps: usize) -> Vec<Vec<(f64, f64)>> {
        let steps = steps.max(1);
        let mut runs: Vec<Vec<(f64, f64)>> = Vec::new();
        let mut run: Vec<(f64, f64)> = Vec::new();
        let mut controls: Vec<(f64, f64)> = Vec::with_capacity(2);
        let mut flush = |run: &mut Vec<(f64, f64)>| {
            if !run.is_empty() {
                runs.push(std::mem::take(run));
            }
        };
        for p in self.points.iter() {
            if !p.is_defined() || !p.y.is_finite() {
                flush(&mut run);
                controls.clear();
                continue;
            }
            let at = (p.x, p.y);
            match p.segment {
                Segment::Move => {
                    flush(&mut run);
                    controls.clear();
                    run.push(at);
                }
                Segment::Control => controls.push(at),
                // a piece before any move draws nothing
                _ if run.is_empty() => controls.clear(),
                Segment::Curve if controls.len() == 2 => {
                    let start = *run.last().expect("a run has a start");
                    let xs = cubic(start.0, controls[0].0, controls[1].0, at.0);
                    let ys = cubic(start.1, controls[0].1, controls[1].1, at.1);
                    for k in 1..steps {
                        let t = k as f64 / steps as f64;
                        run.push((evaluate(&xs, t), evaluate(&ys, t)));
                    }
                    run.push(at);
                    controls.clear();
                }
                Segment::Curve | Segment::Line => {
                    run.push(at);
                    controls.clear();
                }
            }
        }
        flush(&mut run);
        runs
    }

    /// The stroke of saved points (`x, y, code` triplets), as the reference
    /// reader restores them: a move after other points starts a new
    /// subpath, and the stroke ends with a break.
    pub fn from_triplets(triplets: &[f64]) -> Self {
        let mut points = Vec::with_capacity(triplets.len() / 3 + 1);
        for t in triplets.chunks_exact(3) {
            if t[0].is_finite() {
                let segment = Segment::from_code(t[2]);
                if segment == Segment::Move && !points.is_empty() {
                    trailing_break(&mut points);
                }
                points.push(StrokePoint::new(t[0], t[1], segment));
            } else {
                points.push(StrokePoint::BREAK);
            }
        }
        trailing_break(&mut points);
        Self {
            points: Arc::new(points),
            defined: true,
        }
    }

    /// The saved triplets of the points, `NaN,NaN,0` for a break.
    pub fn triplets(&self) -> Vec<f64> {
        self.points
            .iter()
            .flat_map(|p| {
                if p.is_defined() {
                    [p.x, p.y, f64::from(p.segment.code())]
                } else {
                    [f64::NAN, f64::NAN, 0.0]
                }
            })
            .collect()
    }
}

/// The largest number of points a saved stroke restores.
pub const MAX_SAVED_POINTS: usize = 400_000;

/// A number of a saved stroke as the reference reader parses it back.
fn saved_number(v: f64) -> String {
    if v.is_nan() {
        "NaN".into()
    } else if v.is_infinite() {
        if v > 0.0 { "Infinity" } else { "-Infinity" }.into()
    } else {
        format!("{v:?}")
    }
}

/// A stroke number as the reference writes it in a value string or a saved
/// file: `0.0000E0`, five significant digits rounded half up from the
/// exact binary value (Java's `DecimalFormat`).
pub fn reference_number(v: f64) -> String {
    if v.is_nan() {
        return "NaN".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "\u{221e}" } else { "-\u{221e}" }.into();
    }
    let sign = if v.is_sign_negative() { "-" } else { "" };
    if v == 0.0 {
        return format!("{sign}0.0000E0");
    }
    // The exact decimal expansion: a double has at most 767 significant
    // digits.
    let exact = format!("{:.800e}", v.abs());
    let (mantissa, exponent) = exact.split_once('e').expect("scientific");
    let mut exponent: i32 = exponent.parse().expect("exponent");
    let digits: Vec<u8> = mantissa
        .bytes()
        .filter(u8::is_ascii_digit)
        .map(|b| b - b'0')
        .collect();
    let mut kept: Vec<u8> = digits[..5].to_vec();
    if digits[5] >= 5 {
        let mut i = 4;
        loop {
            if kept[i] < 9 {
                kept[i] += 1;
                break;
            }
            kept[i] = 0;
            if i == 0 {
                kept.insert(0, 1);
                kept.truncate(5);
                exponent += 1;
                break;
            }
            i -= 1;
        }
    }
    let text: String = kept.iter().map(|d| char::from(b'0' + d)).collect();
    format!("{sign}{}.{}E{exponent}", &text[..1], &text[1..])
}

/// The saved coordinates of a stroke, `x,y,code,...` without spaces: the
/// value of the `strokeBezierCoords` tag. Every number is written exactly.
pub fn saved_coords(data: &StrokeData) -> String {
    data.triplets()
        .iter()
        .map(|&v| saved_number(v))
        .collect::<Vec<_>>()
        .join(",")
}

/// The numbers of a `strokeBezierCoords` value, split at commas as the
/// reference reader does (trailing empty fields are dropped).
pub fn parse_saved_coords(text: &str) -> Result<Vec<f64>, String> {
    let mut fields: Vec<&str> = text.split(',').collect();
    while fields.last().is_some_and(|f| f.is_empty()) {
        fields.pop();
    }
    if fields.len() > 3 * MAX_SAVED_POINTS {
        return Err(format!(
            "a saved stroke holds at most {MAX_SAVED_POINTS} points"
        ));
    }
    fields
        .iter()
        .map(|f| {
            let f = f.trim();
            match f {
                "Infinity" | "+Infinity" => Ok(f64::INFINITY),
                "-Infinity" => Ok(f64::NEG_INFINITY),
                "NaN" | "+NaN" | "-NaN" => Ok(f64::NAN),
                _ if f
                    .bytes()
                    .any(|b| b.is_ascii_alphabetic() && !matches!(b, b'e' | b'E')) =>
                {
                    Err(format!("`{f}` is not a saved stroke number"))
                }
                _ => f
                    .parse()
                    .map_err(|_| format!("`{f}` is not a saved stroke number")),
            }
        })
        .collect()
}

/// The journal line of a stroke restored from saved coordinates: an
/// undefined stroke is `PenStroke()`, any other one `$penStroke(...)`.
pub fn saved_definition(data: &StrokeData) -> String {
    if data.defined {
        format!("$penStroke({})", saved_coords(data).replace(',', ", "))
    } else {
        "PenStroke()".into()
    }
}

/// The largest number of input points one stroke smooths; a longer input
/// makes an undefined stroke instead of unbounded work.
pub const MAX_STROKE_POINTS: usize = 100_000;

/// The smallest turn, in radians, that makes a piece curved.
const MIN_CURVE_ANGLE: f64 = std::f64::consts::PI / 60.0;
/// The longest piece, in pixels, that is not subdivided.
const MAX_SEGMENT_PIXELS: f64 = 50.0;
/// Neighbours closer than this many pixels in both directions are averaged.
const CLOSE_PIXELS: f64 = 4.0;

/// The window the smoothing measures pixels in: the origin and scales of
/// the graphics view.
#[derive(Clone, Copy, Debug)]
pub(crate) struct StrokeView {
    x_zero: f64,
    y_zero: f64,
    x_scale: f64,
    y_scale: f64,
}

impl StrokeView {
    pub(crate) fn of(c: &Construction) -> Self {
        let [xmin, _, _, ymax, x_scale, y_scale] = c.view();
        Self {
            x_zero: -xmin * x_scale,
            y_zero: ymax * y_scale,
            x_scale,
            y_scale,
        }
    }

    fn close(&self, a: StrokePoint, b: StrokePoint) -> bool {
        let sx = |x: f64| self.x_zero + x * self.x_scale;
        let sy = |y: f64| self.y_zero - y * self.y_scale;
        (sx(a.x) - sx(b.x)).abs() < CLOSE_PIXELS && (sy(a.y) - sy(b.y)).abs() < CLOSE_PIXELS
    }
}

/// The smoothed stroke of the input points (a non-finite `x` separates
/// subpaths). Long cubic pieces are subdivided once and the result is
/// smoothed again.
pub(crate) fn smooth(input: &[(f64, f64)], view: StrokeView) -> Vec<StrokePoint> {
    let data: Vec<_> = input
        .iter()
        .map(|&(x, y)| StrokePoint::new(x, y, Segment::Line))
        .collect();
    let points = append(&data, view);
    let dense = denser(&points, (1.0 / view.x_scale) * MAX_SEGMENT_PIXELS);
    if dense.len() > data.len() {
        append(&dense, view)
    } else {
        points
    }
}

/// The length of the subpath that starts at `index`: up to the first break
/// or the next move.
fn part(data: &[StrokePoint], index: usize) -> usize {
    data.iter()
        .skip(index)
        .enumerate()
        .take_while(|(i, p)| p.is_defined() && (p.segment != Segment::Move || *i == 0))
        .count()
}

fn trailing_break(points: &mut Vec<StrokePoint>) {
    if points.last().is_some_and(StrokePoint::is_defined) {
        points.push(StrokePoint::BREAK);
    }
}

fn append(data: &[StrokePoint], view: StrokeView) -> Vec<StrokePoint> {
    let mut out = Vec::with_capacity(data.len() * 3 + 2);
    let mut index = 0;
    while index <= data.len() {
        let size = part(data, index);
        if size != 0 {
            out.push(data[index].with(Segment::Move));
            if size < 3 {
                // one point draws a line to itself, two a plain segment
                out.push(data[index + size - 1].with(Segment::Line));
            } else {
                curve(&mut out, &data[index..index + size], view);
            }
        }
        if index < data.len() {
            trailing_break(&mut out);
        }
        index += size.max(1);
    }
    out
}

/// Appends the pieces through the points of one subpath of at least three
/// points: cubic where a control point turns away from the chord, straight
/// otherwise.
fn curve(out: &mut Vec<StrokePoint>, stroke: &[StrokePoint], view: StrokeView) {
    let averaged;
    let stroke = if stroke.len() > 9 {
        averaged = average_close(stroke, view);
        &averaged[..]
    } else {
        stroke
    };
    let [x1, y1, x2, y2] = control_points(stroke);
    for i in 1..stroke.len() {
        let c1 = StrokePoint::new(x1[i - 1], y1[i - 1], Segment::Control);
        let c2 = StrokePoint::new(x2[i - 1], y2[i - 1], Segment::Control);
        let (start, end) = (stroke[i - 1], stroke[i]);
        if turn(start, c1, end) > MIN_CURVE_ANGLE || turn(start, c2, end) > MIN_CURVE_ANGLE {
            out.extend([c1, c2, end.with(Segment::Curve)]);
        } else {
            out.push(end.with(Segment::Line));
        }
    }
}

/// `pi` minus the angle at `b` between `a` and `c`; `NaN` for a coincident
/// point.
fn turn(a: StrokePoint, b: StrokePoint, c: StrokePoint) -> f64 {
    let (dx1, dy1) = (a.x - b.x, a.y - b.y);
    let (dx2, dy2) = (c.x - b.x, c.y - b.y);
    std::f64::consts::PI - ((dx1 * dx2 + dy1 * dy2) / dx1.hypot(dy1) / dx2.hypot(dy2)).acos()
}

/// Replaces pairs of points that are close on screen by their midpoint;
/// the first and the last two points stay.
fn average_close(stroke: &[StrokePoint], view: StrokeView) -> Vec<StrokePoint> {
    let mut out = Vec::with_capacity(stroke.len());
    out.push(stroke[0]);
    let end = stroke.len();
    let mut i = 1;
    while i < end {
        if i + 2 < end && view.close(stroke[i], stroke[i + 1]) {
            let (a, b) = (stroke[i], stroke[i + 1]);
            out.push(StrokePoint::new(
                (a.x + b.x) / 2.0,
                (a.y + b.y) / 2.0,
                Segment::Line,
            ));
            i += 1;
        } else {
            out.push(stroke[i]);
        }
        i += 1;
    }
    out
}

/// The two control points of each piece of the smooth curve through the
/// points: the tridiagonal system of equal first and second derivatives at
/// the inner points, solved with the Thomas algorithm.
fn control_points(stroke: &[StrokePoint]) -> [Vec<f64>; 4] {
    let n = stroke.len() - 1;
    let (mut a, mut b, mut c) = (vec![1.0; n], vec![4.0; n], vec![1.0; n]);
    let (mut rx, mut ry) = (vec![0.0; n], vec![0.0; n]);
    a[0] = 0.0;
    b[0] = 2.0;
    c[0] = 1.0;
    rx[0] = stroke[0].x + 2.0 * stroke[1].x;
    ry[0] = stroke[0].y + 2.0 * stroke[1].y;
    for i in 1..n - 1 {
        rx[i] = 4.0 * stroke[i].x + 2.0 * stroke[i + 1].x;
        ry[i] = 4.0 * stroke[i].y + 2.0 * stroke[i + 1].y;
    }
    a[n - 1] = 2.0;
    b[n - 1] = 7.0;
    c[n - 1] = 0.0;
    rx[n - 1] = 8.0 * stroke[n - 1].x + stroke[n].x;
    ry[n - 1] = 8.0 * stroke[n - 1].y + stroke[n].y;
    for i in 1..n {
        let m = a[i] / b[i - 1];
        b[i] -= m * c[i - 1];
        rx[i] -= m * rx[i - 1];
        ry[i] -= m * ry[i - 1];
    }
    let (mut x1, mut y1) = (vec![0.0; n], vec![0.0; n]);
    x1[n - 1] = rx[n - 1] / b[n - 1];
    y1[n - 1] = ry[n - 1] / b[n - 1];
    for i in (0..n - 1).rev() {
        x1[i] = (rx[i] - c[i] * x1[i + 1]) / b[i];
        y1[i] = (ry[i] - c[i] * y1[i + 1]) / b[i];
    }
    let (mut x2, mut y2) = (vec![0.0; n], vec![0.0; n]);
    for i in 0..n - 1 {
        x2[i] = 2.0 * stroke[i + 1].x - x1[i + 1];
        y2[i] = 2.0 * stroke[i + 1].y - y1[i + 1];
    }
    x2[n - 1] = 0.5 * (stroke[n].x + x1[n - 1]);
    y2[n - 1] = 0.5 * (stroke[n].y + y1[n - 1]);
    [x1, y1, x2, y2]
}

/// The stroke points with every cubic piece longer than `length` replaced
/// by four points on it at equal parameter steps, without control points.
fn denser(points: &[StrokePoint], length: f64) -> Vec<StrokePoint> {
    const PARTS: u32 = 5;
    let mut dense = Vec::with_capacity(points.len() * 2);
    dense.extend(points.first().copied());
    let mut i = 1;
    while i < points.len() {
        let (p0, p1) = (points[i - 1], points[i]);
        if p1.segment == Segment::Control {
            let (p2, p3) = (points[i + 1], points[i + 2]);
            if (p3.x - p0.x).hypot(p3.y - p0.y) > length {
                let xs = cubic(p0.x, p1.x, p2.x, p3.x);
                let ys = cubic(p0.y, p1.y, p2.y, p3.y);
                for sub in 1..PARTS {
                    let t = f64::from(sub) / f64::from(PARTS);
                    dense.push(StrokePoint::new(
                        evaluate(&xs, t),
                        evaluate(&ys, t),
                        Segment::Line,
                    ));
                }
            }
            i += 2;
        } else {
            dense.push(p1);
            i += 1;
        }
    }
    dense
}

/// Power basis coefficients of a cubic Bezier coordinate.
fn cubic(p0: f64, p1: f64, p2: f64, p3: f64) -> [f64; 4] {
    [
        -p0 + 3.0 * p1 - 3.0 * p2 + p3,
        3.0 * p0 - 6.0 * p1 + 3.0 * p2,
        -3.0 * p0 + 3.0 * p1,
        p0,
    ]
}

fn evaluate(b: &[f64; 4], t: f64) -> f64 {
    b[0] * t * t * t + b[1] * t * t + b[2] * t + b[3]
}

/// One argument of `PenStroke`, read without resolving any name.
enum Argument {
    Number(f64),
    Point(f64, f64),
}

/// The value of a number literal as the reference parser folds it: signs
/// before a numeral, `?`, `NaN` or `∞`, or one of the unsigned constants.
fn number_literal(text: &str) -> Option<f64> {
    let text = text.trim();
    match text {
        "pi" | "π" => return Some(std::f64::consts::PI),
        "e" => return Some(std::f64::consts::E),
        "°" => return Some(std::f64::consts::PI / 180.0),
        _ => {}
    }
    let mut negative = false;
    let mut rest = text;
    loop {
        rest = rest.trim_start();
        if let Some(r) = rest
            .strip_prefix('-')
            .or_else(|| rest.strip_prefix('\u{2212}'))
        {
            negative = !negative;
            rest = r;
        } else if let Some(r) = rest.strip_prefix('+') {
            rest = r;
        } else {
            break;
        }
    }
    let magnitude = match rest {
        "?" | "NaN" => f64::NAN,
        "∞" => f64::INFINITY,
        _ => numeral(rest)?,
    };
    Some(if negative { -magnitude } else { magnitude })
}

/// A decimal numeral: digits with an optional fraction and an optional
/// exponent written with a capital `E`.
fn numeral(text: &str) -> Option<f64> {
    let (mantissa, exponent) = match text.split_once('E') {
        Some((m, e)) => (m, Some(e)),
        None => (text, None),
    };
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
    let exponent_ok = exponent.is_none_or(|e| {
        let e = e.strip_prefix(['+', '-']).unwrap_or(e);
        !e.is_empty() && digits(e)
    });
    (!(whole.is_empty() && fraction.is_empty()) && digits(whole) && digits(fraction) && exponent_ok)
        .then(|| text.parse().ok())
        .flatten()
}

/// A coordinate of a point literal: evaluated without the construction's
/// objects, so a coordinate that reads a name is undefined.
fn coordinate(empty: &Construction, text: &str) -> f64 {
    crate::expr::argument_number(empty, text).unwrap_or(f64::NAN)
}

/// A point literal `(x, y)` or `(r; theta)`, whose two coordinates are
/// taken as they are written.
fn point_literal(empty: &Construction, text: &str) -> Option<(f64, f64)> {
    let inner = text.trim().strip_prefix('(')?.strip_suffix(')')?;
    let mut depth = 0usize;
    let mut cut = None;
    for (i, ch) in inner.char_indices() {
        match ch {
            '(' | '{' | '[' => depth += 1,
            ')' | '}' | ']' => depth = depth.checked_sub(1)?,
            ',' | ';' if depth == 0 => {
                if cut.is_some() {
                    return None;
                }
                cut = Some(i);
            }
            _ => {}
        }
    }
    let Some(at) = cut else {
        // `((x, y))`: grouping brackets around a point literal
        return point_literal(empty, inner);
    };
    Some((
        coordinate(empty, &inner[..at]),
        coordinate(empty, &inner[at + 1..]),
    ))
}

fn argument(empty: &Construction, text: &str) -> Option<Argument> {
    if let Some(v) = number_literal(text) {
        return Some(Argument::Number(v));
    }
    point_literal(empty, text).map(|(x, y)| Argument::Point(x, y))
}

/// The input points of `PenStroke(args)`: point literals, or consecutive
/// number literals read as coordinate pairs.
pub(crate) fn input_points(args: &[&str]) -> Result<Vec<(f64, f64)>, String> {
    let args = match args {
        [""] => &[][..],
        _ => args,
    };
    if args.len() > 2 * MAX_STROKE_POINTS {
        return Err(format!(
            "a pen stroke takes at most {MAX_STROKE_POINTS} points"
        ));
    }
    let empty = Construction::new();
    let illegal = |arg: &str| format!("illegal argument of `PenStroke`: `{arg}`");
    let mut points = Vec::with_capacity(args.len());
    let mut i = 0;
    while i < args.len() {
        match argument(&empty, args[i]) {
            Some(Argument::Number(x)) if i + 1 < args.len() => {
                // the next argument is evaluated as a number, whatever it is
                let next = args[i + 1];
                let y = number_literal(next).unwrap_or_else(|| coordinate(&empty, next));
                points.push((x, y));
                i += 1;
            }
            Some(Argument::Point(x, y)) => points.push((x, y)),
            _ => return Err(illegal(args[i])),
        }
        i += 1;
    }
    Ok(points)
}

impl Construction {
    /// The frozen points of the pen stroke `id`; `None` for any other object.
    pub fn stroke(&self, id: Id) -> Option<&StrokeData> {
        self.strokes.get(&id)
    }

    /// Adds a pen stroke with the given points, as `PenStroke` freezes
    /// them or a saved file restores them.
    pub fn add_stroke(&mut self, label: &str, data: StrokeData) -> Result<Id, String> {
        let id = Id(self.len());
        self.strokes.insert(id, data);
        self.add(label, crate::kernel::Def::PenStroke)
            .inspect_err(|_| {
                self.strokes.remove(&id);
            })
            .map_err(|e| e.to_string())
    }

    /// `$penStroke(x, y, code, ...)`: a stroke restored from its saved
    /// points, read as the reference reader reads `strokeBezierCoords`.
    pub fn add_saved_stroke(&mut self, label: &str, args: &[&str]) -> Result<Id, String> {
        let args = match args {
            [""] => &[][..],
            _ => args,
        };
        let numbers = parse_saved_coords(&args.join(","))?;
        self.add_stroke(label, StrokeData::from_triplets(&numbers))
    }

    /// `PenStroke(args)`: smooths the points of the arguments in the
    /// current view and adds the frozen stroke.
    pub fn add_pen_stroke(&mut self, label: &str, args: &[&str]) -> Result<Id, String> {
        let input = input_points(args)?;
        let data = if input.is_empty() {
            StrokeData::default()
        } else {
            StrokeData {
                points: Arc::new(smooth(&input, StrokeView::of(self))),
                defined: true,
            }
        };
        self.add_stroke(label, data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_literals_follow_the_folded_forms() {
        assert_eq!(number_literal("--1"), Some(1.0));
        assert_eq!(number_literal("- 1"), Some(-1.0));
        assert_eq!(number_literal("1E1"), Some(10.0));
        assert_eq!(number_literal(".5"), Some(0.5));
        assert_eq!(number_literal("1."), Some(1.0));
        assert!(number_literal("-0").is_some_and(|v| v == 0.0 && v.is_sign_negative()));
        assert_eq!(number_literal("-∞"), Some(f64::NEG_INFINITY));
        for refused in [
            "-e",
            "-pi",
            "1e-2",
            "1/2",
            "2^2",
            "1°",
            "undefined",
            "",
            "-",
            "E1",
        ] {
            assert_eq!(number_literal(refused), None, "{refused}");
        }
    }

    #[test]
    fn a_lone_last_number_is_refused() {
        assert!(input_points(&["1", "2", "3"]).is_err());
        assert_eq!(input_points(&["1", "2"]).unwrap(), vec![(1.0, 2.0)]);
        assert!(input_points(&[""]).unwrap().is_empty());
    }

    #[test]
    fn saved_triplets_round_trip() {
        let data = StrokeData::from_triplets(&[0.0, 0.0, 1.0, 1.0, 1.0, 0.0, 2.0, 2.0, 1.0]);
        let codes: Vec<_> = data.points.iter().map(|p| p.segment.code()).collect();
        assert_eq!(codes, [1, 0, 0, 1, 0]);
        assert!(!data.points[2].is_defined() && !data.points[4].is_defined());
        let again = StrokeData::from_triplets(&data.triplets());
        assert_eq!(again.points.len(), data.points.len());
    }
}
