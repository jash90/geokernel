//! Property tests: random constructions with random moves, and geometric
//! invariants of single commands on random non-degenerate inputs.
//!
//! Every test uses fixed seeds. A failure prints the seed, the script and
//! the moves, so that it can be replayed. Geometric checks use a tolerance
//! of `1e-9` times the size of the numbers involved (at least 1).

mod common;

use common::*;
use geo_kernel::script::exec;
use geo_kernel::{Construction, Def, Error, Id, Kind, Value};
use std::f64::consts::PI;

// ------------------------------------------------ random constructions

const CASES: u64 = 400;
const STEPS: usize = 30;
const COMMANDS_PER_CASE: usize = 14;

#[derive(Default)]
struct Totals {
    cases: usize,
    objects: usize,
    steps: usize,
    undefined_seen: usize,
    script_rebuilds: usize,
    script_rebuilds_skipped: usize,
    /// Fresh builds that differ only in state decided when an object was
    /// created (see `same_up_to_creation_state`).
    creation_state_differs: usize,
    rejected: Vec<String>,
}

/// Whether two definitions are equal up to the state an object takes when it
/// is created: the `special` flag of a circle intersection, which GeoGebra
/// decides once from the positions at creation (`AlgoIntersectConics`), so a
/// build at other positions can decide it otherwise. The incremental update
/// is still checked against a rebuild that keeps the flag after every move.
fn same_up_to_creation_state(a: Def, b: Def) -> bool {
    match (a, b) {
        (
            Def::IntersectCircles { c1, c2, which, .. },
            Def::IntersectCircles {
                c1: d1,
                c2: d2,
                which: w,
                ..
            },
        ) => (c1, c2, which) == (d1, d2, w),
        _ => a == b,
    }
}

/// Builds a fresh construction from the definitions of `orig`: every free
/// object is created directly at its current value.
fn rebuild_from_defs(orig: &Construction) -> Result<Construction, String> {
    let mut fresh = Construction::new();
    for id in orig.ids() {
        fresh
            .add(orig.label(id), orig.def(id))
            .map_err(|e| format!("rebuilding `{}` failed: {e}", orig.label(id)))?;
    }
    Ok(fresh)
}

/// The script of `case` with the free objects at their current values.
fn script_at_current_values(case: &Case) -> Vec<String> {
    let mut lines = Vec::new();
    for id in case.c.ids() {
        let label = case.c.label(id);
        match case.c.def(id) {
            Def::FreePoint { x, y } => lines.push(format!("{label} = ({x:?}, {y:?})")),
            Def::Number(v) => lines.push(format!("{label} = {v:?}")),
            _ => {}
        }
    }
    lines.extend(case.lines.iter().skip(case.free.len()).cloned());
    lines
}

/// `Undefined` must spread: an object read from an undefined one is
/// undefined too.
fn check_contagion(case: &Case) -> Result<(), String> {
    for id in case.c.ids() {
        let parents = &case.deps[id.index()];
        let broken = parents
            .iter()
            .find(|&&p| !case.c.value(id_at(&case.c, p)).is_defined());
        if let Some(&p) = broken {
            let v = case.c.value(id);
            if v.is_defined() {
                return Err(format!(
                    "`{}` is defined ({v:?}) although `{}` is undefined",
                    case.c.label(id),
                    case.c.label(id_at(&case.c, p))
                ));
            }
        }
    }
    Ok(())
}

/// `case.deps` with the objects a rule reads besides its arguments, where
/// they are not already ancestors: `Intersect(k, k)` reads the first point on
/// `k` (GeoGebra's `getPointFrom1on2`), which can be an earlier intersection
/// point of `k`. Every earlier intersection point of `k` is added, which is
/// more than is read and only weakens the checks of which objects a move may
/// change. Contagion of `Undefined` is still checked on the arguments only.
fn reads(case: &Case) -> Vec<Vec<usize>> {
    let mut reads = case.deps.clone();
    let on = |d: Def, k: Id| match d {
        Def::IntersectLineCircle { circle, .. } => circle == k,
        Def::IntersectCircles { c1, c2, .. } => c1 == k || c2 == k,
        _ => false,
    };
    for id in case.c.ids() {
        if let Def::IntersectCircles { c1, c2, .. } = case.c.def(id) {
            if c1 == c2 {
                for s in case.c.ids().take(id.index()) {
                    if on(case.c.def(s), c1) && !reads[id.index()].contains(&s.index()) {
                        reads[id.index()].push(s.index());
                    }
                }
            }
        }
    }
    reads
}

/// The objects that depend on `start` through `reads` (see [`reads`]).
fn dependents(case: &Case, reads: &[Vec<usize>], start: Id) -> Vec<bool> {
    let n = case.c.len();
    let mut reach = vec![false; n];
    reach[start.index()] = true;
    for i in start.index() + 1..n {
        reach[i] = reads[i].iter().any(|&p| reach[p]);
    }
    reach[start.index()] = false;
    reach
}

fn run_case(seed: u64, all_commands: bool, totals: &mut Totals) -> Result<(), String> {
    let mut case = generate(seed, COMMANDS_PER_CASE, all_commands);
    let reads = reads(&case);
    let mut moves: Vec<String> = Vec::new();
    let mut rng = Rng::new(seed ^ 0x5DEE_CE66_D1CE_5EED);
    totals.cases += 1;
    totals.objects += case.c.len();
    totals.rejected.append(&mut case.rejected);

    macro_rules! fail {
        ($($arg:tt)*) => {
            return Err(format!(
                "{}\nscript:\n{}\nmoves:\n{}",
                format!($($arg)*),
                script_of(&case.lines),
                moves.join("\n")
            ))
        };
    }

    if let Some(bad) = first_non_finite(&case.c) {
        fail!("non-finite value after creation: {bad}");
    }
    if let Err(e) = check_contagion(&case) {
        fail!("after creation: {e}");
    }

    for step_no in 0..STEPS {
        totals.steps += 1;
        let step = random_step(&mut rng, &case.c, &case.free);
        let before = values(&case.c);
        let moved = step.id();
        moves.push(format!("{step_no}: {}", step.describe(&case.c)));
        let count = match step.apply(&mut case.c) {
            Ok(n) => n,
            Err(e) => fail!("move {step_no} failed: {e}"),
        };
        let after = values(&case.c);
        totals.undefined_seen += after.iter().filter(|v| !v.is_defined()).count();

        // 1. no non-finite numbers
        if let Some(bad) = first_non_finite(&case.c) {
            fail!("non-finite value after move {step_no}: {bad}");
        }
        // 3. only dependents change, and no more are recomputed than exist
        let reach = dependents(&case, &reads, moved);
        let n_dependents = reach.iter().filter(|&&r| r).count();
        if count > n_dependents {
            fail!("move {step_no} recomputed {count} objects but only {n_dependents} depend on it");
        }
        for i in 0..before.len() {
            if i != moved.index() && !reach[i] && before[i] != after[i] {
                fail!(
                    "move {step_no} changed `{}`, which does not depend on `{}`",
                    case.c.label(id_at(&case.c, i)),
                    case.c.label(moved)
                );
            }
        }
        // 3. incremental update equals a full recompute
        let fresh = rebuild_from_defs(&case.c)?;
        if let Some(d) = first_difference(&case.c, &after, &values(&fresh)) {
            fail!("incremental update differs from recompute after move {step_no}: {d}");
        }
        // 4. undefined is contagious
        if let Err(e) = check_contagion(&case) {
            fail!("after move {step_no}: {e}");
        }
        // 3. moving to the current position recomputes nothing
        match step.apply(&mut case.c) {
            Ok(0) => {}
            Ok(n) => fail!("repeating move {step_no} recomputed {n} objects"),
            Err(e) => fail!("repeating move {step_no} failed: {e}"),
        }
        if let Some(d) = first_difference(&case.c, &after, &values(&case.c)) {
            fail!("repeating move {step_no} changed a value: {d}");
        }

        // 2/4. away and back gives the original values
        if step_no % 3 == 0 {
            let snapshot = values(&case.c);
            let target = *rng.pick(&case.free);
            let home = match case.c.def(target) {
                Def::FreePoint { x, y } => Step::Point { id: target, x, y },
                Def::Number(v) => Step::Number { id: target, v },
                other => fail!("free object has definition {other:?}"),
            };
            let away = random_step(&mut rng, &case.c, &[target]);
            moves.push(format!("  away: {}", away.describe(&case.c)));
            if let Err(e) = away.apply(&mut case.c) {
                fail!("away move failed: {e}");
            }
            moves.push(format!("  back: {}", home.describe(&case.c)));
            if let Err(e) = home.apply(&mut case.c) {
                fail!("back move failed: {e}");
            }
            if let Some(d) = first_difference(&case.c, &snapshot, &values(&case.c)) {
                fail!(
                    "moving `{}` away and back changed a value: {d}",
                    case.c.label(target)
                );
            }
        }
    }

    // 2. history independence against a script built at the final values
    let script = script_at_current_values(&case);
    let mut fresh = Construction::new();
    let mut ok = true;
    for line in &script {
        if exec(&mut fresh, line).is_err() {
            ok = false;
            break;
        }
    }
    let same_layout = ok
        && fresh.len() == case.c.len()
        && fresh
            .ids()
            .zip(case.c.ids())
            .all(|(f, o)| fresh.label(f) == case.c.label(o) && fresh.def(f) == case.c.def(o));
    if same_layout {
        totals.script_rebuilds += 1;
        if let Some(d) = first_difference(&case.c, &values(&case.c), &values(&fresh)) {
            fail!(
                "history differs from a fresh build at the final values: {d}\nfresh script:\n{}",
                script_of(&script)
            );
        }
    } else if ok
        && fresh.len() == case.c.len()
        && fresh.ids().zip(case.c.ids()).all(|(f, o)| {
            fresh.label(f) == case.c.label(o)
                && same_up_to_creation_state(fresh.def(f), case.c.def(o))
        })
    {
        totals.creation_state_differs += 1;
    } else if case.multi_lines.iter().any(|&m| m) {
        // The objects a multi-output command creates are fixed when it runs
        // (see DIFFERENCES.md), so another set of positions can give another
        // set of objects.
        totals.script_rebuilds_skipped += 1;
    } else {
        fail!(
            "a fresh build at the final values has a different layout\nfresh script:\n{}",
            script_of(&script)
        );
    }
    Ok(())
}

