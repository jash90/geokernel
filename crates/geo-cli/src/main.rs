use geo_kernel::svg::{self, View};
use geo_kernel::{script, Construction, Def, Id, Value};
use std::hint::black_box;
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;
use std::time::Instant;

const USAGE: &str = "\
geokernel: proof-of-concept dynamic-geometry kernel

USAGE:
    geokernel run <script.geo> [out.svg]   build a construction from a script
    geokernel run <file.ggb> [out.svg]     open a construction file saved by GeoGebra (any name)
    geokernel export <in> <out.ggb>      save a script or construction as .ggb
    geokernel demo [out-dir]               triangle demo: draw, drag A, draw again
    geokernel bench [steps]                time dragging a point in a large construction
";

const TRIANGLE: &str = include_str!("../../../examples/triangle.geo");

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("run") if args.len() >= 2 => run(&args[1], args.get(2).map(String::as_str)),
        Some("export") if args.len() == 3 => export(&args[1], &args[2]),
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

/// Imports a construction file into script text. What was left out is
/// reported on stderr.
fn import_file(path: &str, bytes: &[u8]) -> Result<geo_import::Imported, String> {
    let imported = geo_import::import_for_replay(bytes).map_err(|e| format!("{path}: {e}"))?;
    for note in &imported.notes {
        eprintln!("note: {note}");
    }
    if !imported.skipped.is_empty() {
        eprintln!("skipped {} objects:", imported.skipped.len());
        for s in imported.skipped.iter().take(20) {
            let label = if s.label.is_empty() {
                "(no label)"
            } else {
                &s.label
            };
            eprintln!("  {label} ({}): {}", s.kind, s.reason);
        }
        if imported.skipped.len() > 20 {
            eprintln!("  ... and {} more", imported.skipped.len() - 20);
        }
    }
    if !imported.mismatched.is_empty() {
        eprintln!(
            "{} objects differ from the value saved in the file:",
            imported.mismatched.len()
        );
        for m in imported.mismatched.iter().take(20) {
            eprintln!("  {}: saved {}, computed {}", m.label, m.saved, m.computed);
        }
        if imported.mismatched.len() > 20 {
            eprintln!("  ... and {} more", imported.mismatched.len() - 20);
        }
    }
    if !imported.hidden.is_empty() {
        eprintln!(
            "{} objects have visibility turned off in the file",
            imported.hidden.len()
        );
    }
    Ok(imported)
}

fn export(path: &str, out: &str) -> Result<(), String> {
    if same_file::is_same_file(path, out).unwrap_or(false) {
        return Err("Export would overwrite its input file.".into());
    }
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let mut wb = geo_app_core::workbench::Workbench::default();
    wb.load_for_export(&bytes, path)?;
    let archive = wb.export_ggb(None).map_err(|e| e.to_string())?;
    write_archive(Path::new(out), &archive).map_err(|e| format!("{out}: {e}"))
}

fn write_archive(path: &Path, bytes: &[u8]) -> io::Result<()> {
    // Stage in the destination directory so replacing a file is atomic.
    let staging = path.with_file_name(format!(".ggb-export-{}.tmp", std::process::id()));
    let mut file = std::fs::File::create_new(&staging)?;
    let written = file.write_all(bytes);
    drop(file);
    let result = written.and_then(|()| std::fs::rename(&staging, path));
    if result.is_err() {
        let _ = std::fs::remove_file(&staging);
    }
    result
}

