//! What the server reports: the state of the window (in the shape a
//! GeoGebra applet exposes, so that the two compare field by field), the
//! named widgets, the counters of a frame, and what changed between two
//! moments.

use geo_app::headless::egui::{epaint, Rect, Shape};
use geo_app::headless::{Headless, Profile, Repaint, Widget, WidgetKind};
use geo_app_core::describe;
use geo_app_core::tools::{Tool, GROUPS};
use geo_kernel::algo::measure::MeasureDef;
use geo_kernel::kernel::{Construction, Def, Id, Value};
use serde_json::{json, Map, Value as Json};

/// The name of a tool in the protocol: its Rust name in snake case
/// (`angle_bisector`, `circle3`).
pub fn tool_id(tool: Tool) -> String {
    let name = format!("{tool:?}");
    let mut out = String::with_capacity(name.len() + 4);
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// The tool named `id` (see [`tool_id`]).
pub fn tool_from_id(id: &str) -> Option<Tool> {
    GROUPS
        .iter()
        .flat_map(|g| g.iter().copied())
        .find(|&t| tool_id(t) == id)
}

/// Every tool id, group by group.
pub fn tool_ids() -> Vec<String> {
    GROUPS
        .iter()
        .flat_map(|g| g.iter().map(|&t| tool_id(t)))
        .collect()
}

pub fn profile_name(p: Profile) -> &'static str {
    match p {
        Profile::Desktop => "desktop",
        Profile::Touch => "touch",
    }
}

/// The value of an object as numbers (homogeneous line coefficients as
/// stored, like GeoGebra's `x`, `y`, `z` of a line).
fn coords(c: &Construction, id: Id) -> Json {
    match c.value(id) {
        Value::Point { x, y } | Value::Vector { x, y } => json!({"x": x, "y": y}),
        Value::Line { a, b, c } => json!({"a": a, "b": b, "c": c}),
        Value::Ray { x, y, a, b, c } => json!({"x": x, "y": y, "a": a, "b": b, "c": c}),
        Value::Segment { x1, y1, x2, y2 } => json!({"x1": x1, "y1": y1, "x2": x2, "y2": y2}),
        Value::Circle { cx, cy, r } => json!({"cx": cx, "cy": cy, "r": r}),
        Value::Number(v) => json!({ "value": v }),
        Value::Bool(v) => json!({ "value": v }),
        Value::Polygon { area } => {
            let vertices: Vec<Json> = c
                .vertices(id)
                .flatten()
                .map(|(x, y)| json!([x, y]))
                .collect();
            json!({"area": area, "vertices": vertices})
        }
        Value::PolyLine { length } => json!({ "length": length }),
        // A function has no coordinates: its values at a few points stand
        // for it, so that a change of what it depends on shows.
        Value::Function => {
            let samples: Vec<Json> = [-2.0, -1.0, 0.0, 1.0, 2.0]
                .iter()
                .map(|&x| json!([x, c.function_value(id, x).filter(|y| y.is_finite())]))
                .collect();
            json!({ "samples": samples })
        }
        _ => Json::Null,
    }
}

/// The value of an object as the algebra view shows it (angles in
/// degrees, as `Workbench::rows` writes them), and the plain value text when
/// that differs.
fn value_texts(c: &Construction, id: Id) -> (String, Option<String>) {
    let value = c.value(id);
    let raw = describe::object_text(c, id);
    let shown = match (c.def(id), value) {
        (Def::Measure(MeasureDef::Angle(..)) | Def::Angle(_), Value::Number(n)) => {
            describe::degrees(n)
        }
        _ => raw.clone(),
    };
    let raw = (raw != shown).then_some(raw);
    (shown, raw)
}

fn object(h: &Headless, id: Id) -> Json {
    let wb = &h.app.wb;
    let c = wb.construction();
    let label = c.label(id);
    let value = c.value(id);
    let free = c.def(id).is_free();
    let (shown, raw) = value_texts(c, id);
    let mut o = json!({
        "name": label,
        "type": c.kind(id).to_string(),
        "coords": coords(c, id),
        "value": shown,
        // As in GeoGebra, a free object has no definition (its value is
        // the current one, not the line that first made it).
        "definition": if free { None } else { wb.definition(label) },
        "defined": value.is_defined(),
        "visible": wb.is_visible(id),
        "free": free,
        // How far the label was dragged from its place, in pixels.
        "label_offset": wb.label_offset(id).map_or([0.0, 0.0], |(x, y)| [x, y]),
    });
    if let Some(raw) = raw {
        o["raw_value"] = json!(raw);
    }
    o
}

