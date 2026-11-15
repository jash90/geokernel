//! The logic of the geokernel window, free of any UI toolkit.
//!
//! The window state (`app`, `keyboard`, `layout`), the document
//! (`workbench`, `scriptlog`, `undo`, `history`), the tools (`tools`), the
//! view (`view`, `interact`, `hit`, `highlight`), how objects look (`style`,
//! `palette`) and the text shown for them (`describe`, `labels`, `labelpos`,
//! `ticks`, `errors`) live here, on the plain screen types of `geom`. The
//! `geo-app` crate turns them into egui calls.

pub mod app;
pub mod arc;
pub mod describe;
pub mod errors;
pub mod geom;
pub mod highlight;
pub mod history;
pub mod hit;
pub mod interact;
pub mod keyboard;
pub mod labelpos;
pub mod labels;
pub mod layout;
pub mod palette;
pub mod scriptlog;
pub mod shape;
pub mod style;
pub mod ticks;
pub mod tools;
pub mod undo;
pub mod view;
pub mod workbench;
