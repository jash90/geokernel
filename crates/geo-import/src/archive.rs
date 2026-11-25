//! Finds the XML text of a construction in the bytes of a file.
//!
//! A `.ggb` file is a zip archive. Only what GeoGebra's writer produces is
//! read: the central directory, and entries that are stored (method 0) or
//! deflated (method 8). Zip64, encryption, split archives and every other
//! method are refused. All sizes are checked against the limits below before
//! anything is allocated, and the decompressor is given the declared size as
//! its output limit, so a deflate bomb stops at the limit.

use crate::ImportError;
use miniz_oxide::inflate::{decompress_to_vec_with_limit, TINFLStatus};
use std::borrow::Cow;
use std::collections::HashMap;

/// The most unpacked XML the importer reads (a guard against zip bombs).
pub const MAX_XML_BYTES: u64 = 256 * 1024 * 1024;

/// The most entries a zip archive may list. A `.ggb` file has a handful.
pub const MAX_ZIP_ENTRIES: usize = 4096;

/// The name of the construction in a `.ggb` archive.
pub const XML_FILE: &str = "geogebra.xml";
pub(crate) const JAVASCRIPT_FILE: &str = "geogebra_javascript.js";
pub(crate) const MAX_JAVASCRIPT_BYTES: u64 = 1 << 20;

/// Whether `bytes` start like a zip archive.
pub fn is_zip(bytes: &[u8]) -> bool {
    bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"PK\x05\x06")
}

/// The XML text of `bytes`: the entry `geogebra.xml` when the bytes are a zip
/// archive, the bytes themselves otherwise. A bare file is borrowed, not
/// copied.
pub fn extract_xml(bytes: &[u8]) -> Result<Cow<'_, str>, ImportError> {
    if is_zip(bytes) {
        let mut text = String::from_utf8(unzip(bytes)?)
            .map_err(|e| ImportError::Xml(format!("not UTF-8: {e}")))?;
        if text.starts_with('\u{feff}') {
            text.drain(..3);
        }
        return Ok(Cow::Owned(text));
    }
    if bytes.len() as u64 > MAX_XML_BYTES {
        return Err(ImportError::TooLarge);
    }
    let raw = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    std::str::from_utf8(raw)
        .map(Cow::Borrowed)
        .map_err(|e| ImportError::Xml(format!("not UTF-8: {e}")))
}

fn zip_error(detail: &str) -> ImportError {
    ImportError::Zip(detail.to_string())
}

fn u16_at(b: &[u8], at: usize) -> Result<usize, ImportError> {
    at.checked_add(2)
        .and_then(|end| b.get(at..end))
        .map(|s| u16::from_le_bytes([s[0], s[1]]) as usize)
        .ok_or_else(|| zip_error("truncated"))
}

fn u32_at(b: &[u8], at: usize) -> Result<u64, ImportError> {
    at.checked_add(4)
        .and_then(|end| b.get(at..end))
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]) as u64)
        .ok_or_else(|| zip_error("truncated"))
}

const EOCD: u32 = 0x0605_4b50;
const CENTRAL: u32 = 0x0201_4b50;
const LOCAL: u32 = 0x0403_4b50;

pub(crate) struct Entry {
    method: usize,
    crc: u32,
    compressed: u64,
    pub(crate) size: u64,
    pub(crate) local_offset: u64,
}

/// The end-of-central-directory record: the offset and the entry count of the
/// central directory.
fn find_end(bytes: &[u8]) -> Result<(usize, usize), ImportError> {
    let sig = EOCD.to_le_bytes();
    let lowest = bytes.len().saturating_sub(22 + 0xFFFF);
    let mut at = bytes
        .len()
        .checked_sub(22)
        .ok_or_else(|| zip_error("truncated"))?;
    loop {
        if bytes[at..at + 4] == sig {
            let comment = u16_at(bytes, at + 20)?;
            if at + 22 + comment == bytes.len() {
                let entries = u16_at(bytes, at + 10)?;
                let offset = u32_at(bytes, at + 16)?;
                if entries == 0xFFFF || offset == 0xFFFF_FFFF {
                    return Err(zip_error("zip64 archives are not supported"));
                }
                if u16_at(bytes, at + 4)? != 0 || u16_at(bytes, at + 6)? != 0 {
                    return Err(zip_error("split archives are not supported"));
                }
                return Ok((offset as usize, entries));
            }
        }
        if at == lowest {
            return Err(zip_error("no end-of-central-directory record"));
        }
        at -= 1;
    }
}

