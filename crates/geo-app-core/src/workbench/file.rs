//! The document as bytes and text. Loading takes a file's contents, not a
//! path, so a platform without plain file paths (Android's content URIs) only
//! has to hand over the bytes; the whole state comes out as a script for
//! saving it.

use super::Workbench;
use crate::scriptlog::{Log, Rejected};

/// The format of `snapshot_text`. Its first line is a script comment, so a
/// snapshot is also a plain script; `restore_text` refuses a newer version.
pub const SNAPSHOT_VERSION: u32 = 1;
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
            self.log.to_script()
        )
    }

    /// Replaces the document with a `snapshot_text`, keeping every line that
    /// loads: a kernel of another version may refuse some line, and the rest
    /// of the user's work must survive. The refused lines (and the lines
    /// that depend on them) are returned and counted in `notice`, the way an
    /// import reports what it left out. A text without the header loads the
    /// same way. Only a header of a newer format is an error, which leaves
    /// the document unchanged.
    ///
    /// Files the user opens (`load_bytes`, `load_script`) stay strict: the
    /// first bad line is an error naming it, as desktop users know it.
    pub fn restore_text(&mut self, text: &str) -> Result<Vec<Rejected>, String> {
        if let Some(v) = text
            .lines()
            .next()
            .and_then(|l| l.strip_prefix(SNAPSHOT_HEADER))
        {
            match v.trim().parse::<u32>() {
                Ok(v) if v <= SNAPSHOT_VERSION => {}
                _ => return Err(format!("Nieznana wersja zapisu: {}", v.trim())),
            }
        }
        let (c, log, rejected) = Log::load_tolerant(text);
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
    if direct > 0 {
        parts.push(format!(
            "Pominięte obiekty: {direct} (jądro ich nie obsługuje)"
        ));
    }
    if dependents > 0 {
        parts.push(format!("zależne od nich: {dependents}"));
    }
    if !imported.mismatched.is_empty() {
        parts.push(format!(
            "wartości różne od zapisanych w pliku: {}",
            imported.mismatched.len()
        ));
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
    use geo_kernel::kernel::{Construction, Kind};

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

    /// The text of `wb` read back gives the same state and an equal journal.
    fn assert_round_trip(wb: &Workbench, what: &str) {
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
            assert_round_trip(&wb, name);
            let c = wb.construction();
            let points: Vec<_> = c.ids().filter(|&id| c.kind(id) == Kind::Point).collect();
            for (k, &id) in points.iter().enumerate() {
                wb.move_point(id, 0.1 * k as f64 + 1.0 / 3.0, -2.0 / 7.0);
            }
            if let Some(&last) = points.last() {
                wb.toggle_visible(last);
            }
            assert_round_trip(&wb, name);
        }
    }

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
        let mut loaded = 0;
        for path in &files {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let mut wb = Workbench::default();
            if wb.load_bytes(&std::fs::read(path).unwrap(), &name).is_ok() {
                assert_round_trip(&wb, &name);
                loaded += 1;
            }
        }
        assert!(loaded * 2 > files.len(), "{loaded} of {}", files.len());
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
        /// A coordinate that is rarely a short decimal.
        fn coord(&mut self) -> f64 {
            (self.below(4001) as f64 - 2000.0) / 7.0
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

    /// Builds a construction from random edits, the way the window makes
    /// them: definitions, moves, new values, hiding, deleting, undo and redo.
    fn generated(seed: u64) -> Workbench {
        let mut rng = Rng(seed);
        let mut wb = Workbench::default();
        for step in 0..40 {
            let points = of_kind(&wb, &[Kind::Point]);
            let curves = of_kind(&wb, &[Kind::Line, Kind::Segment, Kind::Circle]);
            let pick = |rng: &mut Rng, from: &[String]| from[rng.below(from.len())].clone();
            let line = match rng.below(12) {
                0..=2 => Some(format!("P{step} = ({}, {})", rng.coord(), rng.coord())),
                3 => Some(format!("n{step} = {}", rng.coord())),
                4..=6 if points.len() >= 2 => {
                    let (a, b) = (pick(&mut rng, &points), pick(&mut rng, &points));
                    let cmd = ["Segment", "Line", "Circle", "Midpoint"][rng.below(4)];
                    let label = if cmd == "Midpoint" { "M" } else { "f" };
                    Some(format!("{label}{step} = {cmd}({a}, {b})"))
                }
                7 if curves.len() >= 2 => {
                    let (a, b) = (pick(&mut rng, &curves), pick(&mut rng, &curves));
                    Some(format!("I{step} = Intersect({a}, {b})"))
                }
                _ => None,
            };
            if let Some(line) = line {
                // Failing lines (parallel lines, a point with itself) are fine.
                let _ = wb.run_line(&line);
                continue;
            }
            match rng.below(6) {
                0 | 1 if !points.is_empty() => {
                    let id = wb.construction().id(&pick(&mut rng, &points)).unwrap();
                    wb.begin_gesture();
                    for _ in 0..rng.below(3) + 1 {
                        wb.move_point(id, rng.coord(), rng.coord());
                    }
                    wb.end_gesture();
                }
                2 => {
                    let numbers = of_kind(&wb, &[Kind::Number]);
                    if !numbers.is_empty() {
                        let id = wb.construction().id(&pick(&mut rng, &numbers)).unwrap();
                        wb.set_number(id, rng.coord());
                    }
                }
                3 if !wb.construction().is_empty() => {
                    let id = wb
                        .construction()
                        .ids()
                        .nth(rng.below(wb.construction().len()));
                    match rng.below(2) {
                        0 => wb.toggle_visible(id.unwrap()),
                        _ => {
                            wb.delete(id.unwrap());
                        }
                    }
                }
                4 => {
                    wb.undo();
                }
                _ => {
                    wb.redo();
                }
            }
        }
        wb
    }

    #[test]
    fn generated_constructions_round_trip() {
        let mut sizes = 0;
        let mut seen = [0; 4];
        for seed in 1..=200u64 {
            let wb = generated(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15));
            sizes += wb.construction().len();
            let text = wb.snapshot_text();
            for (n, what) in ["SetCoords(", "SetValue(", "SetVisibleInView(", "Intersect("]
                .iter()
                .enumerate()
            {
                seen[n] += usize::from(text.contains(what));
            }
            assert_round_trip(&wb, &format!("seed {seed}"));
        }
        // The generator makes real constructions with every kind of entry.
        assert!(sizes > 200 * 5, "{sizes}");
        assert!(seen.iter().all(|&n| n >= 10), "{seen:?}");
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
            Some("Pominięte linie zapisu: 1 (nie dały się wczytać), zależne od nich: 2")
        );
        // A user-opened file with the same lines fails at the first bad one.
        let err = wb
            .load_bytes(b"A = (0, 0)\nQ = Nope(A)\n", "x.geo")
            .unwrap_err();
        assert!(err.starts_with("x.geo: line 2:"), "{err}");
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
        assert_round_trip(&wb, "imported");
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
            "Pominięte obiekty: 1 (jądro ich nie obsługuje), zależne od nich: 2, \
             wartości różne od zapisanych w pliku: 1, wczytane linie: 7. Uwagi: continuity"
        );
    }
}
