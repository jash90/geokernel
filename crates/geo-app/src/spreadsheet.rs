//! Spreadsheet surface. Only rows and columns intersecting the clip are built.

use crate::{theme, App};
use eframe::egui::{
    pos2, vec2, Align2, FontId, Key, Rect, Role, ScrollArea, Sense, Stroke, TextEdit, TextStyle,
    Ui, UiBuilder, WidgetInfo,
};
use geo_app_core::spreadsheet::{Analysis, AnalysisError, SampleSummary, SAMPLE_STATISTIC_INDICES};
use geo_app_core::text::{label, Label};
use geo_kernel::spreadsheet::{Address, MAX_COLUMNS, MAX_INPUT_BYTES, MAX_ROWS};
use geo_kernel::Construction;

const HEADER_HEIGHT: f32 = 24.0;
const HEADER_WIDTH: f32 = 32.0;

enum GridAction {
    Select(Address, bool),
    Fill(Address),
}

pub(crate) fn show(app: &mut App, ui: &mut Ui) {
    if app.spreadsheet_editor.is_none() {
        let address = app
            .wb
            .spreadsheet_view()
            .and_then(|view| view.selected())
            .unwrap_or(Address { column: 1, row: 1 });
        app.select_spreadsheet_cell(address);
    }
    let analysis_open = matches!(app.spreadsheet_analysis, Ok(Some(_)));
    let (first, last) = app
        .spreadsheet_editor
        .as_ref()
        .expect("spreadsheet selection")
        .selection
        .bounds();
    let paired = first.column != last.column && first.row != last.row;
    let (launch, multiple, close) = if Analysis::available() {
        ui.horizontal_wrapped(|ui| {
            let title = if paired {
                "Analiza dwóch zmiennych"
            } else {
                "Analiza jednej zmiennej"
            };
            let launch = ui.button("Analiza");
            launch.widget_info(|| WidgetInfo::labeled(Role::Button, true, title));
            let launch = launch.on_hover_text(title).clicked();
            let multiple = ui.button("Próby");
            let title = "Analiza wielu prób";
            multiple.widget_info(|| WidgetInfo::labeled(Role::Button, true, title));
            let multiple = multiple.on_hover_text(title).clicked();
            // Reserve the close slot so opening the summary cannot move the grid.
            let close = ui.add_visible(analysis_open, eframe::egui::Button::new("×"));
            let close = analysis_open && {
                let title = "Zamknij podsumowanie";
                close.widget_info(|| WidgetInfo::labeled(Role::Button, true, title));
                close.on_hover_text(title).clicked()
            };
            ui.heading(label(Label::SpreadsheetView));
            let editor = app
                .spreadsheet_editor
                .as_mut()
                .expect("spreadsheet selection");
            ui.checkbox(&mut editor.range_extension_armed, "Zaznacz zakres");
            (launch, multiple, close)
        })
        .inner
    } else {
        ui.heading(label(Label::SpreadsheetView));
        (false, false, false)
    };
    crate::app::file_notices(app, ui);

    let editor = app
        .spreadsheet_editor
        .as_mut()
        .expect("spreadsheet selection");
    let selected = editor.selected;
    let focus = std::mem::take(&mut editor.focus);
    let error = editor.error.as_deref();
    let input_id = ui.make_persistent_id("spreadsheet_input");
    if focus {
        ui.memory_mut(|memory| memory.request_focus(input_id));
    }
    let response = ui
        .horizontal(|ui| {
            ui.label(selected.label());
            let mut text = TextEdit::load_state(ui.ctx(), input_id).unwrap_or_default();
            if focus {
                use eframe::egui::text::{CCursor, CCursorRange};
                text.cursor.set_char_range(Some(CCursorRange::two(
                    CCursor::new(0),
                    CCursor::new(editor.draft.chars().count()),
                )));
                text.store(ui.ctx(), input_id);
            }
            ui.add(
                TextEdit::singleline(&mut editor.draft)
                    .id(input_id)
                    .hint_text(label(Label::SpreadsheetInput))
                    .char_limit(MAX_INPUT_BYTES)
                    .desired_width(f32::INFINITY),
            )
        })
        .inner;
    response
        .widget_info(|| WidgetInfo::labeled(Role::TextInput, true, label(Label::SpreadsheetInput)));
    let enter = (response.has_focus() || response.lost_focus())
        && ui.input(|input| input.key_pressed(Key::Enter));
    if let Some(error) = error {
        ui.colored_label(
            theme::ERROR,
            format!("{} {error}", label(Label::SpreadsheetError)),
        );
    }
    // Close from the header even if only the grid fits below the formula bar.
    if close {
        app.spreadsheet_analysis = Ok(None);
        app.cancel_spreadsheet_range_extension();
        ui.ctx().request_repaint();
    }
    if let Err(error) = app.spreadsheet_analysis {
        ui.colored_label(theme::ERROR, analysis_error(error));
    }
    let mut sheet = ui.available_rect_before_wrap().intersect(ui.clip_rect());
    let summary = if matches!(app.spreadsheet_analysis, Ok(Some(_))) {
        split_summary(ui, &mut sheet)
    } else {
        None
    };
    let clicked = if sheet.width() > HEADER_WIDTH && sheet.height() > HEADER_HEIGHT {
        let reveal = std::mem::take(&mut app.spreadsheet_editor.as_mut().unwrap().reveal);
        let mut grid_ui = ui.new_child(UiBuilder::new().id_salt("sheet").max_rect(sheet));
        grid_ui.set_clip_rect(sheet);
        grid(app, &mut grid_ui, selected, reveal)
    } else {
        None
    };
    if let Ok(Some(analysis)) = &mut app.spreadsheet_analysis {
        let construction = app.wb.construction();
        if let Some(summary) = summary {
            let mut summary_ui =
                ui.new_child(UiBuilder::new().id_salt("summary").max_rect(summary));
            summary_ui.set_clip_rect(summary);
            show_analysis(analysis, construction, &mut summary_ui);
        } else {
            let _ = analysis.refresh_active(construction);
        }
    }
    if enter {
        app.submit_spreadsheet_input();
        ui.ctx().request_repaint();
    } else if multiple {
        app.launch_multiple_spreadsheet_analysis();
        ui.ctx().request_repaint();
    } else if launch {
        if paired {
            app.launch_paired_spreadsheet_analysis();
        } else {
            app.launch_spreadsheet_analysis();
        }
        ui.ctx().request_repaint();
    } else if let Some(action) = clicked {
        match action {
            GridAction::Select(address, true) => app.extend_spreadsheet_selection(address),
            GridAction::Select(address, false) => {
                app.cancel_spreadsheet_range_extension();
                if app.commit_spreadsheet_input() {
                    app.select_spreadsheet_cell(address);
                }
            }
            GridAction::Fill(target) => app.drag_spreadsheet_cell(selected, target),
        }
        ui.ctx().request_repaint();
    }
}

