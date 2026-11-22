//! The document as bytes and text. Loading takes a file's contents, not a
//! path, so a platform without plain file paths (Android's content URIs) only
//! has to hand over the bytes; the whole state comes out as a script for
//! saving it.

use super::Workbench;
use crate::scriptlog::{Log, Rejected};

/// The format of `snapshot_text`. Its first line is a script comment, so a
/// snapshot is also a plain script; `restore_text` reads exactly this
/// version and refuses any other.
pub const SNAPSHOT_VERSION: u32 = 1;
/// How many left-out lines the notice after a restore quotes.
const QUOTED: usize = 3;
const SNAPSHOT_HEADER: &str = "# geokernel snapshot ";

impl Workbench {
    /// Replaces the whole document with a file's contents: a saved
    /// construction (zip or XML, found by its contents, not its name) or a
    /// script. `name` prefixes every error. On error the document is
    /// unchanged.
    pub fn load_bytes(&mut self, bytes: &[u8], name: &str) -> Result<(), String> {
        if geo_import::looks_like_construction(bytes) {
            let imported = geo_import::import(bytes).map_err(|e| format!("{name}: {e}"))?;
            let notice = import_notice(&imported);
            return self
                .load_imported(&imported.script.join("\n"), &imported.hidden, notice)
                .map_err(|e| format!("{name}: {e}"));
        }
        let src = std::str::from_utf8(bytes).map_err(|e| format!("{name}: {e}"))?;
        self.load_script(src).map_err(|e| format!("{name}: {e}"))
    }

    /// The whole document as a script that `restore_text` (or `load_bytes`)
    /// reads back to the same construction, moves and hidden objects. The
    /// selection, the undo steps and the input history are not part of it.
    ///
    /// A hook for a future autosave (for example when Android pauses the
    /// app). It walks the whole journal, so it is meant for such moments and
    /// never for every change; the desktop window calls neither function.
    pub fn snapshot_text(&self) -> String {
        format!(
            "{SNAPSHOT_HEADER}{SNAPSHOT_VERSION}\n{}",
            self.log.to_labelled_script()
        )
    }

    /// Replaces the document with a `snapshot_text`, keeping every line that
    /// loads: a kernel of another version may refuse some line, and the rest
    /// of the user's work must survive. The refused lines (and the lines
    /// that depend on them) are returned and quoted in `notice`, the way an
    /// import reports what it left out. The header may follow a byte order
    /// mark and blank lines; any version but `SNAPSHOT_VERSION` is an error,
    /// which leaves the document unchanged. A text without the header (a
    /// plain script) loads the same way, without the check of the labels
    /// each line makes, which needs the snapshot's label lists.
    ///
    /// Files the user opens (`load_bytes`, `load_script`) stay strict: the
    /// first bad line is an error naming it, as desktop users know it.
    pub fn restore_text(&mut self, text: &str) -> Result<Vec<Rejected>, String> {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let first = text.lines().map(str::trim).find(|l| !l.is_empty());
        let header = first.and_then(|l| l.strip_prefix(SNAPSHOT_HEADER.trim_end()));
        if let Some(v) = header {
            if v.trim() != SNAPSHOT_VERSION.to_string() {
                return Err(format!("Nieznana wersja zapisu: {}", v.trim()));
            }
        }
        let (c, log, rejected) = Log::load_tolerant(text, header.is_some());
        self.replace(c, log);
        self.notice = rejected_notice(&rejected);
        Ok(rejected)
    }
}

/// The notice after a restore that left lines out. `None` when all loaded.
pub fn rejected_notice(rejected: &[Rejected]) -> Option<String> {
    if rejected.is_empty() {
        return None;
    }
    let dependents = rejected.iter().filter(|r| r.dependent).count();
    let direct = rejected.len() - dependents;
    let mut text = format!("Pominięte linie zapisu: {direct} (nie dały się wczytać)");
    if dependents > 0 {
        text.push_str(&format!(", zależne od nich: {dependents}"));
    }
    let quoted: Vec<String> = rejected
        .iter()
        .take(QUOTED)
        .map(|r| format!("linia {}: {}", r.line, r.text))
        .collect();
    text.push_str(&format!(". {}", quoted.join("; ")));
    if rejected.len() > QUOTED {
        text.push_str(&format!(" i {} więcej", rejected.len() - QUOTED));
    }
    Some(text)
}

