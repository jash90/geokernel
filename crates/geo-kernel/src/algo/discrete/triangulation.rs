// Translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/discrete/delaunay/
//   DelaunayTriangulation.java, TriangleDt.java, PointDt.java, ComparePoint.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use super::{dist_sq, Budget, Point};
use crate::expr::math::{is_equal, is_greater};
use std::cmp::Ordering;
use std::collections::VecDeque;

const AB: usize = 0;
const BC: usize = 1;
const CA: usize = 2;
const NONE: usize = usize::MAX;

#[derive(Clone, Copy)]
struct Face {
    /// The infinite faces have only A and B.
    vertex: [usize; 3],
    next: [usize; 3],
    center: Point,
    radius_sq: f64,
}

impl Face {
    fn outer(self) -> bool {
        self.vertex[2] == NONE
    }
}

#[derive(PartialEq, Eq)]
enum Side {
    Left,
    Right,
    On,
    Before,
    After,
}

pub(super) struct Triangulation<'a> {
    points: &'a [Point],
    faces: Vec<Face>,
    start: usize,
    first: usize,
    last: usize,
}

impl<'a> Triangulation<'a> {
    pub(super) fn new(points: &'a [Point], work: &mut Budget) -> Option<Self> {
        let mut dt = Self {
            points,
            faces: Vec::new(),
            start: NONE,
            first: NONE,
            last: NONE,
        };
        let mut vertices = super::ordered::Tree::default();
        for (p, &xy) in points.iter().enumerate() {
            if vertices.insert(xy, point_cmp, work)? {
                dt.insert(p, work)?;
            }
        }
        (dt.start != NONE).then_some(dt)
    }

    fn add(&mut self, a: usize, mut b: usize, mut c: usize) -> usize {
        if c != NONE && self.side(c, a, b) == Side::Right {
            std::mem::swap(&mut b, &mut c);
        }
        let id = self.faces.len();
        self.faces.push(Face {
            vertex: [a, b, c],
            next: [NONE; 3],
            center: (0.0, 0.0),
            radius_sq: 0.0,
        });
        if c != NONE {
            self.circumcircle(id);
        }
        id
    }

    fn circumcircle(&mut self, id: usize) {
        let f = &mut self.faces[id];
        let [a, b, c] = f.vertex.map(|v| self.points[v]);
        let (abx, aby) = (a.0 - b.0, a.1 - b.1);
        let (bcx, bcy) = (b.0 - c.0, b.1 - c.1);
        let u = (abx * (a.0 + b.0) + aby * (a.1 + b.1)) / 2.0;
        let v = (bcx * (b.0 + c.0) + bcy * (b.1 + c.1)) / 2.0;
        let den = abx * bcy - bcx * aby;
        (f.center, f.radius_sq) = if den == 0.0 {
            (a, f64::INFINITY)
        } else {
            let center = ((u * bcy - v * aby) / den, (v * abx - u * bcx) / den);
            (center, dist_sq(center, a))
        };
    }

    fn side(&self, p: usize, a: usize, b: usize) -> Side {
        let ((x, y), (ax, ay), (bx, by)) = (self.points[p], self.points[a], self.points[b]);
        let (dx, dy) = (bx - ax, by - ay);
        let cross = dy * (x - ax) - dx * (y - ay);
        if cross < 0.0 {
            return Side::Left;
        }
        if cross > 0.0 {
            return Side::Right;
        }
        let (d, offset) = if dx == 0.0 {
            (dy, y - ay)
        } else {
            (dx, x - ax)
        };
        if offset != 0.0 && offset.is_sign_positive() != d.is_sign_positive() {
            Side::Before
        } else if offset.abs() > d.abs() {
            Side::After
        } else {
            Side::On
        }
    }

    fn replace_neighbor(&mut self, f: usize, old: usize, new: usize) -> Option<()> {
        let edge = self
            .faces
            .get_mut(f)?
            .next
            .iter_mut()
            .find(|n| **n == old)?;
        *edge = new;
        Some(())
    }

