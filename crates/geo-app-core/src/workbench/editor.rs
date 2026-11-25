use super::Workbench;
use crate::editor::ImageRequest;
use crate::labels::{next_label, next_numbered};
use geo_kernel::kernel::image::{ImageCorner, ImagePlacement, ImageStyle};
use geo_kernel::script::image;
use geo_kernel::script::saved::Property;
use geo_kernel::{Id, Kind, Value};
use std::sync::Arc;

impl Workbench {
    pub fn recognize_stroke(
        &mut self,
        points: &[(f64, f64)],
        view: &crate::interact::ViewTransform,
        function_only: bool,
    ) -> Result<Option<Id>, String> {
        let Some((lines, hidden)) =
            crate::tools::freehand::recognize(self.construction(), points, view, function_only)
        else {
            return Ok(None);
        };
        self.run_tool_lines(&lines)?;
        for label in hidden {
            self.log.hidden.insert(label);
        }
        self.end_tool_use();
        Ok(self.construction().ids().last())
    }

    pub(crate) fn insert_image(
        &mut self,
        request: &ImageRequest,
        bytes: Vec<u8>,
    ) -> Result<Id, String> {
        let c = self.construction();
        let label = next_numbered("pic", |name| {
            c.id(name).is_some() || c.has_image_asset(name)
        });
        let name = Arc::<str>::from(label.as_str());
        let mut lines = Vec::new();
        let (first, (x, y)) = match &request.anchor {
            Some(anchor) => {
                let Some(Value::Point { x, y }) = c.id(anchor).map(|id| c.value(id)) else {
                    return Err("image anchor is unavailable".into());
                };
                (anchor.clone(), (x, y))
            }
            None => {
                let (x, y) = request.at;
                let label = next_label(Kind::Point, |label| c.id(label).is_some());
                lines.push(format!("{label} = ({x}, {y})"));
                (label, (x, y))
            }
        };

        self.construction.reserve_image_input(bytes.len() as u64)?;
        let asset = self
            .construction
            .decode_image_asset(name.clone(), bytes)?
            .expect("asset name was unused");

        self.begin_tool_use();
        self.log.record_saved_property(Property::ImageAsset {
            name,
            asset: asset.clone(),
        });
        self.tool_use.as_mut().expect("tool use open").changed = true;
        let [width, height] = asset.size().map(f64::from);
        let natural_width = width / request.scale;
        let right = x + natural_width;
        let right = if right > request.xmax && request.xmax > x {
            x + 0.9 * (request.xmax - x)
        } else {
            right
        };
        let available_height = request.ymax - y;
        let right = if available_height > 0.0 {
            right.min(x + available_height * width / height)
        } else {
            right
        };
        let second = next_label(Kind::Point, |label| {
            label == first || self.construction.id(label).is_some()
        });
        lines.push(format!("{second} = ({right}, {y})"));
        let definition = image::definition(
            &label,
            ImagePlacement::World {
                centered: false,
                screen_coords: false,
                corners: [
                    Some(ImageCorner::Point(first.as_str())),
                    Some(ImageCorner::Point(second.as_str())),
                    None,
                ],
            },
            ImageStyle::default(),
        );
        lines.push(format!("{label} = {definition}"));
        if let Err(error) = self.run_tool_lines(&lines) {
            self.cancel_tool_use();
            return Err(error);
        }
        self.end_tool_use();
        Ok(self.construction.id(&label).expect("image was placed"))
    }
}