#[test]
fn random_constructions_and_moves() {
    let mut totals = Totals::default();
    for_seeds("random constructions", 0x1111, CASES, |rng| {
        run_case(rng.state(), false, &mut totals)
    });
    println!(
        "{} constructions, {} objects, {} moves ({} undefined values seen), \
         {} script rebuilds compared, {} skipped (multi-output layout), {} rejected commands",
        totals.cases,
        totals.objects,
        totals.steps,
        totals.undefined_seen,
        totals.script_rebuilds,
        totals.script_rebuilds_skipped,
        totals.rejected.len()
    );
    // The commands are generated with arguments of the right kinds, so the
    // script layer must accept all of them.
    assert!(
        totals.rejected.is_empty(),
        "{} command(s) with matching argument kinds were rejected, e.g.\n{}",
        totals.rejected.len(),
        totals
            .rejected
            .iter()
            .take(5)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    // The script rebuild is skipped only for multi-output layouts; it must
    // stay the rule (379 of 400 at the time of writing).
    assert!(
        totals.script_rebuilds >= 300,
        "only {} of {} script rebuilds were compared",
        totals.script_rebuilds,
        totals.cases
    );
    // The generator must reach undefined states, or contagion and recovery
    // would never be exercised.
    assert!(
        totals.undefined_seen > totals.steps,
        "too few undefined values"
    );
}

/// Replays one random construction: `GEO_REPLAY_SEED=<seed from a failure>`,
/// and `GEO_REPLAY_ALL=1` when the failing run used every command.
#[test]
#[ignore = "replays the seed given in GEO_REPLAY_SEED"]
fn replay_seed() {
    let Ok(text) = std::env::var("GEO_REPLAY_SEED") else {
        println!("GEO_REPLAY_SEED is not set: nothing was replayed");
        return;
    };
    let seed = match text.strip_prefix("0x") {
        Some(hex) => u64::from_str_radix(hex, 16),
        None => text.parse(),
    }
    .expect("GEO_REPLAY_SEED is a number");
    let all = std::env::var("GEO_REPLAY_ALL").is_ok();
    if let Err(e) = run_case(seed, all, &mut Totals::default()) {
        panic!("{e}");
    }
}

// ------------------------------------------------ geometric invariants

fn p(label: &str, (x, y): (f64, f64)) -> String {
    format!("{label} = ({x:?}, {y:?})\n")
}

fn far_apart(rng: &mut Rng, from: (f64, f64), min: f64, half: f64) -> (f64, f64) {
    loop {
        let q = rng.point(half);
        if dist(from, q) >= min {
            return q;
        }
    }
}

#[test]
fn midpoints_and_perpendicular_bisectors() {
    for_seeds("midpoint and bisectors", 0x2222, 300, |rng| {
        let a = rng.point(10.0);
        let b = far_apart(rng, a, 1.0, 10.0);
        let src = format!(
            "{}{}M = Midpoint(A, B)\ns = Segment(A, B)\nm2 = Midpoint(s)\nm3 = Center(s)\n\
             l = PerpendicularBisector(A, B)\nl2 = LineBisector(s)\nl3 = LineBisector(A, B)\n",
            p("A", a),
            p("B", b)
        );
        let c = build(&src);
        with_script(
            &src,
            (|| {
                let m = pt(&c, "M");
                let half = dist(a, b) / 2.0;
                expect_near("|MA|", dist(m, a), half, 20.0)?;
                expect_near("|MB|", dist(m, b), half, 20.0)?;
                for other in ["m2", "m3"] {
                    if !same_point(m, pt(&c, other), 20.0) {
                        return Err(format!("{other} differs from M"));
                    }
                }
                for l in ["l", "l2", "l3"] {
                    let ln = line(&c, l);
                    expect_near(&format!("{l} through M"), dist_to_line(m, ln), 0.0, 20.0)?;
                    for q in points_on(ln) {
                        expect_near(&format!("{l}: |QA| = |QB|"), dist(q, a), dist(q, b), 30.0)?;
                    }
                    if !same_line(ln, line(&c, "l"), 20.0) {
                        return Err(format!("{l} is not the same line as l"));
                    }
                }
                Ok(())
            })(),
        )
    });
}

#[test]
fn circles_through_points_and_incircles() {
    for_seeds("circles", 0x3333, 300, |rng| {
        let [a, b, cc] = triangle(rng, 10.0);
        let src = format!(
            "{}{}{}k = Circle(A, B, C)\nO = Center(k)\nO2 = Midpoint(k)\ni = Incircle(A, B, C)\n\
             I = Center(i)\nlab = Line(A, B)\nlbc = Line(B, C)\nlca = Line(C, A)\n\
             sab = Segment(A, B)\nj = Circle(C, sab)\nrk = Radius(k)\nrik = Radius(i)\n\
             per = Circumference(k)\nper2 = Perimeter(k)\nar = Area(k)\nm = Circle(A, B)\n\
             rm = Radius(m)\n",
            p("A", a),
            p("B", b),
            p("C", cc)
        );
        let c = build(&src);
        with_script(
            &src,
            (|| {
                let (cx, cy, r) = circle(&c, "k");
                let o = (cx, cy);
                let scale = r.max(20.0);
                for (n, v) in [("A", a), ("B", b), ("C", cc)] {
                    expect_near(&format!("|O{n}| = r"), dist(o, v), r, scale)?;
                }
                for l in ["O", "O2"] {
                    if !same_point(pt(&c, l), o, scale) {
                        return Err(format!("{l} is not the centre"));
                    }
                }
                expect_near("Radius", num(&c, "rk"), r, scale)?;
                expect_near("Circumference", num(&c, "per"), 2.0 * PI * r, scale)?;
                expect_near("Perimeter", num(&c, "per2"), 2.0 * PI * r, scale)?;
                expect_near("Area", num(&c, "ar"), PI * r * r, r * r)?;
                // incircle
                let (ix, iy, ir) = circle(&c, "i");
                let (la, lb, lc) = (dist(b, cc), dist(cc, a), dist(a, b));
                let s = (la + lb + lc) / 2.0;
                let area = cross3(a, b, cc).abs() / 2.0;
                expect_near("inradius", ir, area / s, 20.0)?;
                expect_near("Radius(i)", num(&c, "rik"), ir, 20.0)?;
                let centre = (
                    (la * a.0 + lb * b.0 + lc * cc.0) / (2.0 * s),
                    (la * a.1 + lb * b.1 + lc * cc.1) / (2.0 * s),
                );
                if !same_point((ix, iy), centre, 20.0) {
                    return Err(format!("incentre {:?} against {centre:?}", (ix, iy)));
                }
                if !same_point(pt(&c, "I"), (ix, iy), 20.0) {
                    return Err("Center(i) is not the incentre".into());
                }
                for side in ["lab", "lbc", "lca"] {
                    expect_near(
                        &format!("incircle tangent to {side}"),
                        dist_to_line((ix, iy), line(&c, side)),
                        ir,
                        20.0,
                    )?;
                }
                // Circle(C, segment)
                let (jx, jy, jr) = circle(&c, "j");
                if !same_point((jx, jy), cc, 20.0) {
                    return Err("Circle(C, s) is not centred at C".into());
                }
                expect_near("Circle(C, s) radius", jr, dist(a, b), 20.0)?;
                // Circle(A, B)
                let (mx, my, mr) = circle(&c, "m");
                if !same_point((mx, my), a, 20.0) {
                    return Err("Circle(A, B) is not centred at A".into());
                }
                expect_near("Circle(A, B) radius", mr, dist(a, b), 20.0)?;
                expect_near("Radius(m)", num(&c, "rm"), mr, 20.0)
            })(),
        )
    });
}

#[test]
fn tangents_and_polars() {
    for_seeds("tangents", 0x4444, 300, |rng| {
        let o = rng.point(8.0);
        let r = rng.range(1.0, 5.0);
        let pnt = loop {
            let q = rng.point(12.0);
            let d = dist(q, o);
            if d >= r * 1.3 && d <= 16.0 {
                break q;
            }
        };
        let a = rng.point(10.0);
        let b = far_apart(rng, a, 2.0, 10.0);
        let src = format!(
            "{}{}{}{}r = {r:?}\nk = Circle(O, r)\nt = Tangent(P, k)\nt2 = Tangent(k, P)\n\
             pol = Polar(P, k)\nQ1 = Intersect(pol, k, 1)\nQ2 = Intersect(pol, k, 2)\n\
             l = Line(A, B)\nu = Tangent(l, k)\n",
            p("O", o),
            p("P", pnt),
            p("A", a),
            p("B", b)
        );
        let c = build(&src);
        with_script(
            &src,
            (|| {
                let scale = 30.0;
                let d = dist(o, pnt);
                for family in ["t", "t2"] {
                    let (t1, t2) = (
                        line(&c, &format!("{family}_{{1}}")),
                        line(&c, &format!("{family}_{{2}}")),
                    );
                    for (n, t) in [(1, t1), (2, t2)] {
                        expect_near(
                            &format!("{family}_{n}: distance to centre"),
                            dist_to_line(o, t),
                            r,
                            scale,
                        )?;
                        expect_near(
                            &format!("{family}_{n}: passes through P"),
                            dist_to_line(pnt, t),
                            0.0,
                            scale,
                        )?;
                    }
                    if same_line(t1, t2, scale) {
                        return Err(format!("{family}_1 and {family}_2 are the same line"));
                    }
                }
                // polar line
                let pol = line(&c, "pol");
                expect_near(
                    "polar distance from centre",
                    dist_to_line(o, pol),
                    r * r / d,
                    scale,
                )?;
                let dir = (pol.1, -pol.0);
                let along =
                    (dir.0 * (pnt.0 - o.0) + dir.1 * (pnt.1 - o.1)) / (dir.0.hypot(dir.1) * d);
                expect_near("polar is perpendicular to OP", along, 0.0, 1.0)?;
                let (t1, t2) = (line(&c, "t_{1}"), line(&c, "t_{2}"));
                for q in ["Q1", "Q2"] {
                    let q = pt(&c, q);
                    let on = dist_to_line(q, t1).min(dist_to_line(q, t2));
                    expect_near("tangent point on a tangent", on, 0.0, scale)?;
                    expect_near("tangent point on the circle", dist(q, o), r, scale)?;
                }
                // tangents parallel to a line
                let l = line(&c, "l");
                let (u1, u2) = (line(&c, "u_{1}"), line(&c, "u_{2}"));
                for (n, u) in [(1, u1), (2, u2)] {
                    expect_near(
                        &format!("u_{n}: distance to centre"),
                        dist_to_line(o, u),
                        r,
                        scale,
                    )?;
                    let cross = (u.0 * l.1 - u.1 * l.0) / (u.0.hypot(u.1) * l.0.hypot(l.1));
                    expect_near(&format!("u_{n} is parallel to l"), cross, 0.0, 1.0)?;
                }
                if same_line(u1, u2, scale) {
                    return Err("u_1 and u_2 are the same line".into());
                }
                Ok(())
            })(),
        )
    });
}

#[test]
fn intersections_lie_on_both_objects() {
    // two lines
    for_seeds("line-line intersection", 0x5555, 300, |rng| {
        let (a, c0) = (rng.point(10.0), rng.point(10.0));
        let b = far_apart(rng, a, 2.0, 10.0);
        let d = loop {
            let d = far_apart(rng, c0, 2.0, 10.0);
            let (u, v) = ((b.0 - a.0, b.1 - a.1), (d.0 - c0.0, d.1 - c0.1));
            let sin = (u.0 * v.1 - u.1 * v.0).abs() / (u.0.hypot(u.1) * v.0.hypot(v.1));
            if sin >= 0.3 {
                break d;
            }
        };
        let src = format!(
            "{}{}{}{}l = Line(A, B)\nm = Line(C, D)\nP = Intersect(l, m)\nP2 = Intersect(m, l)\n",
            p("A", a),
            p("B", b),
            p("C", c0),
            p("D", d)
        );
        let c = build(&src);
        with_script(
            &src,
            (|| {
                let (l, m) = (line(&c, "l"), line(&c, "m"));
                for name in ["P", "P2"] {
                    let q = pt(&c, name);
                    expect_near(&format!("{name} on l"), dist_to_line(q, l), 0.0, 40.0)?;
                    expect_near(&format!("{name} on m"), dist_to_line(q, m), 0.0, 40.0)?;
                }
                Ok(())
            })(),
        )
    });

    // a line and a circle
    for_seeds("line-circle intersection", 0x6666, 300, |rng| {
        let o = rng.point(6.0);
        let r = rng.range(1.0, 6.0);
        let theta = rng.range(0.0, 2.0 * PI);
        let offset = rng.range(-0.8, 0.8) * r;
        let dir = (theta.cos(), theta.sin());
        let normal = (-dir.1, dir.0);
        let foot = (o.0 + offset * normal.0, o.1 + offset * normal.1);
        let a = (foot.0 - 5.0 * dir.0, foot.1 - 5.0 * dir.1);
        let b = (foot.0 + 5.0 * dir.0, foot.1 + 5.0 * dir.1);
        let src = format!(
            "{}{}{}r = {r:?}\nk = Circle(O, r)\nl = Line(A, B)\nP1 = Intersect(l, k, 1)\n\
             P2 = Intersect(l, k, 2)\nR1 = Intersect(k, l, 1)\nR2 = Intersect(k, l, 2)\n\
             P = Intersect(l, k)\n",
            p("O", o),
            p("A", a),
            p("B", b)
        );
        let c = build(&src);
        with_script(
            &src,
            (|| {
                let l = line(&c, "l");
                let chord = 2.0 * (r * r - offset * offset).sqrt();
                let (p1, p2) = (pt(&c, "P1"), pt(&c, "P2"));
                for (n, q) in [("P1", p1), ("P2", p2)] {
                    expect_near(&format!("{n} on the line"), dist_to_line(q, l), 0.0, 20.0)?;
                    expect_near(&format!("{n} on the circle"), dist(q, o), r, 20.0)?;
                }
                expect_near("chord length", dist(p1, p2), chord, 20.0)?;
                if !same_point(pt(&c, "P"), p1, 20.0) {
                    return Err("Intersect(l, k) is not the first point".into());
                }
                for (x, y) in [("R1", p1), ("R2", p2)] {
                    if !same_point(pt(&c, x), y, 20.0) {
                        return Err(format!(
                            "{x} differs from the intersection with the other argument order"
                        ));
                    }
                }
                Ok(())
            })(),
        )
    });

    // two circles
    for_seeds("circle-circle intersection", 0x7777, 300, |rng| {
        let (o1, r1) = (rng.point(5.0), rng.range(1.0, 5.0));
        let r2 = rng.range(1.0, 5.0);
        let lo = ((r1 - r2).abs() + 0.4).max(0.4);
        let hi = r1 + r2 - 0.4;
        if lo >= hi {
            return Ok(());
        }
        let theta = rng.range(0.0, 2.0 * PI);
        let d = rng.range(lo, hi);
        let o2 = (o1.0 + d * theta.cos(), o1.1 + d * theta.sin());
        let src = format!(
            "{}{}r1 = {r1:?}\nr2 = {r2:?}\nk1 = Circle(O1, r1)\nk2 = Circle(O2, r2)\n\
             P1 = Intersect(k1, k2, 1)\nP2 = Intersect(k1, k2, 2)\n",
            p("O1", o1),
            p("O2", o2)
        );
        let c = build(&src);
        with_script(
            &src,
            (|| {
                let (p1, p2) = (pt(&c, "P1"), pt(&c, "P2"));
                for (n, q) in [("P1", p1), ("P2", p2)] {
                    expect_near(&format!("{n} on the first circle"), dist(q, o1), r1, 20.0)?;
                    expect_near(&format!("{n} on the second circle"), dist(q, o2), r2, 20.0)?;
                }
                if dist(p1, p2) < 1e-6 {
                    return Err("the two intersection points coincide".into());
                }
                Ok(())
            })(),
        )
    });
}

fn rotate_about(v: (f64, f64), q: (f64, f64), phi: f64) -> (f64, f64) {
    let (s, c) = phi.sin_cos();
    let (x, y) = (v.0 - q.0, v.1 - q.1);
    (q.0 + x * c - y * s, q.1 + x * s + y * c)
}

fn reflect_in(v: (f64, f64), l: (f64, f64, f64)) -> (f64, f64) {
    let n2 = l.0 * l.0 + l.1 * l.1;
    let t = (l.0 * v.0 + l.1 * v.1 + l.2) / n2;
    (v.0 - 2.0 * t * l.0, v.1 - 2.0 * t * l.1)
}

fn ends_of(c: &Construction, label: &str) -> [(f64, f64); 2] {
    let (p, q) = segment(c, label);
    [p, q]
}

#[test]
fn transformations_preserve_what_they_should() {
    for_seeds("transformations", 0x8888, 300, |rng| {
        let a = rng.point(8.0);
        let b = far_apart(rng, a, 1.5, 8.0);
        let m1 = rng.point(8.0);
        let m2 = far_apart(rng, m1, 2.0, 8.0);
        let q = rng.point(8.0);
        let pnt = far_apart(rng, q, 1.0, 8.0);
        let phi = rng.range(-6.0, 6.0);
        let factor = loop {
            let f = rng.range(-3.0, 3.0);
            if f.abs() >= 0.2 {
                break f;
            }
        };
        let src = format!(
            "{}{}{}{}{}{}phi = {phi:?}\nf = {factor:?}\nmir = Line(M1, M2)\ns = Segment(A, B)\n\
             k = Circle(A, B)\nl0 = Line(A, B)\n\
             A1 = Mirror(A, mir)\nB1 = Mirror(B, mir)\nA2 = Rotate(A, phi, Q)\nB2 = Rotate(B, phi, Q)\n\
             A3 = Dilate(A, f, Q)\nB3 = Dilate(B, f, Q)\nA4 = Dilate(A, f)\nA5 = Mirror(A, Q)\n\
             P1 = Mirror(P, mir)\nP2 = Mirror(P1, mir)\nP3 = Mirror(P, Q)\nP4 = Mirror(P3, Q)\n\
             P5 = Reflect(P, mir)\nA6 = Rotate(A, phi)\n\
             s1 = Mirror(s, mir)\ns2 = Rotate(s, phi, Q)\ns3 = Dilate(s, f, Q)\ns4 = Mirror(s1, mir)\n\
             ls1 = Length(s1)\nls2 = Length(s2)\nls3 = Length(s3)\nls = Length(s)\n\
             k1 = Mirror(k, mir)\nk2 = Rotate(k, phi, Q)\nk3 = Dilate(k, f, Q)\nk4 = Mirror(k1, mir)\n\
             l1 = Mirror(l0, mir)\nl2 = Rotate(l0, phi, Q)\nl3 = Dilate(l0, f, Q)\nl4 = Mirror(l1, mir)\n\
             l1b = Line(A1, B1)\nl2b = Line(A2, B2)\nl3b = Line(A3, B3)\n\
             inv = Mirror(P, k)\n",
            p("A", a),
            p("B", b),
            p("M1", m1),
            p("M2", m2),
            p("Q", q),
            p("P", pnt)
        );
        let c = build(&src);
        with_script(
            &src,
            (|| {
                let sc = 40.0 * factor.abs().max(1.0);
                let ab = dist(a, b);
                let mir = line(&c, "mir");
                let (a1, b1) = (pt(&c, "A1"), pt(&c, "B1"));
                let (a2, b2) = (pt(&c, "A2"), pt(&c, "B2"));
                let (a3, b3) = (pt(&c, "A3"), pt(&c, "B3"));
                expect_near("mirror keeps distances", dist(a1, b1), ab, sc)?;
                expect_near("rotation keeps distances", dist(a2, b2), ab, sc)?;
                expect_near(
                    "rotation keeps the distance to the centre",
                    dist(a2, q),
                    dist(a, q),
                    sc,
                )?;
                expect_near(
                    "dilation scales distances",
                    dist(a3, b3),
                    factor.abs() * ab,
                    sc,
                )?;
                if !same_point(a1, reflect_in(a, mir), sc) {
                    return Err("mirror image differs from the reflection formula".into());
                }
                if !same_point(a2, rotate_about(a, q, phi), sc) {
                    return Err(format!(
                        "rotation image {a2:?} differs from the formula {:?}",
                        rotate_about(a, q, phi)
                    ));
                }
                if !same_point(pt(&c, "A6"), rotate_about(a, (0.0, 0.0), phi), sc) {
                    return Err("Rotate without a centre is not about the origin".into());
                }
                let dil = |v: (f64, f64), centre: (f64, f64)| {
                    (
                        centre.0 + factor * (v.0 - centre.0),
                        centre.1 + factor * (v.1 - centre.1),
                    )
                };
                if !same_point(a3, dil(a, q), sc) {
                    return Err("dilation differs from the formula".into());
                }
                if !same_point(pt(&c, "A4"), dil(a, (0.0, 0.0)), sc) {
                    return Err("Dilate without a centre is not about the origin".into());
                }
                if !same_point(pt(&c, "A5"), (2.0 * q.0 - a.0, 2.0 * q.1 - a.1), sc) {
                    return Err("point reflection differs from the formula".into());
                }
                // the segment between a point and its mirror image is cut in two by the mirror
                let p1 = pt(&c, "P1");
                let mid = ((pnt.0 + p1.0) / 2.0, (pnt.1 + p1.1) / 2.0);
                expect_near(
                    "midpoint of P and its image lies on the mirror",
                    dist_to_line(mid, mir),
                    0.0,
                    sc,
                )?;
                // twice gives the original
                for (twice, orig) in [("P2", pnt), ("P4", pnt)] {
                    if !same_point(pt(&c, twice), orig, sc) {
                        return Err(format!("{twice} is not the original point"));
                    }
                }
                if !same_point(pt(&c, "P5"), p1, sc) {
                    return Err("Reflect differs from Mirror".into());
                }
                // segments
                for (e, expected) in [
                    ("s1", [a1, b1]),
                    ("s2", [a2, b2]),
                    ("s3", [a3, b3]),
                    ("s4", [a, b]),
                ] {
                    let got = ends_of(&c, e);
                    if !same_point(got[0], expected[0], sc) || !same_point(got[1], expected[1], sc)
                    {
                        return Err(format!(
                            "{e}: ends {got:?} differ from the images {expected:?}"
                        ));
                    }
                }
                expect_near("length after mirror", num(&c, "ls1"), num(&c, "ls"), sc)?;
                expect_near("length after rotation", num(&c, "ls2"), num(&c, "ls"), sc)?;
                expect_near(
                    "length after dilation",
                    num(&c, "ls3"),
                    factor.abs() * num(&c, "ls"),
                    sc,
                )?;
                // circles
                let (ox, oy, r) = circle(&c, "k");
                let img = |label: &str, centre: (f64, f64), radius: f64| -> Result<(), String> {
                    let (cx, cy, cr) = circle(&c, label);
                    if !same_point((cx, cy), centre, sc) {
                        return Err(format!("{label}: centre {:?} against {centre:?}", (cx, cy)));
                    }
                    expect_near(&format!("{label}: radius"), cr, radius, sc)
                };
                img("k1", a1, r)?;
                img("k2", a2, r)?;
                img("k3", a3, factor.abs() * r)?;
                img("k4", (ox, oy), r)?;
                // lines
                for (l, other) in [("l1", "l1b"), ("l2", "l2b"), ("l3", "l3b")] {
                    if !same_line(line(&c, l), line(&c, other), sc) {
                        return Err(format!("{l} is not the line through the images of A and B"));
                    }
                }
                if !same_line(line(&c, "l4"), line(&c, "l0"), sc) {
                    return Err("mirroring a line twice does not give the line".into());
                }
                // inversion in the circle k (centre A, radius |AB|)
                let d2 = (pnt.0 - ox).powi(2) + (pnt.1 - oy).powi(2);
                if d2 > 1e-6 {
                    let inv = pt(&c, "inv");
                    let expect = (
                        ox + r * r * (pnt.0 - ox) / d2,
                        oy + r * r * (pnt.1 - oy) / d2,
                    );
                    if !same_point(inv, expect, sc.max(r * r)) {
                        return Err(format!("inversion {inv:?} against {expect:?}"));
                    }
                }
                Ok(())
            })(),
        )
    });
}

#[test]
fn measures_agree_with_direct_formulas() {
    for_seeds("measures", 0x9999, 300, |rng| {
        let a = rng.point(10.0);
        let b = loop {
            let b = far_apart(rng, a, 1.0, 10.0);
            if (b.0 - a.0).abs() >= 0.5 {
                break b;
            }
        };
        let [t0, t1, t2] = triangle(rng, 10.0);
        let d = rng.point(10.0);
        let src = format!(
            "{}{}{}{}{}{}{}\
             s = Segment(A, B)\nl = Line(A, B)\nk = Circle(A, B)\npar = Line(P, l)\n\
             dab = Distance(A, B)\nlen = Length(s)\nrad = Radius(k)\ncirc = Circumference(k)\n\
             per = Perimeter(k)\nar = Area(k)\nar3 = Area(T0, T1, T2)\nar4 = Area(T0, T1, T2, D)\n\
             sl = Slope(l)\nsls = Slope(s)\ndpl = Distance(P, l)\ndlp = Distance(l, P)\n\
             dps = Distance(P, s)\ndll = Distance(l, par)\nang = Angle(T0, T1, T2)\n",
            p("A", a),
            p("B", b),
            p("T0", t0),
            p("T1", t1),
            p("T2", t2),
            p("D", d),
            p("P", rng.point(10.0))
        );
        let c = build(&src);
        with_script(
            &src,
            (|| {
                let sc = 40.0;
                let pnt = pt(&c, "P");
                let ab = dist(a, b);
                expect_near("Distance", num(&c, "dab"), ab, sc)?;
                expect_near("Length", num(&c, "len"), ab, sc)?;
                expect_near("Radius", num(&c, "rad"), ab, sc)?;
                expect_near("Circumference", num(&c, "circ"), 2.0 * PI * ab, sc)?;
                expect_near("Perimeter", num(&c, "per"), 2.0 * PI * ab, sc)?;
                expect_near("Area(circle)", num(&c, "ar"), PI * ab * ab, sc * sc)?;
                expect_near(
                    "Area(triangle)",
                    num(&c, "ar3"),
                    cross3(t0, t1, t2).abs() / 2.0,
                    sc * sc,
                )?;
                let shoelace = [t0, t1, t2, d];
                let mut twice = 0.0;
                for i in 0..4 {
                    let (u, v) = (shoelace[i], shoelace[(i + 1) % 4]);
                    twice += u.0 * v.1 - v.0 * u.1;
                }
                expect_near(
                    "Area(quadrilateral)",
                    num(&c, "ar4"),
                    twice.abs() / 2.0,
                    sc * sc,
                )?;
                let slope = (b.1 - a.1) / (b.0 - a.0);
                expect_near("Slope(line)", num(&c, "sl"), slope, slope.abs())?;
                expect_near("Slope(segment)", num(&c, "sls"), slope, slope.abs())?;
                let to_line = cross3(a, b, pnt).abs() / ab;
                expect_near("Distance(P, line)", num(&c, "dpl"), to_line, sc)?;
                expect_near("Distance(line, P)", num(&c, "dlp"), to_line, sc)?;
                // distance to the segment: the nearest point is the projection
                // of P on the line, clamped to the segment
                let t = (((pnt.0 - a.0) * (b.0 - a.0) + (pnt.1 - a.1) * (b.1 - a.1)) / (ab * ab))
                    .clamp(0.0, 1.0);
                let nearest = (a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1));
                expect_near(
                    "Distance(P, segment)",
                    num(&c, "dps"),
                    dist(pnt, nearest),
                    sc,
                )?;
                expect_near("Distance(line, parallel)", num(&c, "dll"), to_line, sc)?;
                // the angle at T1 from T1T0 to T1T2, counter-clockwise, in [0, 2pi)
                let (v, w) = ((t0.0 - t1.0, t0.1 - t1.1), (t2.0 - t1.0, t2.1 - t1.1));
                let mut angle = (v.0 * w.1 - v.1 * w.0).atan2(v.0 * w.0 + v.1 * w.1);
                if angle < 0.0 {
                    angle += 2.0 * PI;
                }
                expect_near("Angle", num(&c, "ang"), angle, 1.0)
            })(),
        )
    });
}