    fn insert(&mut self, p: usize, work: &mut Budget) -> Option<()> {
        work.charge(1)?;
        if self.start != NONE {
            let face = self.find(p, work)?;
            self.start = if self.faces[face].outer() {
                self.extend_outside(face, p, work)?
            } else {
                self.extend_inside(face, p, work)?
            };
            let mut t = self.start;
            loop {
                work.charge(1)?;
                self.flip(t, work)?;
                t = self.faces[t].next[CA];
                if t == self.start || self.faces.get(t)?.outer() {
                    break;
                }
            }
        } else if self.last == NONE {
            if self.first == NONE {
                self.first = p;
            } else {
                let mut a = self.first;
                let mut b = p;
                if point_cmp(self.points[a], self.points[b]) == Ordering::Greater {
                    std::mem::swap(&mut a, &mut b);
                }
                let f = self.add(b, a, NONE);
                let g = self.add(a, b, NONE);
                self.faces[f].next = [g; 3];
                self.faces[g].next = [f; 3];
                self.first = f;
                self.last = f;
            }
        } else {
            let first = self.faces[self.first].vertex[1];
            let last = self.faces[self.last].vertex[0];
            match self.side(p, first, last) {
                Side::Left => {
                    self.start = self.extend_outside(self.faces[self.first].next[AB], p, work)?;
                }
                Side::Right => self.start = self.extend_outside(self.first, p, work)?,
                Side::Before => {
                    let f = self.first;
                    let g = self.faces[f].next[AB];
                    let t = self.add(first, p, NONE);
                    let u = self.add(p, first, NONE);
                    self.faces[t].next = [u, u, f];
                    self.faces[u].next = [t, g, t];
                    self.faces[f].next[BC] = t;
                    self.faces[g].next[CA] = u;
                    self.first = t;
                }
                Side::After => {
                    let f = self.last;
                    let g = self.faces[f].next[AB];
                    let t = self.add(p, last, NONE);
                    let u = self.add(last, p, NONE);
                    self.faces[t].next = [u, f, u];
                    self.faces[u].next = [t, t, g];
                    self.faces[f].next[CA] = t;
                    self.faces[g].next[BC] = u;
                    self.last = t;
                }
                Side::On => {
                    let mut f = self.first;
                    while point_cmp(self.points[p], self.points[self.faces[f].vertex[0]])
                        == Ordering::Greater
                    {
                        work.charge(1)?;
                        f = self.faces[f].next[CA];
                    }
                    let old = self.faces[f];
                    let g = old.next[AB];
                    let t = self.add(p, old.vertex[1], NONE);
                    let u = self.add(old.vertex[1], p, NONE);
                    self.faces[f].vertex[1] = p;
                    self.faces[g].vertex[0] = p;
                    self.faces[t].next = [u, old.next[BC], f];
                    self.faces[old.next[BC]].next[CA] = t;
                    self.faces[f].next[BC] = t;
                    // With two old sites, the write above updates g's CA
                    // link as well. Read it after splitting that edge.
                    let gca = self.faces[g].next[CA];
                    self.faces[u].next = [t, g, gca];
                    self.faces[gca].next[BC] = u;
                    self.faces[g].next[CA] = u;
                    if self.first == f {
                        self.first = t;
                    }
                }
            }
        }
        Some(())
    }

    fn find(&self, p: usize, work: &mut Budget) -> Option<usize> {
        let mut face = self.start;
        let mut previous = NONE;
        loop {
            work.charge(1)?;
            let f = *self.faces.get(face)?;
            let [a, b, c] = f.vertex;
            let sides = [self.side(p, a, b), self.side(p, b, c), self.side(p, c, a)];
            let mut outer = None;
            let mut next = None;
            for (side, neighbor) in sides.into_iter().zip(f.next) {
                if side == Side::Right {
                    if self.faces.get(neighbor)?.outer() {
                        outer = outer.or(Some(neighbor));
                    } else {
                        next = Some(neighbor);
                        break;
                    }
                }
            }
            if let Some(n) = next {
                if n == previous {
                    return None;
                }
                (previous, face) = (face, n);
            } else {
                return outer.or(Some(face));
            }
        }
    }

