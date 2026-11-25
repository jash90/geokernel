// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/EuclidianController.java
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/modes/ModeDeleteLocus.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoDispatcher.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! How a construction tool takes the pointer: it picks when the button goes
//! down, a drag-creating tool takes its second point at the release, and
//! the new points go on the objects under the pointer. No UI types: the
//! window hands in positions in the drawing area and the view transform it
//! painted.
//!
//! - The press picks at once (`switchModeForMousePressedND`; spec E1). A
//!   new point is resolved by `new_point_at` (spec E2).
//! - The line, segment, ray, vector, circle and regular polygon tools also
//!   make their object by press, drag and release
//!   (`isModeCreatingObjectsByDrag`; spec E3): with the first point picked,
//!   a press waits for its release, and a release that ends a drag away
//!   from the press (beyond the point selection distance) picks there.
//! - The point tool's press makes or finds a point that a drag then moves
//!   (`wrapMouseDraggedND`; spec E4).
//! - The delete tool's drag beyond the click threshold is an eraser: every
//!   object that a square of 20 px around the pointer meets is deleted, the
//!   whole stroke one undo step (`ModeDeleteLocus.handleMouseDraggedForDelete`,
//!   `storeUndoAfterDrag`; spec E11).

use crate::app::App;
use crate::hit::{self, Reach};
use crate::interact::{self, ViewTransform};
use crate::tools::{
    creates_by_drag, intersection_pair, new_points_on_objects, NewPoint, Outcome, Pick, Tool,
    MAX_RIGID_VERTICES,
};
use geo_kernel::kernel::{Id, Kind, Value};

/// Polygon preparation and vertex reads allowed for one constraint pick.
const CONSTRAINT_VERTEX_WORK: usize = hit::SHAPE_HIT_WORK;

/// Label probes allowed for the outputs of one Roots or Extremum click.
const ANALYSIS_LABEL_WORK: usize = 4096;

fn rigid_copy_size(c: &geo_kernel::Construction, polygon: Id) -> Option<usize> {
    let mut left = CONSTRAINT_VERTEX_WORK;
    let count = c.vertices_with_work(polygon, &mut left)?.size_hint().0;
    (3..=MAX_RIGID_VERTICES).contains(&count).then_some(count)
}

/// The side of the eraser's square, in pixels
/// (`EuclidianConstants.DEFAULT_ERASER_SIZE`).
pub const ERASER_SIZE: f64 = 20.0;

/// What a construction tool's press did, kept until its release.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToolPress {
    /// Where the press was, in the drawing area.
    pub at: (f64, f64),
    /// The press picks nothing: a drag-creating tool with its first point
    /// waits for the release.
    pub deferred: bool,
    /// The press picked or placed something (a drag after it does not pan).
    pub acted: bool,
    /// The point the press made or found with the point tool, which a drag
    /// moves.
    pub point: Option<Id>,
    attachment: Option<Id>,
}

/// The kinds a new point can be put on (`Path` in the reference; polygons
/// are left out, `hits.removePolygons`, their sides are segments). A conic
/// of any type is a path there, hit by its stroked outline
/// (`PATH_NO_FILL_HIT`).
fn is_path(c: &geo_kernel::kernel::Construction, id: Id) -> bool {
    use geo_kernel::algo::arc::ArcDef;
    use geo_kernel::kernel::Def;
    match c.kind(id) {
        Kind::Line | Kind::Segment | Kind::Ray | Kind::Circle | Kind::Conic | Kind::PolyLine => {
            true
        }
        Kind::Arc | Kind::Sector => match c.def(id) {
            Def::Arc(ArcDef::ConicParameters { conic, .. }) => c.kind(conic) == Kind::Circle,
            _ => true,
        },
        _ => false,
    }
}

impl App {
    /// Whether the object `id` is drawn (and so can be hit); the show/hide
    /// tool shows hidden objects too.
    fn hittable(&self, id: Id) -> bool {
        self.tool.tool() == Tool::ShowHide || self.wb.is_visible(id)
    }

    /// What a new point made by a click at `pos` (in the area, `t` the view
    /// it was made in) is (`EuclidianController.getNewPoint`; spec E2): the
    /// crossing of the first two paths under the pointer when the kernel
    /// intersects them and the point is defined
    /// (`getSingleIntersectionPoint`, of two points the one nearer the
    /// pointer), else a point on the path under the pointer (`chooseGeo`
    /// among the path hits), else a free point; the position is captured to
    /// the grid (`transformCoords`). Only the tools that allow it put points
    /// on objects (`new_points_on_objects`). A point under the pointer is
    /// not this function's business: the tool picks it.
    pub fn new_point_at(&mut self, t: &ViewTransform, pos: (f64, f64), reach: &Reach) -> NewPoint {
        let (x, y) = self
            .snap(t.xscale(), t.yscale())
            .apply(t.screen_to_world(pos.0, pos.1));
        if !new_points_on_objects(self.tool.tool(), self.tool.picks()) {
            return NewPoint::Free(x, y);
        }
        let c = self.wb.construction();
        let paths = hit::hits(c, t, pos, reach, |id| {
            self.hittable(id) && (is_path(c, id) || c.kind(id) == Kind::Conic)
        });
        if let [a, b, ..] = paths[..] {
            if let Some(two) = intersection_pair(c.kind(a), c.kind(b)) {
                let (a, b) = (c.label(a).to_owned(), c.label(b).to_owned());
                let raw = t.screen_to_world(pos.0, pos.1);
                if let Some(index) = self.nearest_intersection(&a, &b, two, raw) {
                    return NewPoint::Intersection { a, b, index };
                }
            }
        }
        let c = self.wb.construction();
        if self.tool.tool() == Tool::PointOnObject {
            if let Some((object, region)) = self.constraint_at(t, pos, reach).or_else(|| {
                self.decor
                    .axes
                    .then(|| {
                        ["xAxis", "yAxis"]
                            .into_iter()
                            .filter_map(|name| c.id(name))
                            .find(|&id| hit::hit(c, id, t, pos, reach.threshold, reach).is_some())
                    })
                    .flatten()
                    .map(|axis| (axis, false))
            }) {
                let label = c.label(object).to_owned();
                return if region {
                    NewPoint::InRegion {
                        region: label,
                        x,
                        y,
                    }
                } else {
                    NewPoint::OnPath { path: label, x, y }
                };
            }
            return NewPoint::Free(x, y);
        }
        let path = hit::pick(c, t, pos, reach, None, |id| {
            self.hittable(id)
                && (is_path(c, id)
                    || self.tool.tool() == Tool::SegmentFixed
                        && c.kind(id) == Kind::Function
                        && geo_kernel::algo::path::function_is_path(c, id))
        });
        match path {
            Some(p) => NewPoint::OnPath {
                path: c.label(p).to_owned(),
                x,
                y,
            },
            None => NewPoint::Free(x, y),
        }
    }