/// The texts the last frame painted, with where they were painted.
fn painted_texts(h: &Headless) -> Vec<(Rect, String)> {
    fn walk(shape: &Shape, out: &mut Vec<(Rect, String)>) {
        match shape {
            Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            Shape::Text(t) => out.push((t.visual_bounding_rect(), t.galley.text().to_owned())),
            _ => {}
        }
    }
    let mut out = Vec::new();
    if let Some(o) = h.last_output() {
        for epaint::ClippedShape { shape, .. } in &o.shapes {
            walk(shape, &mut out);
        }
    }
    out
}

/// The algebra rows as the last frame painted them, in display order: the
/// first line (`head`) and what follows it (`value`: the second line of a
/// dependent object, the field of a free number). Read from the painted
/// texts, so reading them changes nothing.
fn algebra_rows(h: &Headless) -> Vec<Json> {
    let texts = painted_texts(h);
    let mut rows: Vec<Widget> = h
        .widgets()
        .into_iter()
        .filter(|w| w.kind == WidgetKind::Row)
        .collect();
    rows.sort_by(|a, b| a.rect.top().total_cmp(&b.rect.top()));
    let model = h.app.wb.current_rows();
    rows.iter()
        .map(|row| {
            let mut inside: Vec<&(Rect, String)> = texts
                .iter()
                .filter(|(r, _)| row.rect.contains(r.center()))
                .collect();
            // Reading order: lines (texts whose centres lie within 8 points
            // of the line's first one), each left to right.
            inside.sort_by(|a, b| a.0.center().y.total_cmp(&b.0.center().y));
            let mut line = 0;
            let mut line_y = f32::NEG_INFINITY;
            let mut keyed: Vec<(usize, f32, &(Rect, String))> = Vec::new();
            for t in inside {
                if t.0.center().y - line_y > 8.0 {
                    line += 1;
                    line_y = t.0.center().y;
                }
                keyed.push((line, t.0.left(), t));
            }
            keyed.sort_by(|a, b| {
                (a.0, a.1)
                    .partial_cmp(&(b.0, b.1))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            let inside: Vec<&(Rect, String)> = keyed.into_iter().map(|(_, _, t)| t).collect();
            let mut head = inside.first().map(|(_, t)| t.clone());
            let rest: Vec<&str> = inside
                .iter()
                .skip(1)
                .map(|(_, t)| t.as_str())
                .filter(|t| *t != "=")
                .collect();
            let mut value = (!rest.is_empty()).then(|| rest.join(" "));
            // A row outside the list's viewport is not painted: its texts
            // come from the row model, as the row would show them.
            if head.is_none() && !row.visible {
                if let Some(m) = model.iter().find(|m| Some(&m.label) == row.object.as_ref()) {
                    head = Some(m.head.clone());
                    value = m.value.clone().or_else(|| {
                        m.free_number.map(|n| {
                            let (n, suffix) = if m.angle {
                                (n.to_degrees(), "°")
                            } else {
                                (n, "")
                            };
                            format!(
                                "{}{suffix}",
                                geo_app::headless::egui::emath::format_with_decimals_in_range(
                                    n,
                                    0..=2
                                )
                            )
                        })
                    });
                }
            }
            json!({
                "name": row.object,
                "head": head,
                "value": value,
                "in_view": row.visible,
            })
        })
        .collect()
}

fn rect(r: geo_app::headless::egui::Rect) -> Json {
    json!([r.min.x, r.min.y, r.width(), r.height()])
}

fn label_of(h: &Headless, id: Option<Id>) -> Json {
    let c = h.app.wb.construction();
    id.filter(|id| id.index() < c.len())
        .map_or(Json::Null, |id| json!(c.label(id)))
}

/// The state of the window. `verbose` adds the algebra rows, the regions,
/// the pointer automaton and the decoration switches.
pub fn state(h: &Headless, verbose: bool) -> Json {
    let objects: Vec<Json> = h.app.wb.object_ids().map(|id| object(h, id)).collect();
    let app = &h.app;
    let tool = app.tool.tool();
    let asking = app.tool.asking().map(|r| {
        json!({"title": r.title, "prompt": r.prompt, "text": app.number_text, "degrees": r.degrees})
    });
    let view = h.view().map(|(r, t)| {
        let (ox, oy) = t.world_to_screen(0.0, 0.0);
        json!({
            "rect": rect(r),
            "scale": t.scale(),
            // The window position of the world origin (GeoGebra's xZero,
            // yZero, plus the area's offset in the window).
            "origin": [r.min.x as f64 + ox, r.min.y as f64 + oy],
        })
    });
    let camera = app
        .camera
        .map(|c| json!({"cx": c.cx, "cy": c.cy, "scale": c.scale}));
    let (w, hgt) = h.size();
    let (undo_depth, redo_depth) = app.wb.undo_depth();
    let mut s = json!({
        "window": {
            "width": w,
            "height": hgt,
            "profile": profile_name(h.profile()),
            "scale": h.scale(),
            "focused": h.focused(),
        },
        "time": h.time(),
        "frames": h.frames(),
        "objects": objects,
        "tool": {
            "id": tool_id(tool),
            "group": tool.group(),
            "picks": app.tool.picks().iter().map(|p| p.label()).collect::<Vec<_>>(),
            "hint": app.tool.hint(),
            "asking": asking,
        },
        "camera": camera,
        "view": view,
        "undo": {
            "can_undo": app.can_undo(),
            "can_redo": app.wb.can_redo(),
            "undo_depth": undo_depth,
            "redo_depth": redo_depth,
        },
        "selected": label_of(h, app.wb.selected),
        "input": {"text": app.input, "error": app.input_error},
        "tool_error": app.tool_error,
        "notice": app.wb.notice,
        "load_error": app.load_error,
        "keyboard_open": app.keyboard.open,
    });
    if verbose {
        let gesture = json!({
            "pressed": app.gesture.pressed(),
            "panning": app.gesture.panning(),
            "dragging": app.drag.map_or(Json::Null, |d| label_of(h, Some(d.id()))),
        });
        let hovered = label_of(h, app.hovered);
        // The grid step new and dragged free points are captured to, or
        // null while capturing is off.
        let snap = app.view.map(|(_, t)| app.snap(t.scale()).step());
        let decor = json!({
            "axes": app.decor.axes,
            "grid": app.decor.grid,
            "snap_step": snap.flatten(),
        });
        let style_bar = app.style_bar;
        let regions = app.regions;
        let region = |r: geo_app_core::geom::Rect| json!([r.min.x, r.min.y, r.width(), r.height()]);
        let regions = json!({
            "toolbar": region(regions.toolbar),
            "algebra": region(regions.algebra),
            "graphics": region(regions.graphics),
            "keyboard": regions.keyboard.map(region),
            "portrait": regions.portrait,
        });
        let m = s.as_object_mut().expect("an object");
        m.insert("algebra".into(), json!(algebra_rows(h)));
        m.insert("gesture".into(), gesture);
        m.insert("hovered".into(), hovered);
        m.insert("decor".into(), decor);
        m.insert("style_bar".into(), json!(style_bar));
        m.insert("regions".into(), regions);
    }
    s
}

fn kind_name(k: WidgetKind) -> &'static str {
    match k {
        WidgetKind::ToolGroup => "tool_group",
        WidgetKind::ToolEntry => "tool_entry",
        WidgetKind::ToolbarButton => "toolbar_button",
        WidgetKind::Row => "row",
        WidgetKind::RowMarker => "row_marker",
        WidgetKind::RowMenu => "row_menu",
        WidgetKind::RowValue => "row_value",
        WidgetKind::InputBar => "input_bar",
        WidgetKind::ViewButton => "view_button",
        WidgetKind::StyleButton => "style_button",
        WidgetKind::Key => "key",
        WidgetKind::HintCard => "hint_card",
        WidgetKind::HintClose => "hint_close",
        WidgetKind::Notice => "notice",
        WidgetKind::NoticeClose => "notice_close",
        WidgetKind::Other => "other",
    }
}

/// The short, stable id of a widget, before duplicates are numbered.
fn base_id(w: &Widget) -> String {
    use geo_app_core::text::{label, Label};
    let object = w.object.as_deref().unwrap_or("");
    let named = |pairs: &[(Label, &str)]| {
        pairs
            .iter()
            .find(|(l, _)| label(*l) == w.label)
            .map(|(_, id)| (*id).to_owned())
    };
    match w.kind {
        WidgetKind::ToolGroup => format!("group:{}", w.group.unwrap_or(0)),
        WidgetKind::ToolEntry => format!("tool:{}", w.tool.map(tool_id).unwrap_or_default()),
        WidgetKind::ToolbarButton => named(&[
            (Label::Undo, "undo"),
            (Label::Redo, "redo"),
            (Label::Menu, "menu"),
        ])
        .unwrap_or_else(|| format!("button:{}", w.label)),
        WidgetKind::Row => format!("row:{object}"),
        WidgetKind::RowMarker => format!("marker:{object}"),
        WidgetKind::RowMenu => format!("row_menu:{object}"),
        WidgetKind::RowValue => format!("value:{object}"),
        WidgetKind::InputBar => "input_bar".to_owned(),
        WidgetKind::ViewButton => named(&[
            (Label::ZoomIn, "zoom_in"),
            (Label::ZoomOut, "zoom_out"),
            (Label::ShowAllObjects, "fit"),
            (Label::Fullscreen, "fullscreen"),
        ])
        .unwrap_or_else(|| format!("view:{}", w.label)),
        WidgetKind::StyleButton => named(&[
            (Label::StyleBar, "style_bar"),
            (Label::Axes, "style:axes"),
            (Label::Grid, "style:grid"),
            (Label::StandardView, "style:standard_view"),
        ])
        .unwrap_or_else(|| format!("style:{}", w.label)),
        WidgetKind::Key => named(&[(Label::CloseKeyboard, "keyboard_close")])
            .unwrap_or_else(|| format!("key:{}", w.label)),
        WidgetKind::HintCard => "hint_card".to_owned(),
        WidgetKind::HintClose => "hint_close".to_owned(),
        WidgetKind::Notice => "notice".to_owned(),
        WidgetKind::NoticeClose => "notice_close".to_owned(),
        WidgetKind::Other => format!("item:{}", w.label),
    }
}

/// The widgets on screen with their ids; a repeated id gets `#2`, `#3`, ...
/// in screen order.
pub fn named_widgets(h: &Headless) -> Vec<(String, Widget)> {
    let mut seen: Vec<String> = Vec::new();
    h.widgets()
        .into_iter()
        .map(|w| {
            let base = base_id(&w);
            let n = seen.iter().filter(|s| **s == base).count();
            seen.push(base.clone());
            let id = if n == 0 {
                base
            } else {
                format!("{base}#{}", n + 1)
            };
            (id, w)
        })
        .collect()
}

pub fn widget_json(id: &str, w: &Widget) -> Json {
    let mut m = Map::new();
    m.insert("id".into(), json!(id));
    m.insert("kind".into(), json!(kind_name(w.kind)));
    m.insert("label".into(), json!(w.label));
    m.insert("rect".into(), rect(w.rect));
    m.insert("enabled".into(), json!(w.enabled));
    if let Some(t) = w.tool {
        m.insert("tool".into(), json!(tool_id(t)));
    }
    if let Some(g) = w.group {
        m.insert("group".into(), json!(g));
    }
    if let Some(o) = &w.object {
        m.insert("object".into(), json!(o));
    }
    if let Some(c) = w.checked {
        m.insert("checked".into(), json!(c));
    }
    if !w.visible {
        // An algebra row scrolled out of the list's viewport.
        m.insert("in_view".into(), json!(false));
    }
    Json::Object(m)
}

/// The counters of the last frame.
pub fn frame_stats(h: &Headless) -> Json {
    let s = h.frame_stats();
    let repaint = match s.repaint {
        Repaint::None => json!("none"),
        Repaint::Immediate => json!("immediate"),
        Repaint::After(ms) => json!({ "after_ms": ms }),
    };
    json!({
        "frame": h.frames(),
        "time": h.time(),
        "shapes": s.shapes.shapes,
        "text_shapes": s.shapes.text_shapes,
        "glyphs": s.shapes.glyphs,
        "meshes": s.meshes.meshes,
        "vertices": s.meshes.vertices,
        "indices": s.meshes.indices,
        "passes": s.passes.passes,
        "texture_updates": s.texture_updates,
        "rows_built": s.passes.rows_built,
        "rebuilds": s.passes.rebuilds,
        "repaint": repaint,
        "note": "times, allocation counts and new galleys are left out: they are not deterministic here",
    })
}

/// What can change between two moments, kept small for comparison.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    objects: Vec<(String, Json)>,
    tool: String,
    picks: Vec<String>,
    asking: bool,
    camera: Option<(f64, f64, f64)>,
    selected: Json,
    undo: (bool, bool, usize, usize),
    keyboard_open: bool,
    input: String,
    input_error: Option<String>,
    tool_error: Option<String>,
    notice: Option<String>,
    load_error: Option<String>,
    hovered: Json,
    gesture: (bool, bool, Json),
    style_bar: bool,
}

