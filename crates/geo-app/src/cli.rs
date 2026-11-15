//! Command-line arguments and script file loading.

use geo_kernel::kernel::Construction;
use std::path::{Path, PathBuf};

pub const USAGE: &str = "usage: geokernel-app [script.geo]";

/// What the command line asked for.
#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    /// Open the window, optionally loading a script.
    Run {
        script: Option<PathBuf>,
    },
    Help,
}

/// Parses the arguments after the program name.
pub fn parse_args<I: IntoIterator<Item = String>>(args: I) -> Result<Command, String> {
    let mut script = None;
    for arg in args {
        match arg.as_str() {
            "-h" | "--help" => return Ok(Command::Help),
            a if a.starts_with('-') && a.len() > 1 => {
                return Err(format!("unknown option `{a}`"));
            }
            _ if script.is_some() => return Err("expected at most one script path".into()),
            _ => script = Some(PathBuf::from(arg)),
        }
    }
    Ok(Command::Run { script })
}

/// Builds a construction from script text, with the error as one line.
pub fn load_text(src: &str) -> Result<Construction, String> {
    geo_kernel::script::load(src).map_err(|e| e.to_string())
}

/// Reads and runs a script file. The error names the file.
pub fn load_file(path: &Path) -> Result<Construction, String> {
    let name = path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    let src = std::fs::read_to_string(path).map_err(|e| format!("{name}: {e}"))?;
    load_text(&src).map_err(|e| format!("{name}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Command, String> {
        parse_args(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn no_arguments_means_the_built_in_example() {
        assert_eq!(parse(&[]), Ok(Command::Run { script: None }));
    }

    #[test]
    fn one_path_is_taken() {
        assert_eq!(
            parse(&["a.geo"]),
            Ok(Command::Run {
                script: Some(PathBuf::from("a.geo"))
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
    fn load_text_reports_the_line() {
        let err = load_text("A = (0, 0)\nB = nonsense(")
            .map(|_| ())
            .unwrap_err();
        assert!(err.starts_with("line 2:"), "{err}");
        assert_eq!(load_text("A = (0, 0)").unwrap().len(), 1);
    }

    #[test]
    fn load_file_names_the_file_on_failure() {
        let err = load_file(Path::new("definitely/missing/none.geo"))
            .map(|_| ())
            .unwrap_err();
        assert!(err.starts_with("none.geo: "), "{err}");
    }

    #[test]
    fn load_file_reads_a_script() {
        let dir = std::env::temp_dir().join(format!("geo-app-cli-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let ok = dir.join("ok.geo");
        std::fs::write(&ok, "A = (1, 2)\nB = (3, 4)\n").unwrap();
        assert_eq!(load_file(&ok).unwrap().len(), 2);
        let bad = dir.join("bad.geo");
        std::fs::write(&bad, "A = (1, 2)\nQ = Nope(A)\n").unwrap();
        let err = load_file(&bad).map(|_| ()).unwrap_err();
        assert!(err.starts_with("bad.geo: line 2:"), "{err}");
        std::fs::remove_dir_all(&dir).ok();
    }
}
