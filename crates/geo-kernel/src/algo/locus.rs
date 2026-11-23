// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdLocus.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoDispatcher.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoLocusND.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoLocus.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntegralODE.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoSlopeField.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoLocusSliderND.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoLocusSlider.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoLocusND.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoLocus.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoNumeric.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/PathMover.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/PathMoverGeneric.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/SliderMover.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/scripting/CmdSlider.java
//   source/shared/common/src/main/java/org/geogebra/common/util/MyMath.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! `Locus(Q, P)` and `Locus(Q, slider)`: the path the point `Q` traces while
//! the point `P` runs along its path, or while a slider runs through its
//! interval.
//!
//! GeoGebra copies the part of the construction between the moving object
//! and `Q` into a macro construction and moves the copy (`AlgoLocusND`). This
//! kernel moves the object itself: it saves what sampling changes (the
//! moving object and the objects between it and `Q`), recomputes those
//! objects for each sample in dependency order, and puts everything back
//! afterwards, so no other object sees a sample.
//!
//! The samples are taken as upstream takes them: a parameter runs over the
//! path with a step that halves while the next point of `Q` is too far from
//! the last one and doubles otherwise (`PathMoverGeneric`), and a point is
//! joined to the one before it only when it is close (`lineTo`), so a jump
//! or an undefined stretch of `Q` starts a new piece (`moveTo`). "Close" and
//! "too far" are measured in pixels of the graphics view. Upstream reads the
//! view the locus is shown in. A construction uses the reference headless
//! window until the app supplies its own with [`Construction::set_locus_view`].
//! Upstream also stops sampling when one update of the
//! construction takes longer than 500 ms (`MAX_TIME_FOR_ONE_STEP`); a time
//! limit would make the points depend on the machine, so it is left out, and
//! only the limits on the number of points and steps are kept.
//!
//! A locus does not follow its moving object: upstream leaves it out of the
//! inputs it updates from (`setInputOutput`), so dragging `P` keeps the old
//! points, which start and end at `Q` for the position `P` had when the
//! locus was last computed. It is recomputed when its path or a free object
//! that `Q` depends on changes ([`reads`]).

use crate::algo::conic_nd::ConicType;
use crate::algo::path::{self, inf_function, inverse_inf_function, Geom, PathDef, PathState};
use crate::algo::{Group, Parents, NUMBER, POINT};
use crate::expr::fraction::FunctionFraction;
use crate::kernel::{Construction, Def, Error, Id, Kind, Value};
use std::sync::Arc;

/// `PathMover.MIN_STEPS`.
const MIN_STEPS: f64 = 128.0;
/// `PathMover.STEP_DECREASE_FACTOR`.
const STEP_DECREASE_FACTOR: f64 = 0.5;
/// `PathMover.STEP_INCREASE_FACTOR`.
const STEP_INCREASE_FACTOR: f64 = 2.0;
/// `PathMover.MIN_STEP_WIDTH`.
const MIN_STEP_WIDTH: f64 = 1e-8;
/// `PathMover.NEG_MIN_STEP_WIDTH`.
const NEG_MIN_STEP_WIDTH: f64 = -1e-8;
/// `PathMover.OPEN_BORDER_OFFSET`.
const OPEN_BORDER_OFFSET: f64 = 1e-5;
/// `PathMover.MAX_POINTS`.
const MAX_POINTS: usize = 10000;
/// Graphics window used by the reference harness.
pub(crate) const HEADLESS_VIEW: [f64; 6] = [-4.3, 11.700000000000001, -5.7, 6.3, 50.0, 50.0];
/// `AlgoLocusND.MAX_X_PIXEL_DIST` and `MAX_Y_PIXEL_DIST`.
const MAX_PIXEL_DIST: f64 = 5.0;

#[path = "locus/numeric.rs"]
mod numeric;
pub(crate) use numeric::NumericLocus;

/// A locus command.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LocusDef {
    /// `Locus(Q, P)` with `P` a point on a path (`AlgoLocus`).
    Path {
        point: Id,
        mover: Id,
    },
    /// `Locus(Q, a)` with `a` a slider (`AlgoLocusSlider`).
    Slider {
        point: Id,
        slider: Id,
    },
    /// Integral curve of a slope function through a point.
    Ode {
        point: Id,
        source: Id,
    },
    /// A grid of line segments showing the slope at each sample.
    SlopeField(Id),
    /// `Hull(list, number)`: the convex hull of a list of points as a
    /// closed locus (`AlgoConvexHull`); the number is ignored.
    Hull(Id),
    Numeric(NumericLocus),
}

/// One point of a locus (`MyPoint`): drawn with a line from the point before
/// it when `line_to` is set, and starting a new piece otherwise.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LocusPoint {
    pub x: f64,
    pub y: f64,
    pub line_to: bool,
}

/// The points of a locus (`GeoLocus`); empty when the locus is undefined.
#[derive(Clone, Debug, Default)]
pub struct LocusData {
    pub points: Arc<Vec<LocusPoint>>,
    pub defined: bool,
    bounds: Option<[f64; 4]>,
}

impl LocusData {
    /// Bounds of the finite samples, in the order xmin, xmax, ymin, ymax.
    pub fn bounds(&self) -> Option<[f64; 4]> {
        self.bounds
    }

    pub(crate) fn value(&self) -> Value {
        if self.defined {
            Value::Locus {
                len: u32::try_from(self.points.len()).unwrap_or(u32::MAX),
            }
        } else {
            Value::Undefined
        }
    }

    /// Bit equality of the points.
    fn same(&self, o: &LocusData) -> bool {
        let b = |a: f64, b: f64| a.to_bits() == b.to_bits();
        self.defined == o.defined
            && (Arc::ptr_eq(&self.points, &o.points)
                || self.points.len() == o.points.len()
                    && self
                        .points
                        .iter()
                        .zip(o.points.iter())
                        .all(|(p, q)| b(p.x, q.x) && b(p.y, q.y) && p.line_to == q.line_to))
    }
}

impl Construction {
    /// The points of the locus `id`; `None` for any other object.
    pub fn locus(&self, id: Id) -> Option<&LocusData> {
        let id = self.conditional_source(id)?;
        match self.defs.get(id.0)? {
            Def::Locus(_) | Def::Discrete(_) => {
                self.locus_slot(id.0).map(|slot| &self.loci[slot].1)
            }
            _ => None,
        }
    }

    pub fn has_loci(&self) -> bool {
        !self.loci.is_empty()
    }

    /// The supplied sampling window and scales. `None` uses the headless view.
    pub fn locus_view(&self) -> Option<[f64; 6]> {
        self.locus_view
    }

    /// Sets the sampling window and cascades updates from the loci. The
    /// scales are in pixels per world unit. Coincident horizontal bounds let
    /// a transformed function use its default parameter interval.
    pub fn set_locus_view(
        &mut self,
        xmin: f64,
        xmax: f64,
        ymin: f64,
        ymax: f64,
        xscale: f64,
        yscale: f64,
    ) {
        let view = [xmin, xmax, ymin, ymax, xscale, yscale];
        let valid = view.iter().all(|v| v.is_finite())
            && xmax >= xmin
            && ymax > ymin
            && xscale > 0.0
            && yscale > 0.0;
        if !valid || self.locus_view == Some(view) {
            return;
        }
        self.locus_view = Some(view);
        let mut dirty = std::mem::take(&mut self.dirty);
        dirty.fit(self.defs.len());
        for &(i, _) in &self.loci {
            if matches!(self.defs[i], Def::Locus(_)) {
                dirty.insert(i);
            }
        }
        for (i, def) in self.defs.iter().enumerate() {
            if matches!(def, Def::Point(point) if point.reads_view())
                || matches!(
                    def,
                    Def::Vector(crate::algo::vector::VectorDef::ApplyMatrix {
                        kind: Kind::Function | Kind::Image,
                        ..
                    }) | Def::Transform(crate::algo::transform::TransformDef::Dilate {
                        kind: Kind::Image,
                        ..
                    })
                )
                || matches!(def, Def::Analysis(analysis) if analysis.uses_view())
            {
                dirty.insert(i);
            }
        }
        self.propagate_dirty(dirty, 0, false);
    }