#[test]
fn angle_bisectors_are_equidistant_from_the_sides() {
    // bisector of the angle ABC
    for_seeds("angle bisector of three points", 0xAAAA, 300, |rng| {
        let [a, b, cc] = triangle(rng, 10.0);
        let src = format!(
            "{}{}{}w = AngularBisector(A, B, C)\nw2 = AngleBisector(A, B, C)\n\
             ba = Line(B, A)\nbc = Line(B, C)\n",
            p("A", a),
            p("B", b),
            p("C", cc)
        );
        let c = build(&src);
        with_script(
            &src,
            (|| {
                let (ba, bc) = (line(&c, "ba"), line(&c, "bc"));
                for name in ["w", "w2"] {
                    let w = line(&c, name);
                    expect_near(
                        &format!("{name} through the vertex"),
                        dist_to_line(b, w),
                        0.0,
                        20.0,
                    )?;
                    for q in points_on(w) {
                        expect_near(
                            &format!("{name}: equidistant from the sides"),
                            dist_to_line(q, ba),
                            dist_to_line(q, bc),
                            30.0,
                        )?;
                    }
                    // the interior bisector runs along the sum of the unit vectors
                    let (ua, uc) = (
                        ((a.0 - b.0) / dist(a, b), (a.1 - b.1) / dist(a, b)),
                        ((cc.0 - b.0) / dist(cc, b), (cc.1 - b.1) / dist(cc, b)),
                    );
                    let inside = (b.0 + ua.0 + uc.0, b.1 + ua.1 + uc.1);
                    expect_near(
                        &format!("{name} is the interior bisector"),
                        dist_to_line(inside, w),
                        0.0,
                        20.0,
                    )?;
                }
                Ok(())
            })(),
        )
    });

    // the two bisectors of two lines
    for_seeds("angle bisectors of two lines", 0xBBBB, 300, |rng| {
        let (a, c0) = (rng.point(10.0), rng.point(10.0));
        let b = far_apart(rng, a, 2.0, 10.0);
        let d = loop {
            let d = far_apart(rng, c0, 2.0, 10.0);
            let (u, v) = ((b.0 - a.0, b.1 - a.1), (d.0 - c0.0, d.1 - c0.1));
            let sin = (u.0 * v.1 - u.1 * v.0).abs() / (u.0.hypot(u.1) * v.0.hypot(v.1));
            if sin >= 0.3 {
                break d;
            }
        };
        let src = format!(
            "{}{}{}{}l = Line(A, B)\nm = Line(C, D)\ns1 = Segment(A, B)\ns2 = Segment(C, D)\n\
             X = Intersect(l, m)\nw = AngularBisector(l, m)\nv = AngleBisector(s1, s2)\n",
            p("A", a),
            p("B", b),
            p("C", c0),
            p("D", d)
        );
        let c = build(&src);
        with_script(
            &src,
            (|| {
                let (l, m, x) = (line(&c, "l"), line(&c, "m"), pt(&c, "X"));
                for family in ["w", "v"] {
                    let (w1, w2) = (
                        line(&c, &format!("{family}_{{1}}")),
                        line(&c, &format!("{family}_{{2}}")),
                    );
                    for (n, w) in [(1, w1), (2, w2)] {
                        expect_near(
                            &format!("{family}_{n} through X"),
                            dist_to_line(x, w),
                            0.0,
                            40.0,
                        )?;
                        for q in points_on(w) {
                            expect_near(
                                &format!("{family}_{n}: equidistant from both lines"),
                                dist_to_line(q, l),
                                dist_to_line(q, m),
                                40.0,
                            )?;
                        }
                    }
                    let dot = (w1.0 * w2.0 + w1.1 * w2.1) / (w1.0.hypot(w1.1) * w2.0.hypot(w2.1));
                    expect_near(
                        &format!("{family}_1 is perpendicular to {family}_2"),
                        dot,
                        0.0,
                        1.0,
                    )?;
                }
                Ok(())
            })(),
        )
    });
}

