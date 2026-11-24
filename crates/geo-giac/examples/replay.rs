//! Replays recorded Giac exchanges and reports the ones whose output differs.
//!
//! Usage: `replay <file>`. The input is a JSONL fixture from
//! `tools/giac/replay-fixture.py` or the original delimited replay format.
//! On Android, pass the native library directory as the second argument.

fn main() {
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
    let exchanges: Vec<[String; 3]> = if text.starts_with('[') {
        text.lines()
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
    };
    let (mut same, mut differ) = (0, 0);
    for [case, input, expected] in &exchanges {
        // The recording is taken before evalRaw removes surrounding quotes.
        let expected = match expected.strip_prefix('"').and_then(|e| e.strip_suffix('"')) {
            Some(inner) => inner,
            None => expected.as_str(),
        };
        if trace {
            eprintln!("{case}: {input}");
        }
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
    if same == 0 || differ != 0 {
        std::process::exit(1);
    }
}
