//! The commands that expressions compute themselves, one file per group
//! of the command matrix (the `group` column of `docs/command-matrix.csv`).

pub(super) mod algebra;
mod list;
mod logical;
mod statistics;
