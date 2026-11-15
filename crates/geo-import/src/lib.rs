//! Reads the construction files saved by GeoGebra (`.ggb`, a zip archive with
//! a `geogebra.xml`, or a bare `geogebra.xml`) into lines of the `geo-kernel`
//! script language, so that every consumer of scripts (the command line, the
//! app with its journal of script lines) can use them unchanged.
//!
//! ```
//! let xml = r#"<geogebra><construction>
//!   <expression label="A" exp="(0, 0)" type="point"/>
//!   <element type="point" label="A"><coords x="0" y="0" z="1"/></element>
//!   <expression label="B" exp="(4, 0)" type="point"/>
//!   <element type="point" label="B"><coords x="4" y="0" z="1"/></element>
//!   <command name="Line"><input a0="A" a1="B"/><output a0="f"/></command>
//!   <element type="line" label="f"><coords x="0" y="1" z="0"/></element>
//! </construction></geogebra>"#;
//! let imported = geo_import::import(xml.as_bytes()).unwrap();
//! assert_eq!(imported.script, ["A = (0, 0)", "B = (4, 0)", "f = Line(A, B)"]);
//! assert!(imported.skipped.is_empty());
//! ```
//!
//! # How it works
//!
//! * [`archive`] finds the XML text in the bytes.
//! * [`xml`] turns the construction into a flat list of items (expressions,
//!   commands, elements) with a pull parser. It reads what GeoGebra's
//!   `MyXMLHandler` reads, nothing else.
//! * [`convert`] walks the items in order and writes one script line per
//!   object that the kernel can build. Every line is run on a scratch
//!   construction, so a command or argument form the kernel does not have is
//!   reported with the kernel's own message and everything that depends on
//!   the object is skipped with it.
//!
//! # Adding support for something new
//!
//! * a new command or argument form of the kernel: nothing to do here, the
//!   line is accepted as soon as the kernel runs it;
//! * a new type of free element (a polygon, a text): one arm in
//!   `convert::Converter::free_object`;
//! * an expression (`<expression exp="a + b">` or an argument such as
//!   `2 * r`): `convert::expression_line` for objects and
//!   `convert::pass_argument` for arguments; both return `None` today.

pub mod archive;
pub mod convert;
pub mod xml;

use std::fmt;

/// The result of importing a file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Imported {
    /// Lines of the script language, in construction order.
    pub script: Vec<String>,
    /// Every object that could not be imported, including the objects that
    /// depend on one that was skipped.
    pub skipped: Vec<Skipped>,
    /// Labels of imported objects that the file hides (`<show object="false">`).
    pub hidden: Vec<String>,
    /// The kernel settings of the file.
    pub settings: Settings,
    /// Things worth telling the user that are not an object (for example that
    /// the file was saved with continuity on).
    pub notes: Vec<String>,
    /// Objects the kernel computed differently from the value saved in the
    /// file (relative tolerance `1e-9`). The object stays in the script.
    pub mismatched: Vec<Mismatch>,
}

/// An imported object whose computed value differs from the saved one.
#[derive(Clone, Debug, PartialEq)]
pub struct Mismatch {
    pub label: String,
    /// What the file saved (coordinates, matrix, value), as text.
    pub saved: String,
    /// What the kernel computed.
    pub computed: String,
}

/// An object that was not imported.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Skipped {
    /// The label of the object (empty when the object has none).
    pub label: String,
    /// The command name, or the element type for an object without a command.
    pub kind: String,
    pub reason: String,
}

/// The `<kernel>` settings of the file. None of them changes a value the
/// kernel computes; they are reported so that a consumer can use them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Settings {
    /// Decimal places for display (`<decimals>`).
    pub decimals: Option<u32>,
    /// Significant figures for display (`<significantfigures>`).
    pub significant_figures: Option<u32>,
    /// `degree`, `radiant` or `degreesMinutesSeconds` (`<angleUnit>`).
    pub angle_unit: Option<String>,
    /// Whether the construction was saved with continuity on (`<continuous>`).
    pub continuous: bool,
}

