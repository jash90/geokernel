# geo-app-core

The parts of the geokernel window that do not depend on egui: the document
(`workbench`), its journal (`scriptlog`), the pointer automaton (`gesture`),
the size profiles (`metrics`), the layout, the texts and the colours. A
desktop or Android shell draws them.

## Reserved names

`SetCoords`, `SetValue` and `SetVisibleInView` are file directives: a saved
script sets the coordinates of a free point, the value of a free number and
the visibility of an object with them, after the lines that made those
objects. They are read by `scriptlog`, not by the kernel, and only on lines
without `=`.

No object or function may take one of these names, and no definition may
call one. Such a line is refused in opened files (`line N: ...`), in
restored snapshots and in the input bar, which would otherwise turn a typed
`SetCoords(A, 1, 2)` into `f = SetCoords(A, 1, 2)`.

## Snapshots

`Workbench::snapshot_text` writes the document as a script with a first line
`# geokernel snapshot 1`. A definition whose labels are not just the names
before its `=` (the sides of a polygon, the outputs of `Tangent`) is
followed by the comment `# labels: p, a_1, b, c`. `restore_text` reads only
that version, keeps every line that loads and leaves out a line that makes
other labels than listed, together with every line that uses its labels.
