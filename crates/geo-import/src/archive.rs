//! Finds the XML text of a construction in the bytes of a file.

use crate::ImportError;
use std::io::{Cursor, Read};

/// The most unpacked XML the importer reads (a guard against zip bombs).
pub const MAX_XML_BYTES: u64 = 256 * 1024 * 1024;

/// The name of the construction in a `.ggb` archive.
pub const XML_FILE: &str = "geogebra.xml";

/// The XML text of `bytes`: the entry `geogebra.xml` when the bytes are a zip
/// archive, the bytes themselves otherwise.
pub fn extract_xml(bytes: &[u8]) -> Result<String, ImportError> {
    let raw = if bytes.starts_with(b"PK") {
        unzip(bytes)?
    } else {
        if bytes.len() as u64 > MAX_XML_BYTES {
            return Err(ImportError::TooLarge);
        }
        bytes.to_vec()
    };
    let raw = raw.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&raw);
    String::from_utf8(raw.to_vec()).map_err(|e| ImportError::Xml(format!("not UTF-8: {e}")))
}

fn unzip(bytes: &[u8]) -> Result<Vec<u8>, ImportError> {
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| ImportError::Zip(e.to_string()))?;
    let mut entry = match archive.by_name(XML_FILE) {
        Ok(entry) => entry,
        Err(zip::result::ZipError::FileNotFound) => return Err(ImportError::MissingXml),
        Err(e) => return Err(ImportError::Zip(e.to_string())),
    };
    let mut out = Vec::new();
    // One byte more than the limit tells a file that is too big from one
    // that is exactly as big as allowed.
    (&mut entry)
        .take(MAX_XML_BYTES + 1)
        .read_to_end(&mut out)
        .map_err(|e| ImportError::Zip(e.to_string()))?;
    if out.len() as u64 > MAX_XML_BYTES {
        return Err(ImportError::TooLarge);
    }
    Ok(out)
}