/// Leaves the entire viewport to the editable grid when both panes would be cramped.
fn split_summary(ui: &Ui, sheet: &mut Rect) -> Option<Rect> {
    let spacing = ui.spacing().item_spacing.y;
    // Allow room for the grid header and two ordinary cell targets, and
    // for the summary heading and its first three statistics.
    let grid_min = HEADER_HEIGHT + 2.0 * (ui.spacing().interact_size.y + spacing);
    let summary_min = ui.text_style_height(&TextStyle::Heading)
        + 3.0 * (ui.text_style_height(&TextStyle::Body) + spacing);
    let summary_max = sheet.height() - grid_min - spacing;
    if summary_max < summary_min {
        return None;
    }

    let mut summary = *sheet;
    let height = (sheet.height() * 0.5)
        .min(260.0)
        .clamp(summary_min, summary_max);
    summary.min.y = summary.max.y - height;
    sheet.max.y = summary.min.y - spacing;
    ui.painter().hline(
        sheet.x_range(),
        sheet.bottom() + spacing / 2.0,
        Stroke::new(1.0, theme::SEPARATOR),
    );
    Some(summary)
}

fn analysis_error(error: AnalysisError) -> &'static str {
    match error {
        AnalysisError::Range => "Zaznacz jeden wiersz lub jedną kolumnę (do 256 komórek).",
        AnalysisError::Cells => "Wszystkie komórki zakresu muszą zawierać skończone liczby.",
        AnalysisError::PairedRange => {
            "Zaznacz zakres dwóch wierszy lub dwóch kolumn (do 256 komórek)."
        }
        AnalysisError::SampleRange => {
            "Zaznacz co najmniej dwa wiersze i dwie kolumny (do 256 komórek)."
        }
        AnalysisError::EmptySample => "Każda próba musi zawierać co najmniej jedną liczbę.",
        AnalysisError::Pairs => "Zaznacz zakres z co najmniej jedną parą liczb.",
        AnalysisError::Evaluation => "Nie udało się obliczyć statystyk.",
    }
}

