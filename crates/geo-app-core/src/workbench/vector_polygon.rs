//! A vector polygon stores each later vertex as an offset from its anchor.

use super::Workbench;
use crate::describe::exact;
use crate::labels::{next_label, polygon_label};
use crate::tools::Pick;
use geo_kernel::kernel::{Id, Kind, Value};

impl Workbench {
    /// Only free points without dependents can be redefined as offsets.
    pub(crate) fn can_redefine_vertex(&self, id: Id) -> bool {
        let c = self.construction();
        c.kind(id) == Kind::Point && c.def(id).is_free() && c.direct_dependents(id).next().is_none()
    }

    /// Redefines the chosen vertices, or uses independent copies of a
    /// polygon's vertices. Publish the new construction only after all
    /// definitions have run successfully.
    pub(crate) fn make_vector_polygon(
        &mut self,
        picks: &[Pick],
        copy_offset: f64,
    ) -> Result<Id, String> {
        let c = self.construction();
        let find = |pick: &Pick| c.id(pick.label()).ok_or_else(|| pick.label().to_owned());
        let source = picks.first().ok_or("missing polygon vertices")?;
        let copy = source.kind() == Kind::Polygon;
        let vertices: Vec<_> = if copy {
            c.vertices(find(source)?)
                .map(|v| v.ok_or("undefined polygon vertex"))
                .collect::<Result<_, _>>()?
        } else {
            picks
                .iter()
                .map(|p| match c.value(find(p)?) {
                    Value::Point { x, y } => Ok((x, y)),
                    _ => Err("undefined polygon vertex".to_owned()),
                })
                .collect::<Result<_, _>>()?
        };
        let Some((&(x, y), rest)) = vertices.split_first() else {
            return Err("missing polygon vertices".into());
        };

        let mut log = self.copy(&self.log);
        if !copy {
            for p in &picks[1..] {
                if !self.can_redefine_vertex(find(p)?) || !log.remove(p.label(), c) {
                    return Err(format!("cannot redefine vertex {}", p.label()));
                }
            }
        }
        let (mut draft, mut log) = self.replay(&log)?;
        let mut define = |line: String| -> Result<Id, String> {
            let before = draft.len();
            let id =
                geo_kernel::script::exec(&mut draft, &line)?.ok_or("missing polygon output")?;
            log.record(&draft, before, &line);
            Ok(id)
        };
        // Reserve the old labels even while their vertices are being rebuilt.
        let mut names = std::collections::HashSet::new();
        let mut fresh = |kind| {
            let name = next_label(kind, |l| c.id(l).is_some() || names.contains(l));
            names.insert(name.clone());
            name
        };
        let anchor = if copy {
            let label = fresh(Kind::Point);
            define(format!(
                "{label} = ({}, {})",
                exact(x + copy_offset),
                exact(y - copy_offset)
            ))?;
            label
        } else {
            source.label().to_owned()
        };
        let mut labels = vec![anchor.clone()];
        for (i, &(vx, vy)) in rest.iter().enumerate() {
            let a = fresh(Kind::Number);
            let b = fresh(Kind::Number);
            define(format!("{a} = {}", exact(vx - x)))?;
            define(format!("{b} = {}", exact(vy - y)))?;
            let label = if copy {
                fresh(Kind::Point)
            } else {
                picks[i + 1].label().to_owned()
            };
            define(format!("{label} = ({a} + x({anchor}), {b} + y({anchor}))"))?;
            labels.push(label);
        }
        let polygon = polygon_label(labels.len(), |l| c.id(l).is_some());
        let id = define(format!("{polygon} = Polygon({})", labels.join(", ")))?;
        self.begin_tool_use();
        self.install_replay(draft, log);
        self.tool_use.as_mut().unwrap().changed = true;
        Ok(id)
    }
}