    /// The intersection of `a` and `b` to use: `Some(None)` for the one
    /// point of two lines, `Some(Some(n))` for the defined point of two that
    /// is nearer `(x, y)`, `None` when none is defined (`!showInEuclidianView`).
    fn nearest_intersection(
        &mut self,
        a: &str,
        b: &str,
        two: bool,
        (x, y): (f64, f64),
    ) -> Option<Option<u8>> {
        if !two {
            return self
                .wb
                .probe_point(&format!("Intersect({a}, {b})"))
                .map(|_| None);
        }
        let mut best: Option<(f64, u8)> = None;
        for n in [1u8, 2] {
            if let Some((px, py)) = self.wb.probe_point(&format!("Intersect({a}, {b}, {n})")) {
                let d = (px - x).hypot(py - y);
                if best.is_none_or(|(bd, _)| d < bd) {
                    best = Some((d, n));
                }
            }
        }
        best.map(|(_, n)| Some(n))
    }

    fn constraint_at(
        &self,
        t: &ViewTransform,
        pos: (f64, f64),
        reach: &Reach,
    ) -> Option<(Id, bool)> {
        let c = self.wb.construction();
        let polygon = || {
            hit::pick(c, t, pos, reach, None, |id| {
                self.hittable(id) && c.kind(id) == Kind::Polygon
            })
        };
        let carrier = hit::pick(c, t, pos, reach, None, |id| {
            self.hittable(id)
                && (is_path(c, id)
                    || c.kind(id) == Kind::Vector
                    || c.kind(id) == Kind::Function
                        && geo_kernel::algo::path::function_is_path(c, id))
        });
        let polygon = carrier
            .is_none_or(|id| c.kind(id) == Kind::Segment)
            .then(polygon)
            .flatten();
        let polygon_boundary =
            polygon.is_some_and(|id| hit::polygon_boundary(c, id, t, pos, reach));
        let object = match (carrier, polygon) {
            (Some(_), Some(polygon)) if polygon_boundary => polygon,
            (Some(carrier), _) => carrier,
            (None, polygon) => polygon?,
        };
        let region = match c.kind(object) {
            Kind::Polygon => !polygon_boundary || self.tool.tool() == Tool::AttachDetach,
            Kind::Sector | Kind::Arc => {
                hit::hit(c, object, t, pos, reach.threshold, reach) == Some(hit::HitType::Filling)
            }
            _ => false,
        };
        Some((object, region))
    }

    fn attach_detach_click(
        &mut self,
        t: &ViewTransform,
        pos: (f64, f64),
        reach: &Reach,
    ) -> Outcome {
        use geo_kernel::algo::path::PathDef;
        use geo_kernel::Def;

        let c = self.wb.construction();
        let point = hit::pick(c, t, pos, reach, None, |id| {
            self.hittable(id) && c.kind(id) == Kind::Point
        });
        let existing = self.tool.picks().first().cloned();
        if let Some(id) = point {
            let label = c.label(id).to_owned();
            if matches!(
                c.def(id),
                Def::Path(
                    PathDef::On { .. }
                        | PathDef::OnParam { .. }
                        | PathDef::InRegion { .. }
                        | PathDef::FixedEndpoint { .. }
                )
            ) {
                let Value::Point { x, y } = c.value(id) else {
                    return Outcome::Ignored;
                };
                self.tool.reset();
                return Outcome::Run(vec![format!(
                    "{label} = $point({}, {})",
                    x + 20.0 / t.xscale(),
                    y + 20.0 / t.yscale()
                )]);
            }
            if !matches!(c.def(id), Def::FreePoint { .. }) {
                return Outcome::Ignored;
            }
            if let Some(first) = existing.as_ref().filter(|pick| pick.kind() != Kind::Point) {
                return self.attach_to(
                    t,
                    pos,
                    &label,
                    first.label(),
                    self.tool.attachment_region(),
                );
            }
            if existing.is_none_or(|pick| pick.label() == label) {
                return self.tool.set_attachment_pick(
                    Pick::Object {
                        label,
                        kind: Kind::Point,
                    },
                    false,
                );
            }
            return Outcome::Ignored;
        }
        let Some((id, region)) = self.constraint_at(t, pos, reach) else {
            return Outcome::Ignored;
        };
        let label = c.label(id).to_owned();
        if let Some(first) = existing.as_ref().filter(|pick| pick.kind() == Kind::Point) {
            return self.attach_to(t, pos, first.label(), &label, region);
        }
        if existing.is_none_or(|pick| pick.label() == label) {
            let kind = c.kind(id);
            return self
                .tool
                .set_attachment_pick(Pick::Object { label, kind }, region);
        }
        Outcome::Ignored
    }

    fn attach_detach_drag(
        &mut self,
        t: &ViewTransform,
        pos: (f64, f64),
        reach: &Reach,
        point: Id,
    ) -> Outcome {
        use geo_kernel::algo::path::PathDef;
        use geo_kernel::Def;

        let c = self.wb.construction();
        let label = c.label(point).to_owned();
        let attached = match c.def(point) {
            Def::Path(PathDef::On { path } | PathDef::OnParam { path, .. }) => Some(path),
            Def::Path(PathDef::InRegion { region }) => Some(region),
            Def::FreePoint { .. } => None,
            _ => return Outcome::Ignored,
        };
        if let Some((id, region)) = self.constraint_at(t, pos, reach) {
            if Some(id) != attached {
                let target = c.label(id).to_owned();
                return self.attach_to(t, pos, &label, &target, region);
            }
        } else if attached.is_some() {
            let (x, y) = t.screen_to_world(pos.0, pos.1);
            self.tool.reset();
            return Outcome::Run(vec![format!("{label} = $point({x}, {y})")]);
        }
        Outcome::Ignored
    }

    fn attach_to(
        &mut self,
        t: &ViewTransform,
        pos: (f64, f64),
        point: &str,
        target: &str,
        region: bool,
    ) -> Outcome {
        let command = if region { "PointIn" } else { "Point" };
        let line = format!("{point} = {command}({target})");
        let (x, y) = t.screen_to_world(pos.0, pos.1);
        if self.wb.run_tool_lines(&[line]).is_err() {
            return Outcome::Ignored;
        }
        let id = self
            .wb
            .construction()
            .id(point)
            .expect("point identity is preserved");
        self.wb.move_pending_point(id, x, y);
        self.tool.reset();
        self.tool.remember_attachment(pos);
        Outcome::Select(point.to_owned())
    }

