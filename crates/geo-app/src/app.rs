//! The window layout: toolbar, algebra view, graphics view and the on-screen
//! keyboard, the keyboard shortcuts and the number dialog. The state lives in
//! `geo_app_core::app`.

use crate::algebra;
use crate::canvas;
use crate::geom::{ToCore, ToEgui};
use crate::keyboard;
use crate::layout::Regions;
use crate::theme;
use crate::toolbar;
use eframe::egui::{self, Rect, Stroke, Ui, UiBuilder};
pub use geo_app_core::app::{parse_number, App, ViewRequest};
use geo_app_core::text::{self, Label};
use geo_app_core::workbench::FileView;
use std::io::Write;
use std::path::Path;

/// Opens a script or construction file into the window.
fn open(app: &mut App, ctx: &egui::Context, path: &Path) {
    match crate::cli::load_into(&mut app.wb, path) {
        Ok(()) => {
            app.load_error = None;
            app.show_cas = app.wb.cas_visible_on_open();
            app.show_spreadsheet = app
                .wb
                .spreadsheet_view()
                .and_then(|view| view.visible)
                .unwrap_or(false);
            app.save_error = None;
            app.cancel_cas_edit();
            app.spreadsheet_editor = None;
            app.input_box_edit = None;
            app.inspector = None;
            app.input_box_rects.clear();
            ctx.memory_mut(|memory| {
                if let Some(id) = memory.focused() {
                    memory.surrender_focus(id);
                }
            });
            app.source_path = Some(path.into());
            if app.wb.file_view().is_some() {
                app.decor = geo_app_core::app::Decor::for_document(&app.wb);
            }
            app.drag = None;
            // The tool's picks belonged to the other document.
            app.reset_tool();
            app.view_request = Some(ViewRequest::Initial);
        }
        Err(e) => app.load_error = Some(e),
    }
}

pub(crate) fn file_notices(app: &mut App, ui: &mut Ui) {
    for (notice, prefix) in [
        (&mut app.save_error, None),
        (&mut app.load_error, Some(Label::OpenFailed)),
    ] {
        if let Some(message) = notice.as_deref() {
            let close = ui
                .horizontal_wrapped(|ui| {
                    let display = prefix.map_or(std::borrow::Cow::Borrowed(message), |prefix| {
                        format!("{}: {message}", text::label(prefix)).into()
                    });
                    ui.colored_label(theme::ERROR, display);
                    ui.small_button(text::label(Label::Close)).clicked()
                })
                .inner;
            if close {
                *notice = None;
            }
        }
    }
}

fn shortcuts(app: &mut App, ctx: &egui::Context) {
    // While a text field has focus these keys belong to it: Cmd+Z undoes
    // the text, Escape leaves the field.
    // egui drops the focus itself when Escape arrives, so the field
    // counts as focused if it was a frame ago.
    let typing = ctx.egui_wants_keyboard_input();
    let was_typing = std::mem::replace(&mut app.was_typing, typing);
    let typing = typing || was_typing;
    // egui ends any drag on Escape, whoever takes the key afterwards.
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.input_box_edit = None;
        app.gesture.stop();
    }
    if app.tool.asking().is_some()
        || app.tool.control_request().is_some()
        || app.image_request.is_some()
    {
        return;
    }
    if typing {
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            app.cancel_cas_edit();
            app.cancel_spreadsheet_edit();
            app.input.clear();
            app.input_error = None;
            app.keyboard.open = false;
            ctx.memory_mut(|m| {
                if let Some(id) = m.focused() {
                    m.surrender_focus(id);
                }
            });
        }
    } else {
        let (redo, undo, escape) = ctx.input_mut(|i| {
            (
                i.consume_key(
                    egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                    egui::Key::Z,
                ),
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::Z),
                i.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
            )
        });
        if redo {
            app.redo();
        } else if undo {
            app.undo();
        }
        if escape {
            if app.drag.is_some() {
                // Abandon the drag and put the point (or the label) back.
                app.cancel_drag();
            } else {
                app.leave_tool();
            }
        }
        if ctx.input(|i| {
            i.key_pressed(egui::Key::F)
                && !i.modifiers.ctrl
                && !i.modifiers.command
                && !i.modifiers.alt
        }) {
            app.view_request = Some(ViewRequest::Fit);
        }
        // Ctrl + plus / equals zooms in, Ctrl + minus out, by the wheel
        // factor about the last pointer position (`GlobalKeyDispatcher`;
        // spec D14). The drawing applies it, with the view's own area.
        let key = ctx.input_mut(|i| {
            [
                (egui::Key::Plus, 1.0),
                (egui::Key::Equals, 1.0),
                (egui::Key::Minus, -1.0),
            ]
            .into_iter()
            .find_map(|(k, dir)| i.consume_key(egui::Modifiers::COMMAND, k).then_some(dir))
        });
        if let Some(dir) = key {
            app.key_zoom = Some(geo_app_core::camera::KEY_ZOOM_FACTOR.powf(dir));
        }
    }
}

