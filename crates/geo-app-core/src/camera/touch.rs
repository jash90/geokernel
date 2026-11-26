// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/controller/MouseTouchGestureController.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use crate::app::App;
use crate::hit::{self, Reach};
use crate::interact::ViewTransform;
use crate::view::Camera;
use geo_kernel::{Def, Id, Value};

type Position = (f64, f64);

/// Both object picks may scan the construction, including the touch retry.
const MAX_TOUCH_PICK_OBJECTS: usize = 1024;

fn difference(a: Position, b: Position) -> Position {
    (a.0 - b.0, a.1 - b.1)
}

fn distance(a: Position, b: Position) -> f64 {
    let (x, y) = difference(a, b);
    x.hypot(y)
}

/// A two-finger gesture started on the same axis or object at both touches.
#[derive(Clone, Copy, Debug)]
pub enum TouchTarget {
    Axis {
        x: bool,
        span: f64,
        scale: f64,
    },
    Circle {
        through: Id,
        center: Position,
        start: Position,
        span: f64,
    },
    Line {
        endpoints: [Id; 2],
        points: [Position; 2],
        fingers: [Position; 2],
        crossed: bool,
    },
}

impl TouchTarget {
    pub fn start(
        app: &App,
        camera: Camera,
        t: &ViewTransform,
        [a, b]: [Position; 2],
    ) -> Option<Self> {
        let reach = Reach::of(&app.metrics, true);
        let (ox, oy) = t.world_to_screen(0.0, 0.0);
        if app.decor.axes {
            if a.1 != b.1 && [a, b].iter().all(|p| (p.0 - ox).abs() < reach.threshold) {
                return Some(Self::Axis {
                    x: false,
                    span: a.1 - b.1,
                    scale: camera.yscale,
                });
            }
            if a.0 != b.0 && [a, b].iter().all(|p| (p.1 - oy).abs() < reach.threshold) {
                return Some(Self::Axis {
                    x: true,
                    span: a.0 - b.0,
                    scale: camera.xscale,
                });
            }
        }
        let c = app.wb.construction();
        if c.len() > MAX_TOUCH_PICK_OBJECTS {
            return None;
        }
        let pick = |pos| {
            hit::pick(c, t, pos, &reach, None, |id| {
                app.wb.is_visible(id) && c.appearance(id).selection_allowed.unwrap_or(true)
            })
        };
        let id = pick(a)?;
        if pick(b) != Some(id) || c.appearance(id).fixed == Some(true) || a == b {
            return None;
        }
        let free =
            |id| matches!(c.def(id), Def::FreePoint { .. }) && c.appearance(id).fixed != Some(true);
        let point = |id| match c.value(id) {
            Value::Point { x, y } => Some((x, y)),
            _ => None,
        };
        match c.def(id) {
            Def::CircleThrough { center, through } if free(center) && free(through) => {
                Some(Self::Circle {
                    through,
                    center: point(center)?,
                    start: point(through)?,
                    span: distance(a, b),
                })
            }
            Def::Line(first, second) | Def::Segment(first, second)
                if free(first) && free(second) =>
            {
                let points = [point(first)?, point(second)?];
                let mut fingers = [a, b].map(|(x, y)| t.screen_to_world(x, y));
                let crossed = distance(fingers[0], points[0]) + distance(fingers[1], points[1])
                    > distance(fingers[1], points[0]) + distance(fingers[0], points[1]);
                if crossed {
                    fingers.swap(0, 1);
                }
                Some(Self::Line {
                    endpoints: [first, second],
                    points,
                    fingers,
                    crossed,
                })
            }
            _ => None,
        }
    }

    pub fn moves_geometry(self) -> bool {
        !matches!(self, Self::Axis { .. })
    }

    pub fn update(
        self,
        app: &mut App,
        camera: &mut Camera,
        size: (f64, f64),
        [a, b]: [Position; 2],
    ) {
        if a == b || (self.moves_geometry() && app.wb.construction().len() > MAX_TOUCH_PICK_OBJECTS)
        {
            return;
        }
        match self {
            Self::Axis { x, span, scale } => {
                let span_now = if x { a.0 - b.0 } else { a.1 - b.1 };
                let next = scale * span_now / span;
                if next.is_finite() && next > 0.0 {
                    let next = app.rules.scale_limits.clamp(next);
                    if x {
                        camera.cx *= camera.xscale / next;
                        camera.xscale = next;
                    } else {
                        camera.cy *= camera.yscale / next;
                        camera.yscale = next;
                    }
                }
            }
            Self::Circle {
                through,
                center: (cx, cy),
                start: (x, y),
                span,
            } => {
                let scale = distance(a, b) / span;
                app.wb
                    .move_point(through, cx + (x - cx) * scale, cy + (y - cy) * scale);
            }
            Self::Line {
                endpoints,
                points,
                fingers,
                crossed,
            } => {
                let view = camera.transform(size.0, size.1);
                let mut next = [a, b].map(|(x, y)| view.screen_to_world(x, y));
                if crossed {
                    next.swap(0, 1);
                }
                let (dx, dy) = difference(next[1], next[0]);
                let length2 = dx * dx + dy * dy;
                if !length2.is_finite() || length2 == 0.0 {
                    return;
                }
                let positions = [0, 1].map(|i| {
                    let (x, y) = if next == fingers {
                        // Preserve the original endpoints, including a slight
                        // offset between the line and the touch pixels.
                        points[i]
                    } else {
                        let (px, py) = difference(points[i], next[0]);
                        let along = (px * dx + py * dy) / length2;
                        let (tx, ty) = difference(next[i], fingers[i]);
                        (next[0].0 + dx * along + tx, next[0].1 + dy * along + ty)
                    };
                    (endpoints[i], x, y)
                });
                let c = app.wb.construction();
                if positions.iter().any(|&(id, x, y)| {
                    !matches!(c.value(id), Value::Point { x: old_x, y: old_y }
                        if old_x.to_bits() == x.to_bits() && old_y.to_bits() == y.to_bits())
                }) {
                    app.wb.move_paired_points(positions);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workbench::Workbench;
    use std::fmt::Write;

    #[test]
    fn two_finger_object_pick_refuses_over_budget_constructions() {
        let mut script = String::from("A = (0, 1)\nB = (0, 3)\nc = Circle(A, B)\n");
        for i in 0..MAX_TOUCH_PICK_OBJECTS - 3 {
            writeln!(script, "n_{{{i}}} = {i}").unwrap();
        }
        let mut wb = Workbench::default();
        wb.load_script(&script).unwrap();
        let mut app = App::new(wb, None);
        let camera = Camera::STANDARD;
        let t = camera.transform(892.0, 747.0);
        let fingers = [t.world_to_screen(-2.0, 1.0), t.world_to_screen(2.0, 1.0)];
        assert!(matches!(
            TouchTarget::start(&app, camera, &t, fingers),
            Some(TouchTarget::Circle { .. })
        ));
        app.wb.run_lines(&["extra = 1".into()]).unwrap();
        assert!(TouchTarget::start(&app, camera, &t, fingers).is_none());
    }
}
