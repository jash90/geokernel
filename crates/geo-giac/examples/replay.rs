//! Replays recorded Giac exchanges and reports the ones whose output differs.
//!
//! Usage: `replay <file>`. The input is a JSONL fixture from
//! `tools/giac/replay-fixture.py` or the original delimited replay format.
//! On Android, pass the native library directory as the second argument.

use std::process::ExitCode;

const EXCHANGE_COUNT: usize = 1_966;
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
    let exchanges = parse(&text);
    compare(&exchanges, trace, |input| {
        let seed = geo_giac::session::seed(input, 1);
        geo_giac::eval_raw(input, geo_giac::session::DEFAULT_TIMEOUT_MS, seed)
    })
}

fn parse(text: &str) -> Vec<Exchange> {
    if text.starts_with('{') || text.starts_with('[') {
        let mut lines = text.lines();
        if text.starts_with('{') {
            let header: serde_json::Value =
                serde_json::from_str(lines.next().unwrap()).expect("valid provenance header");
            assert_eq!(header["type"], "provenance", "expected a provenance header");
        }
        lines
            .map(|line| serde_json::from_str(line).expect("case, input and expected output"))
            .collect()
    } else {
        text.split('\x1e')
            .filter(|record| !record.is_empty())
            .map(|record| {
                let mut parts = record.split('\x1f');
                std::array::from_fn(|_| parts.next().expect("complete exchange").to_owned())
            })
            .collect()
    }
}

fn compare<E: std::fmt::Debug>(
    exchanges: &[Exchange],
    trace: bool,
    mut evaluate: impl FnMut(&str) -> Result<String, E>,
) -> ExitCode {
    if exchanges.len() != EXCHANGE_COUNT {
        eprintln!(
            "expected {EXCHANGE_COUNT} exchanges, got {}",
            exchanges.len()
        );
        return ExitCode::FAILURE;
    }
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

    #[test]
    fn changed_output_fails_replay() {
        let mut exchanges =
            vec![["case".into(), "input".into(), "\"output\"".into()]; EXCHANGE_COUNT];
        let evaluate = |_: &str| Ok::<_, ()>("output".to_owned());
        assert_eq!(compare(&exchanges, false, evaluate), ExitCode::SUCCESS);
        exchanges[0][2] = "changed".into();
        assert_eq!(compare(&exchanges, false, evaluate), ExitCode::FAILURE);
    }

    #[test]
    fn missing_or_extra_exchange_fails_before_evaluation() {
        let mut exchanges = vec![["case".into(), "input".into(), "output".into()]; EXCHANGE_COUNT];
        let evaluate =
            |_: &str| -> Result<String, ()> { panic!("evaluated an incomplete fixture") };
        exchanges.pop();
        assert_eq!(compare(&exchanges, false, evaluate), ExitCode::FAILURE);
        exchanges.extend([exchanges[0].clone(), exchanges[0].clone()]);
        assert_eq!(compare(&exchanges, false, evaluate), ExitCode::FAILURE);
    }
}