/// Runs `f` in a child ui that covers exactly `rect`.
fn region(app: &mut App, ui: &mut Ui, rect: Rect, salt: &str, f: impl FnOnce(&mut App, &mut Ui)) {
    let mut child = ui.new_child(
        UiBuilder::new()
            .id_salt(salt)
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    child.set_clip_rect(rect.intersect(ui.clip_rect()));
    f(app, &mut child);
}

/// Lays out the whole window into `ui`.
pub fn show(app: &mut App, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    if let Some(path) = ctx.input(|i| i.raw.dropped_files.first().map(|f| f.path().to_owned())) {
        open(app, &ctx, &path);
    }
    shortcuts(app, &ctx);
    // A press in the graphics view puts the keyboard away.
    let pressed_at = ctx.input(|i| {
        i.pointer
            .primary_pressed()
            .then(|| i.pointer.interact_pos())
            .flatten()
    });
    if pressed_at.is_some_and(|p| app.regions.graphics.egui().contains(p)) {
        app.keyboard.open = false;
    }
    let regions = Regions::new(ui.max_rect().core(), app.keyboard.open, &app.metrics);
    app.regions = regions;
    let splitter = regions.splitter.egui();
    let painter = ui.painter().clone();
    // The root ui of a real window has no fill of its own.
    painter.rect_filled(ui.max_rect(), 0.0, theme::WHITE);
    painter.rect_filled(splitter, 0.0, theme::TOOLBAR_BG);
    if regions.portrait {
        for y in [splitter.top() + 0.5, splitter.bottom() - 0.5] {
            painter.hline(splitter.x_range(), y, Stroke::new(1.0, theme::BORDER));
        }
    } else {
        for x in [splitter.left() + 0.5, splitter.right() - 0.5] {
            painter.vline(x, splitter.y_range(), Stroke::new(1.0, theme::BORDER));
        }
    }
    region(app, ui, regions.algebra.egui(), "algebra", |app, ui| {
        if app.show_spreadsheet {
            crate::spreadsheet::show(app, ui);
        } else if app.show_cas {
            crate::cas::show(app, ui);
        } else {
            algebra::show(app, ui);
        }
    });
    region(app, ui, regions.graphics.egui(), "graphics", |app, ui| {
        canvas::show(app, ui)
    });
    if let Some(rect) = regions.keyboard {
        region(app, ui, rect.egui(), "keyboard", keyboard::show);
    }
    // Last, so that its drop-downs are above the views.
    region(app, ui, regions.toolbar.egui(), "toolbar", |app, ui| {
        toolbar::show(app, ui)
    });
    number_dialog(app, &ctx);
    relation_dialog(app, &ctx);
    control_dialog(app, &ctx);
    save_dialog(app, &ctx);
    image_dialog(app, &ctx);
    function_inspector(app, &ctx);
}

fn save_dialog(app: &mut App, ctx: &egui::Context) {
    let Some(path) = &mut app.save_path else {
        return;
    };
    let (skipped, mismatched) = app.wb.file_import_losses();
    let incomplete = skipped != 0 || mismatched != 0;
    let response = egui::Modal::new(egui::Id::unique("save-ggb")).show(ctx, |ui| {
        ui.heading(text::label(Label::SaveAsGgb));
        if incomplete {
            ui.label(format!("Zapis niedostępny. Pominięte obiekty: {skipped}. Niezgodne wartości: {mismatched}."));
        } else {
            ui.label("Biblioteka JavaScript nie zostanie zapisana. Skrypty obiektów będą wyłączone.");
        }
        ui.label(text::label(Label::SavePath));
        ui.add(egui::TextEdit::singleline(path).desired_width(320.0));
        ui.horizontal(|ui| {
            (
                ui.add_enabled(
                    !incomplete && !path.trim().is_empty(),
                    egui::Button::new(text::label(Label::SaveFile)),
                )
                .clicked(),
                ui.button(text::label(Label::Cancel)).clicked(),
            )
        })
        .inner
    });
    let (save, cancel) = response.inner;
    if save {
        app.save_error = None;
        let path = ggb_path(Path::new(path.trim()));
        if let Err(error) = save_to(app, &path) {
            app.save_error = Some(format!(
                "{} ({})\n{error}",
                text::label(Label::SaveFailed),
                path.display()
            ));
        }
    }
    if save || cancel || response.should_close() {
        app.save_path = None;
    }
}

fn image_dialog(app: &mut App, ctx: &egui::Context) {
    if app.image_request.is_none() {
        return;
    }
    let response = egui::Modal::new(egui::Id::unique("open-image")).show(ctx, |ui| {
        ui.heading(crate::tools::Tool::Image.caption());
        let caption = ui.label("Ścieżka do obrazu PNG, JPEG lub GIF");
        let field_id = ui.make_persistent_id("image-path");
        let submit = ui.memory(|memory| memory.has_focus(field_id))
            && ui.input(|input| input.key_pressed(egui::Key::Enter));
        let field = ui
            .add(
                egui::TextEdit::singleline(&mut app.image_path)
                    .id(field_id)
                    .desired_width(320.0),
            )
            .labelled_by(caption.id);
        if !ui.memory(|memory| memory.focused().is_some()) {
            field.request_focus();
        }
        ui.horizontal(|ui| {
            let ready = !app.image_path.trim().is_empty();
            let open = ui.add_enabled(ready, egui::Button::new("Otwórz")).clicked();
            let cancel = ui.button(text::label(Label::Cancel)).clicked();
            (ready && (submit || open), cancel)
        })
        .inner
    });
    let (open, cancel) = response.inner;
    if cancel || response.should_close() {
        app.select_tool(crate::tools::Tool::Move);
    } else if open {
        let result =
            read_image(Path::new(app.image_path.trim())).and_then(|bytes| app.finish_image(bytes));
        if let Err(error) = result {
            app.select_tool(crate::tools::Tool::Move);
            app.load_error = Some(format!("Nie można wstawić obrazu: {error}"));
        }
    }
}

fn read_image(path: &Path) -> Result<Vec<u8>, String> {
    let regular_file = |metadata: std::fs::Metadata| {
        if metadata.is_file() {
            Ok(())
        } else {
            Err("expected a regular image file".to_owned())
        }
    };
    regular_file(std::fs::metadata(path).map_err(|error| error.to_string())?)?;
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        // The path can be replaced by a pipe between metadata and open.
        options.custom_flags(libc::O_NONBLOCK);
    }
    let file = options.open(path).map_err(|error| error.to_string())?;
    regular_file(file.metadata().map_err(|error| error.to_string())?)?;
    read_image_bytes(file)
}

