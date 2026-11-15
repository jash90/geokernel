# Deliberate differences from GeoGebra

Behaviour that differs from GeoGebra on purpose is recorded here, with the
reason. A difference that is not listed here is a bug.

Cases that the kernel cannot reproduce *yet* do not belong here. They are
listed in `crates/geo-golden/known-gaps.toml`.

| Area | GeoGebra | geokernel | Reason |
| --- | --- | --- | --- |
| Objects created by a command with several results (`Tangent`, `AngularBisector` of two lines) | The set of labelled objects can change over time: a result that is undefined when the command runs gets its label only once it becomes defined. | The objects are fixed when the command runs. All results that are defined at that moment are created (`t_{1}`, `t_{2}`); when at most one is defined, only that one (or the first) is created under the plain name. | A construction whose list of objects depends on the history of moves cannot be replayed or saved reliably. |
| `Diameter(line, circle)` of an undefined circle (`conic-diameter-007`) | The line keeps the value it had while the circle was last defined. | The line is undefined. | The GeoGebra value depends on the history of moves and no longer belongs to any circle. |
| Elementary functions in expressions (`sin(a°)`, `gamma`, `acosh`, `round(a, 2)`) | Some values come from BigDecimal arithmetic (multiples of degrees in `sin`, `cos`, `tan`) or from Apache Commons Math (gamma, some inverse functions). | Plain `f64` arithmetic, Rust's standard functions and an own Lanczos gamma; `round(a, n)` rounds the shortest decimal form of `a` half up. | No dependencies, and Apache Commons Math is not GeoGebra code. The results differ at most in the last bits (well inside the golden tolerance), e.g. `sin(30°)` is `0.49999999999999994` instead of `0.5`. |
| A name that is not an object, such as `ab` when `a` and `b` exist | Split into a product of objects (`a b`). | Rejected; a product needs a space or `*`. | The meaning of a line would change when an object `ab` is created later. |
| A constant angle expression such as `c = 30°` | A free angle: `floor(c)` and `round(c)` work in whole degrees. | A free number with the same value: `floor(c)` works in radians. | The kernel has no free angle object. Angles that read objects (`a°`, `Angle(A, B, C)`) behave as in GeoGebra. |
