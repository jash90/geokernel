//! Hostile input: archives that lie about their sizes, decompression bombs,
//! documents with too many items, and a great many lines that the kernel
//! rejects or whose saved values disagree. Each case is an error or a result
//! within a time bound, never a panic and never an unbounded allocation.
//! Where work must grow linearly with the input, the test counts allocations
//! (a counting global allocator, installed only in this test binary, counts
//! those of the test's own thread) instead of timing it.

use geo_import::archive::{MAX_XML_BYTES, MAX_ZIP_ENTRIES};
use geo_import::xml::MAX_ITEMS;
use geo_import::{import, ImportError};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::io::Write;
use std::time::{Duration, Instant};

thread_local! {
    static ON: Cell<bool> = const { Cell::new(false) };
    static COUNT: Cell<u64> = const { Cell::new(0) };
}

struct Counting;

fn note() {
    // `try_with`: the thread-local storage may be gone while a thread exits.
    if ON.try_with(Cell::get).unwrap_or(false) {
        let _ = COUNT.try_with(|c| c.set(c.get() + 1));
    }
}

// SAFETY: every method forwards to `System` with the same arguments; the
// counters are plain thread-local cells without destructors, so touching
// them cannot allocate or recurse.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        note();
        // SAFETY: same contract as the caller's.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        note();
        // SAFETY: same contract as the caller's.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        note();
        // SAFETY: same contract as the caller's.
        unsafe { System.realloc(ptr, layout, new_size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: same contract as the caller's.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// The result of `f` and the allocations it makes on this thread.
fn allocations<T>(f: impl FnOnce() -> T) -> (T, u64) {
    COUNT.with(|c| c.set(0));
    ON.with(|o| o.set(true));
    let out = f();
    ON.with(|o| o.set(false));
    (out, COUNT.with(Cell::get))
}

const BOUND: Duration = Duration::from_secs(30);

/// The wall-clock bound of the 50,000-item import. A debug build takes about
/// 17 s on a developer machine and 34 to 47 s on loaded hosted macOS and
/// Windows runners (CI runs of 2026-11-23); an importer whose work grew
/// quadratically would take minutes at this size.
const IMPORT_BOUND: Duration = Duration::from_secs(120);

const SMALL: &str = "<geogebra><construction><element type=\"point\" label=\"A\">\
                     <coords x=\"1\" y=\"2\" z=\"1\"/></element></construction></geogebra>";

fn zip_with(entries: &[(&str, &[u8])], method: zip::CompressionMethod) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(&mut out));
        let options = zip::write::SimpleFileOptions::default().compression_method(method);
        for (name, data) in entries {
            w.start_file(*name, options).unwrap();
            w.write_all(data).unwrap();
        }
        w.finish().unwrap();
    }
    out
}

/// Overwrites the little-endian `u32` at `field` bytes after every header
/// that starts with `signature`.
fn patch_u32(bytes: &mut [u8], signature: [u8; 4], field: usize, value: u32) {
    let mut n = 0;
    for i in 0..bytes.len().saturating_sub(field + 4) {
        if bytes[i..i + 4] == signature {
            bytes[i + field..i + field + 4].copy_from_slice(&value.to_le_bytes());
            n += 1;
        }
    }
    assert!(n > 0);
}

const LOCAL: [u8; 4] = *b"PK\x03\x04";
const CENTRAL: [u8; 4] = *b"PK\x01\x02";

fn timed<T>(f: impl FnOnce() -> T) -> (T, Duration) {
    let start = Instant::now();
    let out = f();
    (out, start.elapsed())
}