    /// The graphics window the view-dependent objects read.
    pub fn view(&self) -> [f64; 6] {
        self.view_state.or(self.locus_view).unwrap_or(HEADLESS_VIEW)
    }

    /// Sets the window the view-dependent objects read (`Corner`,
    /// `AxisStepX`, `AxisStepY`) and recomputes them, as GeoGebra's
    /// `euclidianViewUpdate` does on every view change. Returns whether any
    /// view readers needed an update. Loci follow their own sampling window.
    pub fn set_view(
        &mut self,
        xmin: f64,
        xmax: f64,
        ymin: f64,
        ymax: f64,
        xscale: f64,
        yscale: f64,
    ) -> bool {
        let view = [xmin, xmax, ymin, ymax, xscale, yscale];
        let valid = view.iter().all(|v| v.is_finite())
            && xmax > xmin
            && ymax > ymin
            && xscale > 0.0
            && yscale > 0.0;
        if !valid || self.view_state == Some(view) {
            return false;
        }
        let changed = self.view() != view;
        self.view_state = Some(view);
        if !changed || self.view_readers.is_empty() {
            return false;
        }
        let mut dirty = std::mem::take(&mut self.dirty);
        dirty.fit(self.defs.len());
        for &i in &self.view_readers {
            dirty.insert(i);
        }
        self.propagate_dirty(dirty, 0, false);
        true
    }

    pub(super) fn store_locus(&mut self, id: Id, data: LocusData) -> Value {
        let value = data.value();
        self.loci.push((id.0, data));
        value
    }

    fn locus_slot(&self, i: usize) -> Option<usize> {
        self.loci.binary_search_by_key(&i, |&(owner, _)| owner).ok()
    }

    /// Recomputes the points of the locus `i`; tells whether its children
    /// must follow: whether the points changed, bit for bit.
    #[inline(never)]
    pub(super) fn update_locus(
        &mut self,
        i: usize,
        record: impl FnOnce(&mut Self) -> LocusData,
    ) -> bool {
        let Some(slot) = self.locus_slot(i) else {
            return false;
        };
        let new = record(self);
        if new.same(&self.loci[slot].1) {
            return false;
        }
        self.values[i] = new.value();
        self.loci[slot].1 = new;
        true
    }
}

/// Whether `id` is a point made by `Point(path)` or `Point(path, t)`
/// (`GeoPointND.getPath() != null`).
fn on_path(c: &Construction, id: Id) -> bool {
    matches!(
        c.def(id),
        Def::Path(PathDef::On { .. } | PathDef::OnParam { .. })
    )
}

/// The path of a point on a path.
fn path_of(c: &Construction, id: Id) -> Option<Id> {
    match c.def(id) {
        Def::Path(PathDef::On { path } | PathDef::OnParam { path, .. }) => Some(path),
        _ => None,
    }
}

/// Every object `q` depends on, `q` excluded, as flags by index
/// (`GeoElement.getAllPredecessors`).
fn ancestors(c: &Construction, q: Id) -> Vec<bool> {
    let mut seen = vec![false; c.predecessor_storage_len(q)];
    let mut stack = vec![q];
    while let Some(id) = stack.pop() {
        for parent in c.reads(id) {
            if parent.0 < seen.len() && !seen[parent.0] {
                seen[parent.0] = true;
                stack.push(parent);
            }
        }
    }
    seen
}

/// `GeoElement.isParentOf`: whether `b` depends on `a`.
fn is_ancestor(c: &Construction, a: Id, b: Id) -> bool {
    c.evaluation_rank(a) < c.evaluation_rank(b) && ancestors(c, b)[a.0]
}

/// `isPointerChangeable`: a free object, or a point that can be moved on a
/// path or in a region.
fn pointer_changeable(c: &Construction, id: Id) -> bool {
    let def = c.def(id);
    def.is_free()
        || matches!(
            def,
            Def::Path(PathDef::On { .. } | PathDef::InRegion { .. })
        )
}

/// The objects a locus is recomputed from (`setEfficientDependencies`): for
/// `Locus(Q, P)` the path of `P` and every object `Q` depends on that the
/// pointer can change, except `P` itself; for a slider, every free object
/// `Q` depends on (`isChangeable`), so moving a point on a path that `Q`
/// depends on does not recompute it.
pub(crate) fn reads(c: &Construction, d: LocusDef) -> Vec<Id> {
    let point = match d {
        LocusDef::Ode { point, source } => {
            let mut ids = vec![source, point];
            if let Some(function) = slope_source(c.def(source)) {
                ids.push(function);
            }
            return ids;
        }
        LocusDef::SlopeField(source) => return vec![source],
        LocusDef::Hull(list) => return vec![list],
        LocusDef::Numeric(def) => return def.inputs().collect(),
        LocusDef::Slider { point, .. } | LocusDef::Path { point, .. } => point,
    };
    if point.0 >= c.len() {
        return Vec::new();
    }
    let mut ids = Vec::new();
    if let LocusDef::Path { mover, .. } = d {
        ids.extend(path_of(c, mover));
    }
    for (i, &is) in ancestors(c, point).iter().enumerate() {
        let id = Id(i);
        let reads = match d {
            LocusDef::Path { mover, .. } => id != mover && pointer_changeable(c, id),
            LocusDef::Slider { .. } => c.def(id).is_free(),
            LocusDef::Ode { .. }
            | LocusDef::SlopeField(_)
            | LocusDef::Hull(_)
            | LocusDef::Numeric(_) => unreachable!(),
        };
        if is && reads {
            ids.push(id);
        }
    }
    ids
}

/// The objects that depend on `mover` and that `q` depends on, `q`
/// included, in dependency order: what a sample recomputes.
pub(crate) fn chain(c: &Construction, q: Id, mover: Id) -> Vec<usize> {
    let anc = ancestors(c, q);
    let mut desc = vec![false; anc.len()];
    let last = c.evaluation_rank(q);
    let mut stack = vec![mover];
    while let Some(id) = stack.pop() {
        for child in c.children(id) {
            if c.evaluation_rank(child) <= last && !desc[child.0] {
                desc[child.0] = true;
                stack.push(child);
            }
        }
    }
    (0..=last)
        .map(|rank| c.evaluation_id(rank))
        .filter(|&i| desc[i] && (anc[i] || i == q.0))
        .collect()
}

/// `AlgoLocusND.validLocus`: no point between `P` and `Q` is put on a path
/// or in a region by the construction itself.
fn valid(c: &Construction, chain: &[usize], q: Id) -> bool {
    chain.iter().all(|&i| {
        i == q.0
            || !matches!(
                c.def(Id(i)),
                Def::Path(PathDef::On { .. } | PathDef::OnParam { .. } | PathDef::InRegion { .. })
            )
    })
}

/// `Path.isClosedPath`: a full circle or ellipse, a polygon, and a polyline
/// that starts and ends at the same point object.
fn is_closed(c: &Construction, path: Id, g: &Geom<'_>) -> bool {
    match g {
        Geom::Conic(d) => matches!(d.ty, ConicType::Circle | ConicType::Ellipse),
        Geom::Polygon { .. } => true,
        Geom::PolyLine { .. } => {
            let ids: Vec<Id> = c.inputs(path).collect();
            ids.len() > 1 && ids.first() == ids.last()
        }
        _ => false,
    }
}