fn show_analysis(analysis: &mut Analysis, c: &Construction, ui: &mut Ui) {
    let (first, last) = analysis.endpoints();
    let range = if first == last {
        first.label()
    } else {
        format!("{}:{}", first.label(), last.label())
    };
    ui.heading(range);
    if analysis.is_multiple() {
        match analysis.refresh_multiple(c) {
            Ok(samples) => show_samples(ui, c, samples),
            Err(error) => {
                ui.colored_label(theme::ERROR, analysis_error(error));
            }
        }
        return;
    }
    if analysis.is_paired() {
        if ui.button("Zamień X i Y").clicked() {
            analysis.swap_axes();
        }
        let names = [
            "Średnia X",
            "Średnia Y",
            "Odchylenie standardowe próby X",
            "Odchylenie standardowe próby Y",
            "Korelacja Pearsona",
            "Korelacja Spearmana",
            "Sxx",
            "Syy",
            "Sxy",
        ];
        ScrollArea::vertical()
            .min_scrolled_height(0.0)
            .show(ui, |ui| match analysis.refresh_paired(c) {
                Ok(summary) => show_statistic_rows(ui, c, &names, &summary.rows),
                Err(error) => {
                    ui.colored_label(theme::ERROR, analysis_error(error));
                }
            });
        return;
    }
    ScrollArea::vertical()
        .min_scrolled_height(0.0)
        .show(ui, |ui| match analysis.refresh(c) {
            Ok(summary) => {
                show_statistic_rows(ui, c, &SUMMARY_NAMES, &summary.rows);
            }
            Err(error) => {
                ui.colored_label(theme::ERROR, analysis_error(error));
            }
        });
}

const SUMMARY_NAMES: [&str; 11] = [
    "Liczba danych",
    "Średnia",
    "Odchylenie standardowe",
    "Odchylenie standardowe próby",
    "Suma",
    "Suma kwadratów",
    "Minimum",
    "Pierwszy kwartyl",
    "Mediana",
    "Trzeci kwartyl",
    "Maksimum",
];