#[test]
fn a_deflate_bomb_stops_at_the_declared_size() {
    // 8 MiB of zeros deflates to a few KiB; the headers claim 1 KiB.
    let zeros = vec![b' '; 8 << 20];
    let mut zip = zip_with(
        &[("geogebra.xml", &zeros)],
        zip::CompressionMethod::Deflated,
    );
    assert!(zip.len() < 100_000, "the archive is small: {}", zip.len());
    patch_u32(&mut zip, LOCAL, 22, 1024);
    patch_u32(&mut zip, CENTRAL, 24, 1024);
    let (r, took) = timed(|| import(&zip));
    assert!(
        matches!(&r, Err(ImportError::Zip(m)) if m.contains("larger")),
        "{r:?}"
    );
    assert!(took < BOUND);
}

#[test]
fn a_declared_size_over_the_limit_is_refused_without_unpacking() {
    let mut zip = zip_with(
        &[("geogebra.xml", SMALL.as_bytes())],
        zip::CompressionMethod::Deflated,
    );
    patch_u32(&mut zip, CENTRAL, 24, (MAX_XML_BYTES + 1) as u32);
    let (r, took) = timed(|| import(&zip));
    assert_eq!(r, Err(ImportError::TooLarge));
    assert!(took < Duration::from_secs(1));
}

#[test]
fn sizes_that_lie_the_other_way_are_errors() {
    let mut zip = zip_with(
        &[("geogebra.xml", SMALL.as_bytes())],
        zip::CompressionMethod::Deflated,
    );
    // Claims more than the data holds.
    patch_u32(&mut zip, CENTRAL, 24, SMALL.len() as u32 + 100);
    assert!(matches!(import(&zip), Err(ImportError::Zip(_))));
    // A compressed size that runs past the end of the file.
    let mut zip = zip_with(
        &[("geogebra.xml", SMALL.as_bytes())],
        zip::CompressionMethod::Stored,
    );
    patch_u32(&mut zip, CENTRAL, 20, 1 << 30);
    assert!(matches!(import(&zip), Err(ImportError::Zip(_))));
}

#[test]
fn a_zip_with_too_many_entries_is_refused() {
    let names: Vec<String> = (0..=MAX_ZIP_ENTRIES).map(|i| format!("f{i}")).collect();
    let mut entries: Vec<(&str, &[u8])> = names.iter().map(|n| (n.as_str(), &b""[..])).collect();
    entries.push(("geogebra.xml", SMALL.as_bytes()));
    let zip = zip_with(&entries, zip::CompressionMethod::Stored);
    let (r, took) = timed(|| import(&zip));
    assert!(matches!(r, Err(ImportError::TooManyEntries(n)) if n == MAX_ZIP_ENTRIES + 2));
    assert!(took < BOUND);
    // Exactly the limit is fine (the construction is the last entry).
    let mut entries: Vec<(&str, &[u8])> = names
        .iter()
        .take(MAX_ZIP_ENTRIES - 1)
        .map(|n| (n.as_str(), &b""[..]))
        .collect();
    entries.push(("geogebra.xml", SMALL.as_bytes()));
    assert_eq!(entries.len(), MAX_ZIP_ENTRIES);
    let zip = zip_with(&entries, zip::CompressionMethod::Stored);
    assert_eq!(import(&zip).unwrap().script, ["A = (1, 2)"]);
}

#[test]
fn damaged_archives_are_errors() {
    let zip = zip_with(
        &[("geogebra.xml", SMALL.as_bytes())],
        zip::CompressionMethod::Deflated,
    );
    assert_eq!(import(&zip).unwrap().script, ["A = (1, 2)"]);
    // Every truncation and every single flipped byte: an error or an import,
    // never a panic.
    for len in 0..zip.len() {
        let _ = import(&zip[..len]);
    }
    for i in 0..zip.len() {
        let mut copy = zip.clone();
        copy[i] ^= 0xFF;
        let _ = import(&copy);
    }
    // A bad checksum is caught.
    let mut copy = zip.clone();
    patch_u32(&mut copy, CENTRAL, 16, 0x1234_5678);
    assert!(matches!(import(&copy), Err(ImportError::Zip(m)) if m.contains("checksum")));
    // A method the reader does not know.
    let mut copy = zip.clone();
    for i in 0..copy.len() - 12 {
        if copy[i..i + 4] == CENTRAL {
            copy[i + 10] = 12;
        }
    }
    assert!(matches!(import(&copy), Err(ImportError::Zip(m)) if m.contains("method")));
    // An archive without the construction.
    let other = zip_with(
        &[("other.xml", SMALL.as_bytes())],
        zip::CompressionMethod::Stored,
    );
    assert_eq!(import(&other), Err(ImportError::MissingXml));
}