fn read_image_bytes(file: impl std::io::Read) -> Result<Vec<u8>, String> {
    use geo_kernel::kernel::image::MAX_ASSET_BYTES;
    use std::io::Read as _;

    let mut bytes = Vec::new();
    file.take(MAX_ASSET_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > MAX_ASSET_BYTES {
        return Err("image asset is too large".into());
    }
    Ok(bytes)
}

fn function_inspector(app: &mut App, ctx: &egui::Context) {
    let Some(inspector) = &mut app.inspector else {
        return;
    };
    let mut open = true;
    egui::Window::new(crate::tools::Tool::FunctionInspector.caption())
        .open(&mut open)
        .show(ctx, |ui| {
            ui.strong(&inspector.label);
            ui.horizontal(|ui| {
                for (caption, value) in ["Od", "Do", "Krok"]
                    .into_iter()
                    .zip(&mut inspector.interval)
                {
                    let label = ui.label(caption);
                    ui.add(egui::DragValue::new(value).speed(0.01))
                        .labelled_by(label.id);
                }
            });
            let [min, max, step] = inspector.interval;
            let rows = inspector.rows(&app.wb, min, max, step);
            if rows.len() == geo_app_core::editor::MAX_INSPECTOR_ROWS {
                ui.label(format!("Wyświetlono maksymalnie {} wierszy.", rows.len()));
            }
            egui::ScrollArea::vertical()
                .max_height(260.0)
                .show(ui, |ui| {
                    egui::Grid::new("function-values")
                        .striped(true)
                        .show(ui, |ui| {
                            ui.strong("x");
                            ui.strong("f(x)");
                            ui.end_row();
                            for (x, y) in rows {
                                ui.label(x.to_string());
                                ui.label(if y.is_nan() {
                                    "—".into()
                                } else {
                                    y.to_string()
                                });
                                ui.end_row();
                            }
                        });
                });
        });
    if !open {
        app.inspector = None;
    }
}

fn has_ggb_extension(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("ggb"))
}

