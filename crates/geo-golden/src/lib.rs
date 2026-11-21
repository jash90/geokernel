//! Golden-data tests for `geo-kernel`.
//!
//! The data format is described in `golden/SCHEMA.md` at the repository root.
//!
//! * [`model`] parses and validates golden files.
//! * [`compare`] implements the comparison rules of the schema.
//! * [`replay`] runs a case on the kernel and compares every snapshot.
//! * [`gaps`] reads the list of known gaps.
//! * [`report`] ties it together and produces the summary.
//! * [`forms`] checks which command forms golden data shows (the command
//!   matrix), and [`table`] reads the CSV tables it uses.

pub mod compare;
pub mod forms;
pub mod gaps;
pub mod model;
pub mod replay;
pub mod report;
pub mod table;

use std::path::PathBuf;

/// Root of the repository (two levels above this crate).
pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