#[test]
fn xml_at_and_over_the_size_limit() {
    let mut big = vec![b' '; MAX_XML_BYTES as usize + 1];
    let head = b"<geogebra><construction/></geogebra>";
    big[..head.len()].copy_from_slice(head);
    let (r, took) = timed(|| import(&big[..MAX_XML_BYTES as usize]));
    assert!(r.unwrap().script.is_empty());
    assert!(took < BOUND, "{took:?}");
    assert_eq!(import(&big), Err(ImportError::TooLarge));
}

fn points(n: usize) -> String {
    let mut s = String::from("<geogebra><construction>");
    for i in 0..n {
        s.push_str(&format!(
            "<element type=\"point\" label=\"P{i}\"><coords x=\"{i}\" y=\"1\" z=\"1\"/></element>"
        ));
    }
    s.push_str("</construction></geogebra>");
    s
}

#[test]
fn more_items_than_the_limit_are_refused() {
    let (r, took) = timed(|| import(points(MAX_ITEMS + 1).as_bytes()));
    assert_eq!(r, Err(ImportError::TooManyItems));
    assert!(took < BOUND, "{took:?}");
}

#[test]
fn tens_of_thousands_of_items_are_imported_in_bounded_time() {
    let xml = points(50_000);
    let (r, took) = timed(|| import(xml.as_bytes()));
    assert_eq!(r.unwrap().script.len(), 50_000);
    assert!(took < IMPORT_BOUND, "{took:?}");
}

/// The bytes of text an import result holds.
fn held(r: &geo_import::Imported) -> usize {
    r.script.iter().map(String::len).sum::<usize>()
        + r.skipped
            .iter()
            .map(|s| s.label.len() + s.kind.len() + s.reason.len())
            .sum::<usize>()
        + r.hidden.iter().map(String::len).sum::<usize>()
        + r.notes.iter().map(String::len).sum::<usize>()
        + r.mismatched
            .iter()
            .map(|m| m.label.len() + m.saved.len() + m.computed.len())
            .sum::<usize>()
}

fn outputs(n: usize) -> String {
    let mut s = String::from("<output");
    for i in 0..n {
        s.push_str(&format!(" a{i}=\"o{i}\""));
    }
    s.push_str("/>");
    s
}

const AB: &str = "<element type=\"point\" label=\"A\"><coords x=\"0\" y=\"0\" z=\"1\"/></element>\
                  <element type=\"point\" label=\"B\"><coords x=\"4\" y=\"0\" z=\"1\"/></element>";

fn wrap(body: &str) -> String {
    format!("<geogebra><construction>{AB}{body}</construction></geogebra>")
}