impl Snapshot {
    pub fn take(h: &Headless) -> Snapshot {
        let app = &h.app;
        let c = app.wb.construction();
        let objects = app
            .wb
            .object_ids()
            .map(|id| {
                // The value text and the definition too: a function has no
                // coordinates, and a redefinition may keep the value.
                let label = c.label(id);
                let v = json!([
                    coords(c, id),
                    app.wb.is_visible(id),
                    value_texts(c, id).0,
                    app.wb.definition(label),
                    c.kind(id).to_string(),
                ]);
                (c.label(id).to_owned(), v)
            })
            .collect();
        Snapshot {
            objects,
            tool: tool_id(app.tool.tool()),
            picks: app
                .tool
                .picks()
                .iter()
                .map(|p| p.label().to_owned())
                .collect(),
            asking: app.tool.asking().is_some(),
            camera: app.camera.map(|c| (c.cx, c.cy, c.scale)),
            selected: label_of(h, app.wb.selected),
            undo: {
                let (u, r) = app.wb.undo_depth();
                (app.can_undo(), app.wb.can_redo(), u, r)
            },
            keyboard_open: app.keyboard.open,
            input: app.input.clone(),
            input_error: app.input_error.clone(),
            tool_error: app.tool_error.clone(),
            notice: app.wb.notice.clone(),
            load_error: app.load_error.clone(),
            hovered: label_of(h, app.hovered),
            gesture: (
                app.gesture.pressed(),
                app.gesture.panning(),
                app.drag.map_or(Json::Null, |d| label_of(h, Some(d.id()))),
            ),
            style_bar: app.style_bar,
        }
    }

