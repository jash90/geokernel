//! Command-line arguments and script file loading.

use crate::workbench::Workbench;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

pub const USAGE: &str = "usage: geokernel-app [--touch] [script.geo | file.ggb]\n  --touch  larger hit areas and buttons, as on a touch screen";

/// What the command line asked for.
#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    /// Open the window, optionally loading a script.
    Run {
        script: Option<PathBuf>,
        /// Use the touch profile (larger targets) instead of the mouse one.
        touch: bool,
    },
    Help,
}

/// Parses the arguments after the program name. A path need not be valid
/// Unicode.
pub fn parse_args<I: IntoIterator<Item = OsString>>(args: I) -> Result<Command, String> {
    let mut script = None;
    let mut touch = false;
    for arg in args {
        match arg.to_str() {
            Some("-h" | "--help") => return Ok(Command::Help),
            Some("--touch") => touch = true,
            Some(a) if a.starts_with('-') && a.len() > 1 => {
                return Err(format!("unknown option `{a}`"));
            }
            _ if script.is_some() => return Err("expected at most one script path".into()),
            _ => script = Some(PathBuf::from(arg)),
        }
    }
    Ok(Command::Run { script, touch })
}

fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
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

/// Reads a script file, or a construction file saved by GeoGebra, into the
/// workbench. A construction file is recognised by its content (a zip archive,
/// or XML whose root is `geogebra`), not by its name. Errors name the file.
pub fn load_into(wb: &mut Workbench, path: &Path) -> Result<(), String> {
    let name = file_name(path);
    let bytes = std::fs::read(path).map_err(|e| format!("{name}: {e}"))?;
    if geo_import::looks_like_construction(&bytes) {
        let imported = geo_import::import(&bytes).map_err(|e| format!("{name}: {e}"))?;
        let notice = import_notice(&imported);
        return wb
            .load_imported(&imported.script.join("\n"), &imported.hidden, notice)
            .map_err(|e| format!("{name}: {e}"));
    }
    let src = String::from_utf8(bytes).map_err(|e| format!("{name}: {e}"))?;
    wb.load_script(&src).map_err(|e| format!("{name}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Command, String> {
        parse_args(args.iter().map(OsString::from))
    }

    #[test]
    fn no_arguments_means_the_built_in_example() {
        assert_eq!(
            parse(&[]),
            Ok(Command::Run {
                script: None,
                touch: false
            })
        );
    }

    #[test]
    fn one_path_is_taken() {
        assert_eq!(
            parse(&["a.geo"]),
            Ok(Command::Run {
                script: Some(PathBuf::from("a.geo")),
                touch: false
            })
        );
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_path_is_accepted() {
        use std::os::unix::ffi::OsStringExt;
        let raw = OsString::from_vec(vec![b'a', 0xff, b'.', b'g', b'e', b'o']);
        assert_eq!(
            parse_args([raw.clone()]),
            Ok(Command::Run {
                script: Some(PathBuf::from(raw)),
                touch: false
            })
        );
    }

    #[test]
    fn the_touch_flag_goes_with_or_without_a_path() {
        assert_eq!(
            parse(&["--touch", "a.geo"]),
            Ok(Command::Run {
                script: Some(PathBuf::from("a.geo")),
                touch: true
            })
        );
        assert_eq!(
            parse(&["--touch"]),
            Ok(Command::Run {
                script: None,
                touch: true
            })
        );
    }

    #[test]
    fn help_flags() {
        assert_eq!(parse(&["--help"]), Ok(Command::Help));
        assert_eq!(parse(&["a.geo", "-h"]), Ok(Command::Help));
    }

    #[test]
    fn extra_paths_and_unknown_options_are_errors() {
        assert!(parse(&["a.geo", "b.geo"]).is_err());
        assert!(parse(&["--nope"]).unwrap_err().contains("--nope"));
    }

    #[test]
    fn load_into_names_the_file_on_failure() {
        let mut wb = Workbench::default();
        let err = load_into(&mut wb, Path::new("definitely/missing/none.geo")).unwrap_err();
        assert!(err.starts_with("none.geo: "), "{err}");
    }

    #[test]
    fn load_into_reads_a_construction_file() {
        let dir = std::env::temp_dir().join(format!("geo-app-ggb-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let xml = "<geogebra><construction>\
            <element type=\"point\" label=\"A\"><show object=\"false\"/><coords x=\"1\" y=\"2\" z=\"1\"/></element>\
            <element type=\"point\" label=\"B\"><coords x=\"3\" y=\"4\" z=\"1\"/></element>\
            <element type=\"text\" label=\"T\"/>\
            </construction></geogebra>";
        let path = dir.join("a.xml");
        std::fs::write(&path, xml).unwrap();
        let mut wb = Workbench::default();
        load_into(&mut wb, &path).unwrap();
        assert_eq!(wb.construction().len(), 2);
        let a = wb.construction().id("A").unwrap();
        let b = wb.construction().id("B").unwrap();
        assert!(!wb.is_visible(a) && wb.is_visible(b));
        assert!(wb
            .notice
            .as_deref()
            .unwrap()
            .contains("Pominięte obiekty: 1 (jądro ich nie obsługuje)"));
        // a damaged file leaves the document alone and names the file
        std::fs::write(&path, "<geogebra>").unwrap();
        let err = load_into(&mut wb, &path).unwrap_err();
        assert!(err.starts_with("a.xml: "), "{err}");
        assert_eq!(wb.construction().len(), 2);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_script_named_xml_is_still_a_script_and_a_construction_needs_no_extension() {
        let dir = std::env::temp_dir().join(format!("geo-app-kind-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut wb = Workbench::default();
        let script = dir.join("x.xml");
        std::fs::write(&script, "A = (1, 2)\nB = (3, 4)\n").unwrap();
        load_into(&mut wb, &script).unwrap();
        assert_eq!(wb.construction().len(), 2);
        let file = dir.join("saved.dat");
        std::fs::write(
            &file,
            "<geogebra><construction><element type=\"point\" label=\"P\">\
             <coords x=\"1\" y=\"1\" z=\"1\"/></element></construction></geogebra>",
        )
        .unwrap();
        load_into(&mut wb, &file).unwrap();
        assert!(wb.construction().id("P").is_some());
        std::fs::remove_dir_all(&dir).ok();
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

    #[test]
    fn load_into_reads_a_script() {
        let dir = std::env::temp_dir().join(format!("geo-app-cli-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut wb = Workbench::default();
        let ok = dir.join("ok.geo");
        std::fs::write(&ok, "A = (1, 2)\nB = (3, 4)\n").unwrap();
        load_into(&mut wb, &ok).unwrap();
        assert_eq!(wb.construction().len(), 2);
        let bad = dir.join("bad.geo");
        std::fs::write(&bad, "A = (1, 2)\nQ = Nope(A)\n").unwrap();
        let err = load_into(&mut wb, &bad).unwrap_err();
        assert!(err.starts_with("bad.geo: line 2:"), "{err}");
        // A failed load leaves the previous document alone.
        assert_eq!(wb.construction().len(), 2);
        std::fs::remove_dir_all(&dir).ok();
    }
}