/// What sampling changes, saved to be put back.
pub(crate) struct Saved {
    values: Vec<(usize, Value)>,
    defs: Vec<(usize, Def)>,
    paths: Vec<(usize, PathState)>,
    conics: Vec<(usize, crate::algo::conic_nd::ConicData)>,
    arcs: Vec<(usize, crate::algo::arc::ArcData)>,
    discrete: Vec<(usize, LocusData)>,
}

impl Saved {
    pub(crate) fn of(c: &Construction, ids: &[usize]) -> Self {
        let mut s = Saved {
            values: Vec::with_capacity(ids.len()),
            defs: Vec::with_capacity(ids.len()),
            paths: Vec::new(),
            conics: Vec::new(),
            arcs: Vec::new(),
            discrete: Vec::new(),
        };
        for &i in ids {
            s.values.push((i, c.values[i]));
            s.defs.push((i, c.defs[i]));
            if let Some(slot) = c.path_slot(i) {
                s.paths.push((slot, c.paths[slot].1));
            }
            if let Some(slot) = c.conic_slot(i) {
                s.conics.push((slot, c.conics[slot].1));
            }
            if let Some(slot) = c.arc_slot(i) {
                s.arcs.push((slot, c.arcs[slot].1));
            }
            if matches!(c.defs[i], Def::Discrete(_)) {
                let slot = c.locus_slot(i).expect("recorded discrete locus");
                s.discrete.push((slot, c.loci[slot].1.clone()));
            }
        }
        s
    }

    /// Puts the saved state back. The objects of `chain` are recomputed
    /// first, so that what an expression keeps besides its value (its last
    /// raw result) follows the saved inputs again; then every saved value
    /// and record is written back bit for bit.
    pub(crate) fn restore(self, c: &mut Construction, chain: &[usize]) {
        for &(i, def) in &self.defs {
            c.defs[i] = def;
        }
        for &(slot, st) in &self.paths {
            c.paths[slot].1 = st;
        }
        for &(i, v) in &self.values {
            c.values[i] = v;
        }
        for (slot, data) in self.discrete {
            c.loci[slot].1 = data;
        }
        for &i in chain {
            if !matches!(c.defs[i], Def::Discrete(_) | Def::Number(_)) {
                recompute(c, i);
            }
        }
        for (i, v) in self.values {
            c.values[i] = v;
        }
        for (slot, st) in self.paths {
            c.paths[slot].1 = st;
        }
        for (slot, r) in self.conics {
            c.conics[slot].1 = r;
        }
        for (slot, r) in self.arcs {
            c.arcs[slot].1 = r;
        }
    }
}

/// The live cascade shared by optimizers with the same mover. A sample
/// updates the mover's dependents; after copying the result and restoring
/// the parameter, the construction update also recomputes the mover itself.
pub(crate) struct OptimizerCascade {
    mover: Id,
    descendants: Vec<usize>,
    running: Vec<Id>,
    remaining: [usize; Self::MAX_DEPTH],
    start: u64,
    exhausted: bool,
}

impl OptimizerCascade {
    const MAX_DEPTH: usize = 8;
    // Count actual recomputations at each depth. Running optimizers
    // return immediately in AlgoOptimize and consume no search visits.
    const MAX_VISITS_PER_DEPTH: usize = 1_000_000;

    pub(crate) fn new(c: &Construction, owner: Id, mover: Id) -> Option<Self> {
        const MAX_EDGES: usize = 16_384;
        if !c.children(mover).take(MAX_EDGES).any(|id| {
            id != owner && matches!(c.def(id), Def::Optimize(d) if d.independent == mover)
        }) {
            return None;
        }

        let mut seen = std::collections::HashSet::new();
        let mut pending = vec![mover];
        let mut remaining = MAX_EDGES;
        let mut exhausted = false;
        while let Some(id) = pending.pop() {
            let Some(left) = remaining.checked_sub(c.child_count(id)) else {
                exhausted = true;
                break;
            };
            remaining = left;
            pending.extend(c.children(id).filter(|&child| seen.insert(child.0)));
        }
        let mut descendants: Vec<_> = seen.into_iter().collect();
        descendants.sort_unstable_by_key(|&i| c.evaluation_rank(Id(i)));
        Some(Self {
            mover,
            descendants,
            running: Vec::new(),
            remaining: [Self::MAX_VISITS_PER_DEPTH; Self::MAX_DEPTH],
            start: crate::expr::work_done(),
            exhausted,
        })
    }

    pub(crate) fn run(
        &mut self,
        c: &mut Construction,
        owner: Id,
        def: crate::algo::optimize::OptimizeDef,
    ) -> Value {
        if self.exhausted {
            return Value::Undefined;
        }
        let mut ids = self.descendants.clone();
        ids.push(self.mover.0);
        let saved = Saved::of(c, &ids);
        self.running.push(owner);
        let result = crate::algo::optimize::search(c, def, Some(self));
        self.running.pop();
        if self.exhausted {
            saved.restore(c, &self.descendants);
            return Value::Undefined;
        }
        result
    }

    pub(crate) fn set_result(&self, c: &mut Construction, value: Value) {
        if let Some(slot) = c
            .values
            .get_mut(self.running.last().expect("active optimizer").0)
        {
            *slot = value;
        }
    }

    pub(crate) fn update(&mut self, c: &mut Construction, restore: bool) -> bool {
        if self.exhausted {
            return false;
        }
        if restore {
            recompute(c, self.mover.0);
        }
        let depth = self.running.len() - 1;
        for index in 0..self.descendants.len() {
            let i = self.descendants[index];
            let def = c.defs[i];
            if matches!(def, Def::Optimize(d)
                if d.independent == self.mover && self.running.contains(&Id(i)))
            {
                continue;
            }
            let Some(left) = self.remaining[depth].checked_sub(1) else {
                self.exhausted = true;
                break;
            };
            self.remaining[depth] = left;
            if crate::expr::work_done() - self.start > crate::expr::MAX_EVALUATION_WORK {
                self.exhausted = true;
                break;
            }
            if let Def::Optimize(def) = def {
                let owner = Id(i);
                if def.independent != self.mover {
                    continue;
                }
                if self.running.len() == Self::MAX_DEPTH {
                    self.exhausted = true;
                    break;
                }
                self.running.push(owner);
                c.values[i] = crate::algo::optimize::search(c, def, Some(self));
                self.running.pop();
                if self.exhausted {
                    break;
                }
            } else if !matches!(def, Def::Prove(_) | Def::ImplicitCurve(_)) {
                recompute(c, i);
            }
        }
        !self.exhausted
    }
}

/// The undefined-for-good state of the expressions in a locus chain
/// (`SVD`, see `expr::Stuck`). The reference samples a copy of the
/// construction (its macro construction), so nothing a sample does reaches
/// the live objects; the sampling puts the state back before
/// [`Saved::restore`] recomputes the chain. An optimizer's trials update the live objects in the
/// reference, so its search keeps what they stick.
struct SavedStuck(Vec<(crate::expr::ExprDef, Vec<u32>)>);

impl SavedStuck {
    fn of(c: &Construction, ids: &[usize]) -> Self {
        Self(
            ids.iter()
                .filter_map(|&i| match c.defs[i] {
                    Def::Expr(d) => Some((d, crate::expr::stuck_nodes(c, d))),
                    _ => None,
                })
                .collect(),
        )
    }

    fn restore(self, c: &Construction) {
        for (d, nodes) in self.0 {
            crate::expr::restore_stuck_nodes(c, d, nodes);
        }
    }
}