#[test]
fn a_wide_command_with_a_huge_argument_holds_memory_in_proportion_to_the_input() {
    use geo_import::xml::{MAX_ARGUMENTS, MAX_ATTRIBUTE_BYTES};
    // One argument that is not a label, as long as an attribute may be, and
    // as many output labels as a command may list.
    let arg = "1+".repeat(MAX_ATTRIBUTE_BYTES / 2);
    assert_eq!(arg.len(), MAX_ATTRIBUTE_BYTES);
    for name in ["Line", "Intersect"] {
        let input = format!("<input a0=\"{arg}\" a1=\"B\"/>");
        let xml = wrap(&format!(
            "<command name=\"{name}\">{input}{}</command>",
            outputs(MAX_ARGUMENTS)
        ));
        let (r, took) = timed(|| import(xml.as_bytes()).unwrap());
        assert_eq!(r.skipped.len(), MAX_ARGUMENTS, "{name}");
        assert!(
            held(&r) < 2 * xml.len(),
            "{name}: {} of {}",
            held(&r),
            xml.len()
        );
        assert!(took < BOUND, "{name}: {took:?}");
    }
    // One output over the limit, and one attribute over the limit, are errors.
    let xml = wrap(&format!(
        "<command name=\"Line\"><input a0=\"A\" a1=\"B\"/>{}</command>",
        outputs(MAX_ARGUMENTS + 1)
    ));
    assert!(matches!(
        import(xml.as_bytes()),
        Err(ImportError::TooManyArguments(_))
    ));
    let xml = wrap(&format!(
        "<element type=\"point\" label=\"{}\"/>",
        "x".repeat(MAX_ATTRIBUTE_BYTES + 1)
    ));
    assert_eq!(import(xml.as_bytes()), Err(ImportError::AttributeTooLong));
}

#[test]
fn saved_polygon_region_parameters_share_the_attribute_limit() {
    let padded = " ".repeat(geo_import::xml::MAX_ATTRIBUTE_BYTES);
    let xml = wrap(&format!(
        r#"<element type="point" label="P"><coords x="0" y="0" z="1"
        geokernelRegion="1 2 0.25 0.25{padded}"/></element>"#
    ));
    assert_eq!(import(xml.as_bytes()), Err(ImportError::AttributeTooLong));
}

#[test]
fn many_inputs_and_many_outputs_of_a_single_command() {
    // 10000 label arguments, 10000 outputs; the kernel has no such command,
    // so every output is skipped with one short reason.
    let mut input = String::from("<input");
    for _ in 0..10_000 {
        input.push_str(&format!(" a{}=\"A\"", input.matches(" a").count()));
    }
    input.push_str("/>");
    let xml = wrap(&format!(
        "<command name=\"Frob\">{input}{}</command>",
        outputs(10_000)
    ));
    let (r, took) = timed(|| import(xml.as_bytes()).unwrap());
    assert_eq!(r.skipped.len(), 10_000);
    // each skipped output keeps its label, its kind and one short reason
    assert!(held(&r) < 8 * xml.len(), "{} of {}", held(&r), xml.len());
    assert!(took < BOUND, "{took:?}");
}

#[test]
fn outputs_count_toward_the_item_limit() {
    // 25 commands of 10000 outputs are 250000 outputs: over MAX_ITEMS.
    let mut body = String::new();
    for k in 0..25 {
        let out: String = (0..10_000).map(|i| format!(" a{i}=\"o{k}_{i}\"")).collect();
        body.push_str(&format!(
            "<command name=\"Frob\"><input a0=\"A\"/><output{out}/></command>"
        ));
    }
    assert_eq!(
        import(wrap(&body).as_bytes()),
        Err(ImportError::TooManyItems)
    );
}

#[test]
fn an_element_with_a_million_attributes_is_read_in_linear_time() {
    // The label is the last attribute; a duplicate check on every attribute
    // would be quadratic.
    let mut attrs = String::new();
    for i in 0..300_000 {
        attrs.push_str(&format!(" k{i}=\"v\""));
    }
    let xml = wrap(&format!("<element type=\"point\"{attrs} label=\"P\"/>"));
    let (r, took) = timed(|| import(xml.as_bytes()).unwrap());
    assert_eq!(r.skipped.len(), 1);
    assert!(took < BOUND, "{took:?}");
}

