//! The list of known gaps (`known-gaps.toml`).

use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GapFile {
    #[serde(default)]
    gap: Vec<Gap>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Gap {
    id: String,
    reason: String,
    mismatches: Vec<String>,
    /// Every listed mismatch is the kernel answering undefined.
    #[serde(default)]
    undefined: bool,
    /// Every listed mismatch is the kernel refusing a line with this error.
    failure: Option<String>,
}

/// Why a case is listed and exactly where it disagrees.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownGap {
    pub reason: String,
    /// The disagreeing objects as `step:label`, e.g. `2:Q`.
    pub mismatches: BTreeSet<String>,
    /// What the kernel answers at every listed mismatch, when the entry
    /// states it.
    pub kernel: Option<KernelAnswer>,
}

/// The kernel's side of a known gap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KernelAnswer {
    /// The kernel's value is undefined.
    Undefined,
    /// The kernel refuses the line with exactly this error.
    Failure(String),
}

impl KernelAnswer {
    /// Whether a mismatch's description of the kernel's side matches.
    pub fn matches(&self, actual: &str) -> bool {
        match self {
            Self::Undefined => actual.starts_with("Undefined "),
            Self::Failure(error) => actual.ends_with(&format!("`: {error}")),
        }
    }
}

/// Case id to its known gap.
pub type Gaps = BTreeMap<String, KnownGap>;

pub fn parse(text: &str) -> Result<Gaps, String> {
    let file: GapFile = toml::from_str(text).map_err(|e| format!("known-gaps.toml: {e}"))?;
    let mut gaps = Gaps::new();
    for g in file.gap {
        if g.reason.trim().is_empty() {
            return Err(format!("known-gaps.toml: `{}` has an empty reason", g.id));
        }
        if g.mismatches.is_empty() {
            return Err(format!("known-gaps.toml: `{}` lists no mismatches", g.id));
        }
        let kernel = match (g.undefined, g.failure) {
            (false, None) => None,
            (true, None) => Some(KernelAnswer::Undefined),
            (false, Some(error)) => Some(KernelAnswer::Failure(error)),
            (true, Some(_)) => {
                return Err(format!(
                    "known-gaps.toml: `{}` states both `undefined` and `failure`",
                    g.id
                ))
            }
        };
        let gap = KnownGap {
            reason: g.reason,
            mismatches: g.mismatches.into_iter().collect(),
            kernel,
        };
        if gaps.insert(g.id.clone(), gap).is_some() {
            return Err(format!("known-gaps.toml: `{}` is listed twice", g.id));
        }
    }
    Ok(gaps)
}

pub fn load() -> Result<Gaps, String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("known-gaps.toml");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    parse(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reason_is_mandatory() {
        let entry = |rest: &str| parse(&format!("[[gap]]\nid = \"a\"\n{rest}"));
        assert!(entry("reason = \"  \"\nmismatches = [\"0:P\"]\n").is_err());
        assert!(entry("mismatches = [\"0:P\"]\n").is_err());
        assert!(entry("reason = \"r\"\nmismatches = [\"0:P\"]\n").is_ok());
        assert!(parse("").unwrap().is_empty());
    }

    #[test]
    fn the_kernel_answer_is_one_statement() {
        let entry = |rest: &str| {
            parse(&format!(
                "[[gap]]\nid = \"a\"\nreason = \"r\"\nmismatches = [\"0:P\"]\n{rest}"
            ))
        };
        assert_eq!(entry("").unwrap()["a"].kernel, None);
        assert_eq!(
            entry("undefined = true\n").unwrap()["a"].kernel,
            Some(KernelAnswer::Undefined)
        );
        assert_eq!(
            entry("failure = \"e\"\n").unwrap()["a"].kernel,
            Some(KernelAnswer::Failure("e".into()))
        );
        assert!(entry("undefined = true\nfailure = \"e\"\n").is_err());
    }

    #[test]
    fn mismatch_list_is_mandatory() {
        let entry = |rest: &str| parse(&format!("[[gap]]\nid = \"a\"\nreason = \"r\"\n{rest}"));
        assert!(entry("").is_err());
        assert!(entry("mismatches = []\n").is_err());
    }
}