fn show_samples(ui: &mut Ui, c: &Construction, samples: &[SampleSummary]) {
    let row_height = ui
        .text_style_height(&TextStyle::Body)
        .max(ui.spacing().interact_size.y);
    let headings =
        std::iter::once("Próba").chain(SAMPLE_STATISTIC_INDICES.map(|index| SUMMARY_NAMES[index]));
    let font = TextStyle::Body.resolve(ui.style());
    let mut widths: Vec<_> = headings
        .clone()
        .map(|heading| {
            let text_width = ui
                .painter()
                .layout_no_wrap(heading.into(), font.clone(), ui.visuals().text_color())
                .size()
                .x;
            (5.0 * row_height).max(text_width + ui.spacing().item_spacing.x)
        })
        .collect();
    widths[0] = widths[0].max(7.0 * row_height);
    let spacing = ui.spacing().item_spacing;
    let stride = row_height + spacing.y;
    let width = widths.iter().sum::<f32>() + (widths.len() - 1) as f32 * spacing.x;
    ScrollArea::both()
        .id_salt("spreadsheet_samples")
        .min_scrolled_height(0.0)
        .show_viewport(ui, |ui, viewport| {
            let origin = ui.max_rect().min;
            let content_height = (samples.len() + 1) as f32 * stride - spacing.y;
            ui.set_min_size(vec2(width, content_height));

            // Cancel the vertical scroll offset for the header while keeping the shared x offset.
            let header_top = origin.y + viewport.top();
            let header = Rect::from_min_size(pos2(origin.x, header_top), vec2(width, stride));
            let mut headings_ui =
                ui.new_child(UiBuilder::new().id_salt("headings").max_rect(header));
            headings_ui.set_clip_rect(header.intersect(ui.clip_rect()));
            headings_ui.horizontal(|ui| {
                for (name, &width) in headings.zip(&widths) {
                    ui.add_sized([width, row_height], eframe::egui::Label::new(name));
                }
            });

            let first = (viewport.top() / stride).floor() as usize;
            let end = ((viewport.bottom() / stride).ceil() as usize).min(samples.len());
            if first >= end {
                return;
            }
            let row_top = origin.y + (first + 1) as f32 * stride;
            let rows = Rect::from_min_max(
                pos2(origin.x, row_top),
                pos2(origin.x + width, origin.y + content_height),
            );
            let clip = ui.clip_rect();
            let mut rows_ui = ui.new_child(UiBuilder::new().id_salt("rows").max_rect(rows));
            rows_ui.set_clip_rect(Rect::from_min_max(
                pos2(clip.left(), header.bottom().min(clip.bottom())),
                clip.max,
            ));
            for sample in &samples[first..end] {
                rows_ui.horizontal(|ui| {
                    let first = sample.first.label();
                    let title = if sample.first == sample.last {
                        first
                    } else {
                        format!("{first}:{}", sample.last.label())
                    };
                    let values = std::iter::once(title)
                        .chain(sample.rows.map(|number| c.format_display_number(number)));
                    for (value, &width) in values.zip(&widths) {
                        ui.add_sized(
                            [width, row_height],
                            eframe::egui::Label::new(value).truncate(),
                        );
                    }
                });
            }
        });
}

fn show_statistic_rows(ui: &mut Ui, c: &Construction, names: &[&str], rows: &[f64]) {
    eframe::egui::Grid::new("spreadsheet_statistics").show(ui, |ui| {
        for (&name, &value) in names.iter().zip(rows) {
            ui.label(name);
            ui.label(c.format_display_number(value));
            ui.end_row();
        }
    });
}

