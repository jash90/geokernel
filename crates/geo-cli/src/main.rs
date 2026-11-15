use geo_kernel::svg::{self, View};
use geo_kernel::{script, Construction, Def, Id, Value};
use std::hint::black_box;
use std::process::ExitCode;
use std::time::Instant;

const USAGE: &str = "\
geokernel: proof-of-concept dynamic-geometry kernel

USAGE:
    geokernel run <script.geo> [out.svg]   build a construction from a script
    geokernel demo [out-dir]               triangle demo: draw, drag A, draw again
    geokernel bench [steps]                time dragging a point in a large construction
";

const TRIANGLE: &str = include_str!("../../../examples/triangle.geo");

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("run") if args.len() >= 2 => run(&args[1], args.get(2).map(String::as_str)),
        Some("demo") => demo(args.get(1).map(String::as_str).unwrap_or(".")),
        Some("bench") => match args.get(1).map(|s| s.parse::<usize>()) {
            None => bench(15_000),
            Some(Ok(steps)) if steps > 0 => bench(steps),
            Some(_) => Err("steps must be a positive whole number".to_string()),
        },
        _ => {
            eprint!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(path: &str, out: Option<&str>) -> Result<(), String> {
    let src = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let c = script::load(&src).map_err(|e| format!("{path}: {e}"))?;
    print_objects(&c);
    if let Some(out) = out {
        write_svg(&c, View::fit(&c), out)?;
    }
    Ok(())
}

fn demo(dir: &str) -> Result<(), String> {
    let before = script::load(TRIANGLE).map_err(|e| e.to_string())?;
    let mut after = script::load(TRIANGLE).map_err(|e| e.to_string())?;
    let a = after.id("A").ok_or("the demo script must define A")?;

    println!("Before:");
    print_objects(&before);

    let recomputed = after.move_point(a, -2.0, 2.5).map_err(|e| e.to_string())?;
    println!(
        "\nMoved A to (-2, 2.5): {recomputed} of {} objects recomputed.\n",
        after.len()
    );
    println!("After:");
    print_objects(&after);
    println!();

    // One view for both frames so they can be compared directly.
    let view = View::fit(&before).union(View::fit(&after));
    write_svg(&before, view, &format!("{dir}/triangle-before.svg"))?;
    write_svg(&after, view, &format!("{dir}/triangle-after.svg"))?;
    Ok(())
}

fn write_svg(c: &Construction, view: View, path: &str) -> Result<(), String> {
    std::fs::write(path, svg::render(c, view, 800, 600)).map_err(|e| format!("{path}: {e}"))?;
    println!("wrote {path}");
    Ok(())
}

fn print_objects(c: &Construction) {
    for id in c.ids() {
        println!(
            "  {:<4} {:<8} {}",
            c.label(id),
            c.kind(id).to_string(),
            describe(c.value(id))
        );
    }
}

fn describe(v: Value) -> String {
    match v {
        Value::Point { x, y } => format!("({x:.4}, {y:.4})"),
        // Shown with (a, b) of unit length. Adding 0.0 turns a negative zero
        // into a plain zero for display.
        Value::Line { .. } => {
            let (a, b, c) = v.unit_line().unwrap_or((f64::NAN, f64::NAN, f64::NAN));
            format!("{:.4}x + {:.4}y + {:.4} = 0", a + 0.0, b + 0.0, c + 0.0)
        }
        Value::Segment { x1, y1, x2, y2 } => {
            format!(
                "({x1:.4}, {y1:.4}) - ({x2:.4}, {y2:.4}), length {:.4}",
                (x2 - x1).hypot(y2 - y1)
            )
        }
        Value::Circle { cx, cy, r } => format!("centre ({cx:.4}, {cy:.4}), radius {r:.4}"),
        Value::Number(n) => format!("{n:.4}"),
        Value::Vector { x, y } => format!("({x:.4}, {y:.4})"),
        Value::Ray { x, y, a, b, c } => {
            let n = a.hypot(b);
            format!(
                "from ({x:.4}, {y:.4}) on {:.4}x + {:.4}y + {:.4} = 0",
                a / n + 0.0,
                b / n + 0.0,
                c / n + 0.0
            )
        }
        Value::Bool(b) => b.to_string(),
        Value::Undefined => "undefined".to_string(),
        // `Value` is non-exhaustive: a kind this printer does not know yet
        // is shown in its debug form.
        other => format!("{other:?}"),
    }
}

/// Builds a long chain in which every object depends on the free point A,
/// then times dragging A. Each step adds seven objects and turns the
/// previous point by 45 degrees around A, so nothing degenerates.
fn bench(steps: usize) -> Result<(), String> {
    let e = |err: geo_kernel::Error| err.to_string();
    let mut c = Construction::new();
    let a = c.add("A", Def::FreePoint { x: 0.3, y: 0.7 }).map_err(e)?;
    let b = c.add("B", Def::FreePoint { x: 10.0, y: 2.0 }).map_err(e)?;

    let built = Instant::now();
    let mut p = b;
    for i in 0..steps {
        let l = c.add(&format!("l{i}"), Def::Line(a, p)).map_err(e)?;
        let k = c
            .add(
                &format!("k{i}"),
                Def::CircleThrough {
                    center: a,
                    through: p,
                },
            )
            .map_err(e)?;
        let n = c
            .add(&format!("n{i}"), Def::Perpendicular { through: a, to: l })
            .map_err(e)?;
        let x = c
            .add(
                &format!("X{i}"),
                Def::IntersectLineCircle {
                    line: n,
                    circle: k,
                    which: 0,
                },
            )
            .map_err(e)?;
        let m = c.add(&format!("M{i}"), Def::Midpoint(p, x)).map_err(e)?;
        let g = c.add(&format!("g{i}"), Def::Line(a, m)).map_err(e)?;
        p = c
            .add(
                &format!("P{i}"),
                Def::IntersectLineCircle {
                    line: g,
                    circle: k,
                    which: 1,
                },
            )
            .map_err(e)?;
    }
    let build_time = built.elapsed();

    // A separate little construction that does not depend on A.
    let z1 = c
        .add("Z1", Def::FreePoint { x: 50.0, y: 50.0 })
        .map_err(e)?;
    let z2 = c
        .add("Z2", Def::FreePoint { x: 52.0, y: 51.0 })
        .map_err(e)?;
    c.add("Zm", Def::Midpoint(z1, z2)).map_err(e)?;

    println!("objects:            {}", c.len());
    println!(
        "build:              {:.1} ms",
        build_time.as_secs_f64() * 1e3
    );

    // Drag A around a small circle: everything in the chain must follow.
    let drags = 200;
    let mut recomputed = 0usize;
    let start = Instant::now();
    for i in 0..drags {
        let t = i as f64 * 0.05;
        recomputed += c.move_point(a, 0.3 + t.cos(), 0.7 + t.sin()).map_err(e)?;
    }
    let full = start.elapsed();
    let per_drag = full.as_secs_f64() / drags as f64;
    println!(
        "full drag (A):      {:.3} ms per drag, {} objects recomputed each time, {:.1} M objects/s",
        per_drag * 1e3,
        recomputed / drags,
        recomputed as f64 / full.as_secs_f64() / 1e6
    );

    // Drag Z1: only its own midpoint depends on it.
    let local_drags = 1_000_000;
    let mut local = 0usize;
    let start = Instant::now();
    for i in 0..local_drags {
        local += c
            .move_point(z1, 50.0 + ((i % 1000) + 1) as f64 * 0.001, 50.0)
            .map_err(e)?;
    }
    let local_time = start.elapsed();
    println!(
        "local drag (Z1):    {:.1} ns per drag, {} object recomputed each time",
        local_time.as_secs_f64() * 1e9 / local_drags as f64,
        local / local_drags
    );

    let undefined = count_undefined(&c);
    println!("undefined objects:  {undefined}");
    black_box(&c);
    if undefined != 0 {
        return Err("the benchmark construction degenerated; timings are not meaningful".into());
    }
    Ok(())
}

fn count_undefined(c: &Construction) -> usize {
    c.ids().filter(|&id: &Id| !c.value(id).is_defined()).count()
}
