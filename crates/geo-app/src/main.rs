//! `geokernel-app`: the desktop shell of the geokernel window. It reads the
//! command line and the file, and opens the window from the `geo_app`
//! library.

use eframe::egui;
use geo_app::{cli, Metrics, Window, Workbench};

const SCRIPT: &str = include_str!("../../../examples/triangle.geo");

fn main() -> eframe::Result {
    let command = match cli::parse_args(std::env::args_os().skip(1)) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}\n{}", cli::USAGE);
            std::process::exit(2);
        }
    };
    let cli::Command::Run { script, touch } = command else {
        println!("{}", cli::USAGE);
        return Ok(());
    };
    let (wb, load_error, source_path) = match script {
        Some(path) => {
            let mut wb = Workbench::default();
            let result = cli::load_into(&mut wb, &path);
            let source_path = result.is_ok().then_some(path);
            (wb, result.err(), source_path)
        }
        None => match Workbench::from_script(SCRIPT) {
            Ok(wb) => (wb, None, None),
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
        Box::new(move |cc| {
            let metrics = if touch {
                Metrics::touch()
            } else {
                Metrics::desktop()
            };
            Ok(Box::new(
                Window::new(cc, wb, load_error, metrics).with_source_path(source_path),
            ))
        }),
    )
}
