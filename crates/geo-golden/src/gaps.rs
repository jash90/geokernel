//! The list of known gaps (`known-gaps.toml`).

use serde::Deserialize;
use std::collections::BTreeMap;

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
}

/// Case id to reason.
pub type Gaps = BTreeMap<String, String>;

pub fn parse(text: &str) -> Result<Gaps, String> {
    let file: GapFile = toml::from_str(text).map_err(|e| format!("known-gaps.toml: {e}"))?;
    let mut gaps = Gaps::new();
    for g in file.gap {
        if g.reason.trim().is_empty() {
            return Err(format!("known-gaps.toml: `{}` has an empty reason", g.id));
        }
        if gaps.insert(g.id.clone(), g.reason).is_some() {
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
        assert!(parse("[[gap]]\nid = \"a\"\nreason = \"  \"\n").is_err());
        assert!(parse("[[gap]]\nid = \"a\"\n").is_err());
        assert!(parse("[[gap]]\nid = \"a\"\nreason = \"r\"\n").is_ok());
        assert!(parse("").unwrap().is_empty());
    }
}
