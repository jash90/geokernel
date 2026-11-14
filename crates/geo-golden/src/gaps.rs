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
}

/// Why a case is listed and exactly where it disagrees.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownGap {
    pub reason: String,
    /// The disagreeing objects as `step:label`, e.g. `2:Q`.
    pub mismatches: BTreeSet<String>,
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
        let gap = KnownGap {
            reason: g.reason,
            mismatches: g.mismatches.into_iter().collect(),
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
    fn mismatch_list_is_mandatory() {
        let entry = |rest: &str| parse(&format!("[[gap]]\nid = \"a\"\nreason = \"r\"\n{rest}"));
        assert!(entry("").is_err());
        assert!(entry("mismatches = []\n").is_err());
    }
}