/// Recomputes object `i` from its parents, as an update does.
pub(crate) fn recompute(c: &mut Construction, i: usize) {
    let def = c.defs[i];
    match def {
        Def::Expr(d) => {
            c.update_expr(i, d);
        }
        Def::Number(_) if c.slider_bound_parents(Id(i)).next().is_some() => {
            c.update_slider_bounds(Id(i));
        }
        Def::GeneralConic(d) => {
            c.update_conic(i, d);
        }
        Def::Path(d) if d.has_state() => {
            c.update_path(i, d);
        }
        Def::Arc(d) if d.has_record() => {
            c.update_arc(i, d);
        }
        // a locus is never between a moving object and the point of a locus
        Def::Locus(_) => {}
        // The locus copy and an isolated optimizer keep existing optima.
        // A shared optimizer cascade handles this definition separately.
        Def::Optimize(_) => {}
        Def::Discrete(d) => {
            c.update_locus(i, |c| d.record(c, c.locus(Id(i))));
        }
        Def::Analysis(_) => {
            c.analysis.clear();
            c.values[i] = c.compute(def, i);
        }
        _ => c.values[i] = c.compute(def, i),
    }
}

/// The points of a locus for the current construction. `owner` is the locus
/// object being computed: a line whose start it freezes remembers it, so a
/// rebuild that drops this entry knows what it has to place itself.
pub(crate) fn record(c: &mut Construction, d: LocusDef, owner: Id) -> LocusData {
    // The reference samples a copy of the construction: the remembered
    // states of the live objects stay as they were, and the copy keeps its
    // own between updates.
    let live = c.analysis.enter_copy(c, owner.0);
    let data = record_samples(c, d, owner);
    c.analysis.leave_copy(owner.0, live);
    data
}

fn record_samples(c: &mut Construction, d: LocusDef, owner: Id) -> LocusData {
    match d {
        LocusDef::Path { point, mover } => record_path(c, point, mover, owner),
        LocusDef::Slider { point, slider } => record_slider(c, point, slider),
        LocusDef::Ode { point, source } => record_ode(c, point, source),
        LocusDef::SlopeField(source) => record_slope_field(c, source, None, None, None),
        LocusDef::Hull(list) => crate::algo::discrete::DiscreteDef::Points {
            graph: crate::algo::discrete::PointGraph::Hull,
            list,
        }
        .record(c, None),
        LocusDef::Numeric(d) => d.record(c),
    }
}

fn slope_source(def: Def) -> Option<Id> {
    match def {
        Def::Locus(LocusDef::SlopeField(function)) => Some(function),
        Def::Locus(LocusDef::Numeric(d)) => d.slope_source(),
        _ => None,
    }
}

// AlgoIntegralODE uses classical Runge-Kutta with step 0.02 and integrates
// the numerator and denominator in both time directions for 20 units.
fn record_ode(c: &Construction, point: Id, source: Id) -> LocusData {
    let function = slope_source(c.def(source)).unwrap_or(source);
    let (Some((x, y)), Some(function)) = (c.point(point), FunctionFraction::new(c, function))
    else {
        return LocusData::default();
    };
    let mut points = Vec::with_capacity(2004);
    for direction in [1.0, -1.0] {
        let (mut x, mut y) = (x, y);
        let mut time: f64 = 0.0;
        let end = 20.0 * direction;
        points.push(LocusPoint {
            x,
            y,
            line_to: false,
        });
        for _ in 0..1001 {
            let remaining = end - time;
            let last = remaining.abs() <= 0.02;
            let h = if last { remaining } else { direction * 0.02 };
            let (dy1, dx1) = function.evaluate(x, y);
            let (dy2, dx2) = function.evaluate(x + h * dx1 / 2.0, y + h * dy1 / 2.0);
            let (dy3, dx3) = function.evaluate(x + h * dx2 / 2.0, y + h * dy2 / 2.0);
            let (dy4, dx4) = function.evaluate(x + h * dx3, y + h * dy3);
            x += h * (dx1 + 2.0 * dx2 + 2.0 * dx3 + dx4) / 6.0;
            y += h * (dy1 + 2.0 * dy2 + 2.0 * dy3 + dy4) / 6.0;
            points.push(LocusPoint {
                x,
                y,
                line_to: true,
            });
            time += h;
            if last {
                break;
            }
        }
    }
    locus_from_points(points)
}

fn record_slope_field(
    c: &Construction,
    source: Id,
    count: Option<f64>,
    length_ratio: Option<f64>,
    bounds: Option<[f64; 4]>,
) -> LocusData {
    let Some(function) = FunctionFraction::new(c, source) else {
        return LocusData::default();
    };
    let [xmin, xmax, ymin, ymax, xs, ys] = c.locus_view.unwrap_or(HEADLESS_VIEW);
    let [xmin, xmax, ymin, ymax] = bounds.unwrap_or([xmin, xmax, ymin, ymax]);
    if !(xmin < xmax && ymin < ymax) {
        return locus_from_points(Vec::new());
    }
    let ratio = ys / xs;
    let divisions = count.map_or(39, |n| (n - 1.0) as i32);
    let divisions = if (2..=100).contains(&divisions) {
        divisions
    } else {
        39
    };
    let x_step = (xmax - xmin) / f64::from(divisions);
    let y_step = (ymax - ymin) / f64::from(divisions);
    let factor = length_ratio
        .filter(|n| (0.0..=1.0).contains(n))
        .unwrap_or(0.5);
    let length = x_step.min(y_step * ratio) * factor * 0.5;
    let mut points = Vec::with_capacity((2 * (divisions + 1).pow(2)) as usize);
    let mut x = xmin;
    for _ in 0..=divisions {
        let mut y = ymin;
        for _ in 0..=divisions {
            if function.exhausted() {
                return LocusData::default();
            }
            let (dy, dx) = function.evaluate(x, y);
            let scaled = dy * ratio;
            let norm = (dx * dx + scaled * scaled).sqrt();
            let (dx, dy) = if dx.abs() < crate::tol::STANDARD_PRECISION
                && dy.abs() < crate::tol::STANDARD_PRECISION
            {
                (0.0, 0.0)
            } else {
                (dx * length / norm, dy * length / norm)
            };
            points.push(LocusPoint {
                x: x - dx,
                y: y - dy,
                line_to: false,
            });
            points.push(LocusPoint {
                x: x + dx,
                y: y + dy,
                line_to: true,
            });
            y += y_step;
        }
        x += x_step;
    }
    locus_from_points(points)
}

/// `AlgoLocusND.compute` for `Locus(Q, P)`.
fn record_path(c: &mut Construction, q: Id, p: Id, owner: Id) -> LocusData {
    let (Def::Path(def), Some(path), Some(state)) = (c.def(p), path_of(c, p), c.path_state(p))
    else {
        return LocusData::default();
    };
    let chain = chain(c, q, p);
    let (min, max, closed) = {
        let g = path::geom(c, path);
        if !state.is_defined() || !g.is_defined() || !valid(c, &chain, q) {
            return LocusData::default();
        }
        let (min, max) = g.range();
        (min, max, is_closed(c, path, &g))
    };
    let mut ids = chain.clone();
    ids.push(p.0);
    let saved = Saved::of(c, &ids);
    let stuck = SavedStuck::of(c, &ids);

    // `resetMacroConstruction`: the copy of P takes P's coordinates
    // (`GeoPoint.set`, which puts them on the path again), and
    // `updateConstruction` runs its `AlgoPointOnPath`.
    let mut st = state;
    {
        let g = path::geom(c, path);
        let (x, y, z) = state.coords();
        st.set_coords_hom(&g, x, y, z);
    }
    st = def.update(c, &st);
    // lines start at 0 rather than at an infinite end
    if !min.is_finite() && 0.0 < max {
        st.set_t(0.0);
    } else {
        st.set_t(min);
    }
    path::freeze_line_start(c, path, owner);
    let copy = PathCopy { id: p, path, st };
    let mover = Mover::new(copy.st.parameter(), min, max, closed, false);
    let data = sample(c, q, &chain, copy, mover, false);
    // before the chain is recomputed with the saved inputs
    stuck.restore(c);
    saved.restore(c, &chain);
    data
}