fn scan_entries<'a>(
    bytes: &'a [u8],
    mut visit: impl FnMut(&'a [u8], usize) -> bool,
) -> Result<(), ImportError> {
    let (mut at, count) = find_end(bytes)?;
    if count > MAX_ZIP_ENTRIES {
        return Err(ImportError::TooManyEntries(count));
    }
    for _ in 0..count {
        if u32_at(bytes, at)? != CENTRAL as u64 {
            return Err(zip_error("damaged central directory"));
        }
        let name_len = u16_at(bytes, at + 28)?;
        let extra_len = u16_at(bytes, at + 30)?;
        let comment_len = u16_at(bytes, at + 32)?;
        let name = bytes
            .get(at.saturating_add(46)..at.saturating_add(46).saturating_add(name_len))
            .ok_or_else(|| zip_error("truncated"))?;
        if !visit(name, at) {
            return Ok(());
        }
        at = at
            .checked_add(46 + name_len + extra_len + comment_len)
            .ok_or_else(|| zip_error("damaged central directory"))?;
    }
    Ok(())
}

fn read_entry(bytes: &[u8], at: usize) -> Result<Entry, ImportError> {
    let flags = u16_at(bytes, at + 8)?;
    if flags & 1 != 0 {
        return Err(zip_error("encrypted entries are not supported"));
    }
    let entry = Entry {
        method: u16_at(bytes, at + 10)?,
        crc: u32_at(bytes, at + 16)? as u32,
        compressed: u32_at(bytes, at + 20)?,
        size: u32_at(bytes, at + 24)?,
        local_offset: u32_at(bytes, at + 42)?,
    };
    if [entry.compressed, entry.size, entry.local_offset].contains(&0xFFFF_FFFF) {
        return Err(zip_error("zip64 entries are not supported"));
    }
    Ok(entry)
}

/// Index optional entries, keeping those located before a damaged entry.
pub(crate) struct AssetArchive<'a> {
    bytes: &'a [u8],
    offsets: HashMap<&'a [u8], usize>,
}

impl<'a> AssetArchive<'a> {
    pub(crate) fn index(bytes: &'a [u8]) -> (Self, Option<ImportError>) {
        let mut offsets = HashMap::new();
        let result = scan_entries(bytes, |name, offset| {
            offsets.entry(name).or_insert(offset);
            true
        });
        (Self { bytes, offsets }, result.err())
    }

    pub(crate) fn javascript(&self) -> Result<Vec<u8>, ImportError> {
        let name = JAVASCRIPT_FILE.as_bytes();
        let Some(offset) = self.offsets.get(name) else {
            return Ok(Vec::new());
        };
        let entry = read_entry(self.bytes, *offset)?;
        if entry.size > MAX_JAVASCRIPT_BYTES {
            return Err(zip_error("the JavaScript library is too large"));
        }
        let data = entry_data(self.bytes, &entry, name)?;
        extract_entry(entry, data)
    }

    pub(crate) fn spreadsheet_metadata(&self) -> Result<Vec<u8>, ImportError> {
        let name = crate::spreadsheet::METADATA_FILE.as_bytes();
        let Some(offset) = self.offsets.get(name) else {
            return Ok(Vec::new());
        };
        let entry = read_entry(self.bytes, *offset)?;
        if entry.size > crate::spreadsheet::MAX_METADATA_BYTES {
            return Err(ImportError::TooLarge);
        }
        let data = entry_data(self.bytes, &entry, name)?;
        extract_entry(entry, data)
    }

    pub(crate) fn image_entry(&self, name: &str) -> Result<(Entry, &[u8]), String> {
        let offset = self
            .offsets
            .get(name.as_bytes())
            .ok_or("image asset is missing from the archive")?;
        let entry = read_entry(self.bytes, *offset).map_err(|e| e.to_string())?;
        if entry.size > geo_kernel::kernel::image::MAX_ASSET_BYTES {
            return Err("image asset is too large".into());
        }
        let data = entry_data(self.bytes, &entry, name.as_bytes()).map_err(|e| e.to_string())?;
        Ok((entry, data))
    }
}

const CRC_TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut n = 0;
    while n < 256 {
        let mut c = n as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
            k += 1;
        }
        table[n] = c;
        n += 1;
    }
    table
};

fn crc32(data: &[u8]) -> u32 {
    !data.iter().fold(!0u32, |c, &b| {
        CRC_TABLE[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8)
    })
}

fn unzip(bytes: &[u8]) -> Result<Vec<u8>, ImportError> {
    let mut offset = None;
    scan_entries(bytes, |name, at| {
        if name == XML_FILE.as_bytes() {
            offset = Some(at);
            false
        } else {
            true
        }
    })?;
    let entry = read_entry(bytes, offset.ok_or(ImportError::MissingXml)?)?;
    if entry.size > MAX_XML_BYTES {
        return Err(ImportError::TooLarge);
    }
    let data = entry_data(bytes, &entry, XML_FILE.as_bytes())?;
    extract_entry(entry, data)
}