#[test]
fn perpendiculars_parallels_and_diameters() {
    for_seeds("perpendicular and parallel", 0xCCCC, 300, |rng| {
        let a = rng.point(10.0);
        let b = far_apart(rng, a, 2.0, 10.0);
        let pnt = rng.point(10.0);
        let o = rng.point(8.0);
        let src = format!(
            "{}{}{}{}r = {:?}\nl = Line(A, B)\ns = Segment(A, B)\nk = Circle(O, r)\n\
             n1 = Perpendicular(P, l)\nn2 = PerpendicularLine(P, s)\nn3 = OrthogonalLine(P, l)\n\
             p1 = Parallel(P, l)\np2 = Line(P, l)\np3 = Line(P, s)\np4 = Parallel(P, s)\n\
             dm = Diameter(l, k)\ndm2 = Diameter(s, k)\n",
            p("A", a),
            p("B", b),
            p("P", pnt),
            p("O", o),
            rng.range(1.0, 5.0)
        );
        let c = build(&src);
        with_script(
            &src,
            (|| {
                let l = line(&c, "l");
                let cos_between = |u: (f64, f64, f64), v: (f64, f64, f64)| {
                    (u.0 * v.0 + u.1 * v.1) / (u.0.hypot(u.1) * v.0.hypot(v.1))
                };
                let sin_between = |u: (f64, f64, f64), v: (f64, f64, f64)| {
                    (u.0 * v.1 - u.1 * v.0) / (u.0.hypot(u.1) * v.0.hypot(v.1))
                };
                for name in ["n1", "n2", "n3"] {
                    let n = line(&c, name);
                    expect_near(
                        &format!("{name} through P"),
                        dist_to_line(pnt, n),
                        0.0,
                        20.0,
                    )?;
                    expect_near(
                        &format!("{name} perpendicular"),
                        cos_between(n, l),
                        0.0,
                        1.0,
                    )?;
                }
                for name in ["p1", "p2", "p3", "p4"] {
                    let q = line(&c, name);
                    expect_near(
                        &format!("{name} through P"),
                        dist_to_line(pnt, q),
                        0.0,
                        20.0,
                    )?;
                    expect_near(&format!("{name} parallel"), sin_between(q, l), 0.0, 1.0)?;
                }
                for name in ["dm", "dm2"] {
                    let d = line(&c, name);
                    let (cx, cy, _) = circle(&c, "k");
                    expect_near(
                        &format!("{name} through the centre"),
                        dist_to_line((cx, cy), d),
                        0.0,
                        20.0,
                    )?;
                    expect_near(
                        &format!("{name} perpendicular to the line"),
                        cos_between(d, l),
                        0.0,
                        1.0,
                    )?;
                }
                Ok(())
            })(),
        )
    });
}