/// `AlgoLocusSliderND.compute` for `Locus(Q, slider)`.
fn record_slider(c: &mut Construction, q: Id, a: Id) -> LocusData {
    let (Some((min, max)), Value::Number(_)) = (c.slider_interval(a), c.value(a)) else {
        return LocusData::default();
    };
    let chain = chain(c, q, a);
    let mut ids = chain.clone();
    ids.push(a.0);
    let saved = Saved::of(c, &ids);
    let stuck = SavedStuck::of(c, &ids);
    let copy = SliderCopy {
        id: a,
        min,
        max,
        value: min,
    };
    let mover = Mover::new(min, min, max, false, true);
    let data = sample(c, q, &chain, copy, mover, true);
    // before the chain is recomputed with the saved inputs
    stuck.restore(c);
    saved.restore(c, &chain);
    data
}

/// The copy of the moving object (`copyP`).
trait Moving: Clone {
    /// The parameter the cache of `Q` is keyed by.
    fn key(&self) -> f64;
    /// What `PathMover.init` reads.
    fn start_parameter(&self) -> f64;
    /// Writes the copy into the construction, without updating anything.
    fn apply(&self, c: &mut Construction);
    /// `calcPoint`: moves the copy to `param`.
    fn calc(&mut self, c: &Construction, param: f64);
    /// `copyP.set(startPPos)`.
    fn back_to(&mut self, c: &Construction, start: &Self);
}

#[derive(Clone)]
struct PathCopy {
    id: Id,
    path: Id,
    st: PathState,
}

pub(crate) fn sample_path_point(
    c: &mut Construction,
    id: Id,
    path: Id,
    state: PathState,
    parameter: f64,
) -> Value {
    let mut point = PathCopy {
        id,
        path,
        st: state,
    };
    point.calc(c, parameter);
    point.apply(c);
    c.value(id)
}

/// [`sample_path_point`] on the conic record `record` instead of the
/// path's current geometry: a point on a circle that keeps its last record
/// while undefined is still placed on it (`GeoConicND.pathChanged` reads
/// the half axes and midpoint whatever `defined` says).
pub(crate) fn sample_path_point_on(
    c: &mut Construction,
    id: Id,
    record: crate::algo::conic_nd::ConicData,
    mut st: PathState,
    parameter: f64,
) -> Value {
    st.put_t(parameter);
    Geom::Conic(record).path_changed(&mut st);
    st.update_coords();
    if let Some(slot) = c.path_slot(id.0) {
        c.paths[slot].1 = st;
        c.values[id.0] = PathDef::value(&st);
    }
    c.value(id)
}

impl Moving for PathCopy {
    fn key(&self) -> f64 {
        self.st.parameter()
    }

    fn start_parameter(&self) -> f64 {
        self.st.parameter()
    }

    fn apply(&self, c: &mut Construction) {
        if let Some(slot) = c.path_slot(self.id.0) {
            c.paths[slot].1 = self.st;
            c.values[self.id.0] = PathDef::value(&self.st);
        }
    }

    fn calc(&mut self, c: &Construction, param: f64) {
        // `pp.t = param; path.pathChanged(p); p.updateCoords()`
        self.st.put_t(param);
        path::geom(c, self.path).path_changed(&mut self.st);
        self.st.update_coords();
    }

    fn back_to(&mut self, c: &Construction, start: &Self) {
        // `GeoPoint.set` copies the path parameter, then `setCoords` puts
        // the coordinates on the path again
        self.st = start.st;
        let (x, y, z) = start.st.coords();
        self.st.set_coords_hom(&path::geom(c, self.path), x, y, z);
    }
}

#[derive(Clone)]
struct SliderCopy {
    id: Id,
    min: f64,
    max: f64,
    value: f64,
}

impl Moving for SliderCopy {
    fn key(&self) -> f64 {
        self.value
    }

    fn start_parameter(&self) -> f64 {
        self.value
    }

    fn apply(&self, c: &mut Construction) {
        c.defs[self.id.0] = Def::Number(self.value);
        c.values[self.id.0] = Value::Number(self.value);
    }

    fn calc(&mut self, _c: &Construction, param: f64) {
        // `GeoNumeric.setValue` keeps the value in the interval
        self.value = if param.is_nan() {
            f64::NAN
        } else if param < self.min {
            self.min
        } else if param > self.max {
            self.max
        } else {
            param
        };
    }