    /// The fields of `after` that differ from `self`.
    pub fn changes(&self, after: &Snapshot) -> Map<String, Json> {
        let mut m = Map::new();
        if self.objects != after.objects {
            let names = |s: &Snapshot| s.objects.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>();
            let (old, new) = (names(self), names(after));
            let added: Vec<&String> = new.iter().filter(|n| !old.contains(n)).collect();
            let removed: Vec<&String> = old.iter().filter(|n| !new.contains(n)).collect();
            let changed: Vec<&String> = after
                .objects
                .iter()
                .filter(|(n, v)| self.objects.iter().any(|(o, w)| o == n && w != v))
                .map(|(n, _)| n)
                .collect();
            for (key, list) in [("added", added), ("removed", removed), ("changed", changed)] {
                if !list.is_empty() {
                    m.insert(key.into(), json!(list));
                }
            }
        }
        if self.tool != after.tool {
            m.insert("tool".into(), json!(after.tool));
        }
        if self.picks != after.picks {
            m.insert("picks".into(), json!(after.picks));
        }
        if self.asking != after.asking {
            m.insert("asking".into(), json!(after.asking));
        }
        if self.camera != after.camera {
            if let Some((cx, cy, scale)) = after.camera {
                m.insert("camera".into(), json!({"cx": cx, "cy": cy, "scale": scale}));
            }
        }
        if self.selected != after.selected {
            m.insert("selected".into(), after.selected.clone());
        }
        if self.undo != after.undo {
            let (u, r, ud, rd) = after.undo;
            m.insert(
                "undo".into(),
                json!({"can_undo": u, "can_redo": r, "undo_depth": ud, "redo_depth": rd}),
            );
        }
        if self.keyboard_open != after.keyboard_open {
            m.insert("keyboard_open".into(), json!(after.keyboard_open));
        }
        if self.input != after.input {
            m.insert("input".into(), json!(after.input));
        }
        for (key, a, b) in [
            ("input_error", &self.input_error, &after.input_error),
            ("tool_error", &self.tool_error, &after.tool_error),
            ("notice", &self.notice, &after.notice),
            ("load_error", &self.load_error, &after.load_error),
        ] {
            if a != b {
                m.insert(key.into(), json!(b));
            }
        }
        if self.hovered != after.hovered {
            m.insert("hovered".into(), after.hovered.clone());
        }
        if self.gesture != after.gesture {
            let (pressed, panning, dragging) = &after.gesture;
            m.insert(
                "gesture".into(),
                json!({"pressed": pressed, "panning": panning, "dragging": dragging}),
            );
        }
        if self.style_bar != after.style_bar {
            m.insert("style_bar".into(), json!(after.style_bar));
        }
        m
    }
}