fn run(path: &str, out: Option<&str>) -> Result<(), String> {
    // A construction file is told from a script by its content, not its name.
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let (src, hidden, conditions, assets) = if geo_import::looks_like_construction(&bytes) {
        let imported = import_file(path, &bytes)?;
        (
            imported.script.join("\n"),
            imported.hidden,
            imported.conditions,
            imported.image_assets,
        )
    } else {
        (
            String::from_utf8(bytes).map_err(|e| format!("{path}: {e}"))?,
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
    };
    let mut c = script::load_with_assets(&src, &assets).map_err(|e| format!("{path}: {e}"))?;
    print_objects(&c);
    if let Some(out) = out {
        let mut hidden: std::collections::HashSet<_> =
            hidden.iter().filter_map(|label| c.id(label)).collect();
        for (target, source) in &conditions {
            let Some(id) = c.id(target) else { continue };
            let condition =
                geo_kernel::kernel::control::visibility_boolean(&mut c, target, source)?;
            if c.value(condition) == Value::Bool(true) {
                hidden.remove(&id);
            } else {
                hidden.insert(id);
            }
        }
        let rendered = svg::render_visible(&c, View::fit(&c), 800, 600, |id| !hidden.contains(&id));
        std::fs::write(out, rendered).map_err(|e| format!("{out}: {e}"))?;
        println!("wrote {out}");
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
    for id in c.ids().filter(|&id| !c.label(id).starts_with('\0')) {
        let label = c.label(id);
        let kind = c.kind(id).to_string();
        let value = describe(c, id);
        if let Some(source) = c.construction_source(id) {
            println!("  {label:<4} {kind:<8} {value}  [{source}]");
        } else {
            println!("  {label:<4} {kind:<8} {value}");
        }
    }
}

fn describe(c: &Construction, id: Id) -> String {
    let v = c.value(id);
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
        // GeoGebra shows the area of a polygon, always positive.
        Value::Polygon { area } => format!("area {:.4}", area.abs()),
        Value::PolyLine { length } => format!("length {length:.4}"),
        Value::Undefined => "undefined".to_string(),
        // The implicit equation, the reference's default form.
        Value::Conic { .. } => match c.conic(id) {
            Some(d) => geo_kernel::equation::conic_equation(&d, &four_decimals),
            None => "undefined".to_string(),
        },
        // A function stands for its values at a few points.
        Value::Function => {
            let label = c.label(id);
            let values: Vec<String> = [-1.0, 0.0, 1.0]
                .iter()
                .map(|&x| match c.function_value(id, x) {
                    Some(y) if y.is_finite() => {
                        format!("{label}({x}) = {}", four_decimals(y))
                    }
                    _ => format!("{label}({x}) undefined"),
                })
                .collect();
            format!("function, {}", values.join(", "))
        }
        // `Value` is non-exhaustive: a kind this printer does not know yet
        // is shown in its debug form.
        other => format!("{other:?}"),
    }
}

/// Four decimals, never a negative zero.
fn four_decimals(v: f64) -> String {
    let s = format!("{v:.4}");
    if s == "-0.0000" {
        "0.0000".to_string()
    } else {
        s
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
    // A free point whose only dependent is created last (see below).
    let f = c.add("F", Def::FreePoint { x: -3.0, y: 1.0 }).map_err(e)?;

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
    c.add("Fm", Def::Midpoint(f, b)).map_err(e)?;

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

    // Drag F: its one dependent is the last object, far from F.
    let far_drags = 1_000_000;
    let mut far = 0usize;
    let start = Instant::now();
    for i in 0..far_drags {
        far += c
            .move_point(f, -3.0 + ((i % 1000) + 1) as f64 * 0.001, 1.0)
            .map_err(e)?;
    }
    let far_time = start.elapsed();
    println!(
        "far drag (F):       {:.1} ns per drag, {} object recomputed each time",
        far_time.as_secs_f64() * 1e9 / far_drags as f64,
        far / far_drags
    );

    // Drag E: 1000 expression objects depend on it, in four shapes (point,
    // number from coordinates, number with a decimal literal times a free
    // literal number, function value).
    let mut x = Construction::new();
    let lines = ["E = (1.5, 2.5)", "a = 0.1", "f(x) = x^2 + a"];
    for line in lines {
        script::exec(&mut x, line)?;
    }
    let exprs = 1000;
    for i in 0..exprs / 4 {
        for line in [
            format!("M{i} = (E + ({i}, 1)) / 2"),
            format!("c{i} = 0.5 x(E) + {i}"),
            format!("d{i} = (a + 0.2) y(E) - {i}"),
            format!("v{i} = f(x(E)) + {i}"),
        ] {
            script::exec(&mut x, &line)?;
        }
    }
    let e_id = x.id("E").ok_or("no E")?;
    let expr_drags = 2_000;
    let mut expr_count = 0usize;
    let start = Instant::now();
    for i in 0..expr_drags {
        let t = i as f64 * 0.01;
        expr_count += x
            .move_point(e_id, 1.5 + t.cos(), 2.5 + t.sin())
            .map_err(e)?;
    }
    let expr_time = start.elapsed();
    println!(
        "expr drag (E):      {:.1} us per drag, {} expressions recomputed each time, {:.1} ns per expression",
        expr_time.as_secs_f64() * 1e6 / expr_drags as f64,
        expr_count / expr_drags,
        expr_time.as_secs_f64() * 1e9 / expr_count.max(1) as f64
    );
    black_box(&x);

    let undefined = count_undefined(&c) + count_undefined(&x);
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
