//! Replays recorded Giac exchanges and reports the ones whose output differs.
//!
//! Input: a file of records `case \x1f input \x1f expected output \x1e`, where
//! the input is the string the reference passed to `evalRaw` (before its
//! `evalfa` wrapping). Usage: `replay <file>`; needs GEO_GIAC_LIBRARY.

fn main() {
    let path = std::env::args().nth(1).expect("usage: replay <file>");
    let text = std::fs::read_to_string(path).expect("readable file");
    let (mut same, mut differ) = (0, 0);
    for record in text.split('\x1e').filter(|r| !r.is_empty()) {
        let mut parts = record.split('\x1f');
        let (case, input, expected) = (
            parts.next().unwrap(),
            parts.next().unwrap(),
            parts.next().unwrap(),
        );
        // The recording is taken before evalRaw removes surrounding quotes.
        let expected = match expected.strip_prefix('"').and_then(|e| e.strip_suffix('"')) {
            Some(inner) => inner,
            None => expected,
        };
        let seed = geo_giac::session::seed(input, 1);
        match geo_giac::eval_raw(input, geo_giac::session::DEFAULT_TIMEOUT_MS, seed) {
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
}