    fn extend_inside(&mut self, t: usize, p: usize, work: &mut Budget) -> Option<usize> {
        let f = self.faces[t];
        let [a, b, c] = f.vertex;
        for (i, n) in f.next.into_iter().enumerate() {
            let hull = *self.faces.get(n)?;
            if hull.outer() && self.side(p, f.vertex[(i + 1) % 3], f.vertex[i]) == Side::On {
                return self.extend_outside(n, p, work);
            }
        }
        let h1 = self.add(c, a, p);
        let h2 = self.add(b, c, p);
        self.faces[h1].next = [f.next[CA], t, h2];
        self.faces[h2].next = [f.next[BC], h1, t];
        self.replace_neighbor(f.next[CA], t, h1)?;
        self.replace_neighbor(f.next[BC], t, h2)?;
        self.faces[t].vertex[2] = p;
        self.faces[t].next[BC] = h2;
        self.faces[t].next[CA] = h1;
        self.circumcircle(t);
        Some(t)
    }

    fn extend_outside(&mut self, t: usize, p: usize, work: &mut Budget) -> Option<usize> {
        let f = self.faces[t];
        let [a, b, _] = f.vertex;
        if self.side(p, a, b) == Side::On {
            let inner = self.add(a, b, p);
            let outer = self.add(p, b, NONE);
            self.faces[inner].next = [f.next[AB], outer, t];
            self.faces[outer].next = [inner, f.next[BC], t];
            self.replace_neighbor(f.next[AB], t, inner)?;
            self.faces[t].vertex[1] = p;
            self.faces[t].next[AB] = inner;
            self.faces[t].next[BC] = outer;
            self.faces[f.next[BC]].next[CA] = outer;
            return Some(inner);
        }
        let ca = self.extend_hull(t, p, CA, BC, work)?;
        let bc = self.extend_hull(t, p, BC, CA, work)?;
        self.faces[ca].next[BC] = bc;
        self.faces[bc].next[CA] = ca;
        Some(self.faces[bc].next[AB])
    }

    fn extend_hull(
        &mut self,
        mut t: usize,
        p: usize,
        edge: usize,
        other: usize,
        work: &mut Budget,
    ) -> Option<usize> {
        loop {
            work.charge(1)?;
            self.faces[t].vertex[2] = p;
            self.circumcircle(t);
            let n = self.faces[t].next[edge];
            let [a, b, _] = self.faces.get(n)?.vertex;
            if !matches!(self.side(p, a, b), Side::Left | Side::On) {
                let [a, b, _] = self.faces[t].vertex;
                let hull = if edge == CA {
                    self.add(a, p, NONE)
                } else {
                    self.add(p, b, NONE)
                };
                self.faces[hull].next[AB] = t;
                self.faces[t].next[edge] = hull;
                self.faces[hull].next[edge] = n;
                self.faces[n].next[other] = hull;
                return Some(hull);
            }
            t = n;
        }
    }

    fn flip(&mut self, t: usize, work: &mut Budget) -> Option<()> {
        let mut pending = vec![t];
        while let Some(t) = pending.pop() {
            work.charge(1)?;
            let f = self.faces[t];
            let [a, b, c] = f.vertex;
            let u = f.next[AB];
            let g = *self.faces.get(u)?;
            if g.outer()
                || g.radius_sq.partial_cmp(&dist_sq(g.center, self.points[c]))
                    != Some(Ordering::Greater)
            {
                continue;
            }
            let i = g.vertex.iter().position(|&v| v == a)?;
            let v = u;
            let mut corners = [g.vertex[(i + 1) % 3], b, c];
            if self.side(c, corners[0], b) == Side::Right {
                corners.swap(1, 2);
            }
            self.faces[v].vertex = corners;
            self.faces[v].next = [g.next[(i + 1) % 3], f.next[BC], t];
            self.circumcircle(v);
            self.replace_neighbor(self.faces[v].next[AB], u, v)?;
            self.replace_neighbor(f.next[BC], t, v)?;
            self.faces[t].next[AB] = g.next[i];
            self.faces[t].next[BC] = v;
            self.faces[t].vertex[1] = self.faces[v].vertex[0];
            self.replace_neighbor(g.next[i], u, t)?;
            self.circumcircle(t);
            pending.extend([v, t]);
        }
        Some(())
    }