    fn back_to(&mut self, _c: &Construction, start: &Self) {
        self.value = start.value;
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Bounds {
    Fixed,
    Infinite,
    FixedInfinite,
    InfiniteFixed,
}

/// `PathMoverGeneric`, and `SliderMover` when `slider` is set.
struct Mover {
    slider: bool,
    closed: bool,
    path_min: f64,
    path_max: f64,
    mode: Bounds,
    start: f64,
    start_up: f64,
    curr: f64,
    last: f64,
    min: f64,
    max: f64,
    max_step: f64,
    step: f64,
    offset: f64,
    max_border: bool,
    min_border: bool,
    last_max_border: bool,
    last_min_border: bool,
}

impl Mover {
    fn new(param: f64, path_min: f64, path_max: f64, closed: bool, slider: bool) -> Self {
        let mut m = Mover {
            slider,
            closed,
            path_min,
            path_max,
            mode: Bounds::Fixed,
            start: 0.0,
            start_up: 0.0,
            curr: 0.0,
            last: 0.0,
            min: 0.0,
            max: 0.0,
            max_step: 0.0,
            step: 0.0,
            offset: 0.0,
            max_border: false,
            min_border: false,
            last_max_border: false,
            last_min_border: false,
        };
        m.init(param);
        m
    }

    /// `init(double param)`.
    fn init(&mut self, param: f64) {
        let mut start = param;
        let (mut min, mut max) = (self.path_min, self.path_max);
        if !self.slider && min > max {
            // case should not happen, by the way
            std::mem::swap(&mut min, &mut max);
        }
        // make sure start is between min and max
        if start < min || start > max {
            let extent = max - min;
            start = (start - min) % extent;
            if start < min {
                start += extent;
            }
        }
        if min == f64::NEG_INFINITY {
            if max == f64::INFINITY {
                self.mode = Bounds::Infinite;
                min = -1.0 + OPEN_BORDER_OFFSET;
                max = 1.0 - OPEN_BORDER_OFFSET;
                start = inverse_inf_function(start);
            } else {
                self.mode = Bounds::InfiniteFixed;
                start = 0.0;
                self.offset = max;
                min = -1.0 + OPEN_BORDER_OFFSET;
                max = 0.0;
            }
        } else if max == f64::INFINITY {
            self.mode = Bounds::FixedInfinite;
            start = 0.0;
            self.offset = min;
            min = 0.0;
            max = 1.0 - OPEN_BORDER_OFFSET;
        } else {
            self.mode = Bounds::Fixed;
        }
        let extent = max - min;
        self.min = min;
        self.max = max;
        self.start = start;
        self.start_up = start + extent;
        self.max_step = extent / MIN_STEPS;
        if !self.slider
            && (self.max_step < MIN_STEP_WIDTH
                || self.max_step.is_nan()
                || self.max_step.is_infinite())
        {
            self.max_step = MIN_STEP_WIDTH;
        }
        self.reset_start();
    }

    /// `resetStartParameter`.
    fn reset_start(&mut self) {
        self.curr = self.start;
        self.last = self.start;
        self.max_border = false;
        self.last_max_border = false;
        self.min_border = false;
        self.last_min_border = false;
        self.step = self.max_step;
    }

    /// `getNext` without `calcPoint`: the step to the next parameter; false
    /// when it jumped from one end of the parameter range to the other
    /// (always for a slider, only on an open path for a point on a path).
    fn next(&mut self) -> bool {
        let mut line_to = true;
        self.last = self.curr;
        self.last_max_border = self.max_border;
        self.last_min_border = self.min_border;
        if self.max_border {
            self.curr = self.min;
            line_to = !self.slider && self.closed;
            self.max_border = false;
        } else if self.min_border {
            self.curr = self.max;
            line_to = !self.slider && self.closed;
            self.min_border = false;
        } else {
            let mut new = self.curr + self.step;
            if new >= self.max {
                // slow down by making smaller steps
                while new >= self.max && self.smaller() {
                    new = self.curr + self.step;
                }
                if new >= self.max {
                    new = self.max;
                    self.max_border = true;
                }
            } else if new <= self.min {
                while new <= self.min && self.smaller() {
                    new = self.curr + self.step;
                }
                if new <= self.min {
                    new = self.min;
                    self.min_border = true;
                }
            }
            self.curr = new;
        }
        line_to
    }

    /// The parameter of the path or the value of the slider for the
    /// current position (`calcPoint`).
    fn param(&self) -> f64 {
        match self.mode {
            Bounds::Fixed => self.curr,
            Bounds::Infinite => inf_function(self.curr),
            Bounds::FixedInfinite | Bounds::InfiniteFixed => self.offset + inf_function(self.curr),
        }
    }

    /// `hasNext`: whether the next step stays before the start parameter.
    /// The orientation is always positive: it only changes for continuous
    /// constructions, which this kernel does not have.
    fn has_next(&self) -> bool {
        let next = self.curr + self.step;
        !(self.curr < self.start && next >= self.start
            || self.curr < self.start_up && next >= self.start_up)
    }

    fn smaller(&mut self) -> bool {
        self.change_step(self.step * STEP_DECREASE_FACTOR)
    }

    fn bigger(&mut self) -> bool {
        self.change_step(self.step * STEP_INCREASE_FACTOR)
    }

    /// `changeStep`.
    fn change_step(&mut self, new_step: f64) -> bool {
        if new_step < MIN_STEP_WIDTH && new_step > NEG_MIN_STEP_WIDTH {
            if new_step > 0.0 {
                if self.step == MIN_STEP_WIDTH {
                    return false;
                }
                self.step = MIN_STEP_WIDTH;
                return true;
            } else if new_step < -0.0 {
                if self.step == NEG_MIN_STEP_WIDTH {
                    return false;
                }
                self.step = NEG_MIN_STEP_WIDTH;
                return true;
            }
            if self.step == NEG_MIN_STEP_WIDTH || self.step == MIN_STEP_WIDTH {
                return false;
            }
            self.step = if self.step >= 0.0 {
                MIN_STEP_WIDTH
            } else {
                NEG_MIN_STEP_WIDTH
            };
            true
        } else if new_step.abs() > self.max_step {
            if new_step >= 0.0 {
                if self.step == self.max_step {
                    return false;
                }
                self.step = self.max_step;
                return true;
            }
            if self.step == -self.max_step {
                return false;
            }
            self.step = -self.max_step;
            true
        } else {
            self.step = new_step;
            true
        }
    }

    /// `stepBack`.
    fn step_back(&mut self) {
        self.curr = self.last;
        self.max_border = self.last_max_border;
        self.min_border = self.last_min_border;
    }
}

/// The graphics views a locus measures its steps in: upstream's three
/// (two graphics views and the 3D view), of which only the first is shown.
/// The others keep their zero-initialised bounds, which upstream still
/// consults (see [`Run::distance_ok`]).
pub(crate) struct View {
    visible: [bool; 3],
    max_x: [f64; 3],
    max_y: [f64; 3],
    far_xmin: [f64; 3],
    far_xmax: [f64; 3],
    far_ymin: [f64; 3],
    far_ymax: [f64; 3],
}

impl View {
    /// `updateScreenBorders` and `setMaxDistances` for the shown view.
    fn new([xmin, xmax, ymin, ymax, xscale, yscale]: [f64; 6]) -> Self {
        let mut v = View {
            visible: [true, false, false],
            max_x: [0.0; 3],
            max_y: [0.0; 3],
            far_xmin: [0.0; 3],
            far_xmax: [0.0; 3],
            far_ymin: [0.0; 3],
            far_ymax: [0.0; 3],
        };
        v.max_x[0] = MAX_PIXEL_DIST / xscale;
        v.max_y[0] = MAX_PIXEL_DIST / yscale;
        // a bit more than the screen, so that a locus that leaves and
        // enters it again is not lost too often
        let width = xmax - xmin;
        let height = ymax - ymin;
        v.far_xmin[0] = xmin - width / 2.0;
        v.far_xmax[0] = xmax + width / 2.0;
        v.far_ymin[0] = ymin - height / 2.0;
        v.far_ymax[0] = ymax + height / 2.0;
        v
    }

    /// How many views show the locus (`views`).
    fn count(&self) -> usize {
        let mut n = if self.visible[0] && self.visible[1] {
            2
        } else {
            1
        };
        if self.visible[2] {
            n += 1;
        }
        n
    }
}

/// `MyMath.intervalsIntersect`.
fn intervals_intersect(s1: f64, e1: f64, s2: f64, e2: f64) -> bool {
    (s1 <= s2 && s2 <= e1) || (s1 <= e2 && e2 <= e1) || (s2 <= s1 && s1 <= e2)
}

/// The cache of the last three parameters and the points of `Q` they gave
/// (`paramCache`, `qcopyCache`).
struct Cache {
    params: [f64; 3],
    points: [Option<(f64, f64)>; 3],
    index: usize,
}

impl Cache {
    fn get(&self, param: f64) -> Option<Option<(f64, f64)>> {
        (0..3)
            .find(|&i| param == self.params[i])
            .map(|i| self.points[i])
    }

    fn put(&mut self, param: f64, q: Option<(f64, f64)>) {
        self.index = (self.index + 1) % 3;
        self.params[self.index] = param;
        self.points[self.index] = q;
    }
}

/// One run of the sampling loop (`compute` with its fields).
struct Run<'a, M> {
    q: Id,
    chain: &'a [usize],
    view: View,
    mover: Mover,
    cache: Cache,
    points: Vec<LocusPoint>,
    last: (f64, f64),
    last_far: [bool; 3],
    found: bool,
    start: Option<M>,
    start_q: (f64, f64),
}

impl<M: Moving> Run<'_, M> {
    /// `insertPoint`.
    fn insert(&mut self, (x, y): (f64, f64), line_to: bool) {
        self.points.push(LocusPoint { x, y, line_to });
        self.last = (x, y);
        for i in 0..3 {
            self.last_far[i] = self.far((x, y), i);
        }
    }

    /// `isFarAway`.
    fn far(&self, (x, y): (f64, f64), i: usize) -> bool {
        let v = &self.view;
        x > v.far_xmax[i] || x < v.far_xmin[i] || y > v.far_ymax[i] || y < v.far_ymin[i]
    }

    /// `distanceSmall`.
    fn distance_small(&self, (x, y): (f64, f64), or_instead_of_and: bool) -> bool {
        let v = &self.view;
        let small: [bool; 3] = std::array::from_fn(|i| {
            (x - self.last.0).abs() < v.max_x[i] && (y - self.last.1).abs() < v.max_y[i]
        });
        if or_instead_of_and {
            (0..3).any(|i| small[i] && v.visible[i])
        } else {
            !(0..3).any(|i| !small[i] && v.visible[i])
        }
    }

    /// `distanceOK(Q)`: the step to `q` needs no refinement. A step between
    /// two points far outside a view is fine unless the box around it meets
    /// the area near the view. Upstream asks this of all three views, also
    /// of those that do not show the locus, whose area is the origin: a
    /// step far from the shown view whose box contains the origin is
    /// refined too.
    fn distance_ok(&self, q: (f64, f64)) -> bool {
        (0..3).all(|i| {
            if self.last_far[i] && self.far(q, i) {
                self.distance_ok_in(q, i)
            } else {
                self.distance_small(q, false)
            }
        })
    }