// ------------------------------------------------------ known defects
//
// The tests below that fail today are ignored so that the suite passes.
// Run them with `cargo test -p geo-kernel --test properties -- --ignored`.

/// Asserts that moving `moved` to `to` gives the same values as building the
/// construction fresh with the point already there.
fn assert_history_independent(before: &str, moved: &str, to: (f64, f64), after: &str) {
    let mut c = build(before);
    let id = c.id(moved).unwrap();
    c.move_point(id, to.0, to.1).unwrap();
    let fresh = build(after);
    if let Some(d) = first_difference(&c, &values(&c), &values(&fresh)) {
        panic!("moving {moved} to {to:?} gives a different value than building it there: {d}");
    }
}

/// Moving `C` along the line `Line(A, C)` to `(1e-9, 0)` must give the same
/// `s = Slope(l)` as building the construction with `C` already there. A
/// value must not depend on the order of moves.
#[test]
fn slope_of_a_line_depends_on_the_history_of_moves() {
    assert_history_independent(
        "A = (0, 0)\nC = (1, 0)\nl = Line(A, C)\ns = Slope(l)\n",
        "C",
        (1e-9, 0.0),
        "A = (0, 0)\nC = (0.000000001, 0)\nl = Line(A, C)\ns = Slope(l)\n",
    );
}

