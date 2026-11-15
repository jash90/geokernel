//! The geokernel window as a library: the desktop binary (`main.rs`) and a
//! later Android shell both create a [`Window`] and hand it to eframe.
//!
//! Logic lives in the `geo-app-core` crate (`view`, `interact`, `hit`,
//! `tools`, `workbench`, `scriptlog`, `undo`, `ticks`, `labels`, `describe`,
//! ...); the modules here (`app`, `canvas`, `algebra`, `toolbar`, `keyboard`,
//! `grid`, `draw`, `icons`) only turn it into egui calls.

mod algebra;
mod app;
mod canvas;
pub mod cli;
mod draw;
mod floating;
mod geom;
mod grid;
mod icons;
mod keyboard;
mod theme;
mod tool_icons;
mod toolbar;

#[cfg(test)]
mod budget_tests;
#[cfg(test)]
mod cull_tests;
#[cfg(test)]
mod frame_counts;
#[cfg(test)]
mod render_tests;
#[cfg(test)]
mod ui_bench;
#[cfg(test)]
mod ui_tests;

pub use app::App;
use eframe::egui;
pub use geo_app_core::metrics::Metrics;
pub use geo_app_core::workbench::Workbench;
use geo_app_core::{
    arc, describe, gesture, highlight, hit, interact, labelpos, layout, shape, style, ticks, tools,
    view, workbench,
};

/// The window around the application state (the state type lives in the
/// core crate, so the eframe trait needs a local type).
pub struct Window(App);

impl Window {
    /// The window for `wb` with the input profile `metrics`, set up in the
    /// egui context of `cc`: the theme, and egui's own click rules (buttons,
    /// rows) following the profile (the desktop profile holds egui's
    /// defaults). `load_error` is shown until dismissed.
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        wb: Workbench,
        load_error: Option<String>,
        metrics: Metrics,
    ) -> Window {
        theme::apply(&cc.egui_ctx);
        cc.egui_ctx.options_mut(|o| {
            o.input_options.max_click_dist = metrics.drag_slop;
            o.input_options.max_click_duration = metrics.click_time;
        });
        let mut app = App::new(wb, load_error);
        app.set_metrics(metrics);
        Window(app)
    }
}

impl eframe::App for Window {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        app::show(&mut self.0, ui);
    }
}