    /// `distanceOK(Q, i)`.
    fn distance_ok_in(&self, (qx, qy): (f64, f64), i: usize) -> bool {
        let v = &self.view;
        let (mut min_x, mut min_y) = self.last;
        let (mut max_x, mut max_y) = (qx, qy);
        if qx < min_x {
            min_x = qx;
            max_x = self.last.0;
        }
        if qy < min_y {
            min_y = qy;
            max_y = self.last.1;
        }
        !intervals_intersect(min_x, max_x, v.far_xmin[i], v.far_xmax[i])
            || !intervals_intersect(min_y, max_y, v.far_ymin[i], v.far_ymax[i])
    }

    /// `pcopyUpdateCascade`: `Q` for the copy's position, from the cache
    /// when the parameter is one of the last three; the first defined point
    /// starts the locus there.
    fn update(&mut self, c: &mut Construction, copy: &M) -> Option<(f64, f64)> {
        let key = copy.key();
        let q = match self.cache.get(key) {
            Some(q) => q,
            None => {
                copy.apply(c);
                for &i in self.chain {
                    // ConsElementXMLHandler.handleSlider skips slider tags
                    // in the macro construction. The mover still takes
                    // its sampled value, but other sliders keep theirs.
                    if !matches!(c.defs[i], Def::Number(_)) {
                        recompute(c, i);
                    }
                }
                let q = c.point(self.q);
                self.cache.put(key, q);
                q
            }
        };
        if !self.found {
            if let Some(v) = q {
                self.mover.init(copy.start_parameter());
                self.start = Some(copy.clone());
                self.start_q = v;
                self.found = true;
                self.insert(v, false);
            }
        }
        q
    }
}

/// The sampling loop of `AlgoLocusND.compute` and, with `slider`,
/// `AlgoLocusSliderND.compute`, for a single run (constructions here are
/// not continuous).
fn sample<M: Moving>(
    c: &mut Construction,
    q: Id,
    chain: &[usize],
    mut copy: M,
    mover: Mover,
    slider: bool,
) -> LocusData {
    let view = View::new(c.locus_view.unwrap_or(HEADLESS_VIEW));
    let views = view.count();
    let mut run = Run {
        q,
        chain,
        view,
        mover,
        cache: Cache {
            params: [f64::NAN; 3],
            points: [None; 3],
            index: 0,
        },
        points: Vec::new(),
        last: (f64::MAX, f64::MAX),
        last_far: [false; 3],
        found: false,
        start: None,
        start_q: (0.0, 0.0),
    };
    // a point on a path is joined when either coordinate step is small
    // enough in a shown view, a point of a slider when both are
    let either = !slider;
    let mut prev_defined = run.update(c, &copy).is_some();
    let max_loops = 2 * MAX_POINTS * views;
    let mut loops = 0;
    let mut finished = false;
    while !finished && run.points.len() <= MAX_POINTS * views && loops <= max_loops {
        loops += 1;
        // lineTo may be false due to a parameter jump
        let jump = !run.mover.next();
        copy.calc(c, run.mover.param());
        let mut step_changed = false;
        let mut qv = run.update(c, &copy);
        if let Some(v) = qv {
            if !jump {
                // make steps smaller until the distance to the last point
                // is fine
                let mut cur = v;
                while !run.distance_ok(cur) {
                    if !run.mover.smaller() {
                        break;
                    }
                    step_changed = true;
                    run.mover.step_back();
                    run.mover.next();
                    copy.calc(c, run.mover.param());
                    qv = run.update(c, &copy);
                    match qv {
                        Some(next) => cur = next,
                        None => break,
                    }
                }
                if let Some(v) = qv {
                    let line_to = run.distance_small(v, either);
                    run.insert(v, line_to);
                    prev_defined = true;
                }
            } else {
                let line_to = !slider && run.distance_small(v, true);
                run.insert(v, line_to);
                prev_defined = true;
            }
        } else if prev_defined && !jump {
            // from defined to undefined: step back and try a smaller step
            run.mover.step_back();
            if !run.mover.smaller() {
                prev_defined = false;
            } else {
                step_changed = true;
            }
        }
        if !step_changed {
            run.mover.bigger();
        }
        // end of run: the next step would pass the start position
        if !run.mover.has_next() {
            if run.distance_small(run.start_q, either) {
                // draw a line back to the first point when it is close
                let start_q = run.start_q;
                run.insert(start_q, true);
                finished = true;
            } else {
                // decrease the step until another step is possible
                while !run.mover.has_next() && run.mover.smaller() {}
                if !run.mover.has_next() {
                    finished = true;
                }
            }
        }
    }
    let Some(start) = run.start.take() else {
        return LocusData::default();
    };
    // put the copy back at its start and look at Q there; an undefined Q
    // differs from any point, as its NaN coordinates do upstream
    copy.back_to(c, &start);
    let v = run.update(c, &copy).unwrap_or((f64::NAN, f64::NAN));
    if v.0 != run.last.0 || v.1 != run.last.1 {
        let line_to = run.distance_small(v, either);
        run.insert(v, line_to);
    }
    locus_from_points(run.points)
}

pub(crate) fn locus_from_points(points: Vec<LocusPoint>) -> LocusData {
    let mut bounds: Option<[f64; 4]> = None;
    for p in &points {
        if p.x.is_finite() && p.y.is_finite() {
            bounds = Some(match bounds {
                Some([xmin, xmax, ymin, ymax]) => {
                    [xmin.min(p.x), xmax.max(p.x), ymin.min(p.y), ymax.max(p.y)]
                }
                None => [p.x, p.x, p.y, p.y],
            });
        }
    }
    LocusData {
        points: Arc::new(points),
        defined: true,
        bounds,
    }
}

impl Group for LocusDef {
    fn kind(&self) -> Kind {
        Kind::Locus
    }

    fn parents(&self) -> Parents {
        match *self {
            LocusDef::Path { point, mover }
            | LocusDef::Slider {
                point,
                slider: mover,
            }
            | LocusDef::Ode {
                point,
                source: mover,
            } => Parents::of(&[point, mover]),
            LocusDef::SlopeField(source) => Parents::of(&[source]),
            LocusDef::Hull(list) => Parents::of(&[list]),
            LocusDef::Numeric(d) => d.parents(),
        }
    }

    /// `AlgoDispatcher.locusCheck`: the moving object is a point on a path
    /// or a slider, `Q` is not on a path, and `Q` depends on the moving
    /// object.
    fn check(&self, c: &Construction) -> Result<(), Error> {
        let (point, mover) = match *self {
            LocusDef::Ode { point, source } => {
                c.expect(point, POINT)?;
                return if slope_source(c.def(source)).is_some() {
                    Ok(())
                } else {
                    c.expect(source, &[Kind::Function])
                };
            }
            LocusDef::SlopeField(source) => {
                return c.expect(source, &[Kind::Function, Kind::FunctionNVar])
            }
            LocusDef::Hull(list) => return c.expect(list, &[Kind::List]),
            LocusDef::Numeric(d) => return d.check(c),
            LocusDef::Path { point, mover }
            | LocusDef::Slider {
                point,
                slider: mover,
            } => (point, mover),
        };
        c.expect(point, POINT)?;
        let wrong = |expected| Error::WrongKind {
            label: c.label(mover).to_owned(),
            expected,
            found: c.kind(mover),
        };
        match self {
            LocusDef::Path { .. } => {
                c.expect(mover, POINT)?;
                if !on_path(c, mover) {
                    return Err(wrong(POINT));
                }
            }
            LocusDef::Slider { .. } => {
                c.expect(mover, NUMBER)?;
                if !c
                    .slider_interval(mover)
                    .is_some_and(|(min, max)| min <= max)
                    || !c.value(mover).is_defined()
                {
                    return Err(wrong(NUMBER));
                }
            }
            LocusDef::Ode { .. }
            | LocusDef::SlopeField(_)
            | LocusDef::Hull(_)
            | LocusDef::Numeric(_) => unreachable!(),
        }
        if on_path(c, point) || !is_ancestor(c, mover, point) {
            return Err(Error::WrongKind {
                label: c.label(point).to_owned(),
                expected: POINT,
                found: c.kind(point),
            });
        }
        Ok(())
    }