fn grid(app: &App, ui: &mut Ui, selected: Address, reveal: bool) -> Option<GridAction> {
    let mut clicked = None;
    let c = app.wb.construction();
    let editor = app.spreadsheet_editor.as_ref().unwrap();
    let selection = editor.selection;
    let extend = Analysis::available()
        && ui.input(|input| {
            input.modifiers.shift || (editor.range_extension_armed && input.modifiers.is_none())
        });
    let font = FontId::proportional(13.0);
    let layout = app.wb.spreadsheet_grid();
    ScrollArea::both()
        .id_salt("spreadsheet_grid")
        .auto_shrink([false, false])
        .min_scrolled_height(0.0)
        .animated(false)
        .show_viewport(ui, |ui, viewport| {
            let origin = ui.cursor().min;
            ui.set_min_size(vec2(
                HEADER_WIDTH + layout.columns.edge(MAX_COLUMNS),
                HEADER_HEIGHT + layout.rows.edge(MAX_ROWS),
            ));
            let col_at = |x: f32| layout.columns.cell_at(x - HEADER_WIDTH);
            let row_at = |y: f32| layout.rows.cell_at(y - HEADER_HEIGHT);
            let cols = col_at(viewport.left())..=col_at(viewport.right());
            let rows = row_at(viewport.top())..=row_at(viewport.bottom());
            let cell_rect = |column: u16, row: u16| {
                let left = layout.columns.edge(column - 1);
                let top = layout.rows.edge(row - 1);
                let right = layout.columns.edge(column);
                let bottom = layout.rows.edge(row);
                let body = origin + vec2(HEADER_WIDTH, HEADER_HEIGHT);
                Rect::from_min_max(body + vec2(left, top), body + vec2(right, bottom))
            };
            let clip = ui.clip_rect();
            let selected_rect = cell_rect(selected.column, selected.row);
            if reveal && !clip.contains_rect(selected_rect) {
                ui.scroll_to_rect(selected_rect, None);
                ui.ctx().request_repaint();
            }
            let header_y = clip.top();
            let header_x = clip.left();
            let painter = ui.painter().clone();
            painter.rect_filled(
                Rect::from_min_size(
                    pos2(clip.left(), header_y),
                    vec2(clip.width(), HEADER_HEIGHT),
                ),
                0.0,
                theme::TOOLBAR_BG,
            );
            painter.rect_filled(
                Rect::from_min_size(
                    pos2(header_x, clip.top()),
                    vec2(HEADER_WIDTH, clip.height()),
                ),
                0.0,
                theme::TOOLBAR_BG,
            );
            for column in cols.clone() {
                let header = cell_rect(column, 1);
                let pos = pos2(header.center().x, header_y + HEADER_HEIGHT / 2.0);
                if pos.x >= header_x + HEADER_WIDTH {
                    painter.text(
                        pos,
                        Align2::CENTER_CENTER,
                        Address::column_name(column),
                        font.clone(),
                        theme::MUTED,
                    );
                }
            }
            let body = Rect::from_min_max(clip.min + vec2(HEADER_WIDTH, HEADER_HEIGHT), clip.max);
            let painter = painter.with_clip_rect(body);
            let (first, last) = selection.bounds();
            let highlight = cell_rect(first.column, first.row)
                .union(cell_rect(last.column, last.row))
                .intersect(body);
            if highlight.is_positive() {
                painter.rect_filled(highlight, 0.0, theme::TOOLBAR_BG);
            }
            let edge = Stroke::new(1.0, theme::SEPARATOR);
            if layout.show_lines {
                for column in cols.clone() {
                    let right = cell_rect(column, 1).right();
                    if body.x_range().contains(right) {
                        painter.vline(right - 0.5, body.y_range(), edge);
                    }
                }
            }
            for row in rows {
                let row_rect = cell_rect(1, row);
                if layout.show_lines && body.y_range().contains(row_rect.bottom()) {
                    painter.hline(body.x_range(), row_rect.bottom() - 0.5, edge);
                }
                let y = row_rect.center().y;
                if y >= header_y + HEADER_HEIGHT {
                    ui.painter().text(
                        pos2(header_x + HEADER_WIDTH / 2.0, y),
                        Align2::CENTER_CENTER,
                        row.to_string(),
                        font.clone(),
                        theme::MUTED,
                    );
                }
                for column in cols.clone() {
                    let address = Address { column, row };
                    let rect = cell_rect(column, row);
                    let hit = rect.intersect(body);
                    if !hit.is_positive() {
                        continue;
                    }
                    let response =
                        ui.interact(hit, ui.make_persistent_id((column, row)), Sense::click());
                    let active = selection.contains(address);
                    let shown = app.wb.spreadsheet_display(address);
                    response.widget_info(|| {
                        let mut info =
                            WidgetInfo::selected(Role::Button, true, active, address.label());
                        info.current_text_value = Some(shown.clone());
                        info
                    });
                    if response.clicked() {
                        clicked = Some(GridAction::Select(address, extend));
                    }
                    if !shown.is_empty()
                        && rect.right() <= body.right() + 2.0
                        && rect.center().y <= body.bottom() - 5.0
                    {
                        painter.with_clip_rect(hit).text(
                            pos2(rect.right() - 5.0, rect.center().y),
                            Align2::RIGHT_CENTER,
                            shown,
                            font.clone(),
                            theme::TEXT,
                        );
                    }
                }
            }
            if c.spreadsheet_cell(selected).is_some() {
                let corner = selected_rect.right_bottom() - vec2(5.0, 5.0);
                let handle = Rect::from_center_size(corner, vec2(8.0, 8.0));
                if body.contains_rect(handle) {
                    let response = ui.interact(
                        handle,
                        ui.make_persistent_id("spreadsheet_fill_handle"),
                        Sense::drag(),
                    );
                    response.widget_info(|| {
                        WidgetInfo::labeled(Role::Button, true, label(Label::SpreadsheetFill))
                    });
                    if response.drag_stopped() {
                        if let Some(pos) =
                            ui.ctx().pointer_latest_pos().filter(|p| body.contains(*p))
                        {
                            let delta = pos - origin;
                            if let Some(target) = Address::new(col_at(delta.x), row_at(delta.y)) {
                                clicked = Some(GridAction::Fill(target));
                            }
                        }
                    }
                    painter.rect_filled(handle, 1.0, theme::TEXT);
                }
            }
        });
    clicked
}