#[test]
fn deep_nesting_is_refused_at_once() {
    // A saved file nests five levels. Wrappers around a tag would otherwise
    // be read as if they were not there, a million deep.
    let depth = 200_000;
    let xml = format!(
        "<geogebra><construction>{}{}</construction></geogebra>",
        "<a>".repeat(depth),
        "</a>".repeat(depth)
    );
    let (r, took) = timed(|| import(xml.as_bytes()));
    assert_eq!(r.unwrap_err(), geo_import::ImportError::TooDeep);
    assert!(took < BOUND, "{took:?}");
    // The limit itself is read.
    let nested = |levels: usize| {
        format!(
            "<geogebra><construction>{}<element type=\"point\" label=\"A\">\
             <coords x=\"1\" y=\"2\" z=\"1\"/></element>{}</construction></geogebra>",
            "<a>".repeat(levels),
            "</a>".repeat(levels)
        )
    };
    let limit = geo_import::xml::MAX_DEPTH;
    // The root, the construction and the element take three levels.
    assert_eq!(
        import(nested(limit - 3).as_bytes()).unwrap().script.len(),
        1
    );
    assert_eq!(
        import(nested(limit - 2).as_bytes()).unwrap_err(),
        geo_import::ImportError::TooDeep
    );
}

#[test]
fn the_style_expressions_of_a_document_share_one_budget() {
    use geo_import::convert::StyleExpressions;
    let c = geo_kernel::script::load("a = 2").unwrap();
    // Each evaluation is charged, and the budget is for all of them.
    let mut expressions = StyleExpressions::with_budget(1_000);
    let mut accepted = 0;
    while expressions.number(&c, "a + 1").is_ok() {
        accepted += 1;
        assert!(accepted < 1_000, "the budget never runs out");
    }
    assert!(accepted > 1);
    assert!(expressions.work() >= 1_000);
    let reason = expressions.number(&c, "a + 1").unwrap_err();
    assert!(reason.contains("evaluation budget"), "{reason}");
    // The same holds for the channels of a dynamic colour.
    let colour = geo_import::export::DynamicColor {
        red: "a".into(),
        green: "a / 2".into(),
        blue: "1 - a".into(),
        alpha: None,
        space: 0,
    };
    assert!(expressions.dynamic_color(&c, &colour).is_err());
    assert!(StyleExpressions::default()
        .dynamic_color(&c, &colour)
        .is_ok());
    // A command the kernel parses without running is no number.
    assert!(StyleExpressions::default()
        .number(&c, "Sum(Sequence(k, k, 1, 1000000))")
        .is_err());
    // An expression without a value right now is still arithmetic.
    assert!(StyleExpressions::default().number(&c, "sqrt(-a)").is_ok());
}

#[test]
fn point_presentation_shares_the_document_expression_budget() {
    use geo_import::convert::StyleExpressions;
    let construction = geo_kernel::script::load("A = (1, 2)\nL = {1, 2}\nLength = 7").unwrap();
    let anchor = "(x(A) + 1, y(A))";
    let increment = "Length[L]";
    let budget = (anchor.len() + increment.len()) as u64;
    let mut expressions = StyleExpressions::with_budget(budget);
    assert!(expressions.point(&construction, anchor).is_ok());
    assert_eq!(
        expressions
            .point_increment(&construction, increment)
            .unwrap(),
        (increment.to_owned(), vec![construction.id("L").unwrap()])
    );
    assert_eq!(expressions.work(), budget);
    assert!(expressions.point(&construction, anchor).is_err());
    assert!(expressions
        .point_increment(&construction, increment)
        .is_err());
    let mut expressions = StyleExpressions::default();
    assert!(expressions.point(&construction, "(unknown, 1)").is_err());
    assert!(expressions
        .point_increment(&construction, "Length[L] + Sum[L]")
        .is_err());
}