/// The text to show after a file was imported: what was left out (objects the
/// kernel cannot make, objects that depend on them), objects whose value
/// differs from the one saved in the file, and the notes of the importer.
/// `None` when there is nothing to tell.
pub fn import_notice(imported: &geo_import::Imported) -> Option<String> {
    let dependents = imported
        .skipped
        .iter()
        .filter(|s| s.reason.starts_with("depends on"))
        .count();
    let direct = imported.skipped.len() - dependents;
    let mut parts = Vec::new();
    if !imported.mismatched.is_empty() {
        let labels = imported
            .mismatched
            .iter()
            .take(QUOTED)
            .map(|m| geo_import::convert::quote(&m.label))
            .collect::<Vec<_>>()
            .join(", ");
        let more = imported.mismatched.len().saturating_sub(QUOTED);
        let extra = if more > 0 {
            format!(" i {more} więcej")
        } else {
            String::new()
        };
        parts.push(format!(
            "Wartości różne od zapisanych w pliku: {} ({labels}{extra})",
            imported.mismatched.len()
        ));
    }
    if direct > 0 {
        parts.push(format!(
            "Pominięte obiekty: {direct} (jądro ich nie obsługuje)"
        ));
    }
    if dependents > 0 {
        parts.push(format!("zależne od nich: {dependents}"));
    }
    if parts.is_empty() && imported.notes.is_empty() {
        return None;
    }
    parts.push(format!("wczytane linie: {}", imported.script.len()));
    let mut text = parts.join(", ");
    if !imported.notes.is_empty() {
        text.push_str(&format!(". Uwagi: {}", imported.notes.join("; ")));
    }
    Some(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo_kernel::kernel::{Construction, Kind, Value};

    /// Everything a reader of the document can see: each object's label,
    /// kind, value and visibility.
    fn state(wb: &Workbench) -> Vec<String> {
        let c: &Construction = wb.construction();
        c.ids()
            .map(|id| {
                format!(
                    "{} {:?} {:?} {}",
                    c.label(id),
                    c.kind(id),
                    c.value(id),
                    wb.is_visible(id)
                )
            })
            .collect()
    }

    /// The text of `wb` read back gives the same state and an equal journal,
    /// and the copy goes on like the original: after the same move (which
    /// reads the intersections' hints and `special` flags) both are again the
    /// same.
    fn assert_round_trip(wb: &mut Workbench, what: &str) {
        let text = wb.snapshot_text();
        let mut back = Workbench::default();
        let rejected = back
            .restore_text(&text)
            .unwrap_or_else(|e| panic!("{what}: {e}\n{text}"));
        assert!(rejected.is_empty(), "{what}: {rejected:?}");
        assert_eq!(back.notice, None, "{what}");
        // A snapshot is also a plain script that a strict load accepts.
        let mut strict = Workbench::default();
        strict.load_script(&text).unwrap();
        assert_eq!(strict.log, wb.log, "{what}");
        assert_eq!(back.log, wb.log, "{what}:\n{text}");
        assert_eq!(state(&back), state(wb), "{what}:\n{text}");
        // And the text of the copy is the same text.
        assert_eq!(back.snapshot_text(), text, "{what}");
        let c = wb.construction();
        let free: Vec<_> = c
            .ids()
            .filter(|&id| c.def(id).is_free() && c.kind(id) == Kind::Point)
            .take(3)
            .collect();
        for (k, id) in free.into_iter().enumerate() {
            let (x, y) = (0.75 - k as f64 / 3.0, k as f64 / 7.0 - 1.25);
            assert_eq!(wb.move_point(id, x, y), back.move_point(id, x, y), "{what}");
        }
        assert_eq!(state(&back), state(wb), "{what}: after a move:\n{text}");
        assert_eq!(back.snapshot_text(), wb.snapshot_text(), "{what}");
    }

    #[test]
    fn the_examples_round_trip_after_moves_and_hiding() {
        for (name, src) in [
            (
                "triangle",
                include_str!("../../../../examples/triangle.geo"),
            ),
            (
                "four-lines",
                include_str!("../../../../examples/four-lines.geo"),
            ),
        ] {
            let mut wb = Workbench::from_script(src).unwrap();
            assert_round_trip(&mut wb, name);
            let c = wb.construction();
            let points: Vec<_> = c.ids().filter(|&id| c.kind(id) == Kind::Point).collect();
            for (k, &id) in points.iter().enumerate() {
                wb.move_point(id, 0.1 * k as f64 + 1.0 / 3.0, -2.0 / 7.0);
            }
            if let Some(&last) = points.last() {
                wb.toggle_visible(last);
            }
            assert_round_trip(&mut wb, name);
        }
    }

    /// The saved files of the golden set, and those of them that do not load.
    const GOLDEN_FILES: usize = 39;
    const GOLDEN_FILES_NOT_LOADED: &[&str] = &[];

    #[test]
    fn the_saved_files_of_the_golden_set_round_trip() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/ggb");
        let mut files = Vec::new();
        let mut dirs = vec![root];
        while let Some(dir) = dirs.pop() {
            for entry in std::fs::read_dir(&dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    dirs.push(path);
                } else if path.extension().is_some_and(|e| e == "ggb") {
                    files.push(path);
                }
            }
        }
        assert!(files.len() > 10, "the golden files are missing");
        files.sort();
        let mut skipped = Vec::new();
        for path in &files {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let mut wb = Workbench::default();
            match wb.load_bytes(&std::fs::read(path).unwrap(), &name) {
                Ok(()) => assert_round_trip(&mut wb, &name),
                Err(e) => skipped.push(format!("{name}: {e}")),
            }
        }
        // The files the kernel cannot load yet, by name: a file that starts
        // to load (or stops) shows up here instead of passing unnoticed.
        let names: Vec<&str> = skipped
            .iter()
            .map(|s| s.split(": ").next().unwrap())
            .collect();
        assert_eq!(names, GOLDEN_FILES_NOT_LOADED, "{skipped:#?}");
        assert_eq!(files.len(), GOLDEN_FILES);
    }

    /// A small deterministic generator (xorshift), so the generated cases
    /// are the same on every run.
    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        fn below(&mut self, n: usize) -> usize {
            (self.next() % n as u64) as usize
        }
        /// A coordinate that is rarely a short decimal, and now and then one
        /// whose text is unusual: `-0`, a subnormal, a long integer, a small
        /// fraction.
        fn coord(&mut self) -> f64 {
            match self.below(16) {
                0 => -0.0,
                1 => 5e-324,
                2 => 1e21,
                3 => -1e-7,
                _ => (self.below(4001) as f64 - 2000.0) / 7.0,
            }
        }
    }

    /// The labels of the objects of `kind` in `wb`.
    fn of_kind(wb: &Workbench, kinds: &[Kind]) -> Vec<String> {
        let c = wb.construction();
        c.ids()
            .filter(|&id| kinds.contains(&c.kind(id)))
            .map(|id| c.label(id).to_owned())
            .collect()
    }

    /// The free objects of `kind` in `wb`.
    fn free_of_kind(wb: &Workbench, kind: Kind) -> Vec<String> {
        let c = wb.construction();
        c.ids()
            .filter(|&id| c.kind(id) == kind && c.def(id).is_free())
            .map(|id| c.label(id).to_owned())
            .collect()
    }

    /// What `generated` did, for the test to check that it covers its cases.
    #[derive(Default)]
    struct Done {
        deletes: usize,
        /// Bare definitions that made something.
        bare: usize,
    }

    /// Builds a construction from random edits, the way the window makes
    /// them: definitions (named, or bare so that they take the next free
    /// label, which a delete may have freed), commands with several
    /// outputs, expressions, moves, new values, hiding, deleting, undo and
    /// redo.
    fn generated(seed: u64, done: &mut Done) -> Workbench {
        let mut rng = Rng(seed);
        let mut wb = Workbench::default();
        for step in 0..48 {
            let points = of_kind(&wb, &[Kind::Point]);
            let curves = of_kind(&wb, &[Kind::Line, Kind::Segment, Kind::Circle]);
            let circles = of_kind(&wb, &[Kind::Circle]);
            let numbers = free_of_kind(&wb, Kind::Number);
            let pick = |rng: &mut Rng, from: &[String]| from[rng.below(from.len())].clone();
            let line = match rng.below(21) {
                0..=2 => Some(format!("P{step} = ({}, {})", rng.coord(), rng.coord())),
                3 => {
                    let literal = ["0.1", "2.5", "-3", "0.3"][rng.below(4)];
                    Some(format!("n{step} = {literal}"))
                }
                4..=5 if points.len() >= 2 => {
                    let (a, b) = (pick(&mut rng, &points), pick(&mut rng, &points));
                    let cmd = ["Segment", "Line", "Circle", "Midpoint"][rng.below(4)];
                    let label = if cmd == "Midpoint" { "M" } else { "f" };
                    Some(format!("{label}{step} = {cmd}({a}, {b})"))
                }
                6 if points.len() >= 2 => {
                    // A bare definition takes the next free label.
                    let (a, b) = (pick(&mut rng, &points), pick(&mut rng, &points));
                    let cmd = ["Segment", "Midpoint", "Line"][rng.below(3)];
                    Some(format!("{cmd}({a}, {b})"))
                }
                7..=9 if curves.len() >= 2 => {
                    let (a, b) = (pick(&mut rng, &curves), pick(&mut rng, &curves));
                    Some(match rng.below(2) {
                        0 => format!("I{step} = Intersect({a}, {b})"),
                        _ => format!("Intersect({a}, {b})"),
                    })
                }
                10 if points.len() >= 3 => {
                    let v: Vec<String> = (0..3).map(|_| pick(&mut rng, &points)).collect();
                    Some(match rng.below(2) {
                        0 => format!("q{step} = Polygon({}, {}, {})", v[0], v[1], v[2]),
                        _ => format!("Polygon({}, {}, {})", v[0], v[1], v[2]),
                    })
                }
                11 if !circles.is_empty() && !points.is_empty() => {
                    let (p, k) = (pick(&mut rng, &points), pick(&mut rng, &circles));
                    Some(format!("{{T{step}a, T{step}b}} = Tangent({p}, {k})"))
                }
                12 if points.len() >= 2 => {
                    let (a, b) = (pick(&mut rng, &points), pick(&mut rng, &points));
                    Some(format!("E{step} = ({a} + {b}) / 2"))
                }
                13 => Some(format!("w{step} = {}°", rng.below(360))),
                14 if !numbers.is_empty() => {
                    let n = pick(&mut rng, &numbers);
                    Some(match rng.below(2) {
                        0 => format!("g{step}(x) = x^2 + {n}"),
                        _ => format!("s{step} = {n} + 0.2"),
                    })
                }
                _ => None,
            };
            if let Some(line) = line {
                // Failing lines (parallel lines, a point with itself) are fine.
                let before = wb.construction().len();
                let made = wb.run_line(&line).is_ok() && wb.construction().len() > before;
                done.bare += usize::from(made && !line.contains('='));
                continue;
            }
            match rng.below(7) {
                0 | 1 if !points.is_empty() => {
                    let free = free_of_kind(&wb, Kind::Point);
                    if free.is_empty() {
                        continue;
                    }
                    let id = wb.construction().id(&pick(&mut rng, &free)).unwrap();
                    wb.begin_gesture();
                    for _ in 0..rng.below(3) + 1 {
                        wb.move_point(id, rng.coord(), rng.coord());
                    }
                    wb.end_gesture();
                }
                2 if !numbers.is_empty() => {
                    let id = wb.construction().id(&pick(&mut rng, &numbers)).unwrap();
                    // Now and then the value it has: that drops a typed
                    // decimal, and is a change only then.
                    let v = match (rng.below(4), wb.construction().value(id)) {
                        (0, Value::Number(old)) => old,
                        (1, _) => -0.0,
                        _ => rng.coord(),
                    };
                    wb.set_number(id, v);
                }
                3 if !wb.construction().is_empty() => {
                    let id = wb
                        .construction()
                        .ids()
                        .nth(rng.below(wb.construction().len()));
                    match rng.below(2) {
                        0 => wb.toggle_visible(id.unwrap()),
                        _ => {
                            let before = wb.construction().len();
                            if wb.delete(id.unwrap()) {
                                assert!(wb.construction().len() < before, "the delete deleted");
                                done.deletes += 1;
                            }
                        }
                    }
                }
                4 => {
                    wb.undo();
                }
                5 => {
                    wb.redo();
                }
                _ => {}
            }
        }
        wb
    }

    /// Whether an intersection's curves have a point that the text moves
    /// after the intersection was made.
    fn moves_a_parent_of_an_intersection(text: &str) -> bool {
        let lines: Vec<&str> = text.lines().collect();
        let args = |line: &str| -> Vec<String> {
            let inner = line.split_once('(').map_or("", |(_, r)| r);
            inner
                .trim_end_matches(|c: char| c == ')' || c.is_whitespace())
                .split(',')
                .map(|a| a.trim().to_owned())
                .collect()
        };
        let defining = |label: &str| {
            lines
                .iter()
                .find(|l| {
                    l.split(" #")
                        .next()
                        .unwrap()
                        .split_once(" = ")
                        .is_some_and(|(lhs, _)| lhs == label)
                })
                .copied()
        };
        lines.iter().enumerate().any(|(i, line)| {
            let body = line.split(" #").next().unwrap();
            if !body.contains("= Intersect(") {
                return false;
            }
            let points: Vec<String> = args(body.split_once(" = ").unwrap().1)
                .iter()
                .filter_map(|curve| defining(curve))
                .flat_map(|d| args(d.split(" #").next().unwrap().split_once(" = ").unwrap().1))
                .collect();
            lines[i + 1..].iter().any(|l| {
                l.strip_prefix("SetCoords(")
                    .and_then(|r| r.split(',').next())
                    .is_some_and(|p| points.iter().any(|q| q == p))
            })
        })
    }

    #[test]
    fn generated_constructions_round_trip() {
        let mut sizes = 0;
        let mut done = Done::default();
        let subnormal = format!("0.{}5", "0".repeat(323));
        type Check = Box<dyn Fn(&str) -> bool>;
        let checks: [(&str, Check); 16] = [
            ("SetCoords", Box::new(|t| t.contains("SetCoords("))),
            ("SetValue", Box::new(|t| t.contains("SetValue("))),
            (
                "SetVisibleInView",
                Box::new(|t| t.contains("SetVisibleInView(")),
            ),
            ("Intersect", Box::new(|t| t.contains("Intersect("))),
            ("Polygon", Box::new(|t| t.contains("Polygon("))),
            ("{a, b} = Tangent", Box::new(|t| t.contains("} = Tangent("))),
            ("label list", Box::new(|t| t.contains("# labels: "))),
            ("free angle", Box::new(|t| t.contains("°"))),
            ("function", Box::new(|t| t.contains("(x) = x^2 + "))),
            ("point sum", Box::new(|t| t.contains(") / 2"))),
            ("number sum", Box::new(|t| t.contains(" + 0.2"))),
            (
                "-0",
                Box::new(|t| t.contains("(-0, ") || t.contains(", -0)") || t.contains(", -0,")),
            ),
            ("subnormal", Box::new(move |t| t.contains(&subnormal))),
            ("1e21", Box::new(|t| t.contains("1000000000000000000000"))),
            ("-1e-7", Box::new(|t| t.contains("-0.0000001"))),
            (
                "typed decimal",
                Box::new(|t| t.contains(" = 0.1\n") || t.contains(" = 0.3\n")),
            ),
        ];
        let mut seen = [0; 16];
        let mut moved_parents = 0;
        for seed in 1..=200u64 {
            let mut wb = generated(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15), &mut done);
            sizes += wb.construction().len();
            let text = wb.snapshot_text();
            for (n, (_, check)) in checks.iter().enumerate() {
                seen[n] += usize::from(check(&text));
            }
            moved_parents += usize::from(moves_a_parent_of_an_intersection(&text));
            assert_round_trip(&mut wb, &format!("seed {seed}"));
        }
        // The generator makes real constructions with every kind of entry.
        assert!(sizes > 200 * 5, "{sizes}");
        let names: Vec<&str> = checks.iter().map(|(n, _)| *n).collect();
        assert!(seen.iter().all(|&n| n >= 10), "{names:?}\n{seen:?}");
        assert!(
            done.deletes >= 50 && done.bare >= 50,
            "{} {}",
            done.deletes,
            done.bare
        );
        assert!(moved_parents >= 10, "{moved_parents}");
    }

    #[test]
    fn a_set_to_minus_zero_or_of_a_typed_decimal_is_a_change() {
        let mut wb = Workbench::from_script("A = (0, 0)\nn = 4\nm = 0.1\ns = m + 0.2\n").unwrap();
        let id = |wb: &Workbench, l: &str| wb.construction().id(l).unwrap();
        let (a, n, m, sum) = (id(&wb, "A"), id(&wb, "n"), id(&wb, "m"), id(&wb, "s"));
        // `-0` is not `0`: the kernel takes it, so the journal must too.
        assert!(wb.move_point(a, -0.0, 0.0));
        assert!(!wb.move_point(a, -0.0, 0.0));
        assert!(wb.set_number(n, 0.0));
        assert!(wb.set_number(n, -0.0));
        assert!(!wb.set_number(n, -0.0));
        // `m` keeps the exact decimal it was typed with until it is set,
        // even to the same value; what reads it changes in the last bits.
        let exact = wb.construction().value(sum);
        assert_eq!(exact, Value::Number(0.3));
        assert!(wb.set_number(m, 0.1), "the decimal went: a change");
        assert_eq!(wb.construction().value(sum), Value::Number(0.1 + 0.2));
        assert!(!wb.set_number(m, 0.1));
        let text = wb.snapshot_text();
        assert!(text.contains("SetCoords(A, -0, 0)\n"), "{text}");
        assert!(
            text.contains("SetValue(n, -0)\nSetValue(m, 0.1)\n"),
            "{text}"
        );
        // Undo takes the set back, decimal and all.
        wb.undo();
        assert_eq!(wb.construction().value(sum), exact);
        wb.redo();
        assert_round_trip(&mut wb, "signed zeros and decimals");
    }

    #[test]
    fn a_free_angle_is_a_free_number_in_degrees_and_stays_an_angle() {
        let mut wb = Workbench::from_script("w = 30°\nA = (1, 0)\n").unwrap();
        let w = wb.construction().id("w").unwrap();
        let row = wb.rows()[0].clone();
        assert_eq!(row.head, "w =");
        assert!((row.free_number.unwrap() - 30f64.to_radians()).abs() < 1e-15);
        assert!(row.angle && !wb.rows()[1].angle);
        assert!(wb.set_number(w, 45f64.to_radians()));
        assert!(matches!(
            wb.construction().def(w),
            geo_kernel::kernel::Def::Angle(_)
        ));
        let text = wb.snapshot_text();
        assert!(text.contains("w = 30°\n"), "{text}");
        assert!(
            text.contains(&format!("SetValue(w, {})", 45f64.to_radians())),
            "{text}"
        );
        assert_round_trip(&mut wb, "angle");
    }

    #[test]
    fn a_function_has_a_row_with_its_definition_and_can_be_picked() {
        let mut wb = Workbench::from_script("A = (5, 5)\nf(x) = x^2\n").unwrap();
        let c = wb.construction();
        let f = c.id("f").unwrap();
        assert_eq!(c.kind(f), Kind::Function);
        assert!(
            crate::describe::is_known(&c.value(f)),
            "the window draws it"
        );
        let t = crate::interact::ViewTransform::fit(c, 400.0, 400.0);
        for (x, y) in [(0.0, 0.0), (1.0, 1.0), (2.0, 4.0)] {
            let at = t.world_to_screen(x, y);
            let reach = crate::hit::Reach::square(3.0);
            assert_eq!(
                crate::hit::pick(c, &t, at, &reach, None, |id| id == f),
                Some(f)
            );
        }
        let rows = wb.rows();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].label, "f");
        assert_eq!(rows[1].head, "f = x^2");
        assert_eq!(rows[1].value, None, "the definition is the value");
        assert_round_trip(&mut wb, "function");
    }

    #[test]
    fn unusual_number_texts_are_read_back_bit_for_bit() {
        let tiny = format!("0.{}5", "0".repeat(323));
        let mut wb = Workbench::default();
        wb.run_line(&format!("P = (-0, {tiny})")).unwrap();
        wb.run_line("Q = (1000000000000000000000, -0.0000001)")
            .unwrap();
        let c = wb.construction();
        let point = |l: &str| match c.value(c.id(l).unwrap()) {
            Value::Point { x, y } => (x.to_bits(), y.to_bits()),
            v => panic!("{v:?}"),
        };
        assert_eq!(point("P"), ((-0.0f64).to_bits(), 5e-324f64.to_bits()));
        assert_eq!(point("Q"), (1e21f64.to_bits(), (-1e-7f64).to_bits()));
        let p = wb.construction().id("P").unwrap();
        assert!(wb.move_point(p, 5e-324, -0.0));
        assert_round_trip(&mut wb, "unusual numbers");
    }

    #[test]
    fn a_snapshot_starts_with_its_version_and_a_newer_one_is_refused() {
        let wb = Workbench::from_script("A = (0, 0)\n").unwrap();
        let text = wb.snapshot_text();
        assert!(
            text.starts_with("# geokernel snapshot 1\nA = (0, 0)\n"),
            "{text}"
        );
        let mut other = Workbench::from_script("B = (1, 1)\n").unwrap();
        let err = other
            .restore_text("# geokernel snapshot 2\nA = (0, 0)\n")
            .unwrap_err();
        assert!(err.contains('2'), "{err}");
        assert!(
            other.construction().id("B").is_some(),
            "the document is unchanged"
        );
    }

    #[test]
    fn a_restore_keeps_the_lines_that_load_and_tells_about_the_rest() {
        let mut wb = Workbench::default();
        let rejected = wb
            .restore_text(
                "# geokernel snapshot 1\nA = (0, 0)\nQ = Nope(A)\nR = Midpoint(Q, A)\n\
                 SetVisibleInView(R, 1, false)\nB = (1, 1)\nSetVisibleInView(B, 1, false)\n",
            )
            .unwrap();
        assert_eq!(rejected.len(), 3);
        let c = wb.construction();
        assert!(c.id("A").is_some() && c.id("Q").is_none());
        assert!(!wb.is_visible(c.id("B").unwrap()));
        assert_eq!(
            wb.notice.as_deref(),
            Some(
                "Pominięte linie zapisu: 1 (nie dały się wczytać), zależne od nich: 2. \
                 linia 3: Q = Nope(A); linia 4: R = Midpoint(Q, A); \
                 linia 5: SetVisibleInView(R, 1, false)"
            )
        );
        // A user-opened file with the same lines fails at the first bad one.
        let err = wb
            .load_bytes(b"A = (0, 0)\nQ = Nope(A)\n", "x.geo")
            .unwrap_err();
        assert!(err.starts_with("x.geo: line 2:"), "{err}");
    }

    /// The snapshot of `lines` run one by one, with the line that starts
    /// with `refused` replaced by one the kernel refuses (the same label).
    fn snapshot_with_a_refused_line(lines: &[&str], refused: &str) -> String {
        let mut wb = Workbench::default();
        for line in lines {
            wb.run_line(line).unwrap_or_else(|e| panic!("{line}: {e}"));
        }
        let text = wb.snapshot_text();
        let mut found = false;
        let out: Vec<String> = text
            .lines()
            .map(|l| match l.strip_prefix(refused) {
                Some(_) if !found => {
                    found = true;
                    format!("{refused}Nope(A)")
                }
                _ => l.to_owned(),
            })
            .collect();
        assert!(found, "no line starts with {refused}:\n{text}");
        out.join("\n")
    }

    fn rejected_lines(rejected: &[Rejected]) -> Vec<(String, bool)> {
        rejected
            .iter()
            .map(|r| (r.text.clone(), r.dependent))
            .collect()
    }

    #[test]
    fn a_line_that_makes_other_labels_than_saved_is_left_out_with_what_uses_it() {
        // In the session `a` was a segment, so the polygon named its first
        // side `a_1`. Restored with `a` refused, the polygon would take `a`
        // for that side and the directive would hide the wrong object.
        let text = snapshot_with_a_refused_line(
            &[
                "A = (0, 0)",
                "B = (1, 0)",
                "C = (0, 1)",
                "a = Segment(B, C)",
                "p = Polygon(A, B, C)",
                "n = Length(a_1)",
            ],
            "a = ",
        );
        let text =
            format!("{text}\nSetVisibleInView(a, 1, false)\nSetVisibleInView(A, 1, false)\n");
        assert!(
            text.contains("p = Polygon(A, B, C) # labels: p, c, a_1, b"),
            "{text}"
        );
        let mut wb = Workbench::default();
        let rejected = wb.restore_text(&text).unwrap();
        let polygon = text.lines().find(|l| l.starts_with("p = ")).unwrap();
        let polygon = polygon.split(" #").next().unwrap();
        assert_eq!(
            rejected_lines(&rejected),
            [
                ("a = Nope(A)".to_owned(), false),
                (polygon.to_owned(), false),
                ("n = Length(a_1)".to_owned(), true),
                ("SetVisibleInView(a, 1, false)".to_owned(), true),
            ]
        );
        assert!(rejected[1].reason.contains("a_1"), "{}", rejected[1].reason);
        let c = wb.construction();
        let labels: Vec<&str> = c.ids().map(|id| c.label(id)).collect();
        assert_eq!(labels, ["A", "B", "C"], "nothing of the polygon stays");
        assert!(!wb.is_visible(c.id("A").unwrap()));
        // A label list is checked wherever it comes; without the header
        // and the lists the text is a plain script: the polygon takes `a`.
        let plain = text.split_once('\n').unwrap().1;
        let mut wb = Workbench::default();
        assert_eq!(wb.restore_text(plain).unwrap().len(), 4);
        let bare: String = plain
            .lines()
            .map(|l| format!("{}\n", l.split(" #").next().unwrap()))
            .collect();
        let rejected = wb.restore_text(&bare).unwrap();
        assert_eq!(rejected[0].text, "a = Nope(A)");
        let c = wb.construction();
        assert_eq!(c.kind(c.id("a").unwrap()), Kind::Segment);
    }

    #[test]
    fn a_refused_line_in_a_chain_leaves_out_what_follows_from_it_only() {
        let lines = [
            "A = (0, 0)",
            "B = (2, 0)",
            "C = (0, 2)",
            "P = (5, 5)",
            "k = Circle(A, B)",
            "{t_{1}, t_{2}} = Tangent(P, k)",
            "q = Polygon(A, B, C)",
            "g = Line(A, C)",
            "I = Intersect(t_{1}, k)",
            "J = Intersect(t_{2}, g)",
            "r = Polygon(P, I, C)",
            "Midpoint(B, C)",
        ];
        let text = snapshot_with_a_refused_line(&lines, "k = ");
        let mut wb = Workbench::default();
        let rejected = wb.restore_text(&text).unwrap();
        let texts: Vec<&str> = rejected.iter().map(|r| r.text.as_str()).collect();
        assert_eq!(texts.len(), 5, "{texts:?}");
        assert_eq!(texts[0], "k = Nope(A)");
        assert!(texts[1].starts_with("{t_{1}, t_{2}} = Tangent"));
        assert!(texts[2].starts_with("I = ") && texts[3].starts_with("J = "));
        assert!(texts[4].starts_with("r = Polygon(P, I, C)"));
        assert!(rejected[1..].iter().all(|r| r.dependent));
        // What does not follow from `k` stays, with the labels it had.
        let mut full = Workbench::default();
        for line in lines {
            full.run_line(line).unwrap();
        }
        let (c, f) = (wb.construction(), full.construction());
        let left_out = ["k", "t_{1}", "t_{2}", "I", "J", "r"];
        let kept: Vec<&str> = f
            .ids()
            .map(|id| f.label(id))
            .filter(|l| {
                !left_out.contains(l)
                    && !full
                        .log
                        .multi_output()
                        .any(|g| g[0] == "r" && g.iter().any(|x| x == l))
            })
            .collect();
        let restored: Vec<&str> = c.ids().map(|id| c.label(id)).collect();
        assert_eq!(restored, kept, "{text}");
        for &label in &restored {
            assert_eq!(
                c.value(c.id(label).unwrap()),
                f.value(f.id(label).unwrap()),
                "{label}"
            );
        }
    }

    #[test]
    fn the_header_may_follow_a_byte_order_mark_and_blank_lines() {
        let body = "A = (0, 0)\nB = (1, 1)\n";
        let mut wb = Workbench::default();
        // No header: a plain script, read without the label checks.
        assert!(wb.restore_text(body).unwrap().is_empty());
        assert_eq!(wb.construction().len(), 2);
        for text in [
            format!("# geokernel snapshot 2\n{body}"),
            format!("\u{feff}# geokernel snapshot 2\n{body}"),
            format!("\u{feff}\n  \n# geokernel snapshot 2\n{body}"),
            format!("# geokernel snapshot 0\n{body}"),
            format!("# geokernel snapshot\n{body}"),
        ] {
            let mut other = Workbench::from_script("Q = (5, 5)\n").unwrap();
            let err = other.restore_text(&text).unwrap_err();
            assert!(err.starts_with("Nieznana wersja zapisu"), "{err}");
            assert!(other.construction().id("Q").is_some(), "unchanged");
        }
        let text = "\u{feff}\n# geokernel snapshot 1\nA = (0, 0)\nb = Nope(A)\n";
        let rejected = wb.restore_text(text).unwrap();
        assert_eq!(rejected.len(), 1);
        assert_eq!(rejected[0].line, 4, "lines count from the top of the text");
        // A script the user opens may start with a byte order mark too.
        wb.load_bytes("\u{feff}A = (1, 2)\n".as_bytes(), "bom.geo")
            .unwrap();
        assert_eq!(wb.construction().len(), 1);
    }

    #[test]
    fn the_notice_quotes_the_first_left_out_lines_and_counts_the_rest() {
        let mut wb = Workbench::default();
        let text = "# geokernel snapshot 1\nA = (0, 0)\nP = Nope(A)\nQ = Nope(A)\n\
                    R = Nope(A)\nS = Nope(A)\nT = Midpoint(S, A)\n";
        let rejected = wb.restore_text(text).unwrap();
        assert_eq!(rejected.len(), 5);
        assert_eq!(
            wb.notice.as_deref(),
            Some(
                "Pominięte linie zapisu: 4 (nie dały się wczytać), zależne od nich: 1. \
                 linia 3: P = Nope(A); linia 4: Q = Nope(A); linia 5: R = Nope(A) i 2 więcej"
            )
        );
    }

    #[test]
    fn the_directive_names_name_no_object() {
        for line in [
            "SetCoords = (1, 2)",
            "SetValue: 3",
            "{SetVisibleInView, B} = Intersect(f, g)",
            "SetValue(x) = x^2",
            "f = SetCoords(A, 1, 2)",
        ] {
            let mut wb = Workbench::from_script("A = (0, 0)\n").unwrap();
            let err = wb.run_line(line).unwrap_err();
            assert!(err.contains("file directive"), "{line}: {err}");
            assert_eq!(wb.construction().len(), 1, "{line}");
            // An opened file names the line.
            let src = format!("A = (0, 0)\n{line}\n");
            let err = wb.load_bytes(src.as_bytes(), "x.geo").unwrap_err();
            assert!(err.starts_with("x.geo: line 2: "), "{line}: {err}");
            assert!(err.contains("file directive"), "{line}: {err}");
        }
        // Typed in the input bar, a directive becomes `f = SetCoords(...)`
        // and is refused with a message that says what to do instead.
        let mut wb = Workbench::from_script("A = (0, 0)\n").unwrap();
        let err = wb.run_line("SetCoords(A, 1, 2)").unwrap_err();
        assert!(
            crate::errors::polish(&err).starts_with("`SetCoords` działa tylko w pliku zapisu"),
            "{err}"
        );
        let err = wb.run_line("SetValue = 3").unwrap_err();
        assert!(
            crate::errors::polish(&err).contains("nie może być nazwą obiektu"),
            "{err}"
        );
    }

    #[test]
    fn load_bytes_tells_a_script_from_a_saved_construction() {
        let mut wb = Workbench::default();
        wb.load_bytes(b"A = (1, 2)\nB = (3, 4)\n", "x.xml").unwrap();
        assert_eq!(wb.construction().len(), 2);
        let xml = "<geogebra><construction>\
            <element type=\"point\" label=\"A\"><show object=\"false\"/><coords x=\"1\" y=\"2\" z=\"1\"/></element>\
            <element type=\"point\" label=\"B\"><coords x=\"3\" y=\"4\" z=\"1\"/></element>\
            <element type=\"text\" label=\"T\"/>\
            </construction></geogebra>";
        wb.load_bytes(xml.as_bytes(), "saved.dat").unwrap();
        let c = wb.construction();
        let (a, b) = (c.id("A").unwrap(), c.id("B").unwrap());
        assert!(!wb.is_visible(a) && wb.is_visible(b));
        assert!(wb
            .notice
            .as_deref()
            .unwrap()
            .starts_with("Pominięte obiekty: 1"));
        // Hidden imported objects survive the text round trip.
        assert_round_trip(&mut wb, "imported");
        // Damaged contents leave the document alone and name the file.
        let err = wb.load_bytes(b"<geogebra>", "a.ggb").unwrap_err();
        assert!(err.starts_with("a.ggb: "), "{err}");
        let err = wb.load_bytes(&[b'A', 0xff, b'\n'], "b.geo").unwrap_err();
        assert!(err.starts_with("b.geo: "), "{err}");
        let err = wb
            .load_bytes(b"A = (1, 2)\nQ = Nope(A)\n", "c.geo")
            .unwrap_err();
        assert!(err.starts_with("c.geo: line 2:"), "{err}");
        assert_eq!(wb.construction().len(), 2);
    }

    #[test]
    fn imported_path_coordinates_and_angle_styles_survive_a_workbench_snapshot() {
        let cases = [
            include_bytes!("../../../../golden/ggb/saved/saved-paths-001.xml").as_slice(),
            include_bytes!("../../../../golden/ggb/saved/saved-review-001.xml").as_slice(),
        ];
        for xml in cases {
            let mut wb = Workbench::default();
            wb.load_bytes(xml, "construction.xml").unwrap();
            assert!(!wb
                .notice
                .as_deref()
                .unwrap_or_default()
                .starts_with("Wartości różne od zapisanych w pliku"));
            let c = wb.construction();
            if let Some(angle) = c.id("reflex") {
                assert_eq!(
                    c.angle_style(angle),
                    Some(geo_kernel::kernel::angle::AngleStyle::Reflex)
                );
                assert_eq!(wb.log.rhs_of("A"), Some("(sqrt(2), 1)"));
            } else {
                assert!(c.path_state(c.id("F").unwrap()).is_some());
            }
            assert_round_trip(&mut wb, "imported construction");
        }
    }

    #[test]
    fn saved_texts_have_free_and_dependent_rows_and_follow_edits() {
        use geo_kernel::kernel::text::TextPosition;

        let mut wb = Workbench::default();
        let mut reference = Workbench::default();
        for (w, bytes) in [
            (
                &mut wb,
                include_bytes!("../../../../golden/ggb/saved/saved-texts-001.ggb").as_slice(),
            ),
            (
                &mut reference,
                include_bytes!("../../../../golden/ggb/saved/saved-texts-002.ggb").as_slice(),
            ),
        ] {
            w.load_bytes(bytes, "text.ggb").unwrap();
            let hidden = w.construction().id("hiddenText").unwrap();
            assert!(!w.is_visible(hidden));
            let rows = w.rows();
            assert!(rows.iter().find(|r| r.label == "marker").unwrap().free);
            assert!(!rows.iter().find(|r| r.label == "readout").unwrap().free);
        }
        let p = wb.construction().id("P").unwrap();
        let a = wb.construction().id("a").unwrap();
        let c = reference.construction();
        let Value::Point { x, y } = c.value(c.id("P").unwrap()) else {
            panic!("expected a point");
        };
        let Value::Number(n) = c.value(c.id("a").unwrap()) else {
            panic!("expected a number");
        };
        let readout = |w: &mut Workbench| {
            w.rows()
                .iter()
                .find(|r| r.label == "readout")
                .unwrap()
                .value
                .clone()
        };
        let before = readout(&mut wb);
        assert!(wb.move_point(p, x, y));
        assert!(wb.set_number(a, n));
        assert_ne!(readout(&mut wb), before);
        assert_eq!(readout(&mut wb), readout(&mut reference));
        assert_round_trip(&mut wb, "saved texts");

        let Value::Point { x, y } = wb.construction().value(p) else {
            panic!("expected the point after the round trip move");
        };
        assert!(wb.delete(p));
        let c = wb.construction();
        assert!(c.id("readout").is_none());
        assert_eq!(
            c.text_position(c.id("marker").unwrap()),
            Some(TextPosition::World { x, y })
        );
        assert!(wb.undo());
        let c = wb.construction();
        assert_eq!(
            c.text_position(c.id("marker").unwrap()),
            Some(TextPosition::Point(c.id("P").unwrap()))
        );
    }

    #[test]
    fn a_saved_value_mismatch_is_named_first_in_the_file_notice() {
        let mut wb = Workbench::default();
        let xml = include_bytes!("../../../../golden/ggb/saved/saved-doubletangent-001.xml");
        wb.load_bytes(xml, "construction.xml").unwrap();
        assert!(wb
            .notice
            .as_deref()
            .unwrap()
            .starts_with("Wartości różne od zapisanych w pliku: 1 (P_{2})"));
        assert!(wb.construction().id("P_{2}").is_some());
    }

    #[test]
    fn the_notice_counts_skipped_dependents_mismatches_and_notes() {
        use geo_import::{Imported, Mismatch, Skipped};
        let skipped = |label: &str, reason: &str| Skipped {
            label: label.into(),
            kind: "Ray".into(),
            reason: reason.into(),
        };
        let none = Imported::default();
        assert_eq!(import_notice(&none), None);
        let imported = Imported {
            script: vec!["A = (0, 0)".into(); 7],
            skipped: vec![
                skipped("a", "an expression"),
                skipped("b", "depends on the skipped object `a`"),
                skipped("c", "depends on the skipped object `a`"),
            ],
            mismatched: vec![Mismatch {
                label: "d".into(),
                saved: String::new(),
                computed: String::new(),
            }],
            notes: vec!["continuity".into()],
            ..Imported::default()
        };
        assert_eq!(
            import_notice(&imported).unwrap(),
            "Wartości różne od zapisanych w pliku: 1 (d), Pominięte obiekty: 1 (jądro ich nie obsługuje), \
             zależne od nich: 2, wczytane linie: 7. Uwagi: continuity"
        );
    }
}