fn ggb_path(path: &Path) -> std::path::PathBuf {
    let mut path = path.to_path_buf();
    if !has_ggb_extension(&path) {
        path.as_mut_os_string().push(".ggb");
    }
    path
}

pub(crate) fn default_save_path(source: Option<&Path>) -> String {
    let path = source.map_or_else(
        || "construction.ggb".into(),
        |path| {
            if has_ggb_extension(path) {
                let mut name = path.file_stem().unwrap_or_default().to_os_string();
                name.push("-copy.ggb");
                path.with_file_name(name)
            } else if path.extension().is_some_and(|ext| {
                ext.eq_ignore_ascii_case("geo") || ext.eq_ignore_ascii_case("xml")
            }) {
                path.with_extension("ggb")
            } else {
                ggb_path(path)
            }
        },
    );
    path.to_string_lossy().into_owned()
}

fn save_to(app: &App, path: &Path) -> Result<(), String> {
    let view = app.view.map(|(rect, transform)| {
        let (x, y) = transform.world_to_screen(0.0, 0.0);
        FileView {
            size: [
                (rect.width().round() as u32).max(1),
                (rect.height().round() as u32).max(1),
            ],
            origin: [x - f64::from(rect.left()), y - f64::from(rect.top())],
            scale: [transform.xscale(), transform.yscale()],
            axes: app
                .wb
                .file_view()
                .filter(|view| view.axes.into_iter().any(|axis| axis) == app.decor.axes)
                .map_or([app.decor.axes; 2], |view| view.axes),
            grid: app.decor.grid,
        }
    });
    let bytes = app
        .wb
        .export_ggb_with_spreadsheet(
            view,
            app.show_cas,
            app.show_spreadsheet,
            app.spreadsheet_editor
                .as_ref()
                .map(|editor| editor.selected),
        )
        .map_err(|e| e.to_string())?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                text::label(Label::SaveDestinationExists).into()
            } else {
                error.to_string()
            }
        })?;
    if let Err(error) = file.write_all(&bytes) {
        drop(file);
        let _ = std::fs::remove_file(path);
        return Err(error.to_string());
    }
    Ok(())
}

fn relation_dialog(app: &mut App, ctx: &egui::Context) {
    let Some(reports) = &app.relation_report else {
        return;
    };
    let modal = egui::Modal::new(egui::Id::unique("relation_dialog")).show(ctx, |ui| {
        ui.heading(crate::tools::Tool::Relation.caption());
        for report in reports {
            ui.label(text::relation_message(report.message));
        }
        ui.button("OK").clicked()
    });
    if modal.inner || modal.should_close() {
        app.relation_report = None;
    }
}

