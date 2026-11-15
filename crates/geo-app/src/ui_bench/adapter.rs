//! Everything the benchmark needs to know about this tree's layout and app
//! state. Porting the benchmark to another revision of the app means
//! changing this file only.

use crate::app::App;
use crate::tools::Tool;
use crate::workbench::Row;
use eframe::egui::Rect;
use geo_kernel::kernel::Construction;

/// A named piece of per-object work over the whole construction.
pub type Work = (&'static str, fn(&Construction) -> u64);

/// Which user interface this tree has.
pub const TREE: &str = "rebuilt";

/// The window has an on-screen keyboard.
pub const HAS_KEYBOARD: bool = true;

/// The drawing area as last painted (`screen` is the whole window).
pub fn canvas(app: &App, _screen: Rect) -> Rect {
    app.view.map_or(app.regions.graphics, |(r, _)| r)
}

/// The algebra list.
pub fn algebra(app: &App, _screen: Rect) -> Rect {
    app.regions.algebra
}

/// The height of an algebra row.
pub fn row_height(row: &Row) -> f32 {
    if row.free {
        50.0
    } else {
        102.0
    }
}

pub fn select_tool(app: &mut App, tool: Tool) {
    app.select_tool(tool);
}

/// Asks for the input row's focus, which brings up the keyboard.
pub fn open_keyboard(app: &mut App) {
    app.focus_input = true;
}

pub fn keyboard_open(app: &App) -> bool {
    app.keyboard.open
}

/// Per-object work outside the kernel that this tree does in every frame,
/// as `(name, work over all objects)`, timed separately.
pub fn per_object_work() -> Vec<Work> {
    vec![("style::color of every object", |c| {
        c.ids()
            .map(|id| u64::from(crate::style::color(c, id).r()))
            .sum()
    })]
}
