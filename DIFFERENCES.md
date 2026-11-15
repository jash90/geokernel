# Deliberate differences from GeoGebra

Behaviour that differs from GeoGebra on purpose is recorded here, with the
reason. A difference that is not listed here is a bug.

Cases that the kernel cannot reproduce *yet* do not belong here. They are
listed in `crates/geo-golden/known-gaps.toml`.

| Area | GeoGebra | geokernel | Reason |
| --- | --- | --- | --- |
| Objects created by a command with several results (`Tangent`, `AngularBisector` of two lines) | The set of labelled objects can change over time: a result that is undefined when the command runs gets its label only once it becomes defined. | The objects are fixed when the command runs. All results that are defined at that moment are created (`t_{1}`, `t_{2}`); when at most one is defined, only that one (or the first) is created under the plain name. | A construction whose list of objects depends on the history of moves cannot be replayed or saved reliably. |
| `Diameter(line, circle)` of an undefined circle (`conic-diameter-007`) | The line keeps the value it had while the circle was last defined. | The line is undefined. | The GeoGebra value depends on the history of moves and no longer belongs to any circle. |
