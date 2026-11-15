//! `geokernel-app`: a desktop workbench for the construction kernel.
//!
//! Logic lives in the `geo-app-core` crate (`view`, `interact`, `hit`,
//! `tools`, `workbench`, `scriptlog`, `undo`, `ticks`, `labels`, `describe`,
//! ...); the modules here (`app`, `canvas`, `algebra`, `toolbar`, `keyboard`,
//! `grid`, `draw`, `icons`) only turn it into egui calls.

mod algebra;
mod app;
mod canvas;
mod cli;
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
mod frame_counts;
#[cfg(test)]
mod render_tests;
#[cfg(test)]
mod ui_tests;

use app::App;
use eframe::egui;
use geo_app_core::workbench::Workbench;
use geo_app_core::{
    arc, describe, highlight, hit, interact, labelpos, layout, shape, style, ticks, tools, view,
    workbench,
};

const SCRIPT: &str = include_str!("../../../examples/triangle.geo");

/// The window around the application state (the state type lives in the
/// core crate, so the eframe trait needs a local type).
struct Window(App);

impl eframe::App for Window {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        app::show(&mut self.0, ui);
    }
}

fn main() -> eframe::Result {
    let command = match cli::parse_args(std::env::args_os().skip(1)) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}\n{}", cli::USAGE);
            std::process::exit(2);
        }
    };
    let cli::Command::Run { script } = command else {
        println!("{}", cli::USAGE);
        return Ok(());
    };
    let (wb, load_error) = match script {
        Some(path) => {
            let mut wb = Workbench::default();
            let result = cli::load_into(&mut wb, &path);
            (wb, result.err())
        }
        None => match Workbench::from_script(SCRIPT) {
            Ok(wb) => (wb, None),
            Err(e) => {
                eprintln!("cannot load the built-in script: {e}");
                std::process::exit(1);
            }
        },
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("geokernel")
            .with_inner_size([1400.0, 900.0]),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        "geokernel",
        options,
        Box::new(|cc| {
            theme::apply(&cc.egui_ctx);
            Ok(Box::new(Window(App::new(wb, load_error))))
        }),
    )
}