#[test]
fn point_list_increments_charge_list_evaluation_to_the_style_budget() {
    use geo_import::convert::StyleExpressions;
    let c = geo_kernel::script::load("L = Sequence(1, 256)\nB = {true, false}\nn = 2").unwrap();
    let length = "Length[L] + n";
    let sum = "Sum[L]";
    let mut measured = StyleExpressions::default();
    assert!(measured.point_increment(&c, length).is_ok());
    let before_sum = measured.work();
    assert!(measured.point_increment(&c, sum).is_ok());
    assert!(measured.work() >= before_sum + sum.len() as u64 + 256);

    let mut limited = StyleExpressions::with_budget(before_sum + sum.len() as u64 + 128);
    assert!(limited.point_increment(&c, length).is_ok());
    let reason = limited.point_increment(&c, sum).unwrap_err();
    assert!(reason.contains("evaluation budget"), "{reason}");

    let mut exact = StyleExpressions::with_budget(measured.work());
    assert!(exact.point_increment(&c, length).is_ok());
    assert!(exact.point_increment(&c, sum).is_ok());
    assert!(exact.number(&c, "n").is_err());

    let mut expressions = StyleExpressions::default();
    assert!(expressions.number(&c, length).is_err());
    assert!(expressions.number(&c, sum).is_err());
    for unsupported in [
        "Sum[Sequence(1, 256)]",
        "Sum[L, L]",
        "Sum[B]",
        "Length[{1, 2}] + 1",
        "If(L == L, Length[L], n)",
        "If(true, Length[L], SetValue(n, 7))",
    ] {
        assert!(
            expressions.point_increment(&c, unsupported).is_err(),
            "{unsupported}"
        );
    }
    assert_eq!(c.arithmetic_value("n"), Some(2.0));
}

#[test]
fn nested_chart_image_tags_count_toward_the_item_limit() {
    let chart = |count| {
        let tags = r#"<tag key="barImage" barNumber="1" value="tile.png"/>"#.repeat(count);
        format!(
            r#"<geogebra><construction><element type="piechart" label="pie"><tags>{tags}</tags></element></construction></geogebra>"#
        )
    };
    assert!(geo_import::xml::parse(&chart(MAX_ITEMS - 2)).is_ok());
    assert!(matches!(
        geo_import::xml::parse(&chart(MAX_ITEMS - 1)),
        Err(ImportError::TooManyItems)
    ));
}

#[test]
fn rejected_lines_are_not_rescanned() {
    // Every refused line costs the work of that line only: 12000 refusals
    // must create nothing and keep the construction as it was, which is what
    // makes the work linear (there is no rebuild any more).
    let mut body = String::new();
    for i in 0..12_000 {
        body.push_str(&format!(
            "<command name=\"Frobnicate\"><input a0=\"A\" a1=\"B\"/><output a0=\"M{i}\"/></command>"
        ));
    }
    let (r, took) = timed(|| import(wrap(&body).as_bytes()).unwrap());
    assert_eq!(r.skipped.len(), 12_000);
    assert_eq!(r.script.len(), 2);
    assert!(took < BOUND, "{took:?}");
}

#[test]
fn thousands_of_mismatching_values_stay_in_the_script() {
    let mut body = String::new();
    for i in 0..12_000 {
        body.push_str(&format!(
            "<command name=\"Midpoint\"><input a0=\"A\" a1=\"B\"/><output a0=\"M{i}\"/></command>\
             <element type=\"point\" label=\"M{i}\"><coords x=\"9\" y=\"9\" z=\"1\"/></element>"
        ));
    }
    let (r, took) = timed(|| import(wrap(&body).as_bytes()).unwrap());
    assert_eq!(r.mismatched.len(), 12_000);
    assert_eq!(r.script.len(), 12_002);
    assert!(took < BOUND, "{took:?}");
}

/// A file with `n` triangles on the same three points.
fn polygons(n: usize) -> String {
    let mut body = String::from(
        "<element type=\"point\" label=\"C\"><coords x=\"0\" y=\"3\" z=\"1\"/></element>",
    );
    for i in 0..n {
        // the sides of a triangle are named after the opposite vertex
        body.push_str(&format!(
            "<command name=\"Polygon\"><input a0=\"A\" a1=\"B\" a2=\"C\"/>\
             <output a0=\"p{i}\"/></command>"
        ));
    }
    wrap(&body)
}