/// Moving `C` from `(3, 1.7)` to `(3, 2.9)` along the line `Line(A, C)` must
/// give the same `d = Distance(l, m)`, to the last bit, as building the
/// construction with `C` already there.
#[test]
fn distance_of_lines_depends_on_the_history_of_moves() {
    assert_history_independent(
        "A = (3, 0.1)\nC = (3, 1.7)\nP = (0.3, 0.7)\nl = Line(A, C)\nm = Line(P, l)\nd = Distance(l, m)\n",
        "C",
        (3.0, 2.9),
        "A = (3, 0.1)\nC = (3, 2.9)\nP = (0.3, 0.7)\nl = Line(A, C)\nm = Line(P, l)\nd = Distance(l, m)\n",
    );
}

/// `Line(A, B)` with `A = (4, 1e300)` must be undefined or a real line, never
/// the triple with `a = b = 0`.
#[test]
fn a_line_through_far_points_is_not_the_zero_line() {
    let c = build("A = (4, 1e300)\nB = (1, 1)\nl = Line(A, B)\n");
    match val(&c, "l") {
        Value::Undefined => {}
        v => {
            let (a, b, _) = line_of(&v).expect("a line");
            assert!(
                a.hypot(b) > 0.0,
                "Line(A, B) with A = (4, 1e300) is defined as {v:?}"
            );
            // The unit-normal form must not overflow to a zero or NaN line.
            let (ua, ub, uc) = v.unit_line().expect("a unit line");
            assert!(
                (ua.hypot(ub) - 1.0).abs() < 1e-12 && uc.is_finite(),
                "unit form of {v:?} is {:?}",
                (ua, ub, uc)
            );
        }
    }
}

/// Random constructions that also use `Slope(line)`, `Distance(line, line)`
/// and `Intersect(circle, circle)`. A circle intersection keeps the `special`
/// flag decided at its creation, as GeoGebra does, so a fresh build at the
/// final values may decide it otherwise; such cases are counted, not
/// compared (a circle intersected with itself after its circle was undefined
/// or a single point at creation, or the reverse).
#[test]
fn random_constructions_with_all_commands() {
    let mut totals = Totals::default();
    for_seeds(
        "random constructions, all commands",
        0x1112,
        CASES * 10,
        |rng| run_case(rng.state(), true, &mut totals),
    );
    println!(
        "{} constructions, {} script rebuilds compared, {} skipped (multi-output layout), \
         {} skipped (circle intersection decided otherwise at creation)",
        totals.cases,
        totals.script_rebuilds,
        totals.script_rebuilds_skipped,
        totals.creation_state_differs
    );
    // The skip for creation state must stay rare (19 of 4000 at the time of
    // writing).
    assert!(
        totals.creation_state_differs * 100 <= totals.cases,
        "{} of {} fresh builds differ in creation state",
        totals.creation_state_differs,
        totals.cases
    );
}

/// Shear, stretch and the affine and cross ratio against closed forms.
#[test]
fn shear_stretch_and_ratios_match_closed_forms() {
    for_seeds("shear, stretch, ratios", 0xDDDD, 300, |rng| {
        let m1 = rng.point(8.0);
        let m2 = far_apart(rng, m1, 2.0, 8.0);
        let a = rng.point(8.0);
        let b = far_apart(rng, a, 1.5, 8.0);
        let n = rng.range(-3.0, 3.0);
        // four points on a line, with well separated parameters
        let origin = rng.point(5.0);
        let theta = rng.range(0.0, 2.0 * PI);
        let u = (theta.cos(), theta.sin());
        let ts = [
            -3.0 + rng.range(-0.5, 0.5),
            -1.0 + rng.range(-0.4, 0.4),
            1.0 + rng.range(-0.4, 0.4),
            3.0 + rng.range(-0.5, 0.5),
        ];
        let on = |t: f64| (origin.0 + t * u.0, origin.1 + t * u.1);
        let src = format!(
            "{}{}{}{}{}{}{}{}n = {n:?}\naxis = Line(M1, M2)\nsg = Segment(M1, M2)\n\
             sh = Shear(A, axis, n)\nsh2 = Shear(A, sg, n)\nst = Stretch(A, axis, n)\nst2 = Stretch(A, sg, n)\n\
             shl = Shear(l0, axis, n)\nstl = Stretch(l0, axis, n)\nsegs = Segment(A, B)\n\
             shs = Shear(segs, axis, n)\nsts = Stretch(segs, axis, n)\nl0 = Line(A, B)\n\
             ar = AffineRatio(R0, R1, R2)\ncr = CrossRatio(R0, R1, R2, R3)\n",
            p("M1", m1),
            p("M2", m2),
            p("A", a),
            p("B", b),
            p("R0", on(ts[0])),
            p("R1", on(ts[1])),
            p("R2", on(ts[2])),
            p("R3", on(ts[3])),
        );
        // `l0` is used before its definition above; build in dependency order
        let src = src
            .replace("shl = Shear(l0, axis, n)\nstl = Stretch(l0, axis, n)\n", "")
            .replace(
                "l0 = Line(A, B)\n",
                "l0 = Line(A, B)\nshl = Shear(l0, axis, n)\nstl = Stretch(l0, axis, n)\n",
            );
        let c = build(&src);
        with_script(
            &src,
            (|| {
                let sc = 40.0 * n.abs().max(1.0);
                let len = dist(m1, m2);
                let d = ((m2.0 - m1.0) / len, (m2.1 - m1.1) / len);
                let nrm = (-d.1, d.0);
                // a shear moves a point along the axis by n times its signed
                // distance from the axis; a stretch scales that distance by n
                let shear = |v: (f64, f64)| {
                    let (x, y) = (v.0 - m1.0, v.1 - m1.1);
                    let h = d.0 * y - d.1 * x;
                    (v.0 + n * h * d.0, v.1 + n * h * d.1)
                };
                let stretch = |v: (f64, f64)| {
                    let (x, y) = (v.0 - m1.0, v.1 - m1.1);
                    let (along, across) = (x * d.0 + y * d.1, x * nrm.0 + y * nrm.1);
                    (
                        m1.0 + along * d.0 + n * across * nrm.0,
                        m1.1 + along * d.1 + n * across * nrm.1,
                    )
                };
                for l in ["sh", "sh2"] {
                    if !same_point(pt(&c, l), shear(a), sc) {
                        return Err(format!("{l}: {:?} against {:?}", pt(&c, l), shear(a)));
                    }
                }
                for l in ["st", "st2"] {
                    if !same_point(pt(&c, l), stretch(a), sc) {
                        return Err(format!("{l}: {:?} against {:?}", pt(&c, l), stretch(a)));
                    }
                }
                // segments and lines go to the images of their end points
                for (label, f) in [("shs", &shear as &dyn Fn(_) -> _), ("sts", &stretch)] {
                    let got = ends_of(&c, label);
                    for (g, e) in got.iter().zip([a, b]) {
                        if !same_point(*g, f(e), sc) {
                            return Err(format!("{label}: end {g:?} against {:?}", f(e)));
                        }
                    }
                }
                let (ia, ib) = (shear(a), shear(b));
                let image = format!("Line through {ia:?} and {ib:?}");
                if dist_to_line(ia, line(&c, "shl")) > TOL * sc
                    || dist_to_line(ib, line(&c, "shl")) > TOL * sc
                {
                    return Err(format!("shl does not pass through the images: {image}"));
                }
                let (ja, jb) = (stretch(a), stretch(b));
                if dist_to_line(ja, line(&c, "stl")) > TOL * sc
                    || dist_to_line(jb, line(&c, "stl")) > TOL * sc
                {
                    return Err("stl does not pass through the images of A and B".into());
                }
                // AffineRatio(A, B, C): the lambda with C = A + lambda (B - A)
                expect_near(
                    "AffineRatio",
                    num(&c, "ar"),
                    (ts[2] - ts[0]) / (ts[1] - ts[0]),
                    1.0,
                )?;
                // CrossRatio(A, B, C, D) = (B, C; D) / (A, C; D) with (S, P; Q) = (Q - S) / (P - S)
                let cross =
                    ((ts[3] - ts[1]) / (ts[2] - ts[1])) / ((ts[3] - ts[0]) / (ts[2] - ts[0]));
                expect_near("CrossRatio", num(&c, "cr"), cross, cross.abs())
            })(),
        )
    });
}

