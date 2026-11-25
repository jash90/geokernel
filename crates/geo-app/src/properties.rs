//! The keyboard-accessible properties inspector.

use crate::App;
use eframe::egui::{self, ComboBox, Id, Key, LayerId, Modal, Modifiers, TextEdit, Ui};
use geo_app_core::properties::{EditError, Field, Input, Tab};
use geo_kernel::Id as ObjectId;

pub(crate) const TITLE: &str = "Właściwości obiektu";

pub(crate) fn canvas_id() -> Id {
    Id::unique("properties_canvas_focus")
}

fn tab_label(tab: Tab) -> &'static str {
    match tab {
        Tab::Basic => "Podstawowe",
        Tab::Color => "Kolor",
        Tab::Style => "Styl",
        Tab::Position => "Położenie",
        Tab::Advanced => "Zaawansowane",
    }
}

pub(crate) fn show(app: &mut App, root: &Ui) {
    let ctx = root.ctx();
    let Some(mut dialog) = app.properties.take() else {
        return;
    };
    if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
        ctx.memory_mut(|m| m.request_focus(canvas_id()));
        return;
    }
    let Some(id) = app.wb.construction().id(dialog.primary()) else {
        ctx.memory_mut(|m| m.request_focus(canvas_id()));
        return;
    };
    let modal = Modal::new(Id::unique("object_properties"));
    let layer = modal.area.layer();
    let response = modal.show(ctx, |ui| {
        ctx.accesskit_node_builder(ui.unique_id(), |node| {
            node.set_role(egui::accesskit::Role::Dialog);
            node.set_label(TITLE);
        });
        ui.set_width(430.0_f32.min(ctx.content_rect().width() - 32.0).max(100.0));
        ui.heading(TITLE);
        if dialog.targets().len() == 1 {
            ui.label(dialog.primary());
        } else {
            ui.label(format!("Wybrane obiekty: {}", dialog.targets().len()));
        }
        let mut next_tab = dialog.tab;
        ui.horizontal_wrapped(|ui| {
            for tab in Tab::ALL {
                if dialog.fields.iter().any(|field| field.tab() == tab)
                    && ui
                        .selectable_label(dialog.tab == tab, tab_label(tab))
                        .clicked()
                {
                    next_tab = tab;
                }
            }
        });
        ui.separator();
        let previous_draft = dialog.draft.take();
        let mut blurred = None;
        let mut changes = Vec::new();
        egui::ScrollArea::vertical()
            .id_salt(dialog.tab as u8)
            .max_height((ctx.content_rect().height() - 155.0).clamp(55.0, 540.0))
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for &field in &dialog.fields {
                    if field.tab() != dialog.tab {
                        continue;
                    }
                    let mut value = app.wb.property_value(id, field);
                    let condition_controls_visibility = field == Field::Visible
                        && !app.wb.property_value(id, Field::Condition).is_empty();
                    let result = ui.add_enabled_ui(!condition_controls_visibility, |ui| {
                        ui.push_id(field, |ui| {
                            field_input(
                                ui,
                                field,
                                field.label_for(app.wb.construction(), id),
                                &mut value,
                                previous_draft.as_ref(),
                                &mut dialog.draft,
                            )
                        })
                        .inner
                    });
                    if result.inner {
                        if previous_draft
                            .as_ref()
                            .is_some_and(|(active, _)| *active == field)
                        {
                            blurred = Some((field, value));
                        } else {
                            changes.push((field, value));
                        }
                    }
                }
            });
        let mut edits = blurred.into_iter().chain(changes).peekable();
        if edits.peek().is_some() {
            dialog.error = None;
        }
        for (field, value) in edits {
            let error = app
                .wb
                .edit_property(
                    dialog.targets(),
                    field,
                    &value,
                    app.view.as_ref().map(|(_, view)| view),
                )
                .err();
            if error.is_none() && matches!(field, Field::Anchor | Field::TextLatex) {
                dialog.refresh_fields(app.wb.construction());
            }
            dialog.error = dialog.error.or(error);
            ctx.request_repaint();
        }
        if next_tab != dialog.tab {
            dialog.tab = next_tab;
            dialog.draft = None;
        }
        if let Some(error) = dialog.error {
            ui.label(match error {
                EditError::SelectionTooLarge => "Wybór jest zbyt duży dla tej zmiany.",
                EditError::MissingTarget => "Wybrany obiekt został usunięty.",
                EditError::InvalidValue => "Nieprawidłowa wartość.",
            });
        }
        ui.separator();
        ui.horizontal(|ui| {
            if ui
                .add_enabled(app.wb.can_undo(), egui::Button::new("Cofnij zmianę"))
                .clicked()
            {
                if app.undo() {
                    dialog.draft = None;
                    dialog.refresh_fields(app.wb.construction());
                }
                ctx.request_repaint();
            }
            if ui
                .add_enabled(app.wb.can_redo(), egui::Button::new("Ponów zmianę"))
                .clicked()
            {
                if app.redo() {
                    dialog.draft = None;
                    dialog.refresh_fields(app.wb.construction());
                }
                ctx.request_repaint();
            }
            ui.button("Zamknij").clicked()
        })
        .inner
    });
    append_to_scene(ctx, root.layer_id(), layer);
    if response.inner {
        ctx.memory_mut(|m| m.request_focus(canvas_id()));
    } else {
        app.properties = Some(dialog);
    }
}

