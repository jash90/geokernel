//! Editor actions shared between the window and the workbench.

use crate::app::App;
use crate::interact::ViewTransform;
use crate::tools::Tool;
use crate::workbench::Workbench;
use geo_kernel::{expr, Id, Kind, Value};
use std::cell::RefCell;

pub const MAX_INSPECTOR_ROWS: usize = 256;
const MAX_INSPECTOR_WORK: u64 = 65_536;

#[derive(Clone, Debug)]
pub struct Inspector {
    pub label: String,
    pub interval: [f64; 3],
    cached: RefCell<Option<InspectorRows>>,
}

#[derive(Clone, Debug)]
struct InspectorRows {
    object: Id,
    version: u64,
    interval: [u64; 3],
    values: Vec<(f64, f64)>,
}

impl Inspector {
    pub fn rows(&self, wb: &Workbench, min: f64, max: f64, step: f64) -> Vec<(f64, f64)> {
        let c = wb.construction();
        let Some(id) = c.id(&self.label).filter(|&id| c.kind(id) == Kind::Function) else {
            return Vec::new();
        };
        if !(min.is_finite() && max.is_finite() && step.is_finite() && min <= max && step > 0.0) {
            return Vec::new();
        }
        let interval = [min, max, step].map(f64::to_bits);
        let version = wb.version();
        let mut cached = self.cached.borrow_mut();
        if let Some(rows) = cached.as_ref().filter(|rows| {
            rows.object == id && rows.version == version && rows.interval == interval
        }) {
            return rows.values.clone();
        }
        let start = expr::work_done();
        let values = expr::with_evaluation_budget(MAX_INSPECTOR_WORK, || {
            let mut values = Vec::new();
            for index in 0..MAX_INSPECTOR_ROWS {
                let x = min + index as f64 * step;
                // The final multiply/add can round one representable number
                // past the endpoint. Keep the endpoint once.
                if x > max.next_up() || !x.is_finite() {
                    break;
                }
                let x = x.min(max);
                let y = c.function_value(id, x).unwrap_or(f64::NAN);
                values.push((x, y));
                if x == max || expr::work_done() - start >= MAX_INSPECTOR_WORK {
                    break;
                }
            }
            values
        });
        *cached = Some(InspectorRows {
            object: id,
            version,
            interval,
            values: values.clone(),
        });
        values
    }
}

#[derive(Clone, Debug)]
pub struct ImageRequest {
    pub anchor: Option<String>,
    pub at: (f64, f64),
    pub scale: f64,
    pub xmax: f64,
    pub ymax: f64,
}

impl App {
    pub fn editor_click(&mut self, hit: Option<Id>, view: &ViewTransform, pos: (f64, f64)) -> bool {
        let c = self.wb.construction();
        let hit = hit.filter(|id| id.index() < c.len());
        match self.tool.tool() {
            Tool::ShowHideLabel => {
                if let Some(id) = hit {
                    self.wb.toggle_label_in_mode(id);
                }
            }
            Tool::CopyVisualStyle => {
                if let Some(id) = hit {
                    match self.style_source.as_deref().and_then(|label| c.id(label)) {
                        Some(source) if source == id => {
                            self.style_source = None;
                            self.select_only(None);
                            self.wb.finish_style_edits();
                        }
                        Some(source) => self.wb.copy_drawing_style(source, id),
                        None => {
                            self.style_source = Some(c.label(id).to_owned());
                            if let Some(target) = self.wb.selected.filter(|&target| {
                                target != id
                                    && target.index() < c.len()
                                    && c.kind(target) == c.kind(id)
                            }) {
                                self.wb.copy_drawing_style(id, target);
                                self.select_only(None);
                            } else {
                                self.select_only(Some(id));
                            }
                        }
                    }
                }
            }
            Tool::FunctionInspector => {
                if let Some(id) = hit.filter(|&id| c.kind(id) == Kind::Function) {
                    let center = view.screen_to_world(pos.0, pos.1).0;
                    let step =
                        crate::ticks::numbering_step(view.xscale(), self.metrics.numbering_px)
                            * 0.25;
                    let inspector = Inspector {
                        label: c.label(id).to_owned(),
                        interval: [center - 4.0 * step, center + 4.0 * step, step],
                        cached: RefCell::default(),
                    };
                    self.select_tool(Tool::Move);
                    self.inspector = Some(inspector);
                }
            }
            Tool::Image => {
                if self.image_request.is_none() {
                    let anchor = hit
                        .filter(|&id| c.kind(id) == Kind::Point)
                        .map(|id| c.label(id).to_owned());
                    let v = view.view();
                    self.image_request = Some(ImageRequest {
                        anchor,
                        at: view.screen_to_world(pos.0, pos.1),
                        scale: view.xscale(),
                        xmax: v.xmax,
                        ymax: v.ymax,
                    });
                    self.image_path.clear();
                }
            }
            _ => return false,
        }
        true
    }

    pub fn finish_image(&mut self, bytes: Vec<u8>) -> Result<(), String> {
        let request = self.image_request.as_ref().ok_or("no image placement")?;
        let id = self.wb.insert_image(request, bytes)?;
        self.select_tool(Tool::Move);
        self.select_only(Some(id));
        Ok(())
    }

    pub fn rotation_press(
        &mut self,
        hit: Option<Id>,
        at: (f64, f64),
    ) -> Option<crate::interact::Drag> {
        let id = hit?;
        let c = self.wb.construction();
        let center = self
            .rotation_center
            .as_deref()
            .and_then(|label| c.id(label));
        match center {
            Some(center) if center == id => {
                self.rotation_center = None;
                self.select_only(None);
                None
            }
            Some(center) => {
                let Value::Point { x, y } = c.value(center) else {
                    return None;
                };
                crate::interact::Drag::rotation(c, id, center, (x, y), at)
            }
            None if c.kind(id) == Kind::Point => {
                self.rotation_center = Some(c.label(id).to_owned());
                self.select_only(Some(id));
                None
            }
            None => None,
        }
    }
}