    fn order(&self, work: &mut Budget) -> Option<Vec<usize>> {
        work.charge(self.faces.len())?;
        let mut seen = vec![false; self.faces.len()];
        let mut queue = VecDeque::from([self.start]);
        let mut order = Vec::new();
        while let Some(t) = queue.pop_front() {
            work.charge(1)?;
            if std::mem::replace(seen.get_mut(t)?, true) {
                continue;
            }
            order.push(t);
            queue.extend(self.faces[t].next.into_iter().filter(|n| *n != NONE));
        }
        Some(order)
    }

    pub(super) fn edges(&self, voronoi: bool, work: &mut Budget) -> Option<Vec<[Point; 2]>> {
        let mut result = super::ordered::Tree::default();
        for t in self.order(work)? {
            work.charge(1)?;
            let f = self.faces[t];
            if !voronoi {
                for i in 0..if f.outer() { 1 } else { 3 } {
                    let edge = [f.vertex[i], f.vertex[(i + 1) % 3]].map(|v| self.points[v]);
                    result.insert(edge, edge_cmp, work)?;
                }
            } else if f.outer() {
                let ray = self.ray(f)?;
                result.insert(ray, edge_cmp, work)?;
                result.insert(ray, edge_cmp, work)?;
            } else {
                for vertex in f.vertex {
                    self.cell_edges(t, vertex, &mut result, work)?;
                }
            }
        }
        result.into_values(work)
    }

    fn cell_edges(
        &self,
        start: usize,
        p: usize,
        result: &mut super::ordered::Tree<[Point; 2]>,
        work: &mut Budget,
    ) -> Option<()> {
        let mut prev = NONE;
        let mut t = start;
        let mut edges = Vec::new();
        loop {
            work.charge(1)?;
            let f = self.faces[t];
            let corner = f.vertex.iter().position(|&v| v == p)?;
            let mut n = f.next[(corner + 2) % 3];
            if n == prev || self.faces.get(n)?.outer() {
                n = f.next[corner];
            }
            if n == start {
                break;
            }
            let face = *self.faces.get(n)?;
            if face.outer() {
                return Some(());
            }
            edges.push([f.center, face.center]);
            (prev, t) = (t, n);
        }
        for edge in edges {
            result.insert(edge, edge_cmp, work)?;
        }
        Some(())
    }

    fn ray(&self, f: Face) -> Option<[Point; 2]> {
        let inner = f.next[..2]
            .iter()
            .filter_map(|&n| self.faces.get(n))
            .find(|n| !n.outer())?;
        let [a, b, _] = f.vertex;
        let c = *inner.vertex.iter().find(|&&v| v != a && v != b)?;
        let (a, b, c) = (self.points[a], self.points[b], self.points[c]);
        let slope = (a.1 - b.1) / (a.0 - b.0);
        let perpendicular = -1.0 / slope;
        let above = slope * (c.0 - a.0) + a.1 <= c.1;
        let dx = if (perpendicular < 0.0 && !above) || (perpendicular > 0.0 && above) {
            -500.0
        } else {
            500.0
        };
        let start = inner.center;
        let x = start.0 + dx;
        Some([start, (x, perpendicular * (x - start.0) + start.1)])
    }
}

fn point_cmp(a: Point, b: Point) -> Ordering {
    for (a, b) in [(a.0, b.0), (a.1, b.1)] {
        if is_greater(a, b) {
            return Ordering::Greater;
        }
        if is_greater(b, a) {
            return Ordering::Less;
        }
    }
    Ordering::Equal
}

fn edge_cmp(a: [Point; 2], b: [Point; 2]) -> Ordering {
    let equal = |p: Point, q: Point| is_equal(p.0, q.0) && is_equal(p.1, q.1);
    if (equal(a[0], b[1]) && equal(a[1], b[0])) || (equal(a[0], b[0]) && equal(a[1], b[1])) {
        Ordering::Equal
    } else if dist_sq(a[0], a[1]) > dist_sq(b[0], b[1]) {
        Ordering::Less
    } else {
        Ordering::Greater
    }
}
