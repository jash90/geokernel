//! The logic of the geokernel window, free of any UI toolkit.
//!
//! The document (`workbench`, `scriptlog`, `undo`, `history`), the tools
//! (`tools`), the view (`view`, `interact`, `hit`, `highlight`) and the text
//! shown for objects (`describe`, `labels`, `labelpos`, `ticks`, `errors`)
//! live here. The `geo-app` crate turns them into egui calls.

pub mod arc;
pub mod describe;
pub mod errors;
pub mod highlight;
pub mod history;
pub mod hit;
pub mod interact;
pub mod labelpos;
pub mod labels;
pub mod scriptlog;
pub mod shape;
pub mod ticks;
pub mod tools;
pub mod undo;
pub mod view;
pub mod workbench;