    fn tool_pick_kind(&self, id: Id) -> Kind {
        let c = self.wb.construction();
        let kind = c.kind(id);
        if self.tool.tool() != Tool::Compasses {
            return kind;
        }
        let circular = match kind {
            Kind::Conic => c
                .conic(id)
                .is_some_and(|d| d.ty == geo_kernel::ConicType::Circle),
            Kind::Arc | Kind::Sector => c
                .arc(id)
                .is_some_and(|part| part.conic.ty == geo_kernel::ConicType::Circle),
            _ => false,
        };
        if circular {
            Kind::Circle
        } else {
            kind
        }
    }

    /// Whether the object can participate in the active tool's next pick.
    pub fn accepts_tool_hit(&self, id: Id) -> bool {
        let c = self.wb.construction();
        let tool = self.tool.tool();
        if tool == Tool::MoveRotate {
            return match self
                .rotation_center
                .as_deref()
                .and_then(|label| c.id(label))
            {
                Some(center) => id == center || interact::can_rotate(c, id, center),
                None => c.kind(id) == Kind::Point,
            };
        }
        if tool == Tool::AngleFixed
            && self.tool.picks().is_empty()
            && c.segment_endpoints(id).is_some()
        {
            return true;
        }
        if matches!(tool, Tool::PointOnObject | Tool::AttachDetach)
            && (is_path(c, id) || matches!(c.kind(id), Kind::Polygon | Kind::Function))
        {
            return true;
        }
        if tool == Tool::AttachDetach {
            return c.kind(id) == Kind::Point && interact::movable(c, id);
        }
        if tool == Tool::Locus {
            let point = c.kind(id) == Kind::Point && !c.def(id).is_free();
            let on_path = |id| {
                matches!(
                    c.def(id),
                    geo_kernel::Def::Path(
                        geo_kernel::algo::path::PathDef::On { .. }
                            | geo_kernel::algo::path::PathDef::OnParam { .. }
                    )
                )
            };
            let driver = on_path(id) || c.has_slider_bounds(id);
            let possible = match self
                .tool
                .picks()
                .first()
                .and_then(|pick| c.id(pick.label()))
            {
                Some(first) if on_path(first) || c.has_slider_bounds(first) => {
                    point && !on_path(id)
                }
                Some(_) => driver,
                None => point || driver,
            };
            if !possible {
                return false;
            }
        }
        self.tool
            .accepts_object(self.wb.construction().label(id), self.tool_pick_kind(id))
    }

    /// One click of a construction tool at `pos`, with the picks it allows.
    fn tool_click(&mut self, t: &ViewTransform, pos: (f64, f64), reach: &Reach) -> Outcome {
        let tool = self.tool.tool();
        let c = self.wb.construction();
        if tool == Tool::AttachDetach {
            let outcome = self.attach_detach_click(t, pos, reach);
            self.apply_outcome(outcome.clone());
            return outcome;
        }
        let analysis = matches!(tool, Tool::Extremum | Tool::Roots);
        let mut id = None;
        if analysis {
            for kinds in [&[Kind::Function][..], &[Kind::Conic, Kind::Circle][..]] {
                id = hit::pick(c, t, pos, reach, None, |id| {
                    self.hittable(id) && kinds.contains(&c.kind(id))
                });
                if id.is_some() {
                    break;
                }
            }
        }
        if id.is_none() {
            let text = if tool == Tool::Relation {
                self.text_at(pos, None, |id| self.hittable(id))
            } else {
                None
            };
            let (picked, refused) =
                hit::pick_with_shape_limit(c, t, pos, reach, None, text, |id| self.hittable(id));
            if refused
                && matches!(tool, Tool::PointOnObject | Tool::RigidPolygon)
                && picked.is_none_or(|id| c.kind(id) == Kind::List)
            {
                return Outcome::Ignored;
            }
            id = picked;
        }
        let accepts = |id| self.accepts_tool_hit(id);
        if matches!(
            tool,
            Tool::Area
                | Tool::Slope
                | Tool::Intersect
                | Tool::Parabola
                | Tool::VectorFromPoint
                | Tool::Locus
                | Tool::Extremum
                | Tool::Roots
        ) && id.is_none_or(|id| !accepts(id))
        {
            id = hit::pick(c, t, pos, reach, None, |id| {
                self.hittable(id) && accepts(id)
            })
            .or(id);
        }
        let picks = self.tool.picks();
        if let Some(id) = id {
            if tool == Tool::Compasses
                && !accepts(id)
                && matches!(
                    self.tool_pick_kind(id),
                    Kind::Conic | Kind::Arc | Kind::Sector
                )
                && (picks.is_empty() || matches!(picks, [point] if point.kind() == Kind::Point))
            {
                self.tool.reset();
                self.apply_outcome(Outcome::SelectionChanged);
                return Outcome::SelectionChanged;
            }
            if tool == Tool::Compasses && !accepts(id) && !new_points_on_objects(tool, picks) {
                // The radius comes from existing objects, unless the click is empty.
                return Outcome::Ignored;
            }
            if tool == Tool::AngleFixed
                && matches!(c.kind(id), Kind::Segment | Kind::Circle | Kind::Conic)
            {
                let Some((start, end)) = c.segment_endpoints(id) else {
                    return Outcome::Ignored;
                };
                self.tool.reset();
                for point in [end, start] {
                    let outcome = self.tool.click(
                        Some((c.label(point), Kind::Point)),
                        (0.0, 0.0),
                        &|label| c.id(label).is_some(),
                    );
                    if matches!(outcome, Outcome::AskNumber(_)) {
                        self.apply_outcome(outcome.clone());
                        return outcome;
                    }
                }
                return Outcome::Ignored;
            }
            if tool == Tool::RigidPolygon
                && c.kind(id) == Kind::Polygon
                && (!picks.is_empty() || rigid_copy_size(c, id).is_none())
            {
                return Outcome::Ignored;
            }
            if matches!(tool, Tool::VectorPolygon | Tool::RigidPolygon)
                && !picks.is_empty()
                && c.kind(id) == Kind::Point
                && picks
                    .first()
                    .is_some_and(|first| first.label() != c.label(id))
                && !self.wb.can_redefine_vertex(id)
            {
                return Outcome::Ignored;
            }
            if tool == Tool::VectorPolygon
                && !picks.is_empty()
                && !matches!(c.kind(id), Kind::Point | Kind::Polygon)
            {
                return Outcome::Ignored;
            }
            if tool == Tool::Parabola
                && matches!(
                    self.tool_pick_kind(id),
                    Kind::Line | Kind::Segment | Kind::Ray
                )
                && !accepts(id)
            {
                return Outcome::Ignored;
            }
        }
        if tool == Tool::Locus
            && id.is_some_and(|id| !crate::tools::locus_pick_is_valid(c, picks.first(), id))
        {
            return Outcome::Ignored;
        }
        let hit = id.map(|id| (c.label(id).to_owned(), self.tool_pick_kind(id)));
        let new_point = if self.tool.accepts_new_point()
            && id.is_none_or(|id| {
                c.kind(id) != Kind::Point && (tool == Tool::PointOnObject || !accepts(id))
            }) {
            self.new_point_at(t, pos, reach)
        } else {
            NewPoint::Free(0.0, 0.0)
        };
        let c = self.wb.construction();
        let label_probes = std::cell::Cell::new(0);
        let is_taken = |label: &str| {
            let used = label_probes.get();
            if analysis {
                label_probes.set(used + 1);
            }
            (!analysis || used < ANALYSIS_LABEL_WORK) && c.id(label).is_some()
        };
        let mut outcome = self.tool.click_with(
            hit.as_ref().map(|(l, k)| (l.as_str(), *k)),
            &new_point,
            &is_taken,
        );
        if let Outcome::VectorPolygon { copy_offset, .. } = &mut outcome {
            *copy_offset = t.view().xmax / 15.0;
        }
        if analysis {
            if label_probes.get() > ANALYSIS_LABEL_WORK {
                return Outcome::Ignored;
            }
            if let Outcome::Run(lines) = &mut outcome {
                let (_, rhs) = lines[0]
                    .split_once(" = ")
                    .expect("tool recipe is an assignment");
                let slots = if hit
                    .as_ref()
                    .is_some_and(|(_, kind)| matches!(kind, Kind::Circle | Kind::Conic))
                {
                    geo_kernel::script::initial_output_slots(c, rhs)
                } else {
                    geo_kernel::script::multi_output_count(c, rhs)
                        .and_then(Result::ok)
                        .map(|count| vec![true; count])
                };
                if let Some(slots) = slots.filter(|slots| slots.len() > 1) {
                    let mut free_labels = crate::labels::free_point_labels(is_taken);
                    let names = slots
                        .into_iter()
                        .map(|named| {
                            if named {
                                free_labels.next().expect("point labels")
                            } else {
                                String::new()
                            }
                        })
                        .collect::<Vec<_>>();
                    if label_probes.get() > ANALYSIS_LABEL_WORK {
                        return Outcome::Ignored;
                    }
                    lines[0] = format!("{{{}}} = {rhs}", names.join(", "));
                }
            }
        }
        if let Outcome::RigidCopy(label) = outcome {
            let c = self.wb.construction();
            let Some(size) = c.id(&label).and_then(|id| rigid_copy_size(c, id)) else {
                return Outcome::Ignored;
            };
            let name = crate::labels::polygon_label(size, |candidate| c.id(candidate).is_some());
            let offset = t.view().xmax / 15.0;
            outcome = Outcome::Run(vec![format!(
                "{name} = RigidPolygon({label}, {offset}, {})",
                -offset
            )]);
        }
        let shown = outcome.clone();
        self.apply_outcome(outcome);
        shown
    }