    /// The points are kept by the construction (see
    /// [`Construction::locus`]); a locus has no value of its own outside it.
    fn compute(&self, _c: &Construction) -> Option<Value> {
        None
    }

    /// `CmdLocus` with two arguments. Of two points, the one on a path
    /// moves; a number must be a slider.
    fn parse(c: &Construction, func: &str, args: &[&str]) -> Option<Result<Self, String>> {
        if let ("SlopeField", [source]) = (func, args) {
            return Some(crate::script::object(c, source).map(LocusDef::SlopeField));
        }
        if func == "Hull" {
            return Some(match args {
                [list, number] => parse_hull(c, list, number),
                _ => Err(format!("`Hull` does not take {} argument(s)", args.len())),
            });
        }
        // `ConvexHull(list)` is the same algorithm with the list alone
        // (`CmdConvexHull` extends `CmdOneListFunction`).
        if func == "ConvexHull" {
            return Some(match args {
                [list] => crate::script::object(c, list)
                    .map(LocusDef::Hull)
                    .and_then(|def| def.check(c).map_err(|e| e.to_string()).map(|_| def)),
                _ => Err(format!(
                    "`ConvexHull` does not take {} argument(s)",
                    args.len()
                )),
            });
        }
        if func != "Locus" {
            return None;
        }
        Some(parse_locus(c, args))
    }
}

fn parse_locus(c: &Construction, args: &[&str]) -> Result<LocusDef, String> {
    let [a, b] = args else {
        return Err(format!("`Locus` does not take {} argument(s)", args.len()));
    };
    let first = crate::script::object(c, a)?;
    let second = crate::script::object(c, b)?;
    let def = match (c.kind(first), c.kind(second)) {
        (Kind::Point, Kind::Point) if on_path(c, second) => LocusDef::Path {
            point: first,
            mover: second,
        },
        (Kind::Point, Kind::Point) => LocusDef::Path {
            point: second,
            mover: first,
        },
        (Kind::Point, Kind::Number) => LocusDef::Slider {
            point: first,
            slider: second,
        },
        (Kind::Function | Kind::Locus, Kind::Point) => LocusDef::Ode {
            point: second,
            source: first,
        },
        (x, y) => return Err(format!("`Locus` of a {x} and a {y} is not supported")),
    };
    def.check(c).map_err(|_| match def {
        LocusDef::Path { .. } => {
            format!("`Locus({a}, {b})` needs a point on a path and a point that depends on it")
        }
        LocusDef::Slider { .. } => {
            format!("`Locus({a}, {b})` needs a point that depends on the slider `{b}`")
        }
        LocusDef::Ode { .. } => {
            format!("`Locus({a}, {b})` needs a slope function or field and a point")
        }
        LocusDef::SlopeField(_) | LocusDef::Hull(_) | LocusDef::Numeric(_) => unreachable!(),
    })?;
    Ok(def)
}

/// `CmdHull`: exactly a list and a number, and the number is only there to
/// be ignored (the command once had a parameter, and `AlgoConvexHull` runs
/// without it).
fn parse_hull(c: &Construction, list: &str, number: &str) -> Result<LocusDef, String> {
    let list = crate::script::object(c, list)?;
    if c.kind(list) != Kind::List {
        return Err(format!(
            "`Hull` needs a list of points, found `{}`",
            c.label(list)
        ));
    }
    let number_is_numeric = crate::expr::plain_number(number).is_some()
        || c.id(number).is_some_and(|id| c.kind(id) == Kind::Number);
    if !number_is_numeric {
        return Err(format!(
            "`Hull` needs a number as its second argument, found `{number}`"
        ));
    }
    Ok(LocusDef::Hull(list))
}

#[cfg(test)]
mod tests {
    use super::OptimizerCascade;
    use crate::{script, Def, Value};

    #[test]
    fn optimizer_visit_limit_restores_values() {
        let mut c =
            script::load("a = Slider(0, 6)\nf = (a - 3)^2\nm = Minimize(f, a)\nn = Minimize(f, a)")
                .unwrap();
        let [a, f, m, n] = ["a", "f", "m", "n"].map(|label| c.id(label).unwrap());
        c.set_number(a, 4.0).unwrap();

        let Def::Optimize(def) = c.def(n) else {
            panic!("expected an optimizer");
        };
        let mut cascade = OptimizerCascade::new(&c, n, a).unwrap();
        // Spend the last visit at this depth, then stop before the next.
        assert_eq!(cascade.remaining[0], OptimizerCascade::MAX_VISITS_PER_DEPTH);
        cascade.remaining[0] = 1;
        assert_eq!(cascade.run(&mut c, n, def), Value::Undefined);
        assert!(cascade.exhausted);
        assert_eq!(cascade.remaining[0], 0);
        assert_eq!(cascade.remaining[1], OptimizerCascade::MAX_VISITS_PER_DEPTH);
        assert_eq!(c.value(a), Value::Number(4.0));
        assert_eq!(c.value(f), Value::Number(1.0));
        assert_eq!(c.value(m), Value::Number(3.0));
    }

    #[test]
    fn slider_bounds_do_not_clamp_a_second_slider_during_a_trace() {
        for (source, mover, endpoint, coordinate) in [
            ("t = Slider(0, 4)", "t", "t", "t"),
            (
                "A = (0, 0)\nB = (4, 0)\ng = Segment(A, B)\nP = Point(g)",
                "P",
                "x(P)",
                "x(P)",
            ),
        ] {
            let mut c = script::load(&format!("{source}\ns = Slider(0, {endpoint})")).unwrap();
            let driver = c.id(mover).unwrap();
            let s = c.id("s").unwrap();
            if mover == "t" {
                c.set_number(driver, 4.0).unwrap();
            } else {
                c.move_point(driver, 4.0, 0.0).unwrap();
            }
            c.set_number(s, 3.0).unwrap();
            script::exec(&mut c, &format!("Q = ({coordinate}, s)")).unwrap();
            script::exec(&mut c, &format!("L = Locus(Q, {mover})")).unwrap();
            let points = &c.locus(c.id("L").unwrap()).unwrap().points;
            assert!(points.len() > 1);
            assert!(points.iter().all(|p| p.y == 3.0));
            assert_eq!(c.value(s), Value::Number(3.0));
            if mover == "t" {
                c.set_number(driver, 2.0).unwrap();
            } else {
                c.move_point(driver, 2.0, 0.0).unwrap();
            }
            assert_eq!(c.value(s), Value::Number(2.0));
        }
    }

    /// A locus samples a copy of the construction in the reference: a
    /// sample where the `SVD` input is undefined must not make the live
    /// result undefined for good (case `funcrest-svd-025`).
    #[test]
    fn locus_sampling_leaves_svd_defined() {
        let mut c = script::load(
            "c = Circle((0, 0), 2)\nA = Point(c)\nm = If(x(A) > 0, {{x(A), 1}, {2, 3}})\n\
             s = SVD(m)\nQ = (Length(s), y(A))\nL = Locus(Q, A)\n",
        )
        .unwrap();
        let (a, s) = (c.id("A").unwrap(), c.id("s").unwrap());
        c.move_point(a, 1.2, 1.6).unwrap();
        assert!(matches!(c.value(s), Value::List { len: 3 }));
        c.move_point(a, 2.0, 0.0).unwrap();
        assert!(matches!(c.value(s), Value::List { len: 3 }));
        // an ordinary update with an undefined input still sticks
        c.move_point(a, -2.0, 0.0).unwrap();
        c.move_point(a, 2.0, 0.0).unwrap();
        assert!(!c.value(s).is_defined());
    }
}