/// Rebuilding a construction from its definitions (`add(label, def(id))` for
/// every object) reproduces circle intersections: the `special` flag of
/// `Def::IntersectCircles` is state from the creation of the intersection,
/// and `add` keeps a flag that is already decided. With `M` moved to `(0, 0)`
/// after the intersection was created at `M = (2, 1)`, the intersection is
/// not at `(2, 1)`, rebuilt from the definitions or not.
#[test]
fn circle_intersections_survive_a_rebuild_from_definitions() {
    let mut c = build(
        "M = (2, 1)\nA = (2, 1)\nN = (5, 5)\nk = Circle(M, A)\nc = Circle(N, A)\n\
         P = Intersect(k, c, 1)\n",
    );
    let m = c.id("M").unwrap();
    c.move_point(m, 0.0, 0.0).unwrap();
    let mut rebuilt = Construction::new();
    for id in c.ids() {
        rebuilt.add(c.label(id), c.def(id)).unwrap();
    }
    if let Some(d) = first_difference(&c, &values(&c), &values(&rebuilt)) {
        panic!("rebuilding from the definitions changed a value: {d}");
    }
}

// ----------------------------------------------------- API robustness

struct Snapshot {
    labels: Vec<String>,
    values: Vec<Value>,
    defs: Vec<Def>,
}

fn snapshot(c: &Construction) -> Snapshot {
    Snapshot {
        labels: c.ids().map(|id| c.label(id).to_string()).collect(),
        values: values(c),
        defs: c.ids().map(|id| c.def(id)).collect(),
    }
}

fn assert_unchanged(c: &Construction, before: &Snapshot, what: &str) {
    let now = snapshot(c);
    assert_eq!(now.labels, before.labels, "{what}: labels changed");
    assert_eq!(now.defs, before.defs, "{what}: definitions changed");
    assert_eq!(now.values, before.values, "{what}: values changed");
}

const BASE: &str = "A = (0, 0)\nB = (4, 0)\nC = (1, 3)\nr = 2\nl = Line(A, B)\ns = Segment(A, C)\n\
                    k = Circle(A, r)\nd = Distance(B, C)\nM = Midpoint(A, B)\n";

#[test]
fn misuse_of_the_api_is_an_error_and_changes_nothing() {
    let mut c = build(BASE);
    let before = snapshot(&c);
    let id = |c: &Construction, l: &str| c.id(l).unwrap();
    let (a, m, r, d, l) = (
        id(&c, "A"),
        id(&c, "M"),
        id(&c, "r"),
        id(&c, "d"),
        id(&c, "l"),
    );

    // duplicate and empty labels
    for def in [
        Def::FreePoint { x: 1.0, y: 1.0 },
        Def::Number(3.0),
        Def::Midpoint(a, a),
    ] {
        assert_eq!(c.add("A", def), Err(Error::DuplicateLabel("A".into())));
        assert_eq!(c.add("M", def), Err(Error::DuplicateLabel("M".into())));
        assert_eq!(c.add("", def), Err(Error::EmptyLabel));
    }
    assert_unchanged(&c, &before, "duplicate or empty labels");

    // non-finite input
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            c.add("X", Def::FreePoint { x: bad, y: 0.0 }),
            Err(Error::NotFinite)
        );
        assert_eq!(
            c.add("X", Def::FreePoint { x: 0.0, y: bad }),
            Err(Error::NotFinite)
        );
        assert_eq!(c.add("X", Def::Number(bad)), Err(Error::NotFinite));
        assert_eq!(c.move_point(a, bad, 1.0), Err(Error::NotFinite));
        assert_eq!(c.move_point(a, 1.0, bad), Err(Error::NotFinite));
        assert_eq!(c.set_number(r, bad), Err(Error::NotFinite));
    }
    assert_unchanged(&c, &before, "non-finite input");

    // moving a dependent object, or the wrong kind of free object
    assert!(matches!(c.move_point(m, 1.0, 1.0), Err(Error::NotFree(_))));
    assert!(matches!(c.move_point(l, 1.0, 1.0), Err(Error::NotFree(_))));
    assert!(matches!(c.move_point(r, 1.0, 1.0), Err(Error::NotFree(_))));
    assert!(matches!(c.set_number(a, 1.0), Err(Error::NotFree(_))));
    assert!(matches!(c.set_number(d, 1.0), Err(Error::NotFree(_))));
    assert!(matches!(c.set_number(m, 1.0), Err(Error::NotFree(_))));
    assert_unchanged(&c, &before, "moving a dependent object");

    // an id from a larger construction
    let big = build(&format!(
        "{BASE}E = (1, 1)\nF = (2, 2)\nG = (3, 3)\nH = Midpoint(E, F)\n"
    ));
    let foreign = big.ids().last().unwrap();
    assert_eq!(
        c.add("X", Def::Midpoint(foreign, a)),
        Err(Error::UnknownObject)
    );
    assert_eq!(c.add("X", Def::Line(a, foreign)), Err(Error::UnknownObject));
    assert_eq!(c.move_point(foreign, 1.0, 1.0), Err(Error::UnknownObject));
    assert_eq!(c.set_number(foreign, 1.0), Err(Error::UnknownObject));
    assert_unchanged(&c, &before, "unknown objects");

    // bad intersection index
    let k = id(&c, "k");
    assert_eq!(
        c.add(
            "X",
            Def::IntersectLineCircle {
                line: l,
                circle: k,
                which: 2
            }
        ),
        Err(Error::BadIndex(2))
    );
    assert_eq!(
        c.add(
            "X",
            Def::IntersectCircles {
                c1: k,
                c2: k,
                which: 255,
                special: None
            }
        ),
        Err(Error::BadIndex(255))
    );
    assert_unchanged(&c, &before, "bad index");

    // every built-in definition with every combination of kinds
    type Make = fn(&[Id]) -> Def;
    let forms: Vec<(&str, Vec<K>, Make)> = vec![
        ("Line", vec![K::Point, K::Point], |i| Def::Line(i[0], i[1])),
        ("Segment", vec![K::Point, K::Point], |i| {
            Def::Segment(i[0], i[1])
        }),
        ("Midpoint", vec![K::Point, K::Point], |i| {
            Def::Midpoint(i[0], i[1])
        }),
        ("Distance", vec![K::Point, K::Point], |i| {
            Def::Distance(i[0], i[1])
        }),
        ("Perpendicular", vec![K::Point, K::Line], |i| {
            Def::Perpendicular {
                through: i[0],
                to: i[1],
            }
        }),
        ("Parallel", vec![K::Point, K::Line], |i| Def::Parallel {
            through: i[0],
            to: i[1],
        }),
        ("CircleThrough", vec![K::Point, K::Point], |i| {
            Def::CircleThrough {
                center: i[0],
                through: i[1],
            }
        }),
        ("CircleRadius", vec![K::Point, K::Number], |i| {
            Def::CircleRadius {
                center: i[0],
                radius: i[1],
            }
        }),
        ("IntersectLines", vec![K::Line, K::Line], |i| {
            Def::IntersectLines(i[0], i[1])
        }),
        ("IntersectLineCircle", vec![K::Line, K::Circle], |i| {
            Def::IntersectLineCircle {
                line: i[0],
                circle: i[1],
                which: 0,
            }
        }),
        ("IntersectCircles", vec![K::Circle, K::Circle], |i| {
            Def::IntersectCircles {
                c1: i[0],
                c2: i[1],
                which: 1,
                special: None,
            }
        }),
    ];
    let all: Vec<Id> = c.ids().collect();
    let mut accepted = 0;
    let mut n = 0;
    for (name, slots, make) in &forms {
        for &x in &all {
            for &y in &all {
                let ids = [x, y];
                let fits = |i: usize| {
                    let kind = c.kind(ids[i]);
                    // a line or a segment both serve as a line-like argument
                    slots[i].fits(kind)
                        || (matches!(*name, "Perpendicular" | "Parallel")
                            && i == 1
                            && kind == Kind::Segment)
                };
                let should_work = fits(0) && fits(1);
                let len = c.len();
                n += 1;
                let result = c.add(&format!("X{n}"), make(&ids));
                match (should_work, result) {
                    (true, Ok(new)) => {
                        accepted += 1;
                        assert_eq!(c.len(), len + 1);
                        assert!(is_all_finite(&c.value(new)), "{name}: {:?}", c.value(new));
                    }
                    (false, Err(Error::WrongKind { .. })) => {
                        assert_eq!(c.len(), len, "{name}: a failed add changed the length");
                        assert!(
                            c.id(&format!("X{n}")).is_none(),
                            "{name}: the label of a failed add stays"
                        );
                    }
                    (should, other) => panic!(
                        "{name}({}, {}): should work = {should}, got {other:?}",
                        c.label(x),
                        c.label(y)
                    ),
                }
            }
        }
    }
    assert!(accepted >= forms.len(), "no form was ever accepted");
    // the construction still works after all the failures
    let a = c.id("A").unwrap();
    assert!(c.move_point(a, 0.5, 0.5).is_ok());
    assert!(c.add("Z", Def::FreePoint { x: 1.0, y: 2.0 }).is_ok());
}