/// Egui flattens separately painted layers into a fresh buffer each frame.
/// With the entire scene already in that buffer, appending even one dialog
/// shape would double its allocation. Preserve the modal's input layer but
/// append its paint to the scene when nothing needs to draw between them.
fn append_to_scene(ctx: &egui::Context, scene: LayerId, modal: LayerId) {
    if ctx.layer_transform_to_global(scene).is_some()
        || ctx.layer_transform_to_global(modal).is_some()
    {
        return;
    }
    let layers: Vec<_> = ctx.memory(|m| {
        m.layer_ids()
            .filter(|layer| {
                *layer != scene
                    && *layer != modal
                    && (scene.order..=modal.order).contains(&layer.order)
            })
            .collect()
    });
    ctx.graphics_mut(|graphics| {
        if layers
            .iter()
            .any(|&layer| graphics.get(layer).is_some_and(|list| !list.is_empty()))
        {
            return;
        }
        let mut shapes = std::mem::take(graphics.entry(modal));
        let target = graphics.entry(scene);
        for index in 0..shapes.next_idx().0 {
            shapes.mutate_shape(egui::layers::ShapeIdx(index), |shape| {
                let paint = std::mem::replace(&mut shape.shape, egui::Shape::Noop);
                let index = target.add(shape.clip_rect, paint);
                target.mutate_shape(index, |target| {
                    target.transform_after_tessellation = shape.transform_after_tessellation;
                });
            });
        }
    });
}

fn field_input(
    ui: &mut Ui,
    field: Field,
    label: &str,
    value: &mut String,
    previous_draft: Option<&(Field, String)>,
    next_draft: &mut Option<(Field, String)>,
) -> bool {
    if matches!(field.input(), Input::Check) {
        let mut checked = value == "true";
        if ui.checkbox(&mut checked, label).changed() {
            *value = checked.to_string();
            return true;
        }
        return false;
    }
    ui.horizontal(|ui| {
        let label = ui.label(label);
        if let Input::Choice(choices) = field.input() {
            let current = value.parse::<usize>().unwrap_or(0).min(choices.len() - 1);
            let mut chosen = None;
            ComboBox::from_id_salt("choice")
                .selected_text(choices[current])
                .show_ui(ui, |ui| {
                    for (i, choice) in choices.iter().enumerate() {
                        if ui.selectable_label(i == current, *choice).clicked() {
                            chosen = Some(i);
                        }
                    }
                })
                .response
                .labelled_by(label.id);
            if let Some(chosen) = chosen {
                *value = chosen.to_string();
                return true;
            }
            return false;
        }
        let current = previous_draft.filter(|(editing, _)| *editing == field);
        let mut edited = current.map_or_else(|| value.clone(), |(_, value)| value.clone());
        let response = ui
            .add(
                TextEdit::singleline(&mut edited)
                    .desired_width(135.0)
                    .char_limit(2048),
            )
            .labelled_by(label.id);
        if response.has_focus() {
            *next_draft = Some((field, edited));
        } else if (current.is_some() || response.lost_focus()) && edited != *value {
            *value = edited;
            return true;
        }
        false
    })
    .inner
}

pub(crate) fn open(app: &mut App, id: ObjectId) {
    app.open_properties(id);
}
