//! Replays recorded Giac exchanges and reports the ones whose output differs.
//!
//! Usage: `replay <file>`. The input is a JSONL fixture from
//! `tools/giac/replay-fixture.py` with provenance for the current target.
//! On Android, pass the native library directory as the second argument.

use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, process::ExitCode};

const EXCHANGE_COUNT: usize = 1_966;
// All three platform recordings share these (case ID, input) pairs. Pin
// their sorted, length-prefixed bytes independently of the fixture under test.
const EXCHANGE_SET_SHA256: &str =
    "12b6cf6eaae8304696399fad3ee5b772a73eaff2b2b35162148c24201c7bcf4f";
type Exchange = [String; 3];

fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1);
    let path = args
        .next()
        .expect("usage: replay <file> [android-library-dir]");
    if let Some(directory) = args.next() {
        #[cfg(target_os = "android")]
        geo_giac::initialize_from_android_dir(std::path::Path::new(&directory))
            .expect("load Giac from the native library directory");
        #[cfg(not(target_os = "android"))]
        panic!("a native library directory is only accepted on Android: {directory:?}");
    }
    let trace = std::env::var_os("GEO_GIAC_REPLAY_TRACE").is_some();
    let text = std::fs::read_to_string(path).expect("readable file");
    let Some(pinned_sha) = geo_giac::pinned_library_sha256() else {
        eprintln!("no Giac library is pinned for this target");
        return ExitCode::FAILURE;
    };
    replay(&text, pinned_sha, trace, |input| {
        let seed = geo_giac::session::seed(input, 1);
        geo_giac::eval_raw(input, geo_giac::session::DEFAULT_TIMEOUT_MS, seed)
    })
}

fn parse(text: &str, pinned_sha: &str) -> Result<Vec<Exchange>, String> {
    let mut lines = text.lines();
    let header: serde_json::Value = serde_json::from_str(lines.next().unwrap_or_default())
        .map_err(|e| format!("invalid provenance header: {e}"))?;
    if header["type"] != "provenance" {
        return Err("expected a provenance header".into());
    }
    if header["library_sha256"].as_str() != Some(pinned_sha) {
        return Err(format!(
            "expected the pinned Giac library SHA-256 {pinned_sha}"
        ));
    }

    let mut exchanges = Vec::new();
    let mut identities = BTreeSet::new();
    for (index, line) in lines.enumerate() {
        if index == EXCHANGE_COUNT {
            return Err(format!(
                "expected {EXCHANGE_COUNT} exchanges, got an extra row"
            ));
        }
        let exchange: Exchange = serde_json::from_str(line)
            .map_err(|e| format!("invalid exchange on line {}: {e}", index + 2))?;
        let [case, input, _] = &exchange;
        if !identities.insert((case.clone(), input.clone())) {
            return Err(format!(
                "duplicate exchange for {case} on line {}",
                index + 2
            ));
        }
        exchanges.push(exchange);
    }
    if exchanges.len() != EXCHANGE_COUNT {
        return Err(format!(
            "expected {EXCHANGE_COUNT} exchanges, got {}",
            exchanges.len()
        ));
    }

    let mut digest = Sha256::new();
    for (case, input) in identities {
        for field in [case, input] {
            digest.update((field.len() as u64).to_be_bytes());
            digest.update(field.as_bytes());
        }
    }
    if format!("{:x}", digest.finalize()) != EXCHANGE_SET_SHA256 {
        return Err("case IDs and inputs differ from the recorded exchange set".into());
    }
    Ok(exchanges)
}

fn replay<E: std::fmt::Debug>(
    text: &str,
    pinned_sha: &str,
    trace: bool,
    evaluate: impl FnMut(&str) -> Result<String, E>,
) -> ExitCode {
    match parse(text, pinned_sha) {
        Ok(exchanges) => compare(&exchanges, trace, evaluate),
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn compare<E: std::fmt::Debug>(
    exchanges: &[Exchange],
    trace: bool,
    mut evaluate: impl FnMut(&str) -> Result<String, E>,
) -> ExitCode {
    let (mut same, mut differ) = (0, 0);
    for [case, input, expected] in exchanges {
        // The recording is taken before evalRaw removes surrounding quotes.
        let expected = match expected.strip_prefix('"').and_then(|e| e.strip_suffix('"')) {
            Some(inner) => inner,
            None => expected.as_str(),
        };
        if trace {
            eprintln!("{case}: {input}");
        }
        match evaluate(input) {
            Ok(got) if got == expected => same += 1,
            got => {
                differ += 1;
                println!(
                    "{case}\n  input:    {input}\n  expected: {expected}\n  got:      {got:?}"
                );
            }
        }
    }
    println!("{same} same, {differ} different");
    if differ == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../../../tools/giac/cas-exchanges.jsonl");
    const LINUX_SHA: &str = "d4c7f38357f06cc6a364d551ac3c375568b703c3abc4da76674e9642ce0316ec";

    fn assert_refused(text: &str) {
        let evaluate = |_: &str| -> Result<String, ()> { panic!("evaluated a refused fixture") };
        assert_eq!(replay(text, LINUX_SHA, false, evaluate), ExitCode::FAILURE);
    }

    #[test]
    fn recorded_exchanges_can_be_reordered() {
        let mut lines: Vec<_> = FIXTURE.lines().collect();
        lines[1..].reverse();
        assert_eq!(
            parse(&lines.join("\n"), LINUX_SHA).unwrap().len(),
            EXCHANGE_COUNT
        );
    }

    #[test]
    fn changed_output_fails_replay() {
        let mut exchanges = [["case".into(), "input".into(), "\"output\"".into()]];
        let evaluate = |_: &str| Ok::<_, ()>("output".to_owned());
        assert_eq!(compare(&exchanges, false, evaluate), ExitCode::SUCCESS);
        exchanges[0][2] = "changed".into();
        assert_eq!(compare(&exchanges, false, evaluate), ExitCode::FAILURE);
    }

    #[test]
    fn duplicate_exchange_fails_before_evaluation() {
        let mut lines: Vec<_> = FIXTURE.lines().collect();
        lines[1] = lines[2];
        assert_refused(&lines.join("\n"));
    }

    #[test]
    fn missing_exchange_fails_before_evaluation() {
        let mut lines: Vec<_> = FIXTURE.lines().collect();
        lines.pop();
        assert_refused(&lines.join("\n"));
    }

    #[test]
    fn extra_exchange_fails_before_evaluation() {
        let extra = r#"["unexpected-case","unexpected-input","output"]"#;
        assert_refused(&format!("{FIXTURE}{extra}\n"));
    }

    #[test]
    fn replaced_case_id_fails_before_evaluation() {
        assert_refused(&FIXTURE.replacen("cas-expand-001", "unexpected-case", 1));
    }

    #[test]
    fn wrong_library_hash_fails_before_evaluation() {
        assert_refused(&FIXTURE.replacen(LINUX_SHA, &"0".repeat(64), 1));
    }

    #[test]
    fn missing_header_fails_before_evaluation() {
        assert_refused(FIXTURE.split_once('\n').unwrap().1);
    }
}
