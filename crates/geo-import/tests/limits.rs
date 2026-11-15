//! Hostile input: archives that lie about their sizes, decompression bombs,
//! documents with too many items, and a great many lines that the kernel
//! rejects or whose saved values disagree. Each case is an error or a result
//! within a time bound, never a panic and never an unbounded allocation.

use geo_import::archive::{MAX_XML_BYTES, MAX_ZIP_ENTRIES};
use geo_import::xml::MAX_ITEMS;
use geo_import::{import, ImportError};
use std::io::Write;
use std::time::{Duration, Instant};

const BOUND: Duration = Duration::from_secs(30);

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
    let mut zip = zip_with(&[("geogebra.xml", &zeros)], zip::CompressionMethod::Deflated);
    assert!(zip.len() < 100_000, "the archive is small: {}", zip.len());
    patch_u32(&mut zip, LOCAL, 22, 1024);
    patch_u32(&mut zip, CENTRAL, 24, 1024);
    let (r, took) = timed(|| import(&zip));
    assert!(matches!(&r, Err(ImportError::Zip(m)) if m.contains("larger")), "{r:?}");
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
    let other = zip_with(&[("other.xml", SMALL.as_bytes())], zip::CompressionMethod::Stored);
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
    assert!(took < BOUND, "{took:?}");
}

/// A and B, then `n` commands that each give one object.
fn many_commands(n: usize, command: &str, coords: &str) -> String {
    let mut s = String::from(
        "<geogebra><construction>\
         <element type=\"point\" label=\"A\"><coords x=\"0\" y=\"0\" z=\"1\"/></element>\
         <element type=\"point\" label=\"B\"><coords x=\"4\" y=\"0\" z=\"1\"/></element>",
    );
    for i in 0..n {
        s.push_str(&format!(
            "<command name=\"{command}\"><input a0=\"A\" a1=\"B\"/><output a0=\"M{i}\"/></command>\
             <element type=\"point\" label=\"M{i}\">{coords}</element>"
        ));
    }
    s.push_str("</construction></geogebra>");
    s
}

#[test]
fn thousands_of_rejected_lines_take_linear_time() {
    // The kernel has no `Frobnicate`: every line is refused.
    let small = many_commands(3_000, "Frobnicate", "");
    let big = many_commands(12_000, "Frobnicate", "");
    let (a, t_small) = timed(|| import(small.as_bytes()).unwrap());
    let (b, t_big) = timed(|| import(big.as_bytes()).unwrap());
    assert_eq!((a.skipped.len(), b.skipped.len()), (3_000, 12_000));
    assert!(t_big < BOUND, "{t_big:?}");
    // Four times the lines must not take sixteen times as long.
    assert!(
        t_big.as_secs_f64() < 10.0 * t_small.as_secs_f64().max(0.05),
        "{t_small:?} -> {t_big:?}"
    );
}

#[test]
fn thousands_of_mismatching_values_take_linear_time() {
    // The midpoint is (2, 0); the file claims (9, 9).
    let coords = "<coords x=\"9\" y=\"9\" z=\"1\"/>";
    let small = many_commands(3_000, "Midpoint", coords);
    let big = many_commands(12_000, "Midpoint", coords);
    let (a, t_small) = timed(|| import(small.as_bytes()).unwrap());
    let (b, t_big) = timed(|| import(big.as_bytes()).unwrap());
    assert_eq!((a.mismatched.len(), b.mismatched.len()), (3_000, 12_000));
    assert_eq!(b.script.len(), 12_002, "every object stays in the script");
    assert!(t_big < BOUND, "{t_big:?}");
    assert!(
        t_big.as_secs_f64() < 10.0 * t_small.as_secs_f64().max(0.05),
        "{t_small:?} -> {t_big:?}"
    );
}