#[test]
fn twenty_thousand_polygons() {
    // The work of importing polygons grows linearly with their number: twice
    // the polygons, about twice the allocations. Every triangle names three
    // sides (`a_{n}`, `b_{n}`, `c_{n}`), so a label search that starts over
    // for each new object instead of at `Construction::label_hint`, or any
    // rescan of the construction per line, makes it quadratic, and the
    // label search allocates each candidate it tries. Allocations do not
    // depend on the speed of the machine, unlike the time bound this
    // replaces. The quadratic term already dominates at a few thousand
    // polygons, so the ratio is taken there, where a regression fails within
    // about a minute (the quadratic import of 4000 polygons is slow, not the
    // check). Then the full size must still import, under two guards that
    // are generous on purpose so no machine trips them: at most 1000
    // allocations a polygon (about 100 today), and done within ten minutes,
    // which catches a hang or quadratic work that does not allocate.
    let (half, a) = allocations(|| import(polygons(2_000).as_bytes()).unwrap());
    let (full, b) = allocations(|| import(polygons(4_000).as_bytes()).unwrap());
    assert_eq!(half.script.len(), 2_003);
    assert_eq!(full.script.len(), 4_003);
    let ratio = b as f64 / a as f64;
    assert!(
        ratio < 2.5,
        "2000 polygons: {a} allocations, 4000: {b} (ratio {ratio:.2}, linear is 2)"
    );
    const N: usize = 20_000;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let xml = polygons(N);
        let _ = tx.send(allocations(|| import(xml.as_bytes()).unwrap().script.len()));
    });
    let (lines, n) = rx
        .recv_timeout(Duration::from_secs(600))
        .unwrap_or_else(|e| panic!("{N} polygons: no import within ten minutes ({e})"));
    assert_eq!(lines, N + 3);
    assert!(
        n <= 1000 * N as u64,
        "{N} polygons: {n} allocations, over 1000 a polygon"
    );
}

/// An archive with a comment, built by hand around a stored entry.
fn with_comment(comment: &[u8]) -> Vec<u8> {
    let mut zip = zip_with(
        &[("geogebra.xml", SMALL.as_bytes())],
        zip::CompressionMethod::Stored,
    );
    let n = zip.len();
    // the comment length is the last two bytes of the 22-byte end record
    zip[n - 2..].copy_from_slice(&(comment.len() as u16).to_le_bytes());
    zip.extend_from_slice(comment);
    zip
}

#[test]
fn archives_with_comments_and_fake_end_records() {
    assert_eq!(
        import(&with_comment(b"hello")).unwrap().script,
        ["A = (1, 2)"]
    );
    // a fake end-of-central-directory signature inside the comment
    let mut fake = b"xx".to_vec();
    fake.extend_from_slice(b"PK\x05\x06");
    fake.extend_from_slice(&[0u8; 30]);
    assert_eq!(import(&with_comment(&fake)).unwrap().script, ["A = (1, 2)"]);
    // a comment as long as a comment may be
    let long = vec![b'x'; 0xFFFF];
    assert_eq!(import(&with_comment(&long)).unwrap().script, ["A = (1, 2)"]);
}

#[test]
fn zip64_markers_are_refused() {
    let zip = zip_with(
        &[("geogebra.xml", SMALL.as_bytes())],
        zip::CompressionMethod::Stored,
    );
    for field in [20, 24, 42] {
        let mut copy = zip.clone();
        patch_u32(&mut copy, CENTRAL, field, 0xFFFF_FFFF);
        assert!(
            matches!(import(&copy), Err(ImportError::Zip(m)) if m.contains("zip64")),
            "field {field}"
        );
    }
    // entry count and directory offset markers of the end record
    let mut copy = zip.clone();
    let n = copy.len();
    copy[n - 12..n - 8].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
    assert!(matches!(import(&copy), Err(ImportError::Zip(m)) if m.contains("zip64")));
}
