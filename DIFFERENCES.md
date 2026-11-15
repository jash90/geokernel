# Deliberate differences from GeoGebra

Behaviour that differs from GeoGebra on purpose is recorded here, with the
reason. A difference that is not listed here is a bug.

Cases that the kernel cannot reproduce *yet* do not belong here. They are
listed in `crates/geo-golden/known-gaps.toml`.

| Area | GeoGebra | geokernel | Reason |
| --- | --- | --- | --- |
| Objects created by a command with several results (`Tangent`, `AngularBisector` of two lines) | The set of labelled objects can change over time: a result that is undefined when the command runs gets its label only once it becomes defined. | The objects are fixed when the command runs. All results that are defined at that moment are created (`t_{1}`, `t_{2}`); when at most one is defined, only that one (or the first) is created under the plain name. | A construction whose list of objects depends on the history of moves cannot be replayed or saved reliably. |
| `Diameter(line, circle)` of an undefined circle (`conic-diameter-007`) | The line keeps the value it had while the circle was last defined. | The line is undefined. | The GeoGebra value depends on the history of moves and no longer belongs to any circle. |
| Objects created with a polygon (`Polygon(A, B, n)`, `Vertex(p)`, `InteriorAngles(p)`, `Angle(p)`) | When `n` grows past its value at creation, `Polygon(A, B, n)` creates more vertices and sides; `Vertex(p)` and the angle commands create more outputs when `p` gets more vertices (and `Vertex(p)` of a polygon that is undefined at creation starts with one). | The objects are fixed when the command runs. The polygon itself still has all `round(n)` vertices (`Construction::vertices`), and its area, perimeter and centroid use them; only the extra point, segment and angle objects are missing. | Same as for commands with several results: the list of objects must not depend on the history of moves. |
| Regular polygon with very many vertices | No limit on `n`. | `Polygon(A, B, n)` with `round(n)` above 1000 is refused when it is created, and the polygon is undefined while `round(n)` is above 1000 later. | A regular polygon creates `n` objects and computes `n` vertices on every update; a stray large number would stall the application. |
| Polyline through a point at infinity | The point counts as defined, so the length is not finite. | The point is undefined (the kernel has no points at infinity), so it leaves a hole and the length stays finite. | The kernel represents a point at infinity as undefined everywhere. |