/// The dialog of a tool that needs a number.
fn number_dialog(app: &mut App, ctx: &egui::Context) {
    let Some(request) = app.tool.asking() else {
        return;
    };
    let mut answer: Option<Option<f64>> = None;
    let modal = egui::Modal::new(egui::Id::unique("number_dialog")).show(ctx, |ui| {
        ui.set_width(300.0);
        ui.heading(request.title);
        ui.add_space(8.0);
        let prompt = ui.label(request.prompt);
        let field = ui.add(
            egui::TextEdit::singleline(&mut app.number_text)
                .id(egui::Id::unique("number_field"))
                .desired_width(f32::INFINITY),
        );
        let field = field.labelled_by(prompt.id);
        if !field.has_focus() && !field.lost_focus() {
            field.request_focus();
            if matches!(
                app.tool.tool(),
                crate::tools::Tool::AngleFixed | crate::tools::Tool::SegmentFixed
            ) {
                let mut editor = egui::TextEdit::load_state(ui.ctx(), field.id).unwrap_or_default();
                editor
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::select_all_str(
                        &app.number_text,
                    )));
                editor.store(ui.ctx(), field.id);
            }
        }
        let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let parsed = app
            .tool_number_value()
            .filter(|&value| app.tool.valid_number(value));
        if parsed.is_none() {
            ui.colored_label(theme::ERROR, text::label(Label::NotANumber));
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("Anuluj").clicked() {
                answer = Some(None);
            }
            let ok = ui.add_enabled(parsed.is_some(), egui::Button::new("OK"));
            if (ok.clicked() || enter) && parsed.is_some() {
                answer = Some(parsed);
            }
        });
    });
    if modal.should_close() && answer.is_none() {
        answer = Some(None);
    }
    match answer {
        Some(Some(v)) => {
            let outcome = {
                let c = app.wb.construction();
                app.tool.finish_number(v, &|l| c.id(l).is_some())
            };
            app.apply_outcome(outcome);
        }
        Some(None) => app.reset_tool(),
        None => {}
    }
}

fn control_dialog(app: &mut App, ctx: &egui::Context) {
    if app.tool.control_request().is_none() {
        return;
    }
    let tool = app.tool.tool();
    let slider = tool == crate::tools::Tool::Slider;
    let input_box = tool == crate::tools::Tool::InputBox;
    let text = tool == crate::tools::Tool::Text;
    let mut confirm = false;
    let mut cancel = false;
    let mut numbers = None;
    let modal = egui::Modal::new(egui::Id::unique("control_dialog")).show(ctx, |ui| {
        ui.set_min_width(240.0);
        ui.heading(tool.caption());
        if slider {
            egui::Grid::new("slider_fields").show(ui, |ui| {
                for (label, field) in ["Minimum", "Maksimum", "Krok"]
                    .into_iter()
                    .zip(&mut app.control_fields)
                {
                    let label = ui.label(label);
                    ui.text_edit_singleline(field).labelled_by(label.id);
                    ui.end_row();
                }
            });
            if let [Some(min), Some(max), Some(step)] =
                app.control_fields.each_ref().map(|s| parse_number(s))
            {
                if min < max && step > 0.0 {
                    numbers = Some((min, max, step));
                }
            }
            if numbers.is_none() {
                ui.colored_label(theme::ERROR, "Podaj rosnący zakres i dodatni krok.");
            }
        } else if text {
            let caption = ui.label("Treść");
            let editor = ui
                .add(
                    egui::TextEdit::singleline(&mut app.control_fields[0])
                        .char_limit(crate::tools::MAX_PLACED_TEXT_CHARS),
                )
                .labelled_by(caption.id);
            if std::mem::take(&mut app.control_focus) {
                editor.request_focus();
            }
        } else {
            let caption = ui.label("Podpis");
            let editor = egui::TextEdit::singleline(&mut app.control_fields[0]);
            let editor = if input_box {
                editor.char_limit(geo_kernel::kernel::control::MAX_INPUT_BOX_CHARS)
            } else {
                editor
            };
            ui.add(editor).labelled_by(caption.id);
            if input_box {
                let link = ui.label("Powiązany obiekt");
                ui.add(
                    egui::TextEdit::singleline(&mut app.control_fields[1])
                        .char_limit(geo_kernel::kernel::control::MAX_INPUT_BOX_CHARS),
                )
                .labelled_by(link.id);
            }
        }
        ui.horizontal(|ui| {
            cancel = ui.button("Anuluj").clicked();
            let link = app.control_fields[1].trim();
            let enabled = (!slider || numbers.is_some())
                && (!input_box || link.is_empty() || app.wb.construction().id(link).is_some())
                && (!text || !app.control_fields[0].trim().is_empty());
            confirm = ui.add_enabled(enabled, egui::Button::new("OK")).clicked()
                || (enabled && ui.input(|i| i.key_pressed(egui::Key::Enter)));
        });
    });
    if confirm {
        let c = app.wb.construction();
        let taken = |label: &str| c.id(label).is_some();
        let outcome = if let Some((min, max, step)) = numbers {
            app.tool.finish_slider(min, max, step, taken)
        } else if text {
            app.tool.finish_text(&app.control_fields[0], taken)
        } else if input_box {
            let link = app.control_fields[1].trim();
            app.tool.finish_input_box(
                &app.control_fields[0],
                (!link.is_empty()).then_some(link),
                taken,
            )
        } else {
            app.tool.finish_checkbox(&app.control_fields[0], taken)
        };
        app.apply_outcome(outcome);
    } else if cancel || modal.should_close() {
        app.reset_tool();
    }
}

