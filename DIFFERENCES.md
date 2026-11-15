# Deliberate differences from GeoGebra

Behaviour that differs from GeoGebra on purpose is recorded here, with the
reason. A difference that is not listed here is a bug.

Cases that the kernel cannot reproduce *yet* do not belong here. They are
listed in `crates/geo-golden/known-gaps.toml`.

| Area | GeoGebra | geokernel | Reason |
| --- | --- | --- | --- |
| Objects created by a command with several results (`Tangent`, `AngularBisector` of two lines) | The set of labelled objects can change over time: a result that is undefined when the command runs gets its label only once it becomes defined. | The objects are fixed when the command runs. All results that are defined at that moment are created (`t_{1}`, `t_{2}`); when at most one is defined, only that one (or the first) is created under the plain name. | A construction whose list of objects depends on the history of moves cannot be replayed or saved reliably. |
| `Diameter(line, circle)` of an undefined circle (`conic-diameter-007`) | The line keeps the value it had while the circle was last defined. | The line is undefined. | The GeoGebra value depends on the history of moves and no longer belongs to any circle. |
| Predicates on undefined circles (`predicates-equal-017`, `predicates-tangent-013`) | An undefined circle keeps the conic matrix of its last defined state, and `AreEqual`, `IsTangent` still use it. | The circle has no value, so the predicate has none either. | The matrix depends on the history of moves. |
| Points at infinity as inputs of `AreEqual` and `ClosestPoint` (`predicates-infinity-001`, `points-infinity-001`) | The intersection of parallel lines is a defined point at infinity; `AreEqual(S, S)` is true and `ClosestPoint(circle, S)` projects its direction. | Points at infinity are undefined, so `AreEqual(S, S)` is false and `ClosestPoint(circle, S)` is undefined. | The kernel keeps no homogeneous weight on points. `AreCollinear`, `AreConcyclic`, `AreCongruent` and `AreEqual(S, F)` with such a point agree in the same case. |
| `AreConcyclic` of intersection points (`predicates-homogeneous-001`) | Reads the homogeneous coordinates, whose weight is the raw cross product of the lines. | Assumes weight 1. | Same reason. Only this one configuration is known to differ; other predicates and `ClosestPoint` on such points are covered by cases that agree, but whether the assumption holds in general is not verified. |
| `ClosestPoint(vector, point)` and `ClosestPoint` of two segments or rays | `GeoVector` is a `Path` in GeoGebra (source: `GeoVector implements Path`), so a vector is accepted. Two segments or rays are `GeoLine`s and give `AlgoClosestPointLines`. | Rejected with an error. Only two lines are supported (their intersection, as `AlgoClosestPointLines` is `AlgoIntersectLines`). | The kernel stores no start point for a vector. Not observed with a golden case: a case script that the kernel cannot load fails the script corpus test, so these forms stay unverified. |
| `TriangleCenter`, `ClosestPointRegion` | Available. | Not implemented. | `TriangleCenter` needs the large `AlgoKimberlingWeights` table. |

## Opening construction files (`geo-import`)

| Area | GeoGebra | geokernel | Reason |
| --- | --- | --- | --- |
| `$point(x, y)` in the script language | The parser's `$point` follows the label rule: with a lower-case label the result is a vector. The XML expresses a lower-case point with `type="point"` on the `<expression>`. | `a = $point(1, 2)` is a free point whatever the label. `$vector(x, y)` is a free vector whatever the label, as in GeoGebra. | A saved file has lower-case points and upper-case vectors, and the script has to reproduce the right kind. |
| Which outputs a command creates, in a saved file | Labelled outputs are those that were defined when the command ran. | `{t_{1}, t_{2}} = Tangent(P, k)` creates exactly the outputs that have a label; an empty slot creates nothing. | The file states the labels, so the kernel does not decide. |
| `Distance(line, line)` read from a file | `AlgoDistanceLineLine` writes its inputs swapped, so the value after reading differs from the value before saving whenever the lines are not parallel. | The importer reads the inputs as written, as GeoGebra's reader does. | The importer follows what GeoGebra shows when it opens the file. Four golden cases show it. |
| Outputs that are undefined when a command runs, in a saved file | Have no label in the file. | Cannot be recovered: the importer reports them as skipped without a label. | The label is not in the file. |