fn entry_data<'a>(bytes: &'a [u8], entry: &Entry, name: &[u8]) -> Result<&'a [u8], ImportError> {
    let at = usize::try_from(entry.local_offset).map_err(|_| zip_error("bad offset"))?;
    if u32_at(bytes, at)? != LOCAL as u64 {
        return Err(zip_error("damaged local header"));
    }
    if u16_at(bytes, at + 6)? & 1 != 0 || u16_at(bytes, at + 8)? != entry.method {
        return Err(zip_error(
            "local header flags or compression method disagree",
        ));
    }
    let name_start = at.checked_add(30).ok_or_else(|| zip_error("bad offset"))?;
    let name_end = name_start
        .checked_add(u16_at(bytes, at + 26)?)
        .ok_or_else(|| zip_error("bad offset"))?;
    if bytes.get(name_start..name_end) != Some(name) {
        return Err(zip_error("local and central entry names differ"));
    }
    let start = name_end
        .checked_add(u16_at(bytes, at + 28)?)
        .ok_or_else(|| zip_error("bad offset"))?;
    let end = start
        .checked_add(entry.compressed as usize)
        .ok_or_else(|| zip_error("bad offset"))?;
    bytes
        .get(start..end)
        .ok_or_else(|| zip_error("the entry runs past the end of the file"))
}

pub(crate) fn extract_entry(entry: Entry, data: &[u8]) -> Result<Vec<u8>, ImportError> {
    let size = entry.size as usize;
    let out = match entry.method {
        0 => {
            if data.len() != size {
                return Err(zip_error("a stored entry whose sizes differ"));
            }
            data.to_vec()
        }
        8 => decompress_to_vec_with_limit(data, size).map_err(|e| match e.status {
            TINFLStatus::HasMoreOutput => zip_error("the entry is larger than its declared size"),
            _ => ImportError::Zip(format!("damaged deflate data: {:?}", e.status)),
        })?,
        other => return Err(ImportError::Zip(format!("compression method {other}"))),
    };
    if out.len() != size {
        return Err(zip_error("the entry is smaller than its declared size"));
    }
    if crc32(&out) != entry.crc {
        return Err(zip_error("the checksum of the entry does not match"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};

    #[test]
    fn large_raster_entry_rejects_alias_names_before_extraction() {
        let mut png = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut png, 4096, 4096);
            encoder.set_depth(png::BitDepth::One);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&vec![0; 4096 * 512])
                .unwrap();
        }
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        zip.start_file("tile.png", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(&png).unwrap();
        let bytes = zip.finish().unwrap().into_inner();
        let (mut archive, error) = AssetArchive::index(&bytes);
        assert!(error.is_none());
        let directory_entry = archive.offsets[b"tile.png".as_slice()];
        assert!(archive.image_entry("tile.png").is_ok());

        // Exercise the index against ambiguous candidates without writing
        // a malformed archive. The name check runs before inflation.
        let aliases: Vec<_> = (0..MAX_ZIP_ENTRIES - 1)
            .map(|i| format!("tile{i}.png"))
            .collect();
        for alias in &aliases {
            archive.offsets.insert(alias.as_bytes(), directory_entry);
        }
        let start = std::time::Instant::now();
        for alias in &aliases {
            assert!(
                matches!(archive.image_entry(alias), Err(reason) if reason.contains("names differ"))
            );
        }
        assert!(start.elapsed() < std::time::Duration::from_secs(2));
    }

    #[test]
    fn spreadsheet_metadata_shares_the_construction_item_limit() {
        let xml = format!(
            "<geogebra><construction>{}</construction></geogebra>",
            "<cellPair/>".repeat(crate::xml::MAX_ITEMS)
        );
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("geogebra.xml", options).unwrap();
        zip.write_all(xml.as_bytes()).unwrap();
        zip.start_file(crate::spreadsheet::METADATA_FILE, options)
            .unwrap();
        zip.write_all(br#"<spreadsheetView><cell label="A1" input="1"/></spreadsheetView>"#)
            .unwrap();
        let bytes = zip.finish().unwrap().into_inner();
        assert!(matches!(
            crate::import(&bytes),
            Err(ImportError::TooManyItems)
        ));
    }

    #[test]
    fn stale_numeric_editor_input_cannot_replace_a_saved_value() {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("geogebra.xml", options).unwrap();
        zip.write_all(
            br#"<geogebra><construction>
            <element type="numeric" label="A1"><value val="2"/></element>
            </construction></geogebra>"#,
        )
        .unwrap();
        zip.start_file(crate::spreadsheet::METADATA_FILE, options)
            .unwrap();
        zip.write_all(br#"<spreadsheetView><cell label="A1" input="1"/></spreadsheetView>"#)
            .unwrap();
        let bytes = zip.finish().unwrap().into_inner();
        let imported = crate::import(&bytes).unwrap();
        let c = geo_kernel::script::load(&imported.script.join("\n")).unwrap();
        let address = geo_kernel::spreadsheet::Address::parse("A1").unwrap();
        assert_eq!(c.spreadsheet_display(address), "2");
        assert_eq!(c.spreadsheet_input(address).as_deref(), Some("2"));
    }
}