#[cfg(test)]
mod export_path_tests {
    use super::*;

    #[test]
    fn failed_save_as_is_visible_from_cas() {
        use egui::accesskit::{Action, ActionRequest, TreeId};

        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut app = App::new(crate::Workbench::default(), None);
        app.show_cas = true;
        let path = std::env::temp_dir().join(format!("geokernel-save-{}.ggb", std::process::id()));
        std::fs::write(&path, "original").unwrap();
        app.save_path = Some(path.to_string_lossy().into_owned());
        let frame = |app: &mut App, events| {
            let screen = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 500.0));
            let mut output = ctx.run_ui(
                crate::headless::raw_input(screen, 0.0, 0.016, events),
                |ui| show(app, ui),
            );
            output.textures_delta.clear();
            output.platform_output.accesskit_update.unwrap()
        };
        frame(&mut app, Vec::new());
        let tree = frame(&mut app, Vec::new());
        let save = tree
            .nodes
            .iter()
            .find(|(_, n)| n.label() == Some(text::label(Label::SaveFile)))
            .unwrap_or_else(|| {
                panic!(
                    "Save button: {:?}",
                    tree.nodes
                        .iter()
                        .filter_map(|(_, n)| n.label().or_else(|| n.value()))
                        .collect::<Vec<_>>()
                )
            })
            .0;
        frame(
            &mut app,
            vec![egui::Event::AccessKitActionRequest(ActionRequest {
                action: Action::Click,
                target_tree: TreeId::ROOT,
                target_node: save,
                data: None,
            })],
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "original");
        std::fs::remove_file(path).unwrap();
        assert!(app.save_path.is_none());
        let tree = frame(&mut app, Vec::new());
        assert!(tree.nodes.iter().any(|(_, n)| n
            .value()
            .is_some_and(|label| label.starts_with(text::label(Label::SaveFailed))
                && label.contains(text::label(Label::SaveDestinationExists)))));
    }

    #[test]
    fn save_as_defaults_next_to_the_source_and_keeps_a_dotted_name() {
        let source = Path::new("worksheets/my.v2.ggb");
        assert_eq!(
            default_save_path(Some(source)),
            source.with_file_name("my.v2-copy.ggb").to_string_lossy()
        );
        assert_eq!(ggb_path(Path::new("my.v2")), Path::new("my.v2.ggb"));
        assert_eq!(
            default_save_path(Some(Path::new("worksheets/my.v2"))),
            Path::new("worksheets/my.v2.ggb").to_string_lossy()
        );
        assert_eq!(ggb_path(Path::new("my.GGB")), Path::new("my.GGB"));
    }
}

#[cfg(test)]
#[test]
fn image_reader_stops_at_the_compressed_byte_limit() {
    use geo_kernel::kernel::image::MAX_ASSET_BYTES;
    use std::io::Read as _;
    let mut input = std::io::repeat(0).take(MAX_ASSET_BYTES + 4096);
    assert_eq!(
        read_image_bytes(&mut input).unwrap_err(),
        "image asset is too large"
    );
    assert_eq!(input.limit(), 4095);
}