    /// A press of a construction tool (one that builds from picks) at `pos`
    /// in the area, in the view `t` the user saw. Picks at once, except the
    /// second point of a drag-creating tool, which waits for the release.
    pub fn press_tool(&mut self, t: &ViewTransform, pos: (f64, f64), reach: &Reach) -> ToolPress {
        let tool = self.tool.tool();
        let mut press = ToolPress {
            at: pos,
            deferred: false,
            acted: false,
            point: None,
            attachment: None,
        };
        if tool == Tool::AttachDetach {
            self.tool.observe_attachment_pointer(pos);
            if self.tool.attachment_at(pos) {
                // A replacement under the same pointer position does not
                // become a new detach hit until that pointer moves.
                press.acted = true;
                return press;
            }
            let c = self.wb.construction();
            press.attachment = hit::pick(c, t, pos, reach, None, |id| {
                self.hittable(id) && c.kind(id) == Kind::Point && interact::movable(c, id)
            });
            press.acted = press.attachment.is_some();
            if press.acted {
                return press;
            }
        }
        if creates_by_drag(tool) && self.tool.picks().len() == 1 && self.tool.asking().is_none() {
            press.deferred = true;
            press.acted = true;
            return press;
        }
        let before = self.wb.construction().len();
        let outcome = self.tool_click(t, pos, reach);
        press.acted = !matches!(outcome, Outcome::Ignored);
        if matches!(tool, Tool::Point | Tool::PointOnObject) {
            let c = self.wb.construction();
            press.point = match outcome {
                // Only a placement that succeeded made the last object.
                Outcome::Place { .. } if c.len() > before => c.ids().last(),
                Outcome::Select(label) => c.id(&label),
                _ => None,
            }
            .filter(|&id| interact::movable(c, id));
        }
        press
    }

    pub fn observe_tool_pointer(&mut self, pos: (f64, f64)) {
        if self.tool.tool() == Tool::AttachDetach {
            self.tool.observe_attachment_pointer(pos);
        }
    }

    /// The release that ends a construction tool's press, at `pos` in the
    /// area (`t` the view now). A drag-creating tool holding its first point
    /// picks here when the press waited for the release, or when the
    /// pointer went beyond the point selection distance from the press
    /// (`withinPointSelectionDistance`, Euclidean, threshold + 12 px)
    /// (`EuclidianController.wrapMouseReleased`).
    pub fn release_tool(
        &mut self,
        press: ToolPress,
        t: &ViewTransform,
        pos: (f64, f64),
        reach: &Reach,
    ) {
        if self.tool.tool() == Tool::AttachDetach {
            if let Some(point) = press.attachment {
                let distance = (pos.0 - press.at.0).hypot(pos.1 - press.at.1);
                if distance <= reach.threshold {
                    self.tool_click(t, pos, reach);
                } else {
                    let outcome = self.attach_detach_drag(t, pos, reach, point);
                    self.apply_outcome(outcome);
                }
            }
            return;
        }
        if !creates_by_drag(self.tool.tool()) || self.tool.picks().len() != 1 {
            return;
        }
        let distance = (pos.0 - press.at.0).hypot(pos.1 - press.at.1);
        let within = distance < reach.threshold + reach.point_min;
        if press.deferred || !within {
            self.tool_click(t, pos, reach);
        }
    }

    /// Where the free end of a tool's preview is drawn for the pointer at
    /// `pos` (in the area): on the point under the pointer, by the hit rule
    /// a click uses, else at the pointer (spec E6).
    pub fn preview_end(&self, t: &ViewTransform, pos: (f64, f64), reach: &Reach) -> (f64, f64) {
        let c = self.wb.construction();
        hit::pick(c, t, pos, reach, None, |id| self.hittable(id))
            .filter(|&id| c.kind(id) == Kind::Point)
            .and_then(|id| match c.value(id) {
                Value::Point { x, y } => Some(t.world_to_screen(x, y)),
                _ => None,
            })
            .unwrap_or(pos)
    }