// ------------------------------------------------- the command matrix

fn world(points: [(f64, f64); 4], r: f64, q: f64) -> String {
    format!(
        "{}{}{}{}r = {r:?}\nq = {q:?}\nl = Line(A, B)\nm = Line(C, D)\ns = Segment(A, C)\n\
         s2 = Segment(B, D)\nk = Circle(A, r)\nk2 = Circle(B, C)\nd = Distance(A, B)\n",
        p("A", points[0]),
        p("B", points[1]),
        p("C", points[2]),
        p("D", points[3]),
    )
}

/// Every command with every combination of arguments in three worlds: an
/// ordinary one, one with numbers near the limits of `f64`, and one where
/// all points coincide. A call whose argument kinds match a form in
/// [`COMMANDS`] must succeed; any call must either create objects with
/// finite values or fail and leave the construction as it was.
#[test]
fn every_command_with_every_argument_kind() {
    let worlds = [
        (
            "ordinary",
            world([(0.0, 0.0), (4.0, 0.0), (1.0, 3.0), (-2.0, 2.5)], 2.0, -1.0),
        ),
        (
            "extreme",
            world(
                [
                    (1e308, 1e308),
                    (-1e308, 1e308),
                    (1e-308, 5e-324),
                    (0.0, -0.0),
                ],
                1e308,
                -1e308,
            ),
        ),
        (
            "all points coincide",
            world([(1.0, 1.0), (1.0, 1.0), (1.0, 1.0), (1.0, 1.0)], 0.0, 0.0),
        ),
    ];
    let mut calls = 0usize;
    let mut accepted = 0usize;
    let mut problems: Vec<String> = Vec::new();
    for (world_name, src) in &worlds {
        for (ci, cmd) in COMMANDS.iter().enumerate() {
            let mut c = build(src);
            let base = c.len();
            let base_values = values(&c);
            let mut rng = Rng::new(case_seed(0xD00D, ci as u64));
            let arity = cmd.slots.len();
            let mut tuples: Vec<Vec<usize>> = Vec::new();
            // tuples whose kinds fit the form
            for _ in 0..100 {
                let fitting: Option<Vec<usize>> = cmd
                    .slots
                    .iter()
                    .map(|slot| {
                        if slot.contains(&K::Index) {
                            return Some(1 + rng.below(2));
                        }
                        let candidates: Vec<usize> = (0..base)
                            .filter(|&i| slot.iter().any(|k| k.fits(c.kind(id_at(&c, i)))))
                            .collect();
                        (!candidates.is_empty()).then(|| *rng.pick(&candidates))
                    })
                    .collect();
                if let Some(t) = fitting {
                    tuples.push(t);
                }
            }
            // arbitrary tuples; all of them while the number stays small
            let space = base.pow(arity as u32);
            if space <= 400 {
                for code in 0..space {
                    let mut rest = code;
                    tuples.push(
                        (0..arity)
                            .map(|_| {
                                let i = rest % base;
                                rest /= base;
                                i
                            })
                            .collect(),
                    );
                }
            } else {
                for _ in 0..250 {
                    tuples.push((0..arity).map(|_| rng.below(base)).collect());
                }
            }
            for (n, t) in tuples.iter().enumerate() {
                let kinds: Vec<Kind> = t.iter().map(|&i| c.kind(id_at(&c, i))).collect();
                let fits = cmd
                    .slots
                    .iter()
                    .zip(t)
                    .zip(&kinds)
                    .all(|((slot, &i), &kind)| {
                        if slot.contains(&K::Index) {
                            i == 1 || i == 2
                        } else {
                            slot.iter().any(|k| k.fits(kind))
                        }
                    });
                let texts: Vec<String> = cmd
                    .slots
                    .iter()
                    .zip(t)
                    .map(|(slot, &i)| {
                        if slot.contains(&K::Index) {
                            i.to_string()
                        } else {
                            c.label(id_at(&c, i)).to_string()
                        }
                    })
                    .collect();
                let line = format!("z{n} = {}({})", cmd.name, texts.join(", "));
                let len = c.len();
                calls += 1;
                let result = match catch(|| exec(&mut c, &line)) {
                    Ok(r) => r,
                    Err(panic) => {
                        problems.push(format!("[{world_name}] `{line}` panicked: {panic}"));
                        continue;
                    }
                };
                match result {
                    Ok(Some(first)) => {
                        accepted += 1;
                        if c.len() <= len || first.index() != len {
                            problems.push(format!(
                                "[{world_name}] `{line}` reported id {} with {} -> {} objects",
                                first.index(),
                                len,
                                c.len()
                            ));
                        }
                        if let Some(bad) =
                            c.ids().skip(len).find(|&id| !is_all_finite(&c.value(id)))
                        {
                            problems.push(format!(
                                "[{world_name}] `{line}` made a non-finite value: `{}` = {:?}",
                                c.label(bad),
                                c.value(bad)
                            ));
                        }
                    }
                    Ok(None) => problems.push(format!("[{world_name}] `{line}` created nothing")),
                    Err(e) => {
                        if c.len() != len
                            || c.ids()
                                .take(base)
                                .map(|id| c.value(id))
                                .ne(base_values.iter().copied())
                        {
                            problems.push(format!(
                                "[{world_name}] `{line}` failed ({e}) but changed the construction"
                            ));
                        }
                        if fits {
                            problems.push(format!(
                                "[{world_name}] `{line}` has matching kinds but failed: {e}"
                            ));
                        }
                    }
                }
                if problems.len() > 20 {
                    break;
                }
            }
        }
    }
    println!("{calls} command calls, {accepted} accepted");
    assert!(
        problems.is_empty(),
        "{} problem(s), first ones:\n{}",
        problems.len(),
        problems
            .iter()
            .take(12)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}
