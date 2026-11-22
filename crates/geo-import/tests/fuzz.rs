//! Fuzzing of the importer in the manner of `geo-kernel/tests/fuzz_script.rs`:
//! mutated construction files must never panic or hang, an import that
//! succeeds must give lines that run on the kernel, and no line may span more
//! than one line of text. Inputs come from fixed seeds and the files GeoGebra
//! wrote for the golden cases.

use geo_golden::repo_root;
use geo_import::import;
use geo_kernel::script::{exec, saved::inline_argument_positions, split_comment};
use geo_kernel::Construction;
use std::fs;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const MIN_INPUTS: usize = 20;
const MUTATIONS: usize = 80;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

const SNIPPETS: &[&str] = &[
    "&",
    "&amp;",
    "&#0;",
    "<",
    "</element>",
    "\"",
    "'",
    "NaN",
    "Infinity",
    "1e999",
    "-0",
    "{",
    "}",
    "_{",
    "a0=\"\"",
    "a7=\"x\"",
    " z=\"0\"",
    "\n",
    "#",
    "=",
    "(",
    ")",
    ",",
    "\u{0}",
    "\u{feff}",
    "<command name=\"Tangent\"><input a0=\"A\" a1=\"B\"/><output a0=\"t\" a1=\"t\"/></command>",
    "<element type=\"point\" label=\"A\"><coords x=\"1\" y=\"1\" z=\"1\"/></element>",
];

fn mutate(input: &[u8], rng: &mut Rng) -> Vec<u8> {
    let mut b = input.to_vec();
    for _ in 0..1 + rng.below(4) {
        if b.is_empty() {
            break;
        }
        let at = rng.below(b.len());
        match rng.below(6) {
            0 => b[at] ^= 1 << rng.below(8),
            1 => {
                let end = (at + 1 + rng.below(40)).min(b.len());
                b.drain(at..end);
            }
            2 => {
                let end = (at + 1 + rng.below(200)).min(b.len());
                let copy = b[at..end].to_vec();
                let to = rng.below(b.len());
                b.splice(to..to, copy);
            }
            3 => {
                let s = SNIPPETS[rng.below(SNIPPETS.len())].as_bytes().to_vec();
                b.splice(at..at, s);
            }
            4 => b.truncate(at),
            _ => {
                // replace a digit by another character
                if let Some(p) = b[at..].iter().position(|c| c.is_ascii_digit()) {
                    b[at + p] = b"x-.e9"[rng.below(5)];
                }
            }
        }
    }
    b
}

fn collect(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect(&path, ext, out);
        } else if path.extension().is_some_and(|e| e == ext) {
            out.push(path);
        }
    }
}

fn check(name: &str, bytes: &[u8]) -> bool {
    let started = Instant::now();
    let result = catch_unwind(AssertUnwindSafe(|| import(bytes)))
        .unwrap_or_else(|_| panic!("{name}: the importer panicked on {} bytes", bytes.len()));
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "{name}: took {:?}",
        started.elapsed()
    );
    let reached = result.is_ok();
    if let Ok(imported) = result {
        let mut c = Construction::new();
        for line in &imported.script {
            assert!(
                !line.contains(['\n', '\r'])
                    && split_comment(line)
                        .1
                        .is_none_or(|comment| inline_argument_positions(comment).is_some()),
                "{name}: a line spans several lines or has a comment: {line:?}"
            );
            if let Err(e) = exec(&mut c, line) {
                panic!("{name}: the imported line {line:?} fails: {e}");
            }
        }
        for label in &imported.hidden {
            assert!(
                c.id(label).is_some(),
                "{name}: hidden `{label}` does not exist"
            );
        }
    }
    reached
}

/// The XML of a `.ggb` archive, mutated, and packed again so that the zip
/// layer accepts it and the mutation reaches the converter.
fn rezip(xml: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut out = Vec::new();
    {
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(&mut out));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        w.start_file("geogebra.xml", options).unwrap();
        w.write_all(xml).unwrap();
        w.finish().unwrap();
    }
    out
}

/// Structure-aware mutation: swaps the values of two attributes of the same
/// name (labels, inputs, outputs), or repeats a tag, so that references go
/// to the wrong or to missing objects while the XML stays well formed.
fn mutate_structure(input: &str, rng: &mut Rng) -> String {
    let mut text = input.to_string();
    for _ in 0..1 + rng.below(3) {
        let attr = ["label=\"", "a0=\"", "a1=\"", "name=\"", "type=\""][rng.below(5)];
        let positions: Vec<usize> = text.match_indices(attr).map(|(i, _)| i).collect();
        if positions.len() < 2 {
            continue;
        }
        let value = |t: &str, at: usize| {
            let start = at + attr.len();
            t[start..].find('"').map(|n| (start, start + n))
        };
        let (p, q) = (
            positions[rng.below(positions.len())],
            positions[rng.below(positions.len())],
        );
        if let (Some((ps, pe)), Some((qs, qe))) = (value(&text, p), value(&text, q)) {
            if ps < qs {
                let (a, b) = (text[ps..pe].to_string(), text[qs..qe].to_string());
                text.replace_range(qs..qe, &a);
                text.replace_range(ps..pe, &b);
            }
        }
    }
    text
}

#[test]
fn mutated_files_never_panic() {
    let root = repo_root().join("golden/ggb");
    let mut xml = Vec::new();
    collect(&root, "xml", &mut xml);
    xml.sort();
    let mut zips = Vec::new();
    collect(&root, "ggb", &mut zips);
    zips.sort();
    let inputs: Vec<PathBuf> = xml
        .iter()
        .step_by(30)
        .chain(zips.iter().step_by(4))
        .cloned()
        .collect();
    assert!(inputs.len() >= MIN_INPUTS, "only {} inputs", inputs.len());
    let mut runs = 0;
    for (n, path) in inputs.iter().enumerate() {
        let original = fs::read(path).unwrap();
        check(&path.display().to_string(), &original);
        let mut rng = Rng(0x9e37_79b9_7f4a_7c15 ^ ((n as u64 + 1) * 0x1000_0000_01b3));
        for m in 0..MUTATIONS {
            let mutated = mutate(&original, &mut rng);
            check(&format!("{} mutation {m}", path.display()), &mutated);
            runs += 1;
        }
    }
    println!("{} inputs, {runs} mutated files", inputs.len());
}

#[test]
fn mutations_inside_a_valid_archive_reach_the_converter() {
    let root = repo_root().join("golden/ggb");
    let mut xml = Vec::new();
    collect(&root, "xml", &mut xml);
    xml.sort();
    let (mut runs, mut reached) = (0, 0);
    for (n, path) in xml.iter().step_by(40).enumerate() {
        let original = fs::read_to_string(path).unwrap();
        let mut rng = Rng(0x2545_f491_4f6c_dd1d ^ ((n as u64 + 1) * 0x1000_0000_01b3));
        for m in 0..40 {
            let text = if m % 2 == 0 {
                mutate_structure(&original, &mut rng)
            } else {
                String::from_utf8_lossy(&mutate(original.as_bytes(), &mut rng)).into_owned()
            };
            let name = format!("{} archive mutation {m}", path.display());
            // as a bare file and inside an archive
            reached += usize::from(check(&name, text.as_bytes()));
            reached += usize::from(check(&name, &rezip(text.as_bytes())));
            runs += 2;
        }
    }
    println!("{runs} mutated files, {reached} imported");
    // most mutations leave a file the converter can read
    assert!(
        reached * 2 > runs,
        "only {reached} of {runs} reached the converter"
    );
}