/// Why a file could not be read at all.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImportError {
    /// The zip archive is damaged or truncated.
    Zip(String),
    /// The archive has no `geogebra.xml`.
    MissingXml,
    /// The XML (or the text inside the archive) is damaged.
    Xml(String),
    /// The file has no `<construction>`.
    NoConstruction,
    /// The unpacked XML is bigger than [`archive::MAX_XML_BYTES`].
    TooLarge,
    /// The archive lists more than [`archive::MAX_ZIP_ENTRIES`] entries.
    TooManyEntries(usize),
    /// The construction has more than [`xml::MAX_ITEMS`] items.
    TooManyItems,
    /// A bug of the importer (the scratch construction disagrees with the
    /// lines it accepted).
    Internal(String),
    /// A number of an object that has to be read is not a number.
    BadNumber { label: String, text: String },
    /// Two objects have the same label.
    DuplicateLabel(String),
    /// The outputs of a command are numbered wrongly (`a0`, `a2`, ...).
    BadOutputs { command: String, detail: String },
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImportError::Zip(e) => write!(f, "not a readable zip archive: {e}"),
            ImportError::MissingXml => write!(f, "the archive has no geogebra.xml"),
            ImportError::Xml(e) => write!(f, "damaged XML: {e}"),
            ImportError::NoConstruction => write!(f, "the file has no construction"),
            ImportError::TooLarge => write!(
                f,
                "the construction is larger than {} MiB",
                archive::MAX_XML_BYTES >> 20
            ),
            ImportError::TooManyEntries(n) => write!(
                f,
                "the archive lists {n} entries, more than the {} allowed",
                archive::MAX_ZIP_ENTRIES
            ),
            ImportError::TooManyItems => {
                write!(f, "the construction has more than {} items", xml::MAX_ITEMS)
            }
            ImportError::Internal(e) => write!(f, "internal error: {e}"),
            ImportError::BadNumber { label, text } => {
                write!(f, "object `{label}`: `{text}` is not a number")
            }
            ImportError::DuplicateLabel(l) => write!(f, "the label `{l}` is used twice"),
            ImportError::BadOutputs { command, detail } => {
                write!(f, "command `{command}`: {detail}")
            }
        }
    }
}

impl std::error::Error for ImportError {}

/// Imports a `.ggb` file or a bare `geogebra.xml`.
///
/// Limits: the unpacked XML at most [`archive::MAX_XML_BYTES`], an archive at
/// most [`archive::MAX_ZIP_ENTRIES`] entries, a construction at most
/// [`xml::MAX_ITEMS`] items. Beyond them the answer is an error, not an
/// attempt.
pub fn import(bytes: &[u8]) -> Result<Imported, ImportError> {
    let doc = {
        let text = archive::extract_xml(bytes)?;
        xml::parse(&text)?
    };
    convert::convert(doc)
}

/// Whether `bytes` look like a construction file (a zip archive, or XML whose
/// root element is `geogebra`) and not like a script. Only the beginning is
/// looked at.
pub fn looks_like_construction(bytes: &[u8]) -> bool {
    if archive::is_zip(bytes) {
        return true;
    }
    let head = &bytes[..bytes.len().min(4096)];
    let head = head.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(head);
    let mut rest = String::from_utf8_lossy(head);
    // Skip the declaration, comments and white space before the root.
    loop {
        let t = rest.trim_start();
        let after = if t.starts_with("<?") {
            t.find("?>").map(|i| &t[i + 2..])
        } else if t.starts_with("<!--") {
            t.find("-->").map(|i| &t[i + 3..])
        } else if t.starts_with("<!") {
            t.find('>').map(|i| &t[i + 1..])
        } else {
            return t.strip_prefix("<geogebra").is_some_and(|r| {
                r.starts_with(|c: char| c.is_whitespace() || c == '>' || c == '/')
            });
        };
        match after {
            Some(a) => rest = std::borrow::Cow::Owned(a.to_string()),
            None => return false,
        }
    }
}