    /// One move of the eraser's stroke at `pos` in the area: deletes every
    /// object that the square of `ERASER_SIZE` around it meets
    /// (`HitDetector.setIntersectionHits`; a point by its drawn disc).
    /// Returns whether something was deleted.
    pub fn erase_at(&mut self, t: &ViewTransform, pos: (f64, f64), point_radius: f64) -> bool {
        let half = ERASER_SIZE / 2.0;
        let reach = Reach {
            threshold: half,
            retry: None,
            point_min: point_radius,
            mouse: half,
        };
        let mut deleted = false;
        loop {
            let c = self.wb.construction();
            let found = hit::hits(c, t, pos, &reach, |id| self.wb.is_visible(id));
            // One at a time: a deletion takes dependents along and renumbers
            // the rest.
            let Some(&id) = found.first() else {
                break;
            };
            if !self.wb.delete(id) {
                break;
            }
            deleted = true;
        }
        deleted
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interact;
    use crate::view::Camera;
    use crate::workbench::Workbench;
    use geo_kernel::algo::path::PathDef;
    use geo_kernel::kernel::Def;

    /// The origin at (400, 300), 50 pixels per unit.
    fn t() -> ViewTransform {
        Camera {
            cx: 0.0,
            cy: 0.0,
            xscale: 50.0,
            yscale: 50.0,
        }
        .transform(800.0, 600.0)
    }

    fn at(x: f64, y: f64) -> (f64, f64) {
        t().world_to_screen(x, y)
    }

    fn app(src: &str, tool: Tool) -> App {
        let mut app = App::new(Workbench::from_script(src).unwrap(), None);
        app.select_tool(tool);
        app
    }

    fn reach(app: &App) -> Reach {
        Reach::of(&app.metrics, false)
    }

    fn labels(app: &App) -> Vec<String> {
        let c = app.wb.construction();
        c.ids().map(|id| c.label(id).to_owned()).collect()
    }

    const CIRCLE_AND_LINE: &str =
        "O = (0, 0)\nP = (2, 0)\nc = Circle(O, P)\nQ = (-3, 1)\nR = (3, 1)\nf = Line(Q, R)\n";

    /// A new point where a circle and a line cross is their intersection,
    /// the one of the two nearer the pointer (spec E2,
    /// `getSingleIntersectionPoint`). Fails if new points were free.
    #[test]
    fn a_new_point_on_a_crossing_is_the_nearer_intersection() {
        let mut app = app(CIRCLE_AND_LINE, Tool::Point);
        let r = reach(&app);
        for x in [3f64.sqrt(), -(3f64.sqrt())] {
            let NewPoint::Intersection { a, b, index } = app.new_point_at(&t(), at(x, 1.0), &r)
            else {
                panic!("not an intersection at {x}");
            };
            let n = index.expect("two points");
            let p = app
                .wb
                .probe_point(&format!("Intersect({a}, {b}, {n})"))
                .unwrap();
            assert!((p.0 - x).abs() < 1e-9 && (p.1 - 1.0).abs() < 1e-9, "{p:?}");
        }
    }

    /// On one path the new point is a point on it; the parallel line tool
    /// puts it there only once a line is picked, the angle tool never
    /// (spec E2, the per-mode cases of `getNewPoint`).
    #[test]
    fn new_points_go_on_paths_only_for_the_tools_that_allow_it() {
        let on_f = at(-2.5, 1.0);
        let mut app = app(CIRCLE_AND_LINE, Tool::Point);
        let r = reach(&app);
        assert!(matches!(
            app.new_point_at(&t(), on_f, &r),
            NewPoint::OnPath { path, .. } if path == "f"
        ));
        app.select_tool(Tool::Angle);
        assert!(matches!(
            app.new_point_at(&t(), on_f, &r),
            NewPoint::Free(..)
        ));
        app.select_tool(Tool::Parallel);
        assert!(matches!(
            app.new_point_at(&t(), on_f, &r),
            NewPoint::Free(..)
        ));
        app.press_tool(&t(), on_f, &r);
        assert_eq!(app.tool.picks().len(), 1, "the line is picked");
        assert!(matches!(
            app.new_point_at(&t(), at(0.0, 2.0), &r),
            NewPoint::OnPath { path, .. } if path == "c"
        ));
    }

    /// The segment tool makes its first point when the button goes down
    /// and, after a drag, its second one where the button goes up (spec
    /// E1, E3); a release near the press makes nothing more, the next press
    /// waits for its release. Fails if a press picked nothing.
    #[test]
    fn the_segment_tool_picks_on_the_press_and_after_a_drag_on_the_release() {
        let mut app = app("", Tool::Segment);
        let r = reach(&app);
        let press = app.press_tool(&t(), at(-2.3, 1.7), &r);
        assert!(press.acted && !press.deferred);
        assert_eq!(labels(&app), ["A"]);
        app.release_tool(press, &t(), at(1.7, -1.3), &r);
        assert_eq!(labels(&app), ["A", "B", "f"]);
        assert!(app.wb.can_undo());
        app.wb.undo();
        assert!(labels(&app).is_empty(), "one undo step for the whole use");

        let press = app.press_tool(&t(), at(-2.3, 1.7), &r);
        let near = (press.at.0 + 5.0, press.at.1);
        app.release_tool(press, &t(), near, &r);
        assert_eq!(labels(&app), ["A"], "a click picks once");
        let press = app.press_tool(&t(), at(1.7, -1.3), &r);
        assert!(press.deferred);
        assert_eq!(labels(&app), ["A"]);
        app.release_tool(press, &t(), at(1.7, -1.3), &r);
        assert_eq!(labels(&app), ["A", "B", "f"]);
    }

    /// The point tool's press makes the point that a drag then moves, or
    /// takes an existing free one (spec E4).
    #[test]
    fn the_point_tools_press_holds_the_point_it_made_or_found() {
        let mut app = app("A = (1, 1)\n", Tool::Point);
        let r = reach(&app);
        let press = app.press_tool(&t(), at(-2.3, 1.7), &r);
        let c = app.wb.construction();
        assert_eq!(press.point, c.id("B"));
        let press = app.press_tool(&t(), at(1.0, 1.0), &r);
        assert_eq!(press.point, app.wb.construction().id("A"));
        assert_eq!(labels(&app), ["A", "B"]);
    }

    /// The preview's free end jumps onto a point under the pointer, by the
    /// hit rule of a click, and stays at the pointer elsewhere (spec E6).
    /// Fails if the end followed the pointer only.
    #[test]
    fn the_preview_end_snaps_to_a_point_under_the_pointer() {
        let app = app("A = (1, 1)\nB = (-2, 1)\nf = Line(A, B)\n", Tool::Segment);
        let r = reach(&app);
        let a = at(1.0, 1.0);
        let near = (a.0 + 6.0, a.1 - 5.0);
        assert_eq!(app.preview_end(&t(), near, &r), a);
        let on_line = at(-0.5, 1.0);
        assert_eq!(app.preview_end(&t(), on_line, &r), on_line);
        let far = (a.0 + 40.0, a.1);
        assert_eq!(app.preview_end(&t(), far, &r), far);
    }

    /// A click on a non-circle conic puts the new point on it, as on a
    /// circle (`GeoConicND` is a path and the boundary hit is a path hit,
    /// `PATH_NO_FILL_HIT`); the point tool places it there and drags it
    /// along the conic.
    #[test]
    fn a_new_point_on_a_conic_is_a_point_on_it() {
        const SRC: &str = "F = (-2, 0)\nG = (2, 0)\nR = (0, 1.5)\nk = Ellipse(F, G, R)\n";
        let on_conic = at(1.5, 1.2);
        let mut app = app(SRC, Tool::Point);
        let r = reach(&app);
        assert!(matches!(
            app.new_point_at(&t(), on_conic, &r),
            NewPoint::OnPath { path, .. } if path == "k"
        ));
        let press = app.press_tool(&t(), on_conic, &r);
        let p = press.point.expect("the point was placed");
        let c = app.wb.construction();
        assert_eq!(
            c.def(p),
            Def::Path(PathDef::On {
                path: c.id("k").unwrap()
            })
        );
        assert!(interact::movable(c, p), "the point moves along its conic");
        let Value::Point { x, y } = c.value(p) else {
            panic!("the point is defined");
        };
        assert!(
            (x - 1.5).abs() < 1e-9 && (y - 1.2).abs() < 1e-9,
            "({x}, {y})"
        );
        // and a drag of it stays on the ellipse
        app.wb.move_point(p, 5.0, 5.0);
        let c = app.wb.construction();
        let Value::Point { x, y } = c.value(p) else {
            panic!("the point stays defined");
        };
        let d = c.conic(c.id("k").unwrap()).unwrap();
        let [a, b] = d.half_axes;
        let on = (x / a).hypot(y / b);
        assert!((on - 1.0).abs() < 1e-9, "({x}, {y}) is not on the ellipse");
    }

    /// A construction tool that makes its points on objects does so on a
    /// non-circle conic too: the first pick of the segment and polygon tools
    /// is a point on the conic (`new_points_on_objects` routes them all
    /// through the same path hit).
    #[test]
    fn the_segment_and_polygon_tools_pick_points_on_a_conic() {
        const SRC: &str = "F = (-2, 0)\nG = (2, 0)\nR = (0, 1.5)\nk = Ellipse(F, G, R)\n";
        for tool in [Tool::Segment, Tool::Polygon] {
            let mut app = app(SRC, tool);
            let r = reach(&app);
            let press = app.press_tool(&t(), at(1.5, 1.2), &r);
            assert!(press.acted && !press.deferred, "{tool:?}");
            let c = app.wb.construction();
            assert_eq!(
                c.def(c.id("A").expect("the pick made a point")),
                Def::Path(PathDef::On {
                    path: c.id("k").unwrap()
                }),
                "{tool:?}"
            );
        }
    }

    /// The eraser deletes what the square around the pointer meets, points
    /// by their drawn disc (spec E11).
    #[test]
    fn the_eraser_deletes_what_its_square_meets() {
        let src = "A = (0, 0)\nB = (4, 0)\nf = Segment(A, B)\nC = (0, 3)\n";
        let mut app = app(src, Tool::Delete);
        let (x, y) = at(2.0, 0.0);
        assert!(!app.erase_at(&t(), (x, y - 11.0), 5.0), "10 px is the edge");
        assert!(app.erase_at(&t(), (x, y - 9.0), 5.0));
        assert_eq!(labels(&app), ["A", "B", "C"]);
        let (x, y) = at(0.0, 3.0);
        assert!(app.erase_at(&t(), (x + 14.0, y), 5.0));
        assert_eq!(labels(&app), ["A", "B"]);
    }

    fn point(c: &geo_kernel::Construction, label: &str) -> (f64, f64) {
        let id = c.id(label).unwrap_or_else(|| panic!("missing {label}"));
        let Value::Point { x, y } = c.value(id) else {
            panic!("{label} is undefined")
        };
        (x, y)
    }

    fn click_tool(app: &mut App, x: f64, y: f64) {
        let reach = reach(app);
        let press = app.press_tool(&t(), at(x, y), &reach);
        app.release_tool(press, &t(), at(x, y), &reach);
    }

    fn answer_tool(app: &mut App, value: f64) {
        let outcome = app
            .tool
            .finish_number(value, &|label| app.wb.construction().id(label).is_some());
        app.apply_outcome(outcome);
    }

    #[test]
    fn tool_constraint_picks_keep_the_selected_path() {
        for (tool, script, x, y, point, carrier) in [
            (Tool::PointOnObject, "", 2.0, 0.0, "A", "xAxis"),
            (
                Tool::PointOnObject,
                "A = (0, 0)\nB = (4, 2)\nu = Vector(A, B)",
                2.0,
                1.0,
                "C",
                "u",
            ),
            (Tool::SegmentFixed, "f(x) = x^2", 1.0, 1.0, "A", "f"),
        ] {
            let mut app = app(script, tool);
            click_tool(&mut app, x, y);
            let c = app.wb.construction();
            assert_eq!(
                c.def(c.id(point).unwrap()),
                Def::Path(PathDef::On {
                    path: c.id(carrier).unwrap(),
                }),
                "{carrier}"
            );
        }
    }

    #[test]
    fn roots_caps_the_search_for_free_output_labels() {
        let mut source = String::from("f(x) = x^2 - 1\n");
        for label in crate::labels::free_point_labels(|_| false).take(2100) {
            use std::fmt::Write;
            writeln!(source, "{label} = (100, 100)").unwrap();
        }
        let mut app = app(&source, Tool::Roots);
        let count = app.wb.construction().len();
        assert_eq!(count, 2101);
        click_tool(&mut app, 0.0, -1.0);
        assert_eq!(app.wb.construction().len(), count);
        assert_eq!(app.wb.undo_depth(), (0, 0));
    }

    #[test]
    fn fixed_tool_numeric_input_is_bounded() {
        let mut app = app("a = 2", Tool::SegmentFixed);
        app.number_text = "a + 1".to_owned();
        assert_eq!(app.tool_number_value(), Some(3.0));
        app.number_text = format!("{}1", " ".repeat(4096));
        assert_eq!(app.tool_number_value(), None);
    }

    #[test]
    fn fixed_segment_creates_a_movable_endpoint_after_placing_its_start() {
        let mut app = app("", Tool::SegmentFixed);
        click_tool(&mut app, 1.0, 1.0);
        assert!(app.tool.asking().is_some());
        answer_tool(&mut app, 4.0);
        let c = app.wb.construction();
        let b = c.id("B").unwrap();
        assert_eq!(point(c, "B"), (5.0, 1.0));
        assert!(interact::movable(c, b));
        assert_eq!(c.kind(c.id("f").unwrap()), Kind::Segment);
        assert_eq!(app.wb.undo_depth(), (2, 0));
        assert!(app.undo());
        assert_eq!(labels(&app), ["A"]);
        assert!(app.undo());
        assert!(labels(&app).is_empty());
    }

    #[test]
    fn fixed_angle_accepts_a_segment_and_creates_a_rotated_point() {
        let mut app = app(
            "A = (2, 0)\nB = (0, 0)\nf = Segment(A, B)",
            Tool::AngleFixed,
        );
        click_tool(&mut app, 1.0, 0.0);
        assert!(app.tool.asking().is_some());
        answer_tool(&mut app, 90.0);
        let c = app.wb.construction();
        let p = point(c, "B'");
        assert!(
            (p.0 - 2.0).abs() < 1e-9 && (p.1 + 2.0).abs() < 1e-9,
            "{p:?}"
        );
        assert!(c.ids().any(|id| c.kind(id) == Kind::Angle));
        assert_eq!(app.wb.undo_depth(), (1, 0));
        assert!(app.undo());
        assert_eq!(labels(&app), ["A", "B", "f"]);
    }

    #[test]
    fn rigid_polygon_closes_at_the_first_vertex() {
        let mut app = app("", Tool::RigidPolygon);
        for (x, y) in [(1.0, 1.0), (4.0, 1.0), (1.0, 4.0)] {
            click_tool(&mut app, x, y);
        }
        assert_eq!(picks(&app), ["A", "B", "C"]);
        click_tool(&mut app, 1.0, 1.0);
        let c = app.wb.construction();
        assert!(c.ids().any(|id| c.kind(id) == Kind::Polygon));
        assert!(!interact::movable(c, c.id("C").unwrap()));
        assert_eq!(app.wb.undo_depth(), (1, 0));
        assert!(app.undo());
        assert!(labels(&app).is_empty());
    }

    #[test]
    fn rigid_polygon_caps_pending_vertices() {
        let mut tool = crate::tools::ToolState::new(Tool::RigidPolygon);
        for i in 0..1001 {
            tool.click(
                Some((&format!("P_{{{i}}}"), Kind::Point)),
                (0.0, 0.0),
                &|_| true,
            );
        }
        assert_eq!(tool.picks().len(), 1000);
        assert!(matches!(
            tool.click(Some(("P_{0}", Kind::Point)), (0.0, 0.0), &|_| false),
            Outcome::Run(_)
        ));
    }

    #[test]
    fn oversized_shapes_do_not_place_free_points_or_start_a_copy() {
        for (tool, command) in [
            (Tool::RigidPolygon, "Polygon"),
            (Tool::PointOnObject, "Polygon"),
            (Tool::PointOnObject, "PolyLine"),
            (Tool::Relation, "Polygon"),
        ] {
            let source = format!("L = Sequence((k, k^2), k, 1, 4097)\np = {command}(L)");
            let mut app = app(&source, tool);
            click_tool(&mut app, 1.0, 1.0);
            assert_eq!(labels(&app), ["L", "p"], "{tool:?}");
            // Relation may pick the separately visible source list.
            if tool != Tool::Relation {
                assert!(picks(&app).is_empty(), "{tool:?}");
            } else {
                assert!(!picks(&app).iter().any(|label| label == "p"), "{tool:?}");
            }
            assert_eq!(app.wb.undo_depth(), (0, 0));
        }
    }

    #[test]
    fn rigid_polygon_copy_checks_the_full_shape_budget() {
        for (count, expected) in [(1000, Some(1000)), (1001, None), (4097, None)] {
            let source = format!("L = Sequence((k, k^2), k, 1, {count})\np = Polygon(L)");
            let app = app(&source, Tool::RigidPolygon);
            let c = app.wb.construction();
            let id = c.id("p").unwrap();
            assert_eq!(rigid_copy_size(c, id), expected);
        }
    }

    #[test]
    fn point_on_object_joins_the_polygon_boundary_instead_of_a_side() {
        let mut app = app(
            "A = (0, 0)\nB = (4, 0)\nC = (0, 4)\np = Polygon(A, B, C)",
            Tool::PointOnObject,
        );
        click_tool(&mut app, 2.0, 0.0);
        let c = app.wb.construction();
        assert_eq!(
            c.def(c.id("D").unwrap()),
            Def::Path(PathDef::On {
                path: c.id("p").unwrap()
            })
        );
    }

    #[test]
    fn a_point_on_an_object_moves_along_a_function() {
        let mut app = app("f(x) = x^2", Tool::PointOnObject);
        click_tool(&mut app, 1.0, 1.0);
        let c = app.wb.construction();
        let p = c.id("A").unwrap();
        assert_eq!(
            c.def(p),
            Def::Path(PathDef::On {
                path: c.id("f").unwrap()
            })
        );
        assert!(app.wb.move_point(p, 2.0, 4.0));
        assert_eq!(point(app.wb.construction(), "A"), (2.0, 4.0));
    }

    #[test]
    fn relation_reports_two_objects_without_creating_one() {
        let mut app = app("A = (1, 1)\nB = (2, 1)", Tool::Relation);
        click_tool(&mut app, 1.0, 1.0);
        click_tool(&mut app, 4.0, 4.0);
        assert_eq!(picks(&app), ["A"]);
        click_tool(&mut app, 2.0, 1.0);
        assert!(!app.relation_report.as_ref().unwrap().is_empty());
        assert_eq!(labels(&app), ["A", "B"]);
        assert_eq!(app.wb.undo_depth(), (0, 0));
    }

    #[test]
    fn extremum_takes_a_function_and_undoes_its_points() {
        let mut app = app("f(x) = (x - 1)^2 + 1", Tool::Extremum);
        click_tool(&mut app, 4.0, 4.0);
        assert_eq!(labels(&app), ["f"]);
        click_tool(&mut app, 1.0, 1.0);
        let c = app.wb.construction();
        assert_eq!(point(c, "A"), (1.0, 1.0));
        assert!(app.undo());
        assert_eq!(labels(&app), ["f"]);
        assert!(!app.undo());
    }

    #[test]
    fn roots_takes_a_function_and_undoes_all_outputs_together() {
        let mut app = app("f(x) = x^2 - 1", Tool::Roots);
        click_tool(&mut app, 4.0, 4.0);
        assert_eq!(labels(&app), ["f"]);
        click_tool(&mut app, 0.0, -1.0);
        let c = app.wb.construction();
        assert_eq!(point(c, "A"), (-1.0, 0.0));
        assert_eq!(point(c, "B"), (1.0, 0.0));
        assert!(app.undo());
        assert_eq!(labels(&app), ["f"]);
        assert!(!app.undo());
    }

    #[test]
    fn point_on_object_places_a_point_inside_a_polygon() {
        let mut app = app(
            "A = (0, 0)\nB = (4, 0)\nC = (0, 4)\np = Polygon(A, B, C)",
            Tool::PointOnObject,
        );
        click_tool(&mut app, 0.0, 0.0);
        assert!(app.wb.construction().id("D").is_none());
        click_tool(&mut app, 1.0, 1.0);
        let c = app.wb.construction();
        let d = c.id("D").unwrap();
        assert_eq!(
            c.def(d),
            Def::Path(PathDef::InRegion {
                region: c.id("p").unwrap()
            })
        );
        assert_eq!(point(c, "D"), (1.0, 1.0));
        assert!(app.undo());
        assert!(app.wb.construction().id("D").is_none());
    }

    #[test]
    fn attach_detach_redefines_the_same_point_and_keeps_its_dependents() {
        let mut app = app(
            "A = (0, 0)\nB = (4, 0)\nf = Segment(A, B)\nP = (2, 2)\nM = Midpoint(A, P)",
            Tool::AttachDetach,
        );
        click_tool(&mut app, 2.0, 2.0);
        assert_eq!(picks(&app), ["P"]);
        click_tool(&mut app, 3.0, 0.0);
        let c = app.wb.construction();
        let p = c.id("P").unwrap();
        assert_eq!(
            c.def(p),
            Def::Path(PathDef::On {
                path: c.id("f").unwrap()
            })
        );
        assert_eq!(point(c, "P"), (3.0, 0.0));
        assert_eq!(point(c, "M"), (1.5, 0.0));
        click_tool(&mut app, 3.0, 0.0);
        assert!(matches!(app.wb.construction().def(p), Def::Path(_)));
        click_tool(&mut app, 3.02, 0.0);
        assert!(app.wb.construction().def(p).is_free());
        assert_eq!(app.wb.undo_depth(), (2, 0));
        assert!(app.undo());
        assert!(matches!(app.wb.construction().def(p), Def::Path(_)));
        assert!(app.undo());
        assert_eq!(point(app.wb.construction(), "P"), (2.0, 2.0));
    }

    fn picks(app: &App) -> Vec<String> {
        app.tool
            .picks()
            .iter()
            .map(|p| p.label().to_owned())
            .collect()
    }

    fn type_line(app: &mut App, line: &str) {
        app.input = line.to_owned();
        app.submit_input();
    }

    /// The segment tool holding its first point A (pending), with one step
    /// to redo (a typed `Y`, undone).
    fn segment_use_with_a_redo() -> App {
        let mut app = app("Z = (9, 9)\n", Tool::Segment);
        let r = reach(&app);
        type_line(&mut app, "Y = (8, 8)");
        assert!(app.undo());
        assert_eq!(app.wb.undo_depth(), (0, 1));
        app.press_tool(&t(), at(1.0, 1.0), &r);
        assert_eq!(picks(&app), ["A"]);
        assert_eq!(labels(&app), ["Z", "A"]);
        app
    }

    /// Undo in the middle of a use takes only the use back and leaves the
    /// redo stack as it was (`Kernel.undo`, `restoreStateForModeStarting`).
    #[test]
    fn undo_mid_use_takes_only_the_use_back() {
        let mut app = segment_use_with_a_redo();
        assert!(app.can_undo());
        assert!(app.undo());
        assert!(picks(&app).is_empty());
        assert_eq!(labels(&app), ["Z"]);
        assert_eq!(app.wb.undo_depth(), (0, 1));
        assert!(!app.can_undo());
        assert!(app.redo());
        assert_eq!(labels(&app), ["Z", "Y"]);
        assert_eq!(app.wb.undo_depth(), (1, 0));
    }

    /// Redo in the middle of a use ends it unfinished, then redoes.
    #[test]
    fn redo_mid_use_takes_the_use_back_then_redoes() {
        let mut app = segment_use_with_a_redo();
        assert!(app.redo());
        assert!(picks(&app).is_empty());
        assert_eq!(labels(&app), ["Z", "Y"]);
        assert_eq!(app.wb.undo_depth(), (1, 0));
        assert!(app.undo());
        assert_eq!(labels(&app), ["Z"]);
        assert!(!app.can_undo());
    }

    /// A line typed in the middle of a use: one that fails changes nothing
    /// and the use goes on; one that runs takes the use back (its point does
    /// not survive as an orphan) and is the only step.
    #[test]
    fn a_line_typed_mid_use_keeps_the_use_only_when_it_fails() {
        let mut app = app("Z = (9, 9)\n", Tool::Segment);
        let r = reach(&app);
        app.press_tool(&t(), at(1.0, 1.0), &r);
        type_line(&mut app, "Nonsense((");
        assert!(app.input_error.is_some());
        assert_eq!(picks(&app), ["A"]);
        assert_eq!(labels(&app), ["Z", "A"]);
        assert_eq!(app.wb.undo_depth(), (0, 0));
        // The use can still finish, as one step.
        let press = app.press_tool(&t(), at(2.0, 2.0), &r);
        app.release_tool(press, &t(), at(2.0, 2.0), &r);
        assert!(picks(&app).is_empty());
        assert_eq!(labels(&app), ["Z", "A", "B", "f"]);
        assert_eq!(app.wb.undo_depth(), (1, 0));
        assert!(app.undo());
        assert_eq!(labels(&app), ["Z"]);

        app.press_tool(&t(), at(1.0, 1.0), &r);
        type_line(&mut app, "Y = (8, 8)");
        assert!(picks(&app).is_empty());
        assert_eq!(labels(&app), ["Z", "Y"]);
        app.select_tool(Tool::Move);
        assert_eq!(labels(&app), ["Z", "Y"]);
        assert_eq!(app.wb.undo_depth(), (1, 0));
        assert!(app.undo());
        assert_eq!(labels(&app), ["Z"]);
    }

    /// A tool switch in the middle of the eraser's stroke stops it, and
    /// what it deleted is stored as one step (`storeUndoAfterDrag` runs at
    /// the release whatever the mode). Escape reaches the same end through
    /// the gesture's stop; this is any other switch.
    #[test]
    fn a_tool_switch_mid_stroke_ends_the_eraser() {
        let mut app = app(CIRCLE_AND_LINE, Tool::Delete);
        app.erasing = true;
        app.wb.begin_gesture();
        assert!(app.erase_at(&t(), at(0.0, 1.0), 4.0));
        assert_eq!(labels(&app), ["O", "P", "c", "Q", "R"]);
        app.select_tool(Tool::Move);
        assert!(!app.erasing);
        assert_eq!(app.wb.undo_depth(), (1, 0));
        assert!(app.undo());
        assert_eq!(labels(&app), ["O", "P", "c", "Q", "R", "f"]);
    }
}
